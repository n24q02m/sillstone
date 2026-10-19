const test = require("node:test");
const assert = require("node:assert/strict");
const sillstone = require("../index.js");

test("node sdk tensor creation, reshape, and matmul", () => {
  const t = sillstone.tensor([2, 3], [1, 2, 3, 4, 5, 6]);
  assert.equal(t.ndim, 2);
  assert.equal(t.size, 6);
  assert.equal(t.sum(), 21);

  const r = t.reshape(3, 2);
  assert.deepEqual(r.shape, [3, 2]);

  const b = sillstone.tensor([3, 2], [7, 8, 9, 1, 2, 3]);
  const c = t.matmul(b);
  assert.deepEqual(c.shape, [2, 2]);
  assert.deepEqual(Array.from(c.data), [31, 19, 85, 55]);
});
