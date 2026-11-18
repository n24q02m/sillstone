// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 sillstone
"use strict";

// The stream parsers and argument checks of index.js on input the reference vectors never
// produce: every malformed signature and describe stream is refused with its own message,
// and hand-built streams decode field by field. Also the Buffer coercion of the digest
// helpers, findCdylib's lookup order and the FfiError messages.

const test = require("node:test");
const assert = require("node:assert/strict");
const crypto = require("node:crypto");
const fs = require("node:fs");
const path = require("node:path");

const sillstone = require("../index.js");

const { FfiError, parseSignature, parseDescribe, detect, contentHash, signature, describe, matchSignatures, findCdylib } = sillstone;

const be64 = (v) => {
  const b = Buffer.alloc(8);
  b.writeBigUInt64BE(BigInt(v));
  return b;
};
const be32 = (v) => {
  const b = Buffer.alloc(4);
  b.writeUInt32BE(v);
  return b;
};
const le64 = (v) => {
  const b = Buffer.alloc(8);
  b.writeBigUInt64LE(BigInt(v));
  return b;
};
const bytes = (...parts) => Buffer.concat(parts.map((p) => (Buffer.isBuffer(p) ? p : Buffer.from(p))));
const typeError = (message) => ({ name: "TypeError", message });

test("parseSignature refuses every malformed stream with its own message", () => {
  const cases = [
    ["not a Buffer", "abc", "signature stream is empty"],
    ["empty", Buffer.alloc(0), "signature stream is empty"],
    ["unknown tag", bytes([5]), "unknown signature tag 5"],
    ["image too short", bytes([0], Buffer.alloc(7)), "image stream is not tag + u64"],
    ["image too long", bytes([0], Buffer.alloc(9)), "image stream is not tag + u64"],
    ["audio header cut", bytes([1], Buffer.alloc(11)), "audio stream is shorter than its 13-byte header"],
    ["audio count above payload", bytes([1], be64(6), be32(0)), "audio peak count disagrees with the payload"],
    ["audio partial peak", bytes([1], be64(5), be32(0), Buffer.alloc(5)), "audio peak count disagrees with the payload"],
    ["text header cut", bytes([2], Buffer.alloc(7)), "text stream is shorter than its 9-byte header"],
    ["text count above payload", bytes([2], be64(8)), "text word count disagrees with the payload"],
    ["text partial word", bytes([2], be64(4), Buffer.alloc(4)), "text word count disagrees with the payload"],
    ["binary header cut", bytes([3], Buffer.alloc(7)), "binary stream is shorter than its 9-byte header"],
    ["binary count above payload", bytes([3], be64(32)), "binary chunk count disagrees with the payload"],
    ["binary partial digest", bytes([3], be64(16), Buffer.alloc(16)), "binary chunk count disagrees with the payload"],
    ["video header cut", bytes([4], Buffer.alloc(19)), "video stream is shorter than its 21-byte header"],
    ["video frames above payload", bytes([4], be32(8), Buffer.alloc(16)), "video frame count disagrees with the payload"],
    ["video partial frame hash", bytes([4], be32(4), Buffer.alloc(16), Buffer.alloc(4)), "video frame count disagrees with the payload"],
    ["video partial minhash word", bytes([4], be32(8), Buffer.alloc(16), Buffer.alloc(11)), "video minhash payload is not whole words"],
  ];
  for (const [name, raw, message] of cases) {
    assert.throws(() => parseSignature(raw), typeError(message), name);
  }
});

test("parseSignature decodes hand-built streams of every modality", () => {
  assert.deepEqual(parseSignature(bytes([0], be64(0x0102030405060708n))), { modality: "image", phash: 0x0102030405060708n });

  const peak = Buffer.alloc(6);
  peak.writeUInt32LE(0x01020304, 0);
  peak.writeUInt16LE(0x0506, 4);
  const audio = parseSignature(bytes([1], be64(6), be32(77), peak));
  assert.deepEqual(audio, { modality: "audio", peakCount: 6, frames: 77, peaks: [[0x01020304, 0x0506]], peaksBytes: peak });

  assert.deepEqual(parseSignature(bytes([2], be64(16), be64(1), be64(0xffffffffffffffffn))), {
    modality: "text",
    wordCount: 16,
    words: [1n, 0xffffffffffffffffn],
  });

  const digest = Buffer.alloc(32, 0xab);
  assert.deepEqual(parseSignature(bytes([3], be64(32), digest)), { modality: "binary", chunkCount: 32, digestBytes: digest });

  const video = parseSignature(bytes([4], be32(8), be32(64), be32(48), be64(0x4000000000000000n), le64(1), le64(2), le64(3)));
  assert.deepEqual(video, {
    modality: "video",
    frameCount: 8,
    width: 64,
    height: 48,
    durationBits: 0x4000000000000000n,
    frameHashes: [1n],
    minhash: [2n, 3n],
  });
});

