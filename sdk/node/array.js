// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 sillstone
"use strict";

// `sillstone.array`: a NumPy-shaped float64 N-d array. Storage is one contiguous
// row-major Float64Array. Matrix products, same-shape addition and full sums run in the
// native core; broadcasting, views of shape, axis reductions and elementwise maths are
// plain JS over the same buffer. Empty (zero-sized) arrays are not supported.

const { call } = require("./native");
const { Image, CHANNELS } = require("./image");

function checkShape(shape) {
  if (!Array.isArray(shape) || shape.length === 0) throw new TypeError("shape must be a non-empty array of integers");
  let size = 1;
  for (const d of shape) {
    if (!Number.isInteger(d) || d < 1) throw new RangeError(`every dimension must be a positive integer, got ${d}`);
    size *= d;
  }
  if (!Number.isSafeInteger(size) || size * 8 > 0x1fffffe8) throw new RangeError("tensor is too large");
  return size;
}

function stridesOf(shape) {
  const strides = new Array(shape.length);
  let s = 1;
  for (let i = shape.length - 1; i >= 0; i--) {
    strides[i] = s;
    s *= shape[i];
  }
  return strides;
}

/** A Buffer view over the typed array's bytes: no copy, handed to the addon. */
function bytesOf(f64) {
  return Buffer.from(f64.buffer, f64.byteOffset, f64.byteLength);
}

/** Copies an addon result Buffer into an owned, aligned Float64Array. */
function f64Of(buf) {
  return new Float64Array(buf.buffer.slice(buf.byteOffset, buf.byteOffset + buf.byteLength));
}

function broadcastShapes(a, b) {
  const n = Math.max(a.length, b.length);
  const out = new Array(n);
  for (let i = 0; i < n; i++) {
    const da = a[a.length - 1 - i] ?? 1;
    const db = b[b.length - 1 - i] ?? 1;
    if (da !== db && da !== 1 && db !== 1) {
      throw new RangeError(`shapes [${a}] and [${b}] cannot be broadcast together`);
    }
    out[n - 1 - i] = Math.max(da, db);
  }
  return out;
}

class Tensor {
  /**
   * @param {number[]} shape positive integer dimensions
   * @param {Float64Array|number[]} [data] row-major values; omitted means zeros
   */
  constructor(shape, data) {
    const size = checkShape(shape);
    this._shape = shape.slice();
    if (data === undefined) {
      this._data = new Float64Array(size);
    } else {
      if (!(data instanceof Float64Array) && !Array.isArray(data)) throw new TypeError("data must be a Float64Array or an array of numbers");
      if (data.length !== size) throw new RangeError(`data length ${data.length} does not match shape [${shape}] (${size} elements)`);
      this._data = Float64Array.from(data); // copy: tensors never alias caller memory
    }
  }

  static zeros(shape) {
    return new Tensor(shape);
  }

  static ones(shape) {
    return Tensor.full(shape, 1);
  }

  static full(shape, value) {
    const t = new Tensor(shape);
    t._data.fill(value);
    return t;
  }

  /** `[start, stop)` stepping by `step`, like `numpy.arange`. */
  static arange(start, stop, step = 1) {
    if (stop === undefined) [start, stop] = [0, start];
    if (step === 0 || !Number.isFinite(step)) throw new RangeError("step must be a non-zero finite number");
    const n = Math.ceil((stop - start) / step);
    if (!(n >= 1)) throw new RangeError("arange produces no elements");
    const t = new Tensor([n]);
    for (let i = 0; i < n; i++) t._data[i] = start + i * step;
    return t;
  }

  /** `num` evenly spaced values over `[start, stop]`, like `numpy.linspace`. */
  static linspace(start, stop, num = 50) {
    if (!Number.isInteger(num) || num < 1) throw new RangeError("num must be a positive integer");
    const t = new Tensor([num]);
    const step = num > 1 ? (stop - start) / (num - 1) : 0;
    for (let i = 0; i < num; i++) t._data[i] = start + i * step;
    if (num > 1) t._data[num - 1] = stop;
    return t;
  }

