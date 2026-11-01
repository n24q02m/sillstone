"""Pillow Parity and Regression Verification against PIL.Image."""

import unittest
from PIL import Image as PilImage

from sillstone import Image as SillImage, Resampling, Transpose


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
        # Pillow's integer luma (19595/38470/7471 >> 16) bit for bit, and the
        # pure layout conversions, over every 8-bit value of each channel.
        n = 256 * 3
        noise = bytes((i * 73 + (i >> 3) * 29 + 11) % 256 for i in range(n * 4))
        for mode, raw in (("RGB", noise[: n * 3]), ("RGBA", noise[: n * 4])):
            pil_im = PilImage.frombytes(mode, (n, 1), raw)
            sill_im = SillImage.frombytes(mode, (n, 1), raw)
            for target in ("L", "RGB", "RGBA"):
                if target == mode:
                    continue
                self.assertEqual(
                    sill_im.convert(target).tobytes(),
                    pil_im.convert(target).tobytes(),
                    f"{mode} -> {target}",
                )
        gray = bytes(range(256))
        for target in ("RGB", "RGBA"):
            self.assertEqual(
                SillImage.frombytes("L", (256, 1), gray).convert(target).tobytes(),
                PilImage.frombytes("L", (256, 1), gray).convert(target).tobytes(),
                f"L -> {target}",
            )

    def test_resample_parity(self):
        # Bit-exact against Pillow for L / RGB / RGBA (RGBA is premultiplied
        # by Pillow, so alpha has to vary), every filter, up and down.
        filters = [
            ("NEAREST", Resampling.NEAREST, PilImage.Resampling.NEAREST),
            ("BOX", Resampling.BOX, PilImage.Resampling.BOX),
            ("BILINEAR", Resampling.BILINEAR, PilImage.Resampling.BILINEAR),
            ("BICUBIC", Resampling.BICUBIC, PilImage.Resampling.BICUBIC),
            ("LANCZOS", Resampling.LANCZOS, PilImage.Resampling.LANCZOS),
        ]
        sizes = [(8, 8), (32, 32), (5, 13), (3, 40), (16, 1), (23, 9)]
        w, h = 37, 29
        for mode, ch in (("L", 1), ("RGB", 3), ("RGBA", 4)):
            raw = bytes((i * 137 + (i >> 2) * 59 + 7) % 256 for i in range(w * h * ch))
            pil_im = PilImage.frombytes(mode, (w, h), raw)
            sill_im = SillImage.frombytes(mode, (w, h), raw)
            for size in sizes:
                for name, sill_f, pil_f in filters:
                    pil_resized = pil_im.resize(size, resample=pil_f)
                    sill_resized = sill_im.resize(size, resample=sill_f)
                    self.assertEqual(sill_resized.size, pil_resized.size)
                    self.assertEqual(
                        sill_resized.tobytes(),
                        pil_resized.tobytes(),
                        f"{mode} {w}x{h} -> {size} {name}",
                    )

    def test_resampling_ids_are_not_pillows_numeric_ids(self):
        # Sillstone numbers its filters NEAREST..LANCZOS as 0..4; Pillow's
        # enum differs (LANCZOS = 1, BOX = 4), so callers must map by name.
        self.assertEqual(int(PilImage.Resampling.LANCZOS), 1)
        self.assertEqual(Resampling.LANCZOS, 4)
        self.assertEqual(int(PilImage.Resampling.BOX), 4)
        self.assertEqual(Resampling.BOX, 1)


if __name__ == "__main__":
    unittest.main()
