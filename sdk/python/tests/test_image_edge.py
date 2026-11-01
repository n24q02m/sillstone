"""Edge cases of the Python ``Image`` facade: pixel access, array conversion, error paths.

``putpixel`` used to raise ``TypeError`` for every call (the pixels are immutable ``bytes``),
``__array_interface__`` used to raise on a read-only buffer, and ``from_array`` accepted
non-uint8 arrays only to fail later with a misleading length error. Pillow, when installed,
is the oracle for the pixel semantics.
"""

import io
import os
import sys
import unittest

sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), "..")))

from sillstone.array import array as sill_array_fn
from sillstone.image import Image, ImageFilter, Transpose

try:
    import numpy as np
except ImportError:  # pragma: no cover - optional
    np = None

try:
    from PIL import Image as PilImage
except ImportError:  # pragma: no cover - optional
    PilImage = None


class PutPixelTests(unittest.TestCase):
    def test_putpixel_writes_only_the_addressed_pixel(self):
        img = Image.new("RGB", (3, 2), (1, 2, 3))
        before = img.tobytes()
        img.putpixel((1, 1), (9, 8, 7))
        self.assertEqual(img.getpixel((1, 1)), (9, 8, 7))
        expected = bytearray(before)
        expected[(1 * 3 + 1) * 3 : (1 * 3 + 1) * 3 + 3] = bytes((9, 8, 7))
        self.assertEqual(img.tobytes(), bytes(expected))

    def test_putpixel_does_not_change_a_previously_taken_bytes_snapshot(self):
        img = Image.new("L", (2, 1), 5)
        snapshot = img.tobytes()
        img.putpixel((0, 0), 200)
        self.assertEqual(snapshot, b"\x05\x05")
        self.assertEqual(img.tobytes(), b"\xc8\x05")

    def test_putpixel_accepts_the_value_shapes_pillow_accepts(self):
        cases = [
            ("L", 5),
            ("L", (5,)),
            ("RGB", (1, 2, 3)),
            ("RGB", (1, 2, 3, 4)),  # the fourth value is dropped
            ("RGBA", (1, 2, 3)),  # opaque
            ("RGBA", (1, 2, 3, 4)),
        ]
        for mode, value in cases:
            with self.subTest(mode=mode, value=value):
                img = Image.new(mode, (2, 1))
                img.putpixel((1, 0), value)
                self.assertEqual(img.getpixel((0, 0)), 0 if mode == "L" else (0,) * len(mode))
                got = img.getpixel((1, 0))
                if PilImage is not None:
                    ref = PilImage.new(mode, (2, 1))
                    ref.putpixel((1, 0), value)
                    self.assertEqual(got, ref.getpixel((1, 0)))
                else:
                    self.assertIsNotNone(got)

    def test_putpixel_clamps_like_pillow(self):
        for mode, value in [("L", 300), ("L", -1), ("RGB", (1, 2, 300)), ("RGB", (-5, 0, 255))]:
            with self.subTest(mode=mode, value=value):
                img = Image.new(mode, (1, 1))
                img.putpixel((0, 0), value)
                if PilImage is not None:
                    ref = PilImage.new(mode, (1, 1))
                    ref.putpixel((0, 0), value)
                    self.assertEqual(img.getpixel((0, 0)), ref.getpixel((0, 0)))
        img = Image.new("L", (1, 1))
        img.putpixel((0, 0), 300)
        self.assertEqual(img.getpixel((0, 0)), 255)
        img.putpixel((0, 0), -1)
        self.assertEqual(img.getpixel((0, 0)), 0)

    def test_putpixel_rejects_wrong_channel_count_and_leaves_the_image_alone(self):
        img = Image.new("RGB", (1, 1), (1, 2, 3))
        for bad in [(1, 2), 5, (1,), ()]:
            with self.subTest(value=bad):
                with self.assertRaises(TypeError):
                    img.putpixel((0, 0), bad)
        self.assertEqual(img.getpixel((0, 0)), (1, 2, 3))
        with self.assertRaises(TypeError):
            Image.new("L", (1, 1)).putpixel((0, 0), (1, 2, 3))

    def test_pixel_access_out_of_range_raises_index_error(self):
        img = Image.new("L", (2, 2))
        for xy in [(2, 0), (0, 2), (-1, 0), (0, -1)]:
            with self.subTest(xy=xy):
                with self.assertRaises(IndexError):
                    img.getpixel(xy)
                with self.assertRaises(IndexError):
                    img.putpixel(xy, 1)