  /** Builds a tensor from nested arrays (`[[1, 2], [3, 4]]`). */
  static fromArray(nested) {
    const shape = [];
    for (let level = nested; Array.isArray(level); level = level[0]) shape.push(level.length);
    if (shape.length === 0) throw new TypeError("expected a nested array");
    const flat = [];
    const walk = (node, depth) => {
      if (depth === shape.length) {
        if (typeof node !== "number") throw new TypeError("array elements must be numbers");
        flat.push(node);
        return;
      }
      if (!Array.isArray(node) || node.length !== shape[depth]) throw new RangeError("ragged nested array");
      for (const child of node) walk(child, depth + 1);
    };
    walk(nested, 0);
    return new Tensor(shape, flat);
  }

  /** `[height, width, channels]` view of an image's samples as float64 (a copy). */
  static fromImage(image) {
    if (!(image instanceof Image)) throw new TypeError("expected a sillstone image");
    return new Tensor([image.height, image.width, CHANNELS[image.mode]], Float64Array.from(image.tobytes()));
  }

  /** Rounds, clamps to 0..255 and wraps as an image; `mode` defaults by channel count. */
  toImage(mode) {
    if (this.ndim !== 3 && this.ndim !== 2) throw new RangeError("toImage needs a [h, w] or [h, w, channels] tensor");
    const ch = this.ndim === 2 ? 1 : this._shape[2];
    const resolved = mode ?? { 1: "L", 3: "RGB", 4: "RGBA" }[ch];
    if (!resolved || CHANNELS[resolved] !== ch) throw new RangeError(`mode ${resolved} does not match ${ch} channel(s)`);
    const bytes = Buffer.alloc(this.size);
    for (let i = 0; i < bytes.length; i++) bytes[i] = Math.min(255, Math.max(0, Math.round(this._data[i])));
    return new Image(resolved, [this._shape[1], this._shape[0]], bytes);
  }

  get shape() {
    return this._shape.slice();
  }
  get ndim() {
    return this._shape.length;
  }
  get size() {
    return this._data.length;
  }
  /** The backing Float64Array (not copied). */
  get data() {
    return this._data;
  }

  /** Nested arrays, the inverse of {@link Tensor.fromArray}. */
  toArray() {
    const build = (depth, offset) => {
      if (depth === this._shape.length - 1) return Array.from(this._data.subarray(offset, offset + this._shape[depth]));
      const stride = this._data.length / this._shape.slice(0, depth + 1).reduce((a, b) => a * b, 1);
      return Array.from({ length: this._shape[depth] }, (_, i) => build(depth + 1, offset + i * stride));
    };
    return build(0, 0);
  }

  /** New shape over the same elements; one dimension may be `-1`. */
  reshape(...dims) {
    let target = Array.isArray(dims[0]) ? dims[0].slice() : dims;
    const infer = target.indexOf(-1);
    if (infer !== -1) {
      const known = target.reduce((a, d, i) => (i === infer ? a : a * d), 1);
      if (known < 1 || this.size % known !== 0) throw new RangeError("reshape size mismatch");
      target = target.map((d, i) => (i === infer ? this.size / known : d));
    }
    if (checkShape(target) !== this.size) throw new RangeError("reshape size mismatch");
    return new Tensor(target, this._data);
  }

  flatten() {
    return this.reshape(this.size);
  }

  /** Permutes axes (reverses them when none are given). */
  transpose(...axes) {
    const nd = this.ndim;
    const perm = axes.length === 0 ? Array.from({ length: nd }, (_, i) => nd - 1 - i) : (Array.isArray(axes[0]) ? axes[0] : axes);
    if (perm.length !== nd || new Set(perm).size !== nd || !perm.every((a) => Number.isInteger(a) && a >= 0 && a < nd)) {
      throw new RangeError(`axes [${perm}] are not a permutation of ${nd} dimensions`);
    }
    const outShape = perm.map((a) => this._shape[a]);
    const inStrides = stridesOf(this._shape);
    const permStrides = perm.map((a) => inStrides[a]);
    const out = new Tensor(outShape);
    const idx = new Array(nd).fill(0);
    let src = 0;
    for (let i = 0; i < out.size; i++) {
      out._data[i] = this._data[src];
      for (let d = nd - 1; d >= 0; d--) {
        idx[d]++;
        src += permStrides[d];
        if (idx[d] < outShape[d]) break;
        src -= permStrides[d] * outShape[d];
        idx[d] = 0;
      }
    }
    return out;
  }

