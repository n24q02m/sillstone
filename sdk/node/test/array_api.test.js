// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 sillstone
"use strict";

// The `sillstone.array` surface beyond the golden cases in array.test.js: argument
// checks, the elementwise maths, broadcasting from either side, the 1-D matmul
// promotions, axis reductions and allclose. Expected values follow NumPy unless a
// comment says otherwise.

const test = require("node:test");
const assert = require("node:assert/strict");
const sillstone = require("../index.js");

const { Tensor, zeros, ones, arange, linspace, dot, fromImage } = sillstone.array;

function closeTo(actual, want, tol = 1e-12) {
  assert.equal(actual.length, want.length);
  for (let i = 0; i < want.length; i++) assert.ok(Math.abs(actual[i] - want[i]) <= tol, `[${i}] ${actual[i]} vs ${want[i]}`);
}

test("constructors refuse shapes and data they cannot hold", () => {
  assert.throws(() => new Tensor([]), { name: "TypeError", message: "shape must be a non-empty array of integers" });
  assert.throws(() => new Tensor("2x2"), { name: "TypeError", message: "shape must be a non-empty array of integers" });
  assert.throws(() => new Tensor([2, 1.5]), { name: "RangeError", message: "every dimension must be a positive integer, got 1.5" });
  assert.throws(() => new Tensor([10000, 10000]), { name: "RangeError", message: "tensor is too large" });
  assert.throws(() => new Tensor([2], new Set([1, 2])), { name: "TypeError", message: "data must be a Float64Array or an array of numbers" });
});

test("data is copied in, and zeros, ones and full fill a new buffer", () => {
  const src = new Float64Array([1, 2]);
  const t = new Tensor([2], src);
  src[0] = 99;
  assert.deepEqual(Array.from(t.data), [1, 2]);
  assert.deepEqual(zeros([2, 2]).toArray(), [[0, 0], [0, 0]]);
  assert.deepEqual(ones([3]).toArray(), [1, 1, 1]);
  assert.deepEqual(Tensor.full([1, 2], 7.5).toArray(), [[7.5, 7.5]]);
});

test("arange and linspace follow numpy", () => {
  assert.deepEqual(arange(4).toArray(), [0, 1, 2, 3]);
  assert.deepEqual(arange(2, 5).toArray(), [2, 3, 4]);
  assert.deepEqual(arange(1, 2, 0.25).toArray(), [1, 1.25, 1.5, 1.75]);
  assert.deepEqual(arange(5, 0, -2).toArray(), [5, 3, 1]);
  assert.throws(() => arange(0, 3, 0), { name: "RangeError", message: "step must be a non-zero finite number" });
  assert.throws(() => arange(0, 3, Infinity), { name: "RangeError", message: "step must be a non-zero finite number" });
  assert.throws(() => arange(3, 3), { name: "RangeError", message: "arange produces no elements" });
  assert.throws(() => arange(0, 3, -1), { name: "RangeError", message: "arange produces no elements" });
  assert.deepEqual(linspace(2, 3, 1).toArray(), [2]);
  const fifty = linspace(0, 1);
  assert.equal(fifty.size, 50);
  assert.equal(fifty.data[49], 1);
  assert.throws(() => linspace(0, 1, 0), { name: "RangeError", message: "num must be a positive integer" });
  assert.throws(() => linspace(0, 1, 2.5), { name: "RangeError", message: "num must be a positive integer" });
});

test("fromArray refuses non-arrays, ragged input and non-numbers", () => {
  assert.throws(() => Tensor.fromArray(5), { name: "TypeError", message: "expected a nested array" });
  assert.throws(() => Tensor.fromArray([[1, "2"]]), { name: "TypeError", message: "array elements must be numbers" });
  assert.throws(() => Tensor.fromArray([[1, 2], 3]), { name: "RangeError", message: "ragged nested array" });
});

