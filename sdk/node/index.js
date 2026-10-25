// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 sillstone
"use strict";

const path = require("node:path");
const fs = require("node:fs");

const { native, call, FfiError, STATUS_OK, STATUS_INVALID, STATUS_REJECTED } = require("./native");
const imageModule = require("./image");
const arrayModule = require("./array");

const FORMATS = Object.freeze(["png", "jpeg", "bmp", "zip", "mp4", "mp3", "wav", "flac", "pdf", "gif", "unknown"]);
const MODALITIES = Object.freeze(["image", "audio", "text", "binary", "video"]);
const MATCH_TAGS = Object.freeze(["image", "audio", "text", "binary", "video"]);
const SIGNATURE_TAGS = Object.freeze(["image", "audio", "text", "binary", "video"]);

function fnv1a64(data) {
  let h = 0xcbf29ce484222325n;
  const prime = 0x100000001b3n;
  for (let i = 0; i < data.length; i++) {
    h ^= BigInt(data[i]);
    h = (h * prime) & 0xffffffffffffffffn;
  }
  return h;
}

function be64(b, off) {
  return b.readBigUInt64BE(off);
}

function parseSignature(raw) {
  if (!Buffer.isBuffer(raw) || raw.length < 1) {
    throw new TypeError("signature stream is empty");
  }
  const tag = raw[0];
  if (tag >= SIGNATURE_TAGS.length) {
    throw new TypeError(`unknown signature tag ${tag}`);
  }
  const modality = SIGNATURE_TAGS[tag];
  if (modality === "image") {
    if (raw.length !== 9) throw new TypeError("image stream is not tag + u64");
    return { modality, phash: be64(raw, 1) };
  }
  if (modality === "audio") {
    if (raw.length < 13) throw new TypeError("audio stream is shorter than its 13-byte header");
    const peakCount = Number(be64(raw, 1));
    const frames = raw.readUInt32BE(9);
    const payload = raw.subarray(13);
    if (peakCount !== payload.length || payload.length % 6 !== 0) {
      throw new TypeError("audio peak count disagrees with the payload");
    }
    const peaks = [];
    for (let i = 0; i < payload.length; i += 6) {
      peaks.push([payload.readUInt32LE(i), payload.readUInt16LE(i + 4)]);
    }
    return { modality, peakCount, frames, peaks, peaksBytes: Buffer.from(payload) };
  }
  if (modality === "text") {
    if (raw.length < 9) throw new TypeError("text stream is shorter than its 9-byte header");
    const wordCount = Number(be64(raw, 1));
    const payload = raw.subarray(9);
    if (wordCount !== payload.length || payload.length % 8 !== 0) {
      throw new TypeError("text word count disagrees with the payload");
    }
    const words = [];
    for (let i = 0; i < payload.length; i += 8) words.push(be64(payload, i));
    return { modality, wordCount, words };
  }
  if (modality === "binary") {
    if (raw.length < 9) throw new TypeError("binary stream is shorter than its 9-byte header");
    const chunkCount = Number(be64(raw, 1));
    const payload = raw.subarray(9);
    if (chunkCount !== payload.length || payload.length % 32 !== 0) {
      throw new TypeError("binary chunk count disagrees with the payload");
    }
    return { modality, chunkCount, digestBytes: Buffer.from(payload) };
  }
  // video
  if (raw.length < 21) throw new TypeError("video stream is shorter than its 21-byte header");
  const frameCount = raw.readUInt32BE(1);
  const width = raw.readUInt32BE(5);
  const height = raw.readUInt32BE(9);
  const durationBits = be64(raw, 13);
  const rest = raw.subarray(21);
  if (frameCount > rest.length || frameCount % 8 !== 0) {
    throw new TypeError("video frame count disagrees with the payload");
  }
  const framesPayload = rest.subarray(0, frameCount);
  const minhashPayload = rest.subarray(frameCount);
  if (minhashPayload.length % 8 !== 0) {
    throw new TypeError("video minhash payload is not whole words");
  }
  const frameHashes = [];
  const minhash = [];
  for (let i = 0; i < framesPayload.length; i += 8) frameHashes.push(framesPayload.readBigUInt64LE(i));
  for (let i = 0; i < minhashPayload.length; i += 8) minhash.push(minhashPayload.readBigUInt64LE(i));
  return { modality, frameCount, width, height, durationBits, frameHashes, minhash };
}

