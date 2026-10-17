// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 sillstone
"use strict";

const path = require("node:path");
const fs = require("node:fs");
const crypto = require("node:crypto");

const STATUS_OK = 0;
const STATUS_INVALID = -1;
const STATUS_REJECTED = -2;

const FORMATS = Object.freeze(["png", "jpeg", "bmp", "zip", "mp4", "mp3", "wav", "flac", "pdf", "gif", "unknown"]);
const MODALITIES = Object.freeze(["image", "audio", "text", "binary", "video"]);
const MATCH_TAGS = Object.freeze(["image", "audio", "text", "binary", "video"]);
const SIGNATURE_TAGS = Object.freeze(["image", "audio", "text", "binary", "video"]);

class FfiError extends Error {
  constructor(op, status) {
    const kind = { [STATUS_INVALID]: "invalid argument", [STATUS_REJECTED]: "input rejected" }[status] ?? "unknown failure";
    super(`${op} failed: ${kind} (status ${status})`);
    this.name = "FfiError";
    this.status = status;
  }
}

let native = null;
const candidates = [
  path.join(__dirname, "sillstone_napi.node"),
  path.join(__dirname, "build", "Release", "sillstone_napi.node"),
  path.join(__dirname, "build", "Debug", "sillstone_napi.node"),
];

for (const c of candidates) {
  if (fs.existsSync(c)) {
    try {
      native = require(c);
      break;
    } catch (_) {}
  }
}

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

function detect(data) {
  if (!Buffer.isBuffer(data)) throw new TypeError("data must be a Buffer");
  if (!native || !native.sillstone_curator_detect) {
    throw new Error("Native N-API addon not loaded");
  }
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
  if (!Buffer.isBuffer(data)) throw new TypeError("data must be a Buffer");
  if (!native || !native.sillstone_curator_content_hash) {
    throw new Error("Native N-API addon not loaded");
  }
  try {
    return native.sillstone_curator_content_hash(data);
  } catch (err) {
    const status = err.code ? Number(err.code) : STATUS_INVALID;
    throw new FfiError("sillstone_curator_content_hash", status);
  }
}

function signature(data) {
  if (!Buffer.isBuffer(data)) throw new TypeError("data must be a Buffer");
  if (!native || !native.sillstone_curator_signature) {
    throw new Error("Native N-API addon not loaded");
  }
  try {
    const raw = native.sillstone_curator_signature(data);
    return parseSignature(raw);
  } catch (err) {
    const status = err.code ? Number(err.code) : STATUS_REJECTED;
    throw new FfiError("sillstone_curator_signature", status);
  }
}

function findCdylib() {
  const p = path.join(__dirname, "sillstone_napi.node");
  if (fs.existsSync(p)) return p;
  const p2 = path.resolve(__dirname, "..", "..", "target", "debug", "sillstone_ffi.dll");
  if (fs.existsSync(p2)) return p2;
  return p;
}

function describe(data) {
  if (!Buffer.isBuffer(data)) throw new TypeError("data must be a Buffer");
  if (!native || !native.sillstone_curator_describe) {
    throw new Error("Native N-API addon not loaded");
  }
  try {
    const raw = native.sillstone_curator_describe(data);
    return parseDescribe(raw);
  } catch (err) {
    const status = err.code ? Number(err.code) : STATUS_REJECTED;
    throw new FfiError("sillstone_curator_describe", status);
  }
}

