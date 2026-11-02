const test = require("node:test");
const assert = require("node:assert");
const sillstone = require("../index.js");

// tlshDistance calls sillstone_text_tlsh_distance_checked, which takes the lengths. The addon
// also refuses any length but 72 up front, so a wrong Buffer throws RangeError.
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

test("tlshDistance rejects a 72-byte value that is not a digest", () => {
  const a = sillstone.tlsh(sample(1));
  assert.throws(() => sillstone.tlshDistance(Buffer.alloc(72, "0"), a));
});