test("parseDescribe refuses short streams, unknown codes and a missing signature", () => {
  const tier1 = Buffer.alloc(32, 0xcd);
  const header = typeError("describe stream is shorter than its 35-byte header");
  assert.throws(() => parseDescribe("x".repeat(40)), header);
  assert.throws(() => parseDescribe(bytes([0, 0], tier1)), header);
  assert.throws(() => parseDescribe(bytes([11, 0], tier1, Buffer.alloc(17))), typeError("unknown describe codes 11/0"));
  assert.throws(() => parseDescribe(bytes([0, 5], tier1, Buffer.alloc(17))), typeError("unknown describe codes 0/5"));
  // 35 bytes leave no room for a video signature before the 24-byte facts block.
  assert.throws(() => parseDescribe(bytes([4, 4], tier1, [0])), typeError("signature stream is empty"));
  // The embedded signature is parsed with parseSignature and refused the same way.
  assert.throws(() => parseDescribe(bytes([0, 0], tier1, [0], Buffer.alloc(7), Buffer.alloc(16))), typeError("image stream is not tag + u64"));
});

test("parseDescribe decodes a hand-built image description", () => {
  const tier1 = Buffer.alloc(32, 0xcd);
  const facts = bytes(be32(3), be32(4), be64(5));
  assert.deepEqual(parseDescribe(bytes([0, 0], tier1, [0], be64(9), facts)), {
    format: "png",
    modality: "image",
    tier1: "cd".repeat(32),
    signature: { modality: "image", phash: 9n },
    facts: { width: 3, height: 4, keypoints: 5 },
  });
});

test("the curator entry points refuse anything but a Buffer before calling the core", () => {
  const notBuffer = typeError("data must be a Buffer");
  for (const fn of [detect, contentHash, signature, describe]) {
    assert.throws(() => fn("text"), notBuffer, fn.name);
    assert.throws(() => fn(new Uint8Array(4)), notBuffer, fn.name);
  }
  const pair = typeError("inputs must be Buffers");
  assert.throws(() => matchSignatures("a", Buffer.from("b")), pair);
  assert.throws(() => matchSignatures(Buffer.from("a"), "b"), pair);
});

test("FfiError names the operation and the status", () => {
  const cases = [
    [-1, "op failed: invalid argument (status -1)"],
    [-2, "op failed: input rejected (status -2)"],
    [-7, "op failed: unknown failure (status -7)"],
  ];
  for (const [status, message] of cases) {
    const err = new FfiError("op", status);
    assert.ok(err instanceof Error);
    assert.equal(err.name, "FfiError");
    assert.equal(err.message, message);
    assert.equal(err.op, "op");
    assert.equal(err.status, status);
  }
});

test("the digest helpers accept any byte array, not only a Buffer", () => {
  const abc = Buffer.from("abc");
  const want = {
    sha256: crypto.createHash("sha256").update(abc).digest("hex"),
    sha3_256: crypto.createHash("sha3-256").update(abc).digest("hex"),
    sha3_512: crypto.createHash("sha3-512").update(abc).digest("hex"),
    blake2b512: crypto.createHash("blake2b512").update(abc).digest("hex"),
  };
  for (const [name, hex] of Object.entries(want)) {
    assert.equal(sillstone[name](new Uint8Array(abc)).toString("hex"), hex, `${name} of a Uint8Array`);
    assert.equal(sillstone.hash[name]([0x61, 0x62, 0x63]).toString("hex"), hex, `hash.${name} of a number array`);
  }
  assert.equal(sillstone.bufferChecksum(new Uint8Array(abc)), sillstone.fnv1a64(abc));
});

test("the flat image hash functions agree with the Image methods and default to gray", () => {
  const rgb = sillstone.image.Image.new("RGB", [9, 8], [10, 200, 30]);
  for (let i = 0; i < 9; i++) rgb.putpixel([i, i % 8], [i * 20, 255 - i * 20, 7 * i]);
  const gray = rgb.convert("L");
  for (const name of ["phash", "ahash", "dhash", "whash", "colorhash"]) {
    assert.equal(sillstone[name](rgb.tobytes(), 9, 8, 2), rgb[name](), `${name} RGB`);
    assert.equal(sillstone[name](gray.tobytes(), 9, 8), gray[name](), `${name} L by default`);
  }
});

test("findCdylib prefers SILLSTONE_CDYLIB, then the first candidate on disk", (t) => {
  const saved = process.env.SILLSTONE_CDYLIB;
  t.after(() => {
    if (saved === undefined) delete process.env.SILLSTONE_CDYLIB;
    else process.env.SILLSTONE_CDYLIB = saved;
  });
  delete process.env.SILLSTONE_CDYLIB;
  const found = findCdylib();
  assert.ok(fs.existsSync(found), found);

  process.env.SILLSTONE_CDYLIB = path.relative(process.cwd(), __filename);
  assert.equal(findCdylib(), __filename);

  process.env.SILLSTONE_CDYLIB = path.join(__dirname, "no-such-library.so");
  assert.equal(findCdylib(), found);

  // Nothing on disk: the first candidate, the addon build output, is still named.
  delete process.env.SILLSTONE_CDYLIB;
  t.mock.method(fs, "existsSync", () => false);
  assert.equal(findCdylib(), path.join(__dirname, "..", "build", "Release", "sillstone_napi.node"));
});