class ArrayConversionTests(unittest.TestCase):
    def test_from_array_keeps_row_major_pixels(self):
        gray = Image.from_array(sill_array_fn([[1, 2, 3], [4, 5, 6]], dtype="uint8"))
        self.assertEqual((gray.mode, gray.size, gray.tobytes()), ("L", (3, 2), bytes([1, 2, 3, 4, 5, 6])))
        rgb = Image.from_array(sill_array_fn([[[1, 2, 3], [4, 5, 6]]], dtype="uint8"))
        self.assertEqual((rgb.mode, rgb.size), ("RGB", (2, 1)))
        self.assertEqual(rgb.getpixel((1, 0)), (4, 5, 6))
        rgba = Image.from_array(sill_array_fn([[[1, 2, 3, 4]]], dtype="uint8"))
        self.assertEqual((rgba.mode, rgba.getpixel((0, 0))), ("RGBA", (1, 2, 3, 4)))
        single = Image.from_array(sill_array_fn([[[7], [8]]], dtype="uint8"))
        self.assertEqual((single.mode, single.tobytes()), ("L", b"\x07\x08"))

    def test_from_array_of_a_non_contiguous_view_follows_the_view(self):
        view = sill_array_fn([[1, 2, 3], [4, 5, 6]], dtype="uint8")[:, ::2]
        self.assertEqual(Image.from_array(view).tobytes(), bytes([1, 3, 4, 6]))

    def test_from_array_refuses_other_dtypes_instead_of_reading_wide_elements(self):
        for dtype in ["int8", "uint16", "int32", "float32", "float64", "bool"]:
            with self.subTest(dtype=dtype):
                with self.assertRaises(TypeError):
                    Image.from_array(sill_array_fn([[1, 0], [0, 1]], dtype=dtype))

    def test_from_array_validates_dimensions_mode_and_channels(self):
        with self.assertRaises(ValueError):
            Image.from_array(sill_array_fn([1, 2, 3], dtype="uint8"))
        with self.assertRaises(ValueError):
            Image.from_array(sill_array_fn([[[[1]]]], dtype="uint8"))
        two_channels = sill_array_fn([[[1, 2]]], dtype="uint8")
        with self.assertRaisesRegex(ValueError, "channel"):
            Image.from_array(two_channels)
        rgb = sill_array_fn([[[1, 2, 3]]], dtype="uint8")
        with self.assertRaisesRegex(ValueError, "channel"):
            Image.from_array(rgb, "L")
        with self.assertRaisesRegex(ValueError, "channel"):
            Image.from_array(sill_array_fn([[1, 2]], dtype="uint8"), "RGB")
        with self.assertRaisesRegex(ValueError, "Unsupported mode"):
            Image.from_array(rgb, "CMYK")
        self.assertEqual(Image.from_array(rgb, "RGB").mode, "RGB")

    def test_to_array_round_trips_through_from_array(self):
        for mode, shape in [("L", (3, 2)), ("RGB", (2, 2)), ("RGBA", (2, 1))]:
            with self.subTest(mode=mode):
                img = Image.new(mode, shape, tuple(range(10, 14)) if mode != "L" else 77)
                back = Image.from_array(img.to_array())
                self.assertEqual((back.mode, back.size, back.tobytes()), (img.mode, img.size, img.tobytes()))

    @unittest.skipIf(np is None, "NumPy not installed")
    def test_array_interface_exposes_the_pixels_to_numpy(self):
        img = Image.new("RGB", (3, 2), (1, 2, 3))
        img.putpixel((2, 1), (200, 100, 50))
        arr = np.asarray(img)
        self.assertEqual((arr.dtype, arr.shape), (np.uint8, (2, 3, 3)))
        self.assertEqual(arr[1, 2].tolist(), [200, 100, 50])
        self.assertEqual(arr[0, 0].tolist(), [1, 2, 3])
        self.assertEqual(np.asarray(Image.new("L", (4, 3), 9)).shape, (3, 4))
        self.assertEqual(img.__array_interface__["version"], 3)

    @unittest.skipIf(np is None, "NumPy not installed")
    def test_from_array_accepts_numpy_uint8_and_refuses_other_dtypes(self):
        data = np.arange(12, dtype=np.uint8).reshape(2, 2, 3)
        img = Image.from_array(data)
        self.assertEqual((img.mode, img.size, img.tobytes()), ("RGB", (2, 2), data.tobytes()))
        transposed = Image.from_array(np.ascontiguousarray(data[:, ::-1]))
        self.assertEqual(transposed.getpixel((0, 0)), tuple(data[0, 1].tolist()))
        # A non-contiguous NumPy view is read in logical order.
        self.assertEqual(Image.from_array(data[:, ::-1]).tobytes(), data[:, ::-1].tobytes())
        for dtype in (np.uint16, np.float32, np.int8):
            with self.subTest(dtype=dtype):
                with self.assertRaises(TypeError):
                    Image.from_array(np.zeros((2, 2), dtype=dtype))