  _zip(other, fn) {
    const rhs = other instanceof Tensor ? other : Tensor.full([1], Number(other));
    const shape = broadcastShapes(this._shape, rhs._shape);
    const out = new Tensor(shape);
    const lhsStr = this._broadcastStrides(shape);
    const rhsStr = rhs._broadcastStrides(shape);
    const idx = new Array(shape.length).fill(0);
    let a = 0;
    let b = 0;
    for (let i = 0; i < out.size; i++) {
      out._data[i] = fn(this._data[a], rhs._data[b]);
      for (let d = shape.length - 1; d >= 0; d--) {
        idx[d]++;
        a += lhsStr[d];
        b += rhsStr[d];
        if (idx[d] < shape[d]) break;
        a -= lhsStr[d] * shape[d];
        b -= rhsStr[d] * shape[d];
        idx[d] = 0;
      }
    }
    return out;
  }

  _broadcastStrides(target) {
    const own = stridesOf(this._shape);
    const lead = target.length - this._shape.length;
    return target.map((_, i) => (i < lead || this._shape[i - lead] === 1 ? 0 : own[i - lead]));
  }

  /** Elementwise sum with broadcasting; equal shapes use the native kernel. */
  add(other) {
    if (other instanceof Tensor && other._shape.length === this._shape.length && other._shape.every((d, i) => d === this._shape[i])) {
      const out = new Tensor(this._shape);
      out._data = f64Of(call("sillstone_tensor_add_f64", bytesOf(this._data), bytesOf(other._data)));
      return out;
    }
    return this._zip(other, (x, y) => x + y);
  }
  subtract(other) {
    return this._zip(other, (x, y) => x - y);
  }
  multiply(other) {
    return this._zip(other, (x, y) => x * y);
  }
  divide(other) {
    return this._zip(other, (x, y) => x / y);
  }
  power(other) {
    return this._zip(other, (x, y) => x ** y);
  }

  map(fn) {
    const out = new Tensor(this._shape);
    for (let i = 0; i < this.size; i++) out._data[i] = fn(this._data[i]);
    return out;
  }
  negative() {
    return this.map((x) => -x);
  }
  abs() {
    return this.map(Math.abs);
  }
  sqrt() {
    return this.map(Math.sqrt);
  }
  exp() {
    return this.map(Math.exp);
  }
  log() {
    return this.map(Math.log);
  }
  sin() {
    return this.map(Math.sin);
  }
  cos() {
    return this.map(Math.cos);
  }

  /** 2-D matrix product through the native GEMM. 1-D operands are promoted like NumPy. */
  matmul(other) {
    if (!(other instanceof Tensor)) throw new TypeError("matmul expects a Tensor");
    if (this.ndim > 2 || other.ndim > 2) throw new RangeError("matmul supports 1-D and 2-D tensors");
    const left = this.ndim === 1 ? this.reshape(1, this.size) : this;
    const right = other.ndim === 1 ? other.reshape(other.size, 1) : other;
    const [m, k1] = left._shape;
    const [k2, n] = right._shape;
    if (k1 !== k2) throw new RangeError(`matmul dimension mismatch: [${this._shape}] @ [${other._shape}]`);
    const data = f64Of(call("sillstone_tensor_matmul_f64", bytesOf(left._data), m, k1, bytesOf(right._data), k2, n));
    const shape = this.ndim === 1 && other.ndim === 1 ? [1] : this.ndim === 1 ? [n] : other.ndim === 1 ? [m] : [m, n];
    return new Tensor(shape, data);
  }