function factsLen(modality) {
  if (modality === "audio") return 18;
  if (modality === "video") return 24;
  return 16;
}

function parseDescribe(raw) {
  if (!Buffer.isBuffer(raw) || raw.length < 35) {
    throw new TypeError("describe stream is shorter than its 35-byte header");
  }
  const fmt = raw[0];
  const mod = raw[1];
  if (fmt >= FORMATS.length || mod >= MODALITIES.length) {
    throw new TypeError(`unknown describe codes ${fmt}/${mod}`);
  }
  const tier1 = raw.subarray(2, 34).toString("hex");
  const modality = MODALITIES[mod];
  const sigEnd = raw.length - factsLen(modality);
  const sig = parseSignature(raw.subarray(34, sigEnd));
  const factsRaw = raw.subarray(sigEnd);
  let facts;
  if (modality === "image") {
    if (factsRaw.length !== 16) throw new TypeError("image facts block is 16 bytes");
    facts = { width: factsRaw.readUInt32BE(0), height: factsRaw.readUInt32BE(4), keypoints: Number(be64(factsRaw, 8)) };
  } else if (modality === "audio") {
    if (factsRaw.length !== 18) throw new TypeError("audio facts block is 18 bytes");
    facts = {
      sampleRate: factsRaw.readUInt32BE(0),
      channels: factsRaw.readUInt16BE(4),
      frames: factsRaw.readUInt32BE(6),
      peaks: Number(be64(factsRaw, 10)),
    };
  } else if (modality === "text") {
    if (factsRaw.length !== 16) throw new TypeError("text facts block is 16 bytes");
    facts = { words: Number(be64(factsRaw, 0)), canonicalLen: Number(be64(factsRaw, 8)) };
  } else if (modality === "binary") {
    if (factsRaw.length !== 16) throw new TypeError("binary facts block is 16 bytes");
    facts = { len: Number(be64(factsRaw, 0)), chunks: Number(be64(factsRaw, 8)) };
  } else {
    if (factsRaw.length !== 24) throw new TypeError("video facts block is 24 bytes");
    facts = {
      width: factsRaw.readUInt32BE(0),
      height: factsRaw.readUInt32BE(4),
      durationBits: be64(factsRaw, 8),
      framesSampled: Number(be64(factsRaw, 16)),
    };
  }
  return { format: FORMATS[fmt], modality, tier1, signature: sig, facts };
}

function requireBuffer(data, name = "data") {
  if (!Buffer.isBuffer(data)) throw new TypeError(`${name} must be a Buffer`);
  return data;
}

function detect(data) {
  requireBuffer(data);
  try {
    const res = native.sillstone_curator_detect(data);
    return {
      format: FORMATS[res.format],
      modality: MODALITIES[res.modality],
      pending: res.pending === 1,
    };
  } catch (err) {
    const status = err.code ? Number(err.code) : STATUS_INVALID;
    throw new FfiError("sillstone_curator_detect", status);
  }
}

function contentHash(data) {
  requireBuffer(data);
  try {
    return native.sillstone_curator_content_hash(data);
  } catch (err) {
    const status = err.code ? Number(err.code) : STATUS_INVALID;
    throw new FfiError("sillstone_curator_content_hash", status);
  }
}

function signature(data) {
  requireBuffer(data);
  try {
    return parseSignature(native.sillstone_curator_signature(data));
  } catch (err) {
    const status = err.code ? Number(err.code) : STATUS_REJECTED;
    throw new FfiError("sillstone_curator_signature", status);
  }
}

function findCdylib() {
  const explicit = process.env.SILLSTONE_CDYLIB;
  if (explicit && fs.existsSync(explicit)) return path.resolve(explicit);
  const candidates = [
    path.join(__dirname, "build", "Release", "sillstone_napi.node"),
    path.join(__dirname, "sillstone_napi.node"),
    path.resolve(__dirname, "..", "..", "target", "debug", "libsillstone_ffi.so"),
    path.resolve(__dirname, "..", "..", "target", "debug", "libsillstone_ffi.dylib"),
    path.resolve(__dirname, "..", "..", "target", "debug", "sillstone_ffi.dll"),
    path.resolve(__dirname, "..", "..", "target", "release", "libsillstone_ffi.so"),
    path.resolve(__dirname, "..", "..", "target", "release", "libsillstone_ffi.dylib"),
    path.resolve(__dirname, "..", "..", "target", "release", "sillstone_ffi.dll"),
  ];
  for (const c of candidates) {
    if (fs.existsSync(c)) return c;
  }
  return candidates[0];
}

