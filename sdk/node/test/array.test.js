const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");
const sillstone = require("../index.js");

const { Tensor } = sillstone.array;
const golden = JSON.parse(fs.readFileSync(path.join(__dirname, "..", "..", "..", "tests", "fixtures", "facade", "golden.json"), "utf8"));

function close(actual, want, tol = 1e-9) {
  assert.strictEqual(actual.length, want.length);
  for (let i = 0; i < want.length; i++) assert.ok(Math.abs(actual[i] - want[i]) <= tol, `[${i}] ${actual[i]} vs ${want[i]}`);
}

const a = new Tensor(golden.tensor.a.shape, golden.tensor.a.data);
const b = new Tensor(golden.tensor.b.shape, golden.tensor.b.data);
const a2 = new Tensor(golden.tensor.a2.shape, golden.tensor.a2.data);

test("matmul, add and sum match the Python SDK", () => {
  const product = a.matmul(b);
  assert.deepStrictEqual(product.shape, [3, 2]);
  close(product.data, golden.tensor.matmul);
  close(a.add(a2).data, golden.tensor.add);
  assert.ok(Math.abs(a.sum() - golden.tensor.sum) <= 1e-9);
});

test("matmul promotes 1-D operands and rejects mismatched shapes", () => {
  const v = new Tensor([3], [1, 2, 3]);
  assert.strictEqual(v.dot(v), 14);
  close(v.matmul(new Tensor([3, 2], [1, 0, 0, 1, 1, 1])).data, [4, 5]);
  assert.throws(() => a.matmul(a), RangeError);
});

test("broadcasting, transpose and axis reductions agree with NumPy semantics", () => {
  const m = Tensor.fromArray([[1, 2, 3], [4, 5, 6]]);
  assert.deepStrictEqual(m.add(Tensor.fromArray([10, 20, 30])).toArray(), [[11, 22, 33], [14, 25, 36]]);
  assert.deepStrictEqual(m.multiply(2).toArray(), [[2, 4, 6], [8, 10, 12]]);
  assert.deepStrictEqual(m.transpose().toArray(), [[1, 4], [2, 5], [3, 6]]);
  assert.deepStrictEqual(m.sum(0).toArray(), [5, 7, 9]);
  assert.deepStrictEqual(m.mean(1).toArray(), [2, 5]);
  assert.strictEqual(m.max(), 6);
  assert.deepStrictEqual(m.argmax(1).toArray(), [2, 2]);
  assert.strictEqual(m.argmin(), 0);
  assert.ok(Math.abs(m.std() - 1.707825127659933) < 1e-12);
  assert.deepStrictEqual(m.reshape(3, -1).shape, [3, 2]);
  const cube = Tensor.arange(24).reshape(2, 3, 4);
  assert.strictEqual(cube.transpose(1, 0, 2).data[4], 12);
  assert.throws(() => m.add(Tensor.fromArray([1, 2])), RangeError);
});

test("constructors reject empty and ragged input; images round-trip through tensors", () => {
  assert.throws(() => new Tensor([0, 3]), RangeError);
  assert.throws(() => new Tensor([2], [1, 2, 3]), RangeError);
  assert.throws(() => Tensor.fromArray([[1, 2], [3]]), RangeError);
  assert.deepStrictEqual(Tensor.linspace(0, 1, 5).toArray(), [0, 0.25, 0.5, 0.75, 1]);
  const img = sillstone.image.Image.new("RGB", [3, 2], [1, 2, 3]);
  const t = Tensor.fromImage(img);
  assert.deepStrictEqual(t.shape, [2, 3, 3]);
  assert.ok(t.toImage().tobytes().equals(img.tobytes()));
});