  /** Inner product of two 1-D tensors, or matrix product for 2-D operands. */
  dot(other) {
    const r = this.matmul(other);
    return this.ndim === 1 && other.ndim === 1 ? r._data[0] : r;
  }

  _reduce(axis, init, step, finish = (acc) => acc) {
    if (axis === undefined) {
      let acc = init;
      for (let i = 0; i < this.size; i++) acc = step(acc, this._data[i], i);
      return finish(acc, this.size);
    }
    if (!Number.isInteger(axis) || axis < -this.ndim || axis >= this.ndim) throw new RangeError(`axis ${axis} is out of range for ${this.ndim} dimensions`);
    const ax = axis < 0 ? axis + this.ndim : axis;
    const inner = this._shape.slice(ax + 1).reduce((a, b) => a * b, 1);
    const len = this._shape[ax];
    const outer = this.size / (inner * len);
    const outShape = this._shape.filter((_, i) => i !== ax);
    const out = new Tensor(outShape.length === 0 ? [1] : outShape);
    for (let o = 0; o < outer; o++) {
      for (let i = 0; i < inner; i++) {
        let acc = init;
        for (let k = 0; k < len; k++) acc = step(acc, this._data[(o * len + k) * inner + i], k);
        out._data[o * inner + i] = finish(acc, len);
      }
    }
    return out;
  }

  /** Sum of all elements (native kernel) or along `axis` (a Tensor). */
  sum(axis) {
    if (axis === undefined) return call("sillstone_tensor_sum_f64", bytesOf(this._data));
    return this._reduce(axis, 0, (a, x) => a + x);
  }
  mean(axis) {
    return this._reduce(axis, 0, (a, x) => a + x, (a, n) => a / n);
  }
  min(axis) {
    return this._reduce(axis, Infinity, (a, x) => (x < a || Number.isNaN(x) ? x : a));
  }
  max(axis) {
    return this._reduce(axis, -Infinity, (a, x) => (x > a || Number.isNaN(x) ? x : a));
  }
  var(axis) {
    const mean = this.mean(axis);
    const centred = this.subtract(axis === undefined ? mean : mean.reshape(this._withAxisOne(axis)));
    return centred.multiply(centred).mean(axis);
  }
  std(axis) {
    const v = this.var(axis);
    return typeof v === "number" ? Math.sqrt(v) : v.sqrt();
  }

  _withAxisOne(axis) {
    const ax = axis < 0 ? axis + this.ndim : axis;
    return this._shape.map((d, i) => (i === ax ? 1 : d));
  }

  /** Index of the first minimum (flat, or along `axis`). */
  argmin(axis) {
    return this._arg(axis, (x, best) => x < best);
  }
  argmax(axis) {
    return this._arg(axis, (x, best) => x > best);
  }

  _arg(axis, better) {
    if (axis === undefined) {
      let best = 0;
      for (let i = 1; i < this.size; i++) if (better(this._data[i], this._data[best])) best = i;
      return best;
    }
    const state = { best: NaN };
    return this._reduce(axis, -1, (acc, x, k) => {
      if (k === 0 || better(x, state.best)) {
        state.best = x;
        return k;
      }
      return acc;
    });
  }

  /** Whether shapes agree and every element differs by at most `tol`. */
  allclose(other, tol = 1e-9) {
    if (!(other instanceof Tensor) || other._shape.length !== this._shape.length || other._shape.some((d, i) => d !== this._shape[i])) return false;
    for (let i = 0; i < this.size; i++) if (!(Math.abs(this._data[i] - other._data[i]) <= tol)) return false;
    return true;
  }
}

function zeros(shape) {
  return Tensor.zeros(shape);
}
function ones(shape) {
  return Tensor.ones(shape);
}
function arange(start, stop, step) {
  return Tensor.arange(start, stop, step);
}
function linspace(start, stop, num) {
  return Tensor.linspace(start, stop, num);
}
function dot(a, b) {
  return a.dot(b);
}

module.exports = { Tensor, zeros, ones, arange, linspace, dot, fromImage: Tensor.fromImage };
