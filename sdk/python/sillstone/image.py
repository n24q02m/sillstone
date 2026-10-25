"""Sillstone Image: Pure-Rust zero-dependency Pillow-compatible Image engine."""

from __future__ import annotations

import ctypes
import io
import os
from typing import BinaryIO, Tuple, Union

from ._generated import (
    sillstone_image_adjust_raw,
    sillstone_image_ahash_raw,
    sillstone_image_bmp_encode_raw,
    sillstone_image_color_convert_raw,
    sillstone_image_colorhash_raw,
    sillstone_image_crop_raw,
    sillstone_image_dhash_raw,
    sillstone_image_filter_raw,
    sillstone_image_free_bytes_raw,
    sillstone_image_phash_raw,
    sillstone_image_qoi_decode_raw,
    sillstone_image_qoi_encode_raw,
    sillstone_image_resize_raw,
    sillstone_image_rotate_raw,
    sillstone_image_transpose_raw,
    sillstone_image_whash_raw,
)

LAYOUT_GRAY8 = 0
LAYOUT_GRAY16 = 1
LAYOUT_RGB8 = 2
LAYOUT_RGB16 = 3
LAYOUT_RGBA8 = 4

MODE_TO_LAYOUT = {
    "L": LAYOUT_GRAY8,
    "RGB": LAYOUT_RGB8,
    "RGBA": LAYOUT_RGBA8,
}

LAYOUT_TO_MODE = {
    LAYOUT_GRAY8: "L",
    LAYOUT_RGB8: "RGB",
    LAYOUT_RGBA8: "RGBA",
}

CHANNELS = {
    "L": 1,
    "RGB": 3,
    "RGBA": 4,
}


class Resampling:
    """Resampling filter identifiers matching Pillow."""
    NEAREST = 0
    BOX = 1
    BILINEAR = 2
    BICUBIC = 3
    LANCZOS = 4


class Transpose:
    """Transpose operations matching PIL.Image.Transpose."""
    FLIP_LEFT_RIGHT = 0
    FLIP_TOP_BOTTOM = 1
    ROTATE_90 = 2
    ROTATE_180 = 3
    ROTATE_270 = 4
    TRANSPOSE = 5
    TRANSVERSE = 6


class ImageFilter:
    """Built-in 2D convolution filters matching PIL.ImageFilter."""
    BLUR = 0
    SHARPEN = 1
    CONTOUR = 2
    DETAIL = 3
    EDGE_ENHANCE = 4
    FIND_EDGES = 5
    EMBOSS = 6


