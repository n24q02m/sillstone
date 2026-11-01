"""BMP decoder hardening: forged headers must be rejected before any large allocation.

A BMP header is attacker-controlled. The decoder used to allocate ``width * height * channels``
bytes straight from it, so a ~100 byte file with a 65535x65535 header asked for ~12 GiB.
Every case below must raise ``ValueError`` quickly and keep Python-level allocation tiny.
"""

import os
import struct
import sys
import time
import tracemalloc
import unittest

sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), "..")))

from sillstone.image import MAX_BUFFER_BYTES, Image

INT32_MAX = 2**31 - 1
INT32_MIN = -(2**31)


def forge(
    w,
    h,
    bpp,
    offset=54,
    pixels=b"\x00" * 100,
    header_size=40,
    compression=0,
    masks=b"",
):
    """A BMP with the given (possibly lying) header fields followed by ``pixels``."""
    info = struct.pack("<IiiHHIIiiII", header_size, w, h, 1, bpp, compression, 0, 2835, 2835, 0, 0)
    body = info + masks
    total = 14 + len(body) + len(pixels)
    return b"BM" + struct.pack("<IHHI", total, 0, 0, offset) + body + pixels


def valid24(w, h, rgb, pad_last_row=True):
    """Bottom-up 24-bit BMP from top-down RGB bytes."""
    stride = (w * 3 + 3) & ~3
    rows = []
    for y in range(h - 1, -1, -1):
        row = b"".join(bytes((rgb[(y * w + x) * 3 + 2], rgb[(y * w + x) * 3 + 1], rgb[(y * w + x) * 3])) for x in range(w))
        rows.append(row + b"\x00" * (stride - w * 3))
    pixels = b"".join(rows)
    if not pad_last_row:
        pixels = pixels[: len(pixels) - (stride - w * 3)]
    return forge(w, h, 24, pixels=pixels)


