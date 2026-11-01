const test = require("node:test");
const assert = require("node:assert");
const sillstone = require("../index.js");

// The C ABI reads exactly 72 bytes from each digest pointer, so the addon must refuse any other
// length instead of reading past the Buffer.
function sample(seed) {
  const out = Buffer.alloc(4000);
  for (let i = 0; i < out.length; i++) out[i] = (i * 31 + seed * Math.floor(i / 7)) % 251;
  return out;
}

test("tlshDistance accepts 72-byte digests", () => {
  const a = sillstone.tlsh(sample(1));
  const b = sillstone.tlsh(sample(2));
  assert.strictEqual(a.length, 72);
  assert.strictEqual(sillstone.tlshDistance(a, a), 0);
  assert.ok(sillstone.tlshDistance(a, b) > 0);
  assert.strictEqual(sillstone.tlshDistance(a, b), sillstone.tlshDistance(b, a));
});

test("tlshDistance refuses digests that are not 72 bytes", () => {
  const a = sillstone.tlsh(sample(1));
  const bad = [a.subarray(0, 71), Buffer.concat([a, Buffer.from("0")]), Buffer.alloc(0)];
  for (const d of bad) {
    assert.throws(() => sillstone.tlshDistance(d, a), RangeError, `length ${d.length} as first argument`);
    assert.throws(() => sillstone.tlshDistance(a, d), RangeError, `length ${d.length} as second argument`);
  }
});