function describe(data) {
  requireBuffer(data);
  try {
    return parseDescribe(native.sillstone_curator_describe(data));
  } catch (err) {
    const status = err.code ? Number(err.code) : STATUS_REJECTED;
    throw new FfiError("sillstone_curator_describe", status);
  }
}

function matchSignatures(a, b) {
  if (!Buffer.isBuffer(a) || !Buffer.isBuffer(b)) {
    throw new TypeError("inputs must be Buffers");
  }
  try {
    const res = native.sillstone_curator_match(a, b);
    return {
      tag: MATCH_TAGS[res.tag],
      slots: Array.from(res.slots),
      matched: res.matched === 1,
    };
  } catch (err) {
    const status = err.code ? Number(err.code) : STATUS_REJECTED;
    throw new FfiError("sillstone_curator_match", status);
  }
}

function asBuffer(buf) {
  return Buffer.isBuffer(buf) ? buf : Buffer.from(buf);
}

/** Float64 values -> native DST -> plain array. */
function dst(name, arr) {
  const input = Float64Array.from(arr);
  const out = native[name](Buffer.from(input.buffer, input.byteOffset, input.byteLength));
  return Array.from(new Float64Array(out.buffer.slice(out.byteOffset, out.byteOffset + out.byteLength)));
}

/** `sillstone.hash`: digests and image hashes, the `hashlib`/`imagehash` replacements. */
const hash = {
  sha256: (buf) => native.sillstone_digest_sha256(asBuffer(buf)),
  sha3_256: (buf) => native.sillstone_digest_sha3_256(asBuffer(buf)),
  sha3_512: (buf) => native.sillstone_digest_sha3_512(asBuffer(buf)),
  blake2b512: (buf) => native.sillstone_digest_blake2b_512(asBuffer(buf)),
  blake3: (buf) => native.sillstone_digest_blake3(asBuffer(buf)),
};

function imageHash(name) {
  return (data, width, height, layout = 0) => call(name, data, width, height, layout);
}

module.exports = {
  STATUS_OK,
  STATUS_INVALID,
  STATUS_REJECTED,
  FORMATS,
  MODALITIES,
  MATCH_TAGS,
  SIGNATURE_TAGS,
  FfiError,
  findCdylib,
  fnv1a64,
  parseSignature,
  parseDescribe,
  detect,
  contentHash,
  signature,
  describe,
  matchSignatures,
  hash,
  image: imageModule,
  array: arrayModule,
  version: () => `${native.sillstone_version_major()}.${native.sillstone_version_minor()}.${native.sillstone_version_patch()}`,
  bufferChecksum: (buf) => native.sillstone_buffer_checksum(asBuffer(buf)),
  sha256: hash.sha256,
  sha3_256: hash.sha3_256,
  sha3_512: hash.sha3_512,
  blake2b512: hash.blake2b512,
  blake3: hash.blake3,
  dst1: (arr) => dst("sillstone_math_dst1", arr),
  dst2: (arr) => dst("sillstone_math_dst2", arr),
  dst3: (arr) => dst("sillstone_math_dst3", arr),
  graphemeCount: (str) => Number(native.sillstone_unicode_grapheme_count(Buffer.from(str, "utf8"))),
  charCategory: (ch) => native.sillstone_unicode_char_category(ch.codePointAt(0)),
  phash: imageHash("sillstone_image_phash"),
  ahash: imageHash("sillstone_image_ahash"),
  dhash: imageHash("sillstone_image_dhash"),
  whash: imageHash("sillstone_image_whash"),
  colorhash: imageHash("sillstone_image_colorhash"),
  tlsh: (data) => native.sillstone_text_tlsh(data),
  tlshDistance: (a, b) => native.sillstone_text_tlsh_distance(a, b),
  tensor: (shape, data) => new arrayModule.Tensor(shape, data),
};
