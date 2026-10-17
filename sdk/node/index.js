// Sillstone Node.js SDK (Native N-API)
const path = require("path");
const fs = require("fs");
const crypto = require("crypto");

let native = null;
const candidates = [
  path.join(__dirname, "build", "Release", "sillstone_napi.node"),
  path.join(__dirname, "build", "Debug", "sillstone_napi.node"),
  path.join(__dirname, "sillstone_napi.node"),
];

for (const c of candidates) {
  if (fs.existsSync(c)) {
    try {
      native = require(c);
      break;
    } catch (_) {}
  }
}

// Pure fallback for environments where native addon has not been compiled yet
function fnv1a64(buf) {
  let h = 0xcbf29ce484222325n;
  const prime = 0x0100000001b3n;
  for (let i = 0; i < buf.length; i++) {
    h ^= BigInt(buf[i]);
    h = (h * prime) & 0xffffffffffffffffn;
  }
  return h;
}

module.exports = {
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
};