class ErrorPathTests(unittest.TestCase):
    """Inputs the native engine rejects must surface as ``RuntimeError``, not garbage."""

    def setUp(self):
        self.rgb = Image.new("RGB", (4, 4), (1, 2, 3))
        self.gray = Image.new("L", (4, 4), 7)

    def test_resize_rejects_zero_sized_targets(self):
        for size in [(0, 3), (3, 0), (0, 0)]:
            with self.subTest(size=size):
                with self.assertRaisesRegex(RuntimeError, "Resize failed"):
                    self.rgb.resize(size)

    def test_crop_rejects_boxes_outside_the_image_or_inverted(self):
        for box in [(0, 0, 9, 9), (3, 3, 1, 1), (-1, 0, 2, 2)]:
            with self.subTest(box=box):
                with self.assertRaisesRegex(RuntimeError, "Crop failed"):
                    self.rgb.crop(box)
        self.assertEqual(self.rgb.crop((1, 1, 3, 4)).size, (2, 3))

    def test_transpose_rejects_unknown_methods_and_flips_dimensions(self):
        with self.assertRaisesRegex(RuntimeError, "Transpose failed"):
            self.rgb.transpose(99)
        wide = Image.new("L", (3, 1))
        self.assertEqual(wide.transpose(Transpose.ROTATE_90).size, (1, 3))
        self.assertEqual(wide.transpose(Transpose.FLIP_LEFT_RIGHT).size, (3, 1))

    def test_rotate_rejects_non_finite_angles(self):
        for angle in [float("nan"), float("inf")]:
            with self.subTest(angle=angle):
                with self.assertRaisesRegex(RuntimeError, "Rotate failed"):
                    self.rgb.rotate(angle)
        self.assertEqual(self.rgb.rotate(90).size, (4, 4))

    def test_filter_rejects_unknown_filters(self):
        with self.assertRaisesRegex(RuntimeError, "Filter failed"):
            self.rgb.filter(99)
        self.assertEqual(self.rgb.filter(ImageFilter.BLUR).size, (4, 4))

    def test_convert_validates_the_mode_and_copies_for_the_same_mode(self):
        with self.assertRaisesRegex(ValueError, "Unsupported mode"):
            self.rgb.convert("CMYK")
        same = self.rgb.convert("RGB")
        self.assertEqual((same.mode, same.tobytes()), ("RGB", self.rgb.tobytes()))
        self.assertEqual(self.gray.convert("RGBA").tobytes(), bytes([7, 7, 7, 255]) * 16)

    def test_hashes_reject_empty_images_and_bad_parameters(self):
        empty = Image.new("RGB", (0, 0))
        for name in ["phash", "ahash", "dhash"]:
            with self.subTest(hash=name):
                with self.assertRaises(RuntimeError):
                    getattr(empty, name)()
        with self.assertRaisesRegex(RuntimeError, "wHash failed"):
            self.gray.whash(99)
        with self.assertRaisesRegex(RuntimeError, "colorhash failed"):
            self.rgb.colorhash(99)

    def test_save_rejects_empty_images_and_unknown_formats(self):
        empty = Image.new("RGB", (0, 0))
        with self.assertRaisesRegex(RuntimeError, "QOI encoding failed"):
            empty.save(io.BytesIO())
        with self.assertRaisesRegex(RuntimeError, "BMP encoding failed"):
            empty.save(io.BytesIO(), "BMP")
        with self.assertRaisesRegex(ValueError, "Unsupported format"):
            self.rgb.save(io.BytesIO(), "PNG")

    def test_open_rejects_a_corrupt_qoi_and_unknown_signatures(self):
        with self.assertRaisesRegex(RuntimeError, "QOI decoding failed"):
            Image.open(b"qoif" + b"\x00" * 20)
        with self.assertRaisesRegex(ValueError, "Unsupported or unrecognized"):
            Image.open(b"\x89PNG\r\n\x1a\n")

    def test_constructor_validates_mode_and_length(self):
        with self.assertRaisesRegex(ValueError, "Unsupported mode"):
            Image("CMYK", (1, 1), b"\x00")
        with self.assertRaisesRegex(ValueError, "does not match"):
            Image("RGB", (2, 2), b"\x00" * 11)

    def test_new_pads_short_color_tuples_with_zero(self):
        self.assertEqual(Image.new("RGB", (1, 1), (5,)).tobytes(), b"\x05\x00\x00")
        self.assertEqual(Image.new("RGBA", (1, 1), (5, 6)).tobytes(), b"\x05\x06\x00\x00")