class TestForgedHeadersAreRejectedCheaply(unittest.TestCase):
    def assertRejected(self, data, message=None):
        tracemalloc.start()
        started = time.perf_counter()
        try:
            with self.assertRaises(ValueError) as caught:
                Image.open(data)
            elapsed = time.perf_counter() - started
            _, peak = tracemalloc.get_traced_memory()
        finally:
            tracemalloc.stop()
        self.assertLess(elapsed, 1.0, "rejection must be fast")
        self.assertLess(peak, 4 << 20, f"peak Python allocation {peak} bytes")
        if message is not None:
            self.assertIn(message, str(caught.exception))

    def test_huge_dimensions_24bit(self):
        self.assertRejected(forge(65535, 65535, 24), "raster limit")
        self.assertRejected(forge(INT32_MAX, INT32_MAX, 24), "raster limit")
        self.assertRejected(forge(INT32_MAX, 1, 24), "raster limit")
        self.assertRejected(forge(INT32_MIN, 1, 24), "raster limit")
        self.assertRejected(forge(1, INT32_MIN, 24), "raster limit")
        self.assertRejected(forge(65535, -65535, 24), "raster limit")

    def test_huge_dimensions_32bit(self):
        self.assertRejected(forge(65535, 65535, 32), "raster limit")
        self.assertRejected(forge(INT32_MAX, INT32_MAX, 32), "raster limit")
        self.assertRejected(forge(INT32_MIN, 1, 32), "raster limit")

    def test_huge_dimensions_32bit_bitfields(self):
        masks = struct.pack("<IIII", 0x00FF0000, 0x0000FF00, 0x000000FF, 0xFF000000)
        data = forge(65535, 65535, 32, offset=70, header_size=56, compression=3, masks=masks)
        self.assertRejected(data, "raster limit")

    def test_just_over_the_raster_limit(self):
        # 3 * 18919 * 18919 = 1_073_815_083 > 2**30 > 3 * 18918 * 18918
        self.assertGreater(3 * 18919 * 18919, MAX_BUFFER_BYTES)
        self.assertLessEqual(3 * 18918 * 18918, MAX_BUFFER_BYTES)
        self.assertRejected(forge(18919, 18919, 24), "raster limit")
        # Under the limit the file is still rejected, because the pixel data is not there.
        self.assertRejected(forge(18918, 18918, 24), "truncated")

    def test_zero_width_or_height(self):
        for bpp in (24, 32):
            self.assertRejected(forge(0, 10, bpp), "empty image size")
            self.assertRejected(forge(10, 0, bpp), "empty image size")
            self.assertRejected(forge(0, 0, bpp), "empty image size")
            self.assertRejected(forge(0, INT32_MAX, bpp), "empty image size")

    def test_pixel_offset_out_of_range(self):
        for bpp in (24, 32):
            self.assertRejected(forge(2, 2, bpp, offset=0), "offset")
            self.assertRejected(forge(2, 2, bpp, offset=53), "offset")
            self.assertRejected(forge(2, 2, bpp, offset=14), "offset")
            self.assertRejected(forge(2, 2, bpp, offset=2**32 - 1), "offset")
            self.assertRejected(forge(2, 2, bpp, offset=54 + 100 + 1), "offset")

    def test_offset_at_end_of_file_means_no_pixels(self):
        for bpp in (24, 32):
            self.assertRejected(forge(1, 1, bpp, offset=54 + 100), "truncated")

    def test_truncated_pixel_data(self):
        self.assertRejected(forge(10, 10, 24, pixels=b"\x00" * 100), "truncated")  # needs 10*3 padded to 32 * 9 + 30
        self.assertRejected(forge(10, 10, 32, pixels=b"\x00" * 399), "truncated")  # needs 400
        self.assertRejected(forge(1, 1, 24, pixels=b""), "truncated")
        self.assertRejected(forge(1, 1, 32, pixels=b"\x00\x00\x00"), "truncated")
        self.assertRejected(forge(-3, -3, 24, pixels=b"\x00" * 10), "truncated")

    def test_truncated_headers(self):
        good = forge(2, 2, 24, pixels=b"\x00" * 16)
        for cut in (2, 10, 14, 26, 30, 53):
            self.assertRejected(good[:cut], "truncated BMP header")

    def test_info_header_smaller_than_bitmapinfoheader(self):
        for size in (0, 12, 39):
            self.assertRejected(forge(2, 2, 24, header_size=size, pixels=b"\x00" * 16), "header size")


class TestValidBmpStillDecodes(unittest.TestCase):
    RGB = bytes(range(1, 1 + 3 * 3 * 2))  # 3x2 pixels

    def test_padded_24bit(self):
        im = Image.open(valid24(3, 2, self.RGB))
        self.assertEqual((im.mode, im.size, im.tobytes()), ("RGB", (3, 2), self.RGB))

    def test_24bit_last_row_without_padding(self):
        im = Image.open(valid24(3, 2, self.RGB, pad_last_row=False))
        self.assertEqual(im.tobytes(), self.RGB)

    def test_top_down_24bit(self):
        top_down = bytearray(valid24(3, 2, self.RGB))
        # flip to top-down: negative height and rows in file order = image order
        stride = 12
        rows = [bytes(top_down[54 + i * stride : 54 + (i + 1) * stride]) for i in range(2)]
        top_down[54:] = rows[1] + rows[0]
        top_down[22:26] = struct.pack("<i", -2)
        self.assertEqual(Image.open(bytes(top_down)).tobytes(), self.RGB)

    def test_roundtrip_through_own_encoder_for_every_mode(self):
        import io

        for mode, ch in (("RGB", 3), ("RGBA", 4)):
            src = Image.frombytes(mode, (5, 3), bytes((i * 7) % 256 for i in range(5 * 3 * ch)))
            buf = io.BytesIO()
            src.save(buf, format="BMP")
            out = Image.open(buf.getvalue())
            self.assertEqual((out.mode, out.size), (mode, (5, 3)))
            self.assertEqual(out.tobytes(), src.tobytes())

    def test_one_by_one(self):
        self.assertEqual(Image.open(valid24(1, 1, b"\x0a\x0b\x0c")).tobytes(), b"\x0a\x0b\x0c")


if __name__ == "__main__":
    unittest.main()
