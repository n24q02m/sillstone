// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 sillstone
"use strict";

// `sillstone.image`: the Pillow-shaped facade over the native image engine. Pixels are
// 8-bit, row-major, channel-interleaved: modes "L" (1), "RGB" (3) and "RGBA" (4), the
// same layouts the Python and Go SDKs expose. Every operation runs in the Rust core.

const { call } = require("./native");

const LAYOUT = Object.freeze({ L: 0, RGB: 2, RGBA: 4 });
const CHANNELS = Object.freeze({ L: 1, RGB: 3, RGBA: 4 });
const MODE_OF_LAYOUT = Object.freeze({ 0: "L", 2: "RGB", 4: "RGBA" });

const Resampling = Object.freeze({ NEAREST: 0, BOX: 1, BILINEAR: 2, BICUBIC: 3, LANCZOS: 4 });
const Transpose = Object.freeze({
  FLIP_LEFT_RIGHT: 0,
  FLIP_TOP_BOTTOM: 1,
  ROTATE_90: 2,
  ROTATE_180: 3,
  ROTATE_270: 4,
  TRANSPOSE: 5,
  TRANSVERSE: 6,
});
const ImageFilter = Object.freeze({
  BLUR: 0,
  SHARPEN: 1,
  CONTOUR: 2,
  DETAIL: 3,
  EDGE_ENHANCE: 4,
  FIND_EDGES: 5,
  EMBOSS: 6,
});

const U32_MAX = 0xffffffff;

function checkDim(value, name) {
  if (!Number.isInteger(value) || value < 1 || value > U32_MAX) {
    throw new RangeError(`${name} must be an integer in [1, ${U32_MAX}], got ${value}`);
  }
  return value;
}

function checkPair(pair, name) {
  if (!Array.isArray(pair) || pair.length !== 2) throw new TypeError(`${name} must be [width, height]`);
  return [checkDim(pair[0], `${name}[0]`), checkDim(pair[1], `${name}[1]`)];
}

function checkMode(mode) {
  if (!Object.hasOwn(LAYOUT, mode)) throw new RangeError(`unsupported mode ${mode}; expected "L", "RGB" or "RGBA"`);
  return mode;
}

class Image {
  /**
   * @param {"L"|"RGB"|"RGBA"} mode
   * @param {[number, number]} size `[width, height]`
   * @param {Buffer|Uint8Array} data interleaved pixels, `width * height * channels` bytes
   */
  constructor(mode, size, data) {
    checkMode(mode);
    const [w, h] = checkPair(size, "size");
    if (!(data instanceof Uint8Array)) throw new TypeError("data must be a Buffer or Uint8Array");
    const want = w * h * CHANNELS[mode];
    if (data.length !== want) {
      throw new RangeError(`data length ${data.length} does not match size ${w}x${h} and mode ${mode} (${want} bytes)`);
    }
    this._mode = mode;
    this._width = w;
    this._height = h;
    this._data = Buffer.from(data); // copy: an Image is immutable except through putpixel
  }

  get mode() {
    return this._mode;
  }
  get width() {
    return this._width;
  }
  get height() {
    return this._height;
  }
  get size() {
    return [this._width, this._height];
  }

  /** Raw interleaved pixel bytes (a copy). */
  tobytes() {
    return Buffer.from(this._data);
  }

  /** A new image filled with `color` (a number for every channel, or one value per channel). */
  static new(mode, size, color = 0) {
    checkMode(mode);
    const [w, h] = checkPair(size, "size");
    const ch = CHANNELS[mode];
    const px = Buffer.alloc(ch);
    for (let c = 0; c < ch; c++) px[c] = typeof color === "number" ? color : (color[c] ?? 0);
    const data = Buffer.alloc(w * h * ch);
    for (let i = 0; i < data.length; i += ch) px.copy(data, i);
    return new Image(mode, [w, h], data);
  }

  static frombytes(mode, size, data) {
    return new Image(mode, size, data);
  }

  /** Decodes a QOI stream. */
  static qoiDecode(bytes) {
    if (!Buffer.isBuffer(bytes)) throw new TypeError("bytes must be a Buffer");
    const res = call("sillstone_image_qoi_decode", bytes);
    const mode = MODE_OF_LAYOUT[res.layout];
    if (!mode) throw new RangeError(`decoded layout ${res.layout} has no 8-bit facade mode`);
    return new Image(mode, [res.width, res.height], res.data);
  }

  _op(name, outSize, ...scalars) {
    const [ow, oh] = outSize;
    const outLen = ow * oh * CHANNELS[this._mode];
    const out = call(name, this._data, this._width, this._height, LAYOUT[this._mode], ...scalars, outLen);
    return new Image(this._mode, outSize, out);
  }

  /** @param {[number, number]} size @param {number} resample a {@link Resampling} value */
  resize(size, resample = Resampling.BICUBIC) {
    const out = checkPair(size, "size");
    return this._op("sillstone_image_resize", out, out[0], out[1], resample);
  }

