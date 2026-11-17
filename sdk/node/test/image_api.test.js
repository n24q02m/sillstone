// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 sillstone
"use strict";

// The `sillstone.image` argument checks and pixel accessors that the golden cases in
// image.test.js do not reach. Every refusal here happens in JavaScript, before the core
// is called.

const test = require("node:test");
const assert = require("node:assert/strict");
const sillstone = require("../index.js");

const { Image } = sillstone.image;

test("the constructor checks the size pair and the pixel buffer type", () => {
  assert.throws(() => new Image("L", 4, Buffer.alloc(4)), { name: "TypeError", message: "size must be [width, height]" });
  assert.throws(() => new Image("L", [2, 2, 1], Buffer.alloc(4)), { name: "TypeError", message: "size must be [width, height]" });
  assert.throws(() => new Image("L", [2, 2], [1, 2, 3, 4]), { name: "TypeError", message: "data must be a Buffer or Uint8Array" });
  const fromU8 = Image.frombytes("L", [2, 1], new Uint8Array([5, 6]));
  assert.deepEqual(Array.from(fromU8.tobytes()), [5, 6]);
});

test("new fills each pixel from one number or from one value per channel", () => {
  assert.deepEqual(Image.new("RGBA", [2, 1], [9, 8]).getpixel([1, 0]), [9, 8, 0, 0]);
  assert.deepEqual(Image.new("RGB", [1, 1], 4).getpixel([0, 0]), [4, 4, 4]);
  assert.equal(Image.new("L", [1, 1]).getpixel([0, 0]), 0);
});

test("putpixel writes one number to every channel or one value per channel", () => {
  const img = Image.new("RGB", [2, 2]);
  img.putpixel([0, 1], [1, 2]);
  img.putpixel([1, 1], 7);
  assert.deepEqual(img.getpixel([0, 1]), [1, 2, 0]);
  assert.deepEqual(img.getpixel([1, 1]), [7, 7, 7]);
  assert.deepEqual(img.getpixel([0, 0]), [0, 0, 0]);
  assert.throws(() => img.putpixel([0, -1], 1), { name: "RangeError", message: "pixel (0, -1) lies outside the 2x2 image" });
  assert.throws(() => img.getpixel([0.5, 0]), { name: "RangeError", message: "pixel (0.5, 0) lies outside the 2x2 image" });
});

test("operations refuse malformed arguments before calling the core", () => {
  const img = Image.new("RGB", [4, 3], [10, 20, 30]);
  const box = { name: "TypeError", message: "box must be [left, upper, right, lower] integers" };
  assert.throws(() => img.crop([0, 0, 2]), box);
  assert.throws(() => img.crop("0,0,2,2"), box);
  assert.throws(() => img.crop([0, 0, 2.5, 2]), box);
  const angle = { name: "TypeError", message: "angle must be a finite number" };
  assert.throws(() => img.rotate(Number.NaN), angle);
  assert.throws(() => img.rotate("90"), angle);
  assert.throws(() => img.brightness(Infinity), { name: "TypeError", message: "factor must be a finite number" });
  assert.throws(() => Image.qoiDecode(new Uint8Array(4)), { name: "TypeError", message: "bytes must be a Buffer" });
  assert.throws(() => img.convert("P"), { name: "RangeError", message: 'unsupported mode P; expected "L", "RGB" or "RGBA"' });
});

test("convert to the same mode returns an independent copy", () => {
  const img = Image.new("RGB", [1, 1], [1, 2, 3]);
  const copy = img.convert("RGB");
  assert.notEqual(copy, img);
  assert.equal(copy.mode, "RGB");
  copy.putpixel([0, 0], 9);
  assert.deepEqual(img.getpixel([0, 0]), [1, 2, 3]);
  assert.deepEqual(copy.getpixel([0, 0]), [9, 9, 9]);
});
