"""Tests for Sillstone Pillow-compatible Image facade."""

import io
import unittest

from sillstone import Image, ImageFilter, Resampling, Transpose


class TestPillowImage(unittest.TestCase):
    def test_image_new_and_properties(self):
        im = Image.new("RGB", (10, 20), (255, 128, 0))
        self.assertEqual(im.size, (10, 20))
        self.assertEqual(im.width, 10)
        self.assertEqual(im.height, 20)
        self.assertEqual(im.mode, "RGB")
        self.assertEqual(len(im.tobytes()), 10 * 20 * 3)
        self.assertEqual(im.tobytes()[:3], bytes([255, 128, 0]))

    def test_image_resize(self):
        im = Image.new("RGB", (4, 4), (100, 150, 200))
        resized = im.resize((8, 8), resample=Resampling.BICUBIC)
        self.assertEqual(resized.size, (8, 8))
        self.assertEqual(resized.mode, "RGB")
        # Constant color should remain constant
        self.assertEqual(resized.tobytes()[:3], bytes([100, 150, 200]))

    def test_image_crop(self):
        # Create a 4x4 image
        pixels = bytearray(4 * 4 * 3)
        for i in range(16):
            pixels[i * 3] = i
        im = Image.frombytes("RGB", (4, 4), bytes(pixels))
        cropped = im.crop((1, 1, 3, 3))
        self.assertEqual(cropped.size, (2, 2))
        self.assertEqual(cropped.mode, "RGB")

    def test_image_transpose(self):
        im = Image.new("RGB", (4, 6), (10, 20, 30))
        rot = im.transpose(Transpose.ROTATE_90)
        self.assertEqual(rot.size, (6, 4))
        self.assertEqual(rot.mode, "RGB")

    def test_image_convert(self):
        im = Image.new("RGB", (2, 2), (255, 255, 255))
        gray = im.convert("L")
        self.assertEqual(gray.size, (2, 2))
        self.assertEqual(gray.mode, "L")
        self.assertEqual(gray.tobytes(), bytes([255, 255, 255, 255]))

    def test_image_save_qoi_and_open(self):
        im = Image.new("RGB", (8, 8), (42, 84, 126))
        buf = io.BytesIO()
        im.save(buf, format="QOI")
        data = buf.getvalue()
        self.assertTrue(data.startswith(b"qoif"))

        loaded = Image.open(data)
        self.assertEqual(loaded.size, (8, 8))
        self.assertEqual(loaded.mode, "RGB")
        self.assertEqual(loaded.tobytes(), im.tobytes())

    def test_image_save_bmp_and_open(self):
        im = Image.new("RGB", (4, 4), (70, 80, 90))
        buf = io.BytesIO()
        im.save(buf, format="BMP")
        data = buf.getvalue()
        self.assertTrue(data.startswith(b"BM"))

        loaded = Image.open(data)
        self.assertEqual(loaded.size, (4, 4))
        self.assertEqual(loaded.mode, "RGB")
        self.assertEqual(loaded.tobytes(), im.tobytes())

    def test_image_hashes(self):
        im = Image.new("RGB", (32, 32), (120, 120, 120))
        p = im.phash()
        a = im.ahash()
        d = im.dhash()
        w = im.whash()
        c = im.colorhash()
        self.assertIsInstance(p, int)
        self.assertIsInstance(a, int)
        self.assertIsInstance(d, int)
        self.assertIsInstance(w, int)
        self.assertIsInstance(c, int)


if __name__ == "__main__":
    unittest.main()