class OpenSourceTests(unittest.TestCase):
    """``Image.open`` reads a path, bytes or a file-like object."""

    def setUp(self):
        self.img = Image.new("RGB", (3, 2), (10, 20, 30))
        self.img.putpixel((2, 1), (1, 2, 3))
        buf = io.BytesIO()
        self.img.save(buf, "QOI")
        self.qoi = buf.getvalue()

    def test_open_accepts_bytes_file_object_and_path(self):
        from_bytes = Image.open(self.qoi)
        from_file = Image.open(io.BytesIO(self.qoi))
        for opened in (from_bytes, from_file):
            self.assertEqual((opened.mode, opened.size, opened.tobytes()), ("RGB", (3, 2), self.img.tobytes()))
        import tempfile

        with tempfile.TemporaryDirectory() as tmp:
            path = os.path.join(tmp, "x.qoi")
            self.img.save(path)
            self.assertEqual(Image.open(path).tobytes(), self.img.tobytes())
            bmp_path = os.path.join(tmp, "x.bmp")
            self.img.save(bmp_path, "bmp")
            self.assertEqual(Image.open(bmp_path).tobytes(), self.img.tobytes())

    def test_open_refuses_bmp_variants_the_python_decoder_does_not_support(self):
        buf = io.BytesIO()
        Image.new("RGB", (1, 1)).save(buf, "BMP")
        bmp = bytearray(buf.getvalue())
        bmp[28:30] = (8).to_bytes(2, "little")  # 8 bits per pixel
        with self.assertRaises(NotImplementedError):
            Image.open(bytes(bmp))
        with self.assertRaisesRegex(ValueError, "truncated"):
            Image.open(b"BM" + b"\x00" * 20)


if __name__ == "__main__":
    unittest.main()
