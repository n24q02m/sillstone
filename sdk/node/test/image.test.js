const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");
const sillstone = require("../index.js");

const { Image, Resampling, Transpose, ImageFilter } = sillstone.image;
const golden = JSON.parse(fs.readFileSync(path.join(__dirname, "..", "..", "..", "tests", "fixtures", "facade", "golden.json"), "utf8"));

function fromGolden(rec) {
  return Image.frombytes(rec.mode, [rec.width, rec.height], Buffer.from(rec.hex, "hex"));
}

function expectImage(actual, name) {
  const want = golden.ops[name];
  assert.deepStrictEqual([actual.mode, actual.width, actual.height], [want.mode, want.width, want.height], `${name} geometry`);
  assert.strictEqual(actual.tobytes().toString("hex"), want.hex, name);
}

const base = fromGolden(golden.image);
const rgba = fromGolden(golden.image_rgba);

test("resize matches the Python SDK byte-for-byte for every filter", () => {
  for (const name of ["NEAREST", "BOX", "BILINEAR", "BICUBIC", "LANCZOS"]) {
    expectImage(base.resize([5, 4], Resampling[name]), `resize_${name.toLowerCase()}_5x4`);
  }
  expectImage(base.resize([14, 11], Resampling.BICUBIC), "resize_bicubic_14x11");
  expectImage(rgba.resize([3, 8], Resampling.BILINEAR), "resize_rgba_bilinear_3x8");
});

test("crop, transpose, rotate, filter, convert and adjust match Python", () => {
  expectImage(base.crop([2, 1, 8, 6]), "crop_2_1_8_6");
  for (const name of Object.keys(Transpose)) expectImage(base.transpose(Transpose[name]), `transpose_${name.toLowerCase()}`);
  expectImage(base.rotate(30, Resampling.BILINEAR, false), "rotate_30_bilinear");
  expectImage(base.rotate(30, Resampling.BILINEAR, true), "rotate_30_bilinear_expand");
  expectImage(base.rotate(45, Resampling.NEAREST, true), "rotate_45_nearest_expand");
  expectImage(base.rotate(90, Resampling.BILINEAR, false), "rotate_90");
  expectImage(base.rotate(-20, Resampling.NEAREST, false), "rotate_neg20_nearest");
  for (const name of Object.keys(ImageFilter)) expectImage(base.filter(ImageFilter[name]), `filter_${name.toLowerCase()}`);
  expectImage(base.convert("L"), "convert_rgb_to_l");
  expectImage(base.convert("RGBA"), "convert_rgb_to_rgba");
  expectImage(rgba.convert("RGB"), "convert_rgba_to_rgb");
  expectImage(base.brightness(1.5), "brightness_1_5");
  expectImage(base.invert(), "invert");
});

test("QOI and BMP encoders are byte-identical to Python and QOI round-trips", () => {
  assert.strictEqual(base.qoiEncode().toString("hex"), golden.qoi_rgb);
  assert.strictEqual(base.bmpEncode().toString("hex"), golden.bmp_rgb);
  assert.strictEqual(rgba.qoiEncode().toString("hex"), golden.qoi_rgba);
  const back = Image.qoiDecode(Buffer.from(golden.qoi_rgba, "hex"));
  assert.strictEqual(back.mode, "RGBA");
  assert.deepStrictEqual(back.size, [6, 5]);
  assert.ok(back.tobytes().equals(rgba.tobytes()));
});

test("image hashes equal the imagehash-exact Python values", () => {
  assert.strictEqual(base.phash(), BigInt(golden.hashes.phash));
  assert.strictEqual(base.ahash(), BigInt(golden.hashes.ahash));
  assert.strictEqual(base.dhash(), BigInt(golden.hashes.dhash));
  assert.strictEqual(base.whash(), BigInt(golden.hashes.whash));
  assert.strictEqual(base.colorhash(), BigInt(golden.hashes.colorhash));
});

test("rotate swaps the dimensions of a non-square 90 degree turn", () => {
  const wide = Image.new("RGB", [6, 2], [10, 20, 30]);
  const turned = wide.rotate(90);
  assert.deepStrictEqual(turned.size, [2, 6]);
});

test("constructors and accessors validate their input", () => {
  assert.throws(() => new Image("RGB", [2, 2], Buffer.alloc(11)), RangeError);
  assert.throws(() => new Image("CMYK", [1, 1], Buffer.alloc(4)), RangeError);
  assert.throws(() => base.crop([0, 0, 10, 7]), RangeError);
  assert.throws(() => base.resize([0, 4]), RangeError);
  assert.throws(() => base.getpixel([9, 0]), RangeError);
  const img = Image.new("L", [2, 2], 7);
  img.putpixel([1, 1], 200);
  assert.strictEqual(img.getpixel([1, 1]), 200);
  assert.strictEqual(img.getpixel([0, 0]), 7);
  assert.throws(() => Image.qoiDecode(Buffer.from("not qoi")), sillstone.FfiError);
});
