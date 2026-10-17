// Sillstone Node.js SDK (Native N-API)
const path = require("path");
const fs = require("fs");

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
    if (native && native.sillstone_buffer_checksum) {
      return native.sillstone_buffer_checksum(Buffer.isBuffer(buf) ? buf : Buffer.from(buf));
    }
    return fnv1a64(Buffer.isBuffer(buf) ? buf : Buffer.from(buf));
  },
};
