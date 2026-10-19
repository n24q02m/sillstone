"""Pillow Parity and Regression Verification against PIL.Image."""

import math
import os
import unittest
from PIL import Image as PilImage

from sillstone import Image as SillImage, Resampling, Transpose


def compute_psnr(data_a: bytes, data_b: bytes) -> float:
    if len(data_a) != len(data_b) or not data_a:
        return 0.0
    mse = sum((a - b) ** 2 for a, b in zip(data_a, data_b)) / len(data_a)
    if mse == 0:
        return 999.0
    return 10.0 * math.log10((255.0 ** 2) / mse)


class TestPillowParity(unittest.TestCase):
    def setUp(self):
        # Create a synthetic 16x16 gradient test image in PIL and Sillstone
        self.w = 16
        self.h = 16
        pixels = bytearray(self.w * self.h * 3)
        for y in range(self.h):
            for x in range(self.w):
                idx = (y * self.w + x) * 3
                pixels[idx] = (x * 255) // (self.w - 1)
                pixels[idx + 1] = (y * 255) // (self.h - 1)
                pixels[idx + 2] = ((x + y) * 255) // (self.w + self.h - 2)

        self.bytes = bytes(pixels)
        self.pil_im = PilImage.frombytes("RGB", (self.w, self.h), self.bytes)
        self.sill_im = SillImage.frombytes("RGB", (self.w, self.h), self.bytes)

    def test_crop_parity(self):
        box = (2, 3, 10, 12)
        pil_cropped = self.pil_im.crop(box)
        sill_cropped = self.sill_im.crop(box)

        self.assertEqual(sill_cropped.size, pil_cropped.size)
        self.assertEqual(sill_cropped.tobytes(), pil_cropped.tobytes())

    def test_transpose_parity(self):
        methods = [
            (Transpose.FLIP_LEFT_RIGHT, PilImage.Transpose.FLIP_LEFT_RIGHT),
            (Transpose.FLIP_TOP_BOTTOM, PilImage.Transpose.FLIP_TOP_BOTTOM),
            (Transpose.ROTATE_90, PilImage.Transpose.ROTATE_90),
            (Transpose.ROTATE_180, PilImage.Transpose.ROTATE_180),
            (Transpose.ROTATE_270, PilImage.Transpose.ROTATE_270),
        ]
        for sill_m, pil_m in methods:
            pil_res = self.pil_im.transpose(pil_m)
            sill_res = self.sill_im.transpose(sill_m)
            self.assertEqual(sill_res.size, pil_res.size)
            self.assertEqual(sill_res.tobytes(), pil_res.tobytes())

    def test_color_convert_parity(self):
        pil_gray = self.pil_im.convert("L")
        sill_gray = self.sill_im.convert("L")

        self.assertEqual(sill_gray.size, pil_gray.size)
        psnr = compute_psnr(sill_gray.tobytes(), pil_gray.tobytes())
        # BT.601 integer luma matches Pillow exactly or within rounding threshold (> 50 dB)
        self.assertGreater(psnr, 50.0)

    def test_resample_parity(self):
        # Test downsampling to 8x8 and upsampling to 32x32
        for target_size in [(8, 8), (32, 32)]:
            for sill_f, pil_f, min_psnr in [
                (Resampling.NEAREST, PilImage.Resampling.NEAREST, 28.0),
                (Resampling.BILINEAR, PilImage.Resampling.BILINEAR, 35.0),
                (Resampling.BICUBIC, PilImage.Resampling.BICUBIC, 35.0),
            ]:
                pil_resized = self.pil_im.resize(target_size, resample=pil_f)
                sill_resized = self.sill_im.resize(target_size, resample=sill_f)

                self.assertEqual(sill_resized.size, pil_resized.size)
                psnr = compute_psnr(sill_resized.tobytes(), pil_resized.tobytes())
                self.assertGreater(psnr, min_psnr, f"Failed at {target_size} with filter {sill_f}")

if __name__ == "__main__":
    unittest.main()
