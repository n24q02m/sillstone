const test = require("node:test");
const assert = require("node:assert");
const sillstone = require("../index.js");

test("version matches expected", () => {
  assert.strictEqual(sillstone.version(), "0.2.0");
});

test("checksum produces deterministic golden output", () => {
  const data = Buffer.from("sillstone pure rust zero dep");
  const sum = sillstone.bufferChecksum(data);
  // Verify with exact golden bigint
  let h = 0xcbf29ce484222325n;
  const prime = 0x0100000001b3n;
  for (let i = 0; i < data.length; i++) {
    h ^= BigInt(data[i]);
    h = (h * prime) & 0xffffffffffffffffn;
  }
  assert.strictEqual(sum, h);
});
