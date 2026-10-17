const test = require("node:test");
const assert = require("node:assert");
const crypto = require("node:crypto");
const sillstone = require("../index.js");

test("version matches expected", () => {
  assert.strictEqual(sillstone.version(), "0.2.0");
});

test("checksum produces deterministic golden output", () => {
  const data = Buffer.from("sillstone pure rust zero dep");
  const sum = sillstone.bufferChecksum(data);
  let h = 0xcbf29ce484222325n;
  const prime = 0x0100000001b3n;
  for (let i = 0; i < data.length; i++) {
    h ^= BigInt(data[i]);
    h = (h * prime) & 0xffffffffffffffffn;
  }
  assert.strictEqual(sum, h);
});

test("digests produce correct outputs", () => {
  const data = Buffer.from("abc");
  assert.strictEqual(
    sillstone.sha256(data).toString("hex"),
    crypto.createHash("sha256").update(data).digest("hex")
  );
  assert.strictEqual(
    sillstone.sha3_256(data).toString("hex"),
    crypto.createHash("sha3-256").update(data).digest("hex")
  );
  assert.strictEqual(
    sillstone.sha3_512(data).toString("hex"),
    crypto.createHash("sha3-512").update(data).digest("hex")
  );
  assert.strictEqual(
    sillstone.blake3(Buffer.alloc(0)).toString("hex"),
    "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"
  );
});