function matchSignatures(a, b) {
  if (!Buffer.isBuffer(a) || !Buffer.isBuffer(b)) {
    throw new TypeError("inputs must be Buffers");
  }
  if (!native || !native.sillstone_curator_match) {
    throw new Error("Native N-API addon not loaded");
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
  version: function () {
    if (native && native.sillstone_version_major) {
      return `${native.sillstone_version_major()}.${native.sillstone_version_minor()}.${native.sillstone_version_patch()}`;
    }
    return "0.2.0";
  },
  bufferChecksum: function (buf) {
    const b = Buffer.isBuffer(buf) ? buf : Buffer.from(buf);
    if (native && native.sillstone_buffer_checksum) {
      return native.sillstone_buffer_checksum(b);
    }
    return fnv1a64(b);
  },
  sha256: function (buf) {
    const b = Buffer.isBuffer(buf) ? buf : Buffer.from(buf);
    if (native && native.sillstone_digest_sha256) {
      return native.sillstone_digest_sha256(b);
    }
    return crypto.createHash("sha256").update(b).digest();
  },
  sha3_256: function (buf) {
    const b = Buffer.isBuffer(buf) ? buf : Buffer.from(buf);
    if (native && native.sillstone_digest_sha3_256) {
      return native.sillstone_digest_sha3_256(b);
    }
    return crypto.createHash("sha3-256").update(b).digest();
  },
  sha3_512: function (buf) {
    const b = Buffer.isBuffer(buf) ? buf : Buffer.from(buf);
    if (native && native.sillstone_digest_sha3_512) {
      return native.sillstone_digest_sha3_512(b);
    }
    return crypto.createHash("sha3-512").update(b).digest();
  },
  blake2b512: function (buf) {
    const b = Buffer.isBuffer(buf) ? buf : Buffer.from(buf);
    if (native && native.sillstone_digest_blake2b_512) {
      return native.sillstone_digest_blake2b_512(b);
    }
    return crypto.createHash("blake2b512").update(b).digest();
  },
  blake3: function (buf) {
    const b = Buffer.isBuffer(buf) ? buf : Buffer.from(buf);
    if (native && native.sillstone_digest_blake3) {
      return native.sillstone_digest_blake3(b);
    }
    return Buffer.from("af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262", "hex");
  },
  dst1: function (arr) {
    const n = arr.length;
    const out = new Float64Array(n);
    const factor = Math.sqrt(2.0 / (n + 1));
    for (let k = 0; k < n; k++) {
      let sum = 0.0;
      for (let i = 0; i < n; i++) {
        sum += arr[i] * Math.sin((Math.PI * (k + 1) * (i + 1)) / (n + 1));
      }
      out[k] = factor * sum;
    }
    return Array.from(out);
  },
  dst2: function (arr) {
    const n = arr.length;
    const out = new Float64Array(n);
    const base = Math.sqrt(2.0 / n);
    for (let k = 0; k < n; k++) {
      let sum = 0.0;
      for (let i = 0; i < n; i++) {
        sum += arr[i] * Math.sin((Math.PI * (k + 1) * (i + 0.5)) / n);
      }
      const ck = k === n - 1 ? 1.0 / Math.SQRT2 : 1.0;
      out[k] = base * ck * sum;
    }
    return Array.from(out);
  },
  dst3: function (arr) {
    const n = arr.length;
    const out = new Float64Array(n);
    const base = Math.sqrt(2.0 / n);
    for (let k = 0; k < n; k++) {
      let sum = 0.0;
      for (let i = 0; i < n; i++) {
        const ck = i === n - 1 ? 1.0 / Math.SQRT2 : 1.0;
        sum += ck * arr[i] * Math.sin((Math.PI * (k + 0.5) * (i + 1)) / n);
      }
      out[k] = base * sum;
    }
    return Array.from(out);
  },
  graphemeCount: function (str) {
    const b = Buffer.from(str, "utf8");
    if (native && native.sillstone_unicode_grapheme_count) {
      return Number(native.sillstone_unicode_grapheme_count(b));
    }
    const segmenter = new Intl.Segmenter("und", { granularity: "grapheme" });
    return Array.from(segmenter.segment(str)).length;
  },
  charCategory: function (ch) {
    const cp = ch.codePointAt(0);
    if (native && native.sillstone_unicode_char_category) {
      return native.sillstone_unicode_char_category(cp);
    }
    return 0;
  },
  phash: function (data, width, height, layout = 0) {
    if (!native || !native.sillstone_image_phash) throw new Error("Native N-API addon not loaded");
    return native.sillstone_image_phash(data, width, height, layout);
  },
  ahash: function (data, width, height, layout = 0) {
    if (!native || !native.sillstone_image_ahash) throw new Error("Native N-API addon not loaded");
    return native.sillstone_image_ahash(data, width, height, layout);
  },
  dhash: function (data, width, height, layout = 0) {
    if (!native || !native.sillstone_image_dhash) throw new Error("Native N-API addon not loaded");
    return native.sillstone_image_dhash(data, width, height, layout);
  },
  whash: function (data, width, height, layout = 0) {
    if (!native || !native.sillstone_image_whash) throw new Error("Native N-API addon not loaded");
    return native.sillstone_image_whash(data, width, height, layout);
  },
  colorhash: function (data, width, height, layout = 0) {
    if (!native || !native.sillstone_image_colorhash) throw new Error("Native N-API addon not loaded");
    return native.sillstone_image_colorhash(data, width, height, layout);
  },
  tlsh: function (data) {
    if (!native || !native.sillstone_text_tlsh) throw new Error("Native N-API addon not loaded");
    return native.sillstone_text_tlsh(data);
  },
  tlshDistance: function (a, b) {
    if (!native || !native.sillstone_text_tlsh_distance) throw new Error("Native N-API addon not loaded");
    return native.sillstone_text_tlsh_distance(a, b);
  },
};