  /** Crops `[left, upper, right, lower]` (right/lower exclusive, inside the image). */
  crop(box) {
    if (!Array.isArray(box) || box.length !== 4 || !box.every(Number.isInteger)) {
      throw new TypeError("box must be [left, upper, right, lower] integers");
    }
    const [x0, y0, x1, y1] = box;
    if (x0 < 0 || y0 < 0 || x1 <= x0 || y1 <= y0 || x1 > this._width || y1 > this._height) {
      throw new RangeError(`box ${box} lies outside the ${this._width}x${this._height} image`);
    }
    return this._op("sillstone_image_crop", [x1 - x0, y1 - y0], x0, y0, x1, y1);
  }

  /** @param {number} method a {@link Transpose} value */
  transpose(method) {
    const swaps = [Transpose.ROTATE_90, Transpose.ROTATE_270, Transpose.TRANSPOSE, Transpose.TRANSVERSE].includes(method);
    const out = swaps ? [this._height, this._width] : [this._width, this._height];
    return this._op("sillstone_image_transpose", out, method);
  }

  /** Rotates counter-clockwise by `angle` degrees; `expand` grows the canvas to fit. */
  rotate(angle, resample = Resampling.BILINEAR, expand = false) {
    if (typeof angle !== "number" || !Number.isFinite(angle)) throw new TypeError("angle must be a finite number");
    const dims = call("sillstone_image_rotate_size", this._width, this._height, angle, expand ? 1 : 0);
    return this._op("sillstone_image_rotate", [dims.width, dims.height], angle, expand ? 1 : 0, resample);
  }

  /** @param {number} filterId an {@link ImageFilter} value */
  filter(filterId) {
    return this._op("sillstone_image_filter", this.size, filterId);
  }

  /** Scales every sample by `factor` (1 = identity, 0 = black, > 1 brighter), clamped. */
  brightness(factor) {
    if (typeof factor !== "number" || !Number.isFinite(factor)) throw new TypeError("factor must be a finite number");
    return this._op("sillstone_image_adjust", this.size, 0, factor);
  }

  invert() {
    return this._op("sillstone_image_adjust", this.size, 1, 0);
  }

  convert(mode) {
    checkMode(mode);
    if (mode === this._mode) return new Image(this._mode, this.size, this._data);
    const out = call(
      "sillstone_image_color_convert",
      this._data,
      this._width,
      this._height,
      LAYOUT[this._mode],
      LAYOUT[mode],
      this._width * this._height * CHANNELS[mode],
    );
    return new Image(mode, this.size, out);
  }

  /** Encodes as QOI (lossless). */
  qoiEncode() {
    return call("sillstone_image_qoi_encode", this._data, this._width, this._height, LAYOUT[this._mode]);
  }

  /** Encodes as BMP. */
  bmpEncode() {
    return call("sillstone_image_bmp_encode", this._data, this._width, this._height, LAYOUT[this._mode]);
  }

  /** The pixel at `(x, y)`: a number for "L", else an array of channel values. */
  getpixel([x, y]) {
    const off = this._offset(x, y);
    const ch = CHANNELS[this._mode];
    return ch === 1 ? this._data[off] : Array.from(this._data.subarray(off, off + ch));
  }

  putpixel([x, y], value) {
    const off = this._offset(x, y);
    const ch = CHANNELS[this._mode];
    for (let c = 0; c < ch; c++) this._data[off + c] = typeof value === "number" ? value : (value[c] ?? 0);
  }

  _offset(x, y) {
    if (!Number.isInteger(x) || !Number.isInteger(y) || x < 0 || y < 0 || x >= this._width || y >= this._height) {
      throw new RangeError(`pixel (${x}, ${y}) lies outside the ${this._width}x${this._height} image`);
    }
    return (y * this._width + x) * CHANNELS[this._mode];
  }

  _hash(name) {
    return call(name, this._data, this._width, this._height, LAYOUT[this._mode]);
  }

  /** 64-bit perceptual hash, bit-exact with `imagehash.phash` (BigInt). */
  phash() {
    return this._hash("sillstone_image_phash");
  }
  /** Bit-exact with `imagehash.average_hash`. */
  ahash() {
    return this._hash("sillstone_image_ahash");
  }
  /** Bit-exact with `imagehash.dhash`. */
  dhash() {
    return this._hash("sillstone_image_dhash");
  }
  /** Bit-exact with `imagehash.whash` (Haar, default scale). */
  whash() {
    return this._hash("sillstone_image_whash");
  }
  /** Bit-exact with `imagehash.colorhash` (3 bits per bin). */
  colorhash() {
    return this._hash("sillstone_image_colorhash");
  }
}

module.exports = { Image, Resampling, Transpose, ImageFilter, LAYOUT, CHANNELS };