test("tensors become images by channel count, rounded and clamped to 0..255", () => {
  assert.throws(() => Tensor.fromImage({ mode: "L" }), { name: "TypeError", message: "expected a sillstone image" });
  const gray = new Tensor([2, 3], [0, 64.4, 64.5, 300, -5, 255]).toImage();
  assert.equal(gray.mode, "L");
  assert.deepEqual(gray.size, [3, 2]);
  assert.deepEqual(Array.from(gray.tobytes()), [0, 64, 65, 255, 0, 255]);
  assert.deepEqual(fromImage(gray).shape, [2, 3, 1]);
  assert.equal(new Tensor([1, 1, 4], [1, 2, 3, 4]).toImage().mode, "RGBA");
  assert.equal(new Tensor([1, 1, 3], [1, 2, 3]).toImage("RGB").mode, "RGB");
  assert.throws(() => new Tensor([4]).toImage(), { name: "RangeError", message: "toImage needs a [h, w] or [h, w, channels] tensor" });
  assert.throws(() => new Tensor([1, 1, 2]).toImage(), { name: "RangeError", message: "mode undefined does not match 2 channel(s)" });
  assert.throws(() => new Tensor([1, 1, 3]).toImage("RGBA"), { name: "RangeError", message: "mode RGBA does not match 3 channel(s)" });
});

test("reshape, flatten and transpose check their arguments", () => {
  const t = arange(6);
  assert.deepEqual(t.reshape([2, 3]).toArray(), [[0, 1, 2], [3, 4, 5]]);
  assert.deepEqual(t.reshape([-1, 2]).shape, [3, 2]);
  assert.throws(() => t.reshape(4, -1), { name: "RangeError", message: "reshape size mismatch" });
  assert.throws(() => t.reshape(0, -1), { name: "RangeError", message: "reshape size mismatch" });
  assert.throws(() => t.reshape(4), { name: "RangeError", message: "reshape size mismatch" });
  const m = t.reshape(2, 3);
  assert.deepEqual(m.flatten().toArray(), [0, 1, 2, 3, 4, 5]);
  assert.deepEqual(m.transpose([1, 0]).toArray(), [[0, 3], [1, 4], [2, 5]]);
  assert.throws(() => m.transpose(0), { name: "RangeError", message: "axes [0] are not a permutation of 2 dimensions" });
  assert.throws(() => m.transpose(0, 0), { name: "RangeError", message: "axes [0,0] are not a permutation of 2 dimensions" });
  assert.throws(() => m.transpose(0, 2), { name: "RangeError", message: "axes [0,2] are not a permutation of 2 dimensions" });
});

test("elementwise maths applies the matching Math function to every element", () => {
  const x = Tensor.fromArray([[1, 4], [9, 16]]);
  assert.deepEqual(x.add(5).toArray(), [[6, 9], [14, 21]]);
  assert.deepEqual(x.subtract(1).toArray(), [[0, 3], [8, 15]]);
  assert.deepEqual(x.divide(Tensor.fromArray([1, 2])).toArray(), [[1, 2], [9, 8]]);
  assert.deepEqual(x.power(0.5).toArray(), [[1, 2], [3, 4]]);
  assert.deepEqual(x.negative().toArray(), [[-1, -4], [-9, -16]]);
  assert.deepEqual(x.negative().abs().toArray(), x.toArray());
  assert.deepEqual(x.sqrt().toArray(), [[1, 2], [3, 4]]);
  for (const [name, fn] of [["exp", Math.exp], ["log", Math.log], ["sin", Math.sin], ["cos", Math.cos]]) {
    assert.deepEqual(x[name]().toArray(), x.toArray().map((row) => row.map(fn)), name);
  }
  assert.deepEqual(x.map((v) => v % 3).toArray(), [[1, 1], [0, 1]]);
});

test("broadcasting stretches the shorter operand on either side", () => {
  const row = Tensor.fromArray([1, 2, 3]);
  const col = Tensor.fromArray([[10], [20]]);
  assert.deepEqual(row.add(col).toArray(), [[11, 12, 13], [21, 22, 23]]);
  assert.deepEqual(row.add(Tensor.fromArray([10])).toArray(), [11, 12, 13]);
  assert.deepEqual(row.subtract(Tensor.fromArray([[1, 1, 1], [2, 2, 2]])).toArray(), [[0, 1, 2], [-1, 0, 1]]);
  assert.throws(() => Tensor.fromArray([1, 2]).multiply(Tensor.fromArray([[1, 2, 3]])), {
    name: "RangeError",
    message: "shapes [2] and [1,3] cannot be broadcast together",
  });
});