class Image:
    """Pillow-compatible Image object powered by the Sillstone zero-dependency Rust engine."""

    def __init__(self, mode: str, size: Tuple[int, int], data: bytes) -> None:
        if mode not in MODE_TO_LAYOUT:
            raise ValueError(f"Unsupported mode {mode}; expected 'L', 'RGB', or 'RGBA'")
        self._mode = mode
        self._width, self._height = size
        expected_len = self._width * self._height * CHANNELS[mode]
        if len(data) != expected_len:
            raise ValueError(f"Data length {len(data)} does not match size {size} and mode {mode}")
        self._data = bytes(data)

    @property
    def mode(self) -> str:
        """Color mode: 'L', 'RGB', or 'RGBA'."""
        return self._mode

    @property
    def size(self) -> Tuple[int, int]:
        """(width, height) in pixels."""
        return (self._width, self._height)

    @property
    def width(self) -> int:
        """Image width in pixels."""
        return self._width

    @property
    def height(self) -> int:
        """Image height in pixels."""
        return self._height

    def tobytes(self) -> bytes:
        """Returns raw interleaved pixel bytes in row-major order."""
        return self._data

    @classmethod
    def new(cls, mode: str, size: Tuple[int, int], color: Union[int, Tuple[int, ...]] = 0) -> Image:
        """Creates a new Image filled with the given color."""
        w, h = size
        ch = CHANNELS[mode]
        if isinstance(color, int):
            c_tuple = (color,) * ch
        else:
            c_tuple = tuple(color)
            if len(c_tuple) < ch:
                c_tuple = c_tuple + (0,) * (ch - len(c_tuple))
        pixel_bytes = bytes(c_tuple[:ch])
        data = pixel_bytes * (w * h)
        return cls(mode, size, data)

    @classmethod
    def frombytes(cls, mode: str, size: Tuple[int, int], data: bytes) -> Image:
        """Creates an Image from raw pixel bytes."""
        return cls(mode, size, data)

    @classmethod
    def open(cls, fp: Union[str, bytes, BinaryIO]) -> Image:
        """Opens and decodes an image from a file path, raw bytes, or file-like object."""
        if isinstance(fp, str):
            with open(fp, "rb") as f:
                content = f.read()
        elif isinstance(fp, bytes):
            content = fp
        else:
            content = fp.read()

        # Sniff format
        if content.startswith(b"qoif"):
            # QOI decoding
            out_w = ctypes.c_uint32()
            out_h = ctypes.c_uint32()
            out_layout = ctypes.c_uint32()
            out_ptr = ctypes.c_void_p()
            out_len = ctypes.c_size_t()
            status = sillstone_image_qoi_decode_raw(
                content, len(content),
                ctypes.byref(out_w), ctypes.byref(out_h), ctypes.byref(out_layout),
                ctypes.byref(out_ptr), ctypes.byref(out_len),
            )
            if status != 0:
                raise RuntimeError(f"QOI decoding failed with status {status}")
            try:
                buf = ctypes.string_at(out_ptr, out_len.value)
                mode = LAYOUT_TO_MODE.get(out_layout.value, "RGB")
                return cls(mode, (out_w.value, out_h.value), buf)
            finally:
                sillstone_image_free_bytes_raw(out_ptr, out_len.value)

        elif content.startswith(b"BM"):
            # BMP decoding: parse 14-byte file header + 40-byte info header
            w = int.from_bytes(content[18:22], "little", signed=True)
            h = int.from_bytes(content[22:26], "little", signed=True)
            bpp = int.from_bytes(content[28:30], "little")
            w_abs = abs(w)
            h_abs = abs(h)
            offset = int.from_bytes(content[10:14], "little")

            if bpp == 24:
                # 24-bit BGR with row padding to 4 bytes
                stride = (w_abs * 3 + 3) & ~3
                pixels = bytearray(w_abs * h_abs * 3)
                for y in range(h_abs):
                    src_y = (h_abs - 1 - y) if h > 0 else y
                    row_start = offset + src_y * stride
                    dst_start = y * w_abs * 3
                    row = content[row_start : row_start + w_abs * 3]
                    for x in range(w_abs):
                        b = row[x * 3]
                        g = row[x * 3 + 1]
                        r = row[x * 3 + 2]
                        pixels[dst_start + x * 3] = r
                        pixels[dst_start + x * 3 + 1] = g
                        pixels[dst_start + x * 3 + 2] = b
                return cls("RGB", (w_abs, h_abs), bytes(pixels))
            else:
                raise NotImplementedError(f"BMP bpp {bpp} not supported in direct python decoder")

        else:
            raise ValueError("Unsupported or unrecognized image format signature")

    def resize(self, size: Tuple[int, int], resample: int = Resampling.BICUBIC) -> Image:
        """Resizes the image to the specified size using the given resampling filter."""
        out_w, out_h = size
        layout = MODE_TO_LAYOUT[self._mode]
        ch = CHANNELS[self._mode]
        out_len = out_w * out_h * ch
        out_buf = ctypes.create_string_buffer(out_len)

        status = sillstone_image_resize_raw(
            self._data, len(self._data),
            self._width, self._height, layout,
            out_w, out_h, resample,
            out_buf, out_len,
        )
        if status != 0:
            raise RuntimeError(f"Resize failed with status {status}")
        return Image(self._mode, size, out_buf.raw)

    def crop(self, box: Tuple[int, int, int, int]) -> Image:
        """Crops a rectangular region (left, upper, right, lower)."""
        x0, y0, x1, y1 = box
        layout = MODE_TO_LAYOUT[self._mode]
        out_w = x1 - x0
        out_h = y1 - y0
        ch = CHANNELS[self._mode]
        out_len = out_w * out_h * ch
        out_buf = ctypes.create_string_buffer(out_len)

        status = sillstone_image_crop_raw(
            self._data, len(self._data),
            self._width, self._height, layout,
            x0, y0, x1, y1,
            out_buf, out_len,
        )
        if status != 0:
            raise RuntimeError(f"Crop failed with status {status}")
        return Image(self._mode, (out_w, out_h), out_buf.raw)

    def transpose(self, method: int) -> Image:
        """Transposes or rotates by 90-degree steps."""
        layout = MODE_TO_LAYOUT[self._mode]
        if method in (Transpose.ROTATE_90, Transpose.ROTATE_270, Transpose.TRANSPOSE, Transpose.TRANSVERSE):
            out_size = (self._height, self._width)
        else:
            out_size = (self._width, self._height)

        ch = CHANNELS[self._mode]
        out_len = out_size[0] * out_size[1] * ch
        out_buf = ctypes.create_string_buffer(out_len)

        status = sillstone_image_transpose_raw(
            self._data, len(self._data),
            self._width, self._height, layout,
            method, out_buf, out_len,
        )
        if status != 0:
            raise RuntimeError(f"Transpose failed with status {status}")
        return Image(self._mode, out_size, out_buf.raw)

    def rotate(self, angle: float, resample: int = Resampling.BILINEAR, expand: bool = False) -> Image:
        """Rotates the image counter-clockwise."""
        layout = MODE_TO_LAYOUT[self._mode]
        # In expand=False, output size matches input size
        out_size = (self._width, self._height)
        ch = CHANNELS[self._mode]
        out_len = out_size[0] * out_size[1] * ch
        out_buf = ctypes.create_string_buffer(out_len)

        status = sillstone_image_rotate_raw(
            self._data, len(self._data),
            self._width, self._height, layout,
            float(angle), 1 if expand else 0, resample,
            out_buf, out_len,
        )
        if status != 0:
            raise RuntimeError(f"Rotate failed with status {status}")
        return Image(self._mode, out_size, out_buf.raw)

    def filter(self, filter_id: int) -> Image:
        """Applies a built-in 2D convolution filter."""
        layout = MODE_TO_LAYOUT[self._mode]
        out_buf = ctypes.create_string_buffer(len(self._data))

        status = sillstone_image_filter_raw(
            self._data, len(self._data),
            self._width, self._height, layout,
            filter_id, out_buf, len(self._data),
        )
        if status != 0:
            raise RuntimeError(f"Filter failed with status {status}")
        return Image(self._mode, (self._width, self._height), out_buf.raw)

    def convert(self, mode: str) -> Image:
        """Converts image to another color mode (e.g. 'RGB', 'RGBA', 'L')."""
        if mode == self._mode:
            return Image(self._mode, self.size, self._data)

        from_layout = MODE_TO_LAYOUT[self._mode]
        to_layout = MODE_TO_LAYOUT[mode]
        out_len = self._width * self._height * CHANNELS[mode]
        out_buf = ctypes.create_string_buffer(out_len)

        status = sillstone_image_color_convert_raw(
            self._data, len(self._data),
            self._width, self._height, from_layout, to_layout,
            out_buf, out_len,
        )
        if status != 0:
            raise RuntimeError(f"Color conversion from {self._mode} to {mode} failed with status {status}")
        return Image(mode, self.size, out_buf.raw)

    def save(self, fp: Union[str, BinaryIO], format: str = "QOI") -> None:
        """Saves image to a file path or file-like object in the given format ('QOI' or 'BMP')."""
        fmt = format.upper()
        layout = MODE_TO_LAYOUT[self._mode]

        if fmt == "QOI":
            out_ptr = ctypes.c_void_p()
            out_len = ctypes.c_size_t()
            status = sillstone_image_qoi_encode_raw(
                self._data, len(self._data),
                self._width, self._height, layout,
                ctypes.byref(out_ptr), ctypes.byref(out_len),
            )
            if status != 0:
                raise RuntimeError(f"QOI encoding failed with status {status}")
            try:
                buf = ctypes.string_at(out_ptr, out_len.value)
            finally:
                sillstone_image_free_bytes_raw(out_ptr, out_len.value)

        elif fmt == "BMP":
            out_ptr = ctypes.c_void_p()
            out_len = ctypes.c_size_t()
            status = sillstone_image_bmp_encode_raw(
                self._data, len(self._data),
                self._width, self._height, layout,
                ctypes.byref(out_ptr), ctypes.byref(out_len),
            )
            if status != 0:
                raise RuntimeError(f"BMP encoding failed with status {status}")
            try:
                buf = ctypes.string_at(out_ptr, out_len.value)
            finally:
                sillstone_image_free_bytes_raw(out_ptr, out_len.value)
        else:
            raise ValueError(f"Unsupported format '{format}'; expected 'QOI' or 'BMP'")

        if isinstance(fp, str):
            with open(fp, "wb") as f:
                f.write(buf)
        else:
            fp.write(buf)

    # Perceptual hash convenience methods on the Image instance
    def phash(self) -> int:
        """64-bit pHash, bit-exact with `imagehash.phash`."""
        layout = MODE_TO_LAYOUT[self._mode]
        out = ctypes.c_uint64()
        status = sillstone_image_phash_raw(self._data, len(self._data), self._width, self._height, layout, ctypes.byref(out))
        if status != 0:
            raise RuntimeError(f"pHash failed with status {status}")
        return out.value

    def ahash(self) -> int:
        """Computes 64-bit average hash (aHash)."""
        layout = MODE_TO_LAYOUT[self._mode]
        out = ctypes.c_uint64()
        status = sillstone_image_ahash_raw(self._data, len(self._data), self._width, self._height, layout, ctypes.byref(out))
        if status != 0:
            raise RuntimeError(f"aHash failed with status {status}")
        return out.value

    def dhash(self) -> int:
        """Computes 64-bit difference hash (dHash)."""
        layout = MODE_TO_LAYOUT[self._mode]
        out = ctypes.c_uint64()
        status = sillstone_image_dhash_raw(self._data, len(self._data), self._width, self._height, layout, ctypes.byref(out))
        if status != 0:
            raise RuntimeError(f"dHash failed with status {status}")
        return out.value

    def whash(self, level: int = 0) -> int:
        """64-bit wHash, bit-exact with `imagehash.whash`; `level` = log2(image_scale), 0 = default."""
        layout = MODE_TO_LAYOUT[self._mode]
        out = ctypes.c_uint64()
        status = sillstone_image_whash_raw(self._data, len(self._data), self._width, self._height, layout, level, ctypes.byref(out))
        if status != 0:
            raise RuntimeError(f"wHash failed with status {status}")
        return out.value

    def colorhash(self, bins: int = 3) -> int:
        """64-bit colorhash, bit-exact with `imagehash.colorhash(binbits=bins)`."""
        layout = MODE_TO_LAYOUT[self._mode]
        out = ctypes.c_uint64()
        status = sillstone_image_colorhash_raw(self._data, len(self._data), self._width, self._height, layout, bins, ctypes.byref(out))
        if status != 0:
            raise RuntimeError(f"colorhash failed with status {status}")
        return out.value
    @property
    def __array_interface__(self) -> dict:
        """NumPy Array Interface (version 3) for zero-copy buffer sharing with numpy.asarray()."""
        channels = CHANNELS[self._mode]
        shape = (self._height, self._width) if channels == 1 else (self._height, self._width, channels)
        buf_type = ctypes.c_char * len(self._data)
        addr = ctypes.addressof(buf_type.from_buffer(self._data))
        return {
            "version": 3,
            "shape": shape,
            "typestr": "|u1",
            "data": (addr, False),
            "strides": None,
        }

    def to_array(self):
        """Converts Image to a Sillstone Array (NumPy-compatible array)."""
        from .array import Array
        channels = CHANNELS[self._mode]
        shape = (self._height, self._width) if channels == 1 else (self._height, self._width, channels)
        return Array(shape=shape, dtype="uint8", buffer=self._data)

    to_tensor = to_array

    @classmethod
    def from_array(cls, arr, mode: Optional[str] = None) -> "Image":
        """Creates an Image from a Sillstone Array or NumPy-compatible array."""
        shape = arr.shape
        if len(shape) == 2:
            h, w = shape
            m = mode or "L"
        elif len(shape) == 3:
            h, w, c = shape
            if mode:
                m = mode
            else:
                m = "RGB" if c == 3 else ("RGBA" if c == 4 else "L")
        else:
            raise ValueError("Array must have 2 or 3 dimensions to convert to Image")
        raw_bytes = arr.tobytes() if hasattr(arr, "tobytes") else bytes(arr)
        return cls.frombytes(m, (w, h), raw_bytes)

    from_tensor = from_array
    def getpixel(self, xy: Tuple[int, int]) -> Union[int, Tuple[int, ...]]:
        """Returns the pixel value at a given coordinate."""
        x, y = xy
        if x < 0 or x >= self._width or y < 0 or y >= self._height:
            raise IndexError("pixel coordinate out of range")
        c = CHANNELS[self._mode]
        off = (y * self._width + x) * c
        if c == 1:
            return self._data[off]
        return tuple(self._data[off : off + c])

    def putpixel(self, xy: Tuple[int, int], value: Union[int, Tuple[int, ...]]):
        """Sets the pixel value at a given coordinate."""
        x, y = xy
        if x < 0 or x >= self._width or y < 0 or y >= self._height:
            raise IndexError("pixel coordinate out of range")
        c = CHANNELS[self._mode]
        off = (y * self._width + x) * c
        if c == 1:
            self._data[off] = int(value)
        else:
            for i, v in enumerate(value):
                self._data[off + i] = int(v)