test("matmul refuses non-tensors and higher ranks and promotes 1-D operands", () => {
  const m = Tensor.fromArray([[1, 2], [3, 4]]);
  const v = Tensor.fromArray([1, 1]);
  assert.throws(() => m.matmul([[1, 0], [0, 1]]), { name: "TypeError", message: "matmul expects a Tensor" });
  assert.throws(() => m.matmul(zeros([2, 2, 1])), { name: "RangeError", message: "matmul supports 1-D and 2-D tensors" });
  assert.throws(() => zeros([1, 2, 2]).matmul(m), { name: "RangeError", message: "matmul supports 1-D and 2-D tensors" });
  const mv = m.matmul(v);
  assert.deepEqual(mv.shape, [2]);
  assert.deepEqual(mv.toArray(), [3, 7]);
  const vm = v.matmul(m);
  assert.deepEqual(vm.shape, [2]);
  assert.deepEqual(vm.toArray(), [4, 6]);
  assert.deepEqual(dot(m, m).toArray(), [[7, 10], [15, 22]]);
  assert.equal(dot(v, Tensor.fromArray([2, 3])), 5);
});

test("reductions take an axis, count negative axes from the end and propagate NaN", () => {
  const m = Tensor.fromArray([[1, 5, 3], [4, 2, 6]]);
  assert.deepEqual(m.sum(-1).toArray(), [9, 12]);
  assert.deepEqual(m.min(0).toArray(), [1, 2, 3]);
  assert.equal(m.min(), 1);
  assert.deepEqual(m.max(-2).toArray(), [4, 5, 6]);
  // A 1-D tensor reduced along its only axis gives a one-element tensor (NumPy returns a
  // 0-d scalar there).
  assert.deepEqual(arange(3).sum(0).toArray(), [3]);
  assert.throws(() => m.sum(2), { name: "RangeError", message: "axis 2 is out of range for 2 dimensions" });
  assert.throws(() => m.mean(-3), { name: "RangeError", message: "axis -3 is out of range for 2 dimensions" });
  assert.throws(() => m.max(0.5), { name: "RangeError", message: "axis 0.5 is out of range for 2 dimensions" });
  const withNan = Tensor.fromArray([1, Number.NaN, 2]);
  assert.ok(Number.isNaN(withNan.max()));
  assert.ok(Number.isNaN(withNan.min()));
});

test("argmin and argmax return the first extreme, flat or along an axis", () => {
  const m = Tensor.fromArray([[1, 5, 3], [4, 2, 6]]);
  assert.deepEqual(m.argmin(0).toArray(), [0, 1, 0]);
  assert.deepEqual(m.argmax(0).toArray(), [1, 0, 1]);
  assert.equal(m.argmax(), 5);
  assert.equal(Tensor.fromArray([3, 1, 1]).argmin(), 1);
});

test("var and std reduce along an axis", () => {
  const m = Tensor.fromArray([[1, 5, 3], [4, 2, 6]]);
  assert.deepEqual(m.var(0).toArray(), [2.25, 2.25, 2.25]);
  assert.deepEqual(m.std(0).toArray(), [1.5, 1.5, 1.5]);
  closeTo(m.var(1).data, [8 / 3, 8 / 3]);
  closeTo(m.var(-1).data, m.var(1).data, 0);
  closeTo(m.std(1).data, [Math.sqrt(8 / 3), Math.sqrt(8 / 3)]);
});

test("allclose compares shapes and every element within the tolerance", () => {
  const a = Tensor.fromArray([1, 2]);
  assert.equal(a.allclose(Tensor.fromArray([1, 2 + 1e-12])), true);
  assert.equal(a.allclose(Tensor.fromArray([1, 2.1])), false);
  assert.equal(a.allclose(Tensor.fromArray([1, 2.1]), 0.2), true);
  assert.equal(a.allclose(Tensor.fromArray([1, Number.NaN])), false);
  assert.equal(a.allclose([1, 2]), false);
  assert.equal(a.allclose(Tensor.fromArray([[1, 2]])), false);
  assert.equal(a.allclose(Tensor.fromArray([1, 2, 3])), false);
});
