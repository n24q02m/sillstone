"""Sillstone Image: Pure-Rust zero-dependency Pillow-compatible Image engine."""

from __future__ import annotations

import ctypes
import io
import os
from typing import BinaryIO, Optional, Tuple, Union

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
    sillstone_image_rotate_size_raw,
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

# Same cap as ``sillstone_image::raster::MAX_BUFFER_BYTES`` (crates/image/src/raster/raster.rs):
# no decoded raster may exceed 1 GiB.
MAX_BUFFER_BYTES = 1 << 30

_BMP_FILE_HEADER = 14
_BMP_MIN_INFO_HEADER = 40


def _bmp_geometry(
    content: bytes, w: int, h: int, offset: int, src_row_bytes: int, src_stride: int, out_channels: int
) -> Tuple[int, int]:
    """Validates BMP header fields against the real file length and returns ``(width, height)``.

    Every check uses Python integers and runs before any pixel buffer is allocated, so a forged
    header cannot trigger a large allocation or a late ``IndexError``.
    ``src_row_bytes`` is the pixel bytes of one row, ``src_stride`` the distance between rows
    (the last row may omit its padding), ``out_channels`` the channels of the decoded raster.
    """
    w_abs, h_abs = abs(w), abs(h)
    if w_abs == 0 or h_abs == 0:
        raise ValueError(f"BMP has an empty image size ({w_abs}x{h_abs})")
    header_size = int.from_bytes(content[14:18], "little")
    if header_size < _BMP_MIN_INFO_HEADER:
        raise ValueError(f"unsupported BMP info header size {header_size}")
    first_pixel = _BMP_FILE_HEADER + header_size
    if offset < first_pixel or offset > len(content):
        raise ValueError(f"BMP pixel data offset {offset} is outside the file ({first_pixel}..{len(content)})")
    if w_abs * h_abs * out_channels > MAX_BUFFER_BYTES:
        raise ValueError(f"BMP image {w_abs}x{h_abs} exceeds the {MAX_BUFFER_BYTES} byte raster limit")
    needed = src_stride * (h_abs - 1) + src_row_bytes
    if needed > len(content) - offset:
        raise ValueError(f"truncated BMP pixel data: {needed} bytes needed, {len(content) - offset} present")
    return w_abs, h_abs


class Resampling:
    """Resampling filters, bit-exact with the Pillow filter of the same name.

    The numeric ids are Sillstone's own and differ from Pillow's enum
    (Pillow: LANCZOS = 1, BOX = 4); map by name, not by value.
    """
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
        # Mutable pixel store so putpixel is O(1); native calls and tobytes() read an
        # immutable ``bytes`` snapshot that is rebuilt lazily after a write.
        self._buf = bytearray(data)
        self._snapshot: Optional[bytes] = None

    @property
    def _data(self) -> bytes:
        if self._snapshot is None:
            self._snapshot = bytes(self._buf)
        return self._snapshot

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
            if len(content) < _BMP_FILE_HEADER + _BMP_MIN_INFO_HEADER:
                raise ValueError("truncated BMP header")
            w = int.from_bytes(content[18:22], "little", signed=True)
            h = int.from_bytes(content[22:26], "little", signed=True)
            bpp = int.from_bytes(content[28:30], "little")
            offset = int.from_bytes(content[10:14], "little")

            if bpp == 24:
                # 24-bit BGR with row padding to 4 bytes
                stride = (abs(w) * 3 + 3) & ~3
                w_abs, h_abs = _bmp_geometry(content, w, h, offset, abs(w) * 3, stride, 3)
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
            elif bpp == 32:
                return cls._open_bmp32(content, w, h, offset)
            else:
                raise NotImplementedError(f"BMP bpp {bpp} not supported in direct python decoder")

        else:
            raise ValueError("Unsupported or unrecognized image format signature")

    @classmethod
    def _open_bmp32(cls, content: bytes, w: int, h: int, offset: int) -> Image:
        """Decodes a 32-bit BMP: ``BI_RGB`` (fourth byte is padding, as Pillow reads it) or
        ``BI_BITFIELDS`` with 8-bit-wide channel masks (alpha kept when the file declares an alpha mask)."""
        compression = int.from_bytes(content[30:34], "little")
        header_size = int.from_bytes(content[14:18], "little")
        if compression == 0:
            masks = (0x00FF0000, 0x0000FF00, 0x000000FF, 0)
        elif compression == 3 and len(content) >= 14 + 52:
            r, g, b = (int.from_bytes(content[o : o + 4], "little") for o in (54, 58, 62))
            a = int.from_bytes(content[66:70], "little") if header_size >= 56 and len(content) >= 70 else 0
            masks = (r, g, b, a)
        else:
            raise NotImplementedError(f"32-bit BMP compression {compression} not supported in direct python decoder")

        shifts = []
        for m in masks:
            if m == 0:
                shifts.append(None)
                continue
            shift = (m & -m).bit_length() - 1
            if (m >> shift) != 0xFF:
                raise NotImplementedError("32-bit BMP channel masks other than 8 bits wide are not supported")
            shifts.append(shift)
        if shifts[0] is None or shifts[1] is None or shifts[2] is None:
            raise ValueError("32-bit BMP is missing a colour channel mask")

        has_alpha = shifts[3] is not None
        ch = 4 if has_alpha else 3
        w_abs, h_abs = _bmp_geometry(content, w, h, offset, abs(w) * 4, abs(w) * 4, ch)
        pixels = bytearray(w_abs * h_abs * ch)
        for y in range(h_abs):
            src_y = (h_abs - 1 - y) if h > 0 else y
            row = content[offset + src_y * w_abs * 4 : offset + (src_y + 1) * w_abs * 4]
            dst = y * w_abs * ch
            for x in range(w_abs):
                px = int.from_bytes(row[x * 4 : x * 4 + 4], "little")
                o = dst + x * ch
                pixels[o] = (px >> shifts[0]) & 0xFF
                pixels[o + 1] = (px >> shifts[1]) & 0xFF
                pixels[o + 2] = (px >> shifts[2]) & 0xFF
                if has_alpha:
                    pixels[o + 3] = (px >> shifts[3]) & 0xFF
        return cls("RGBA" if has_alpha else "RGB", (w_abs, h_abs), bytes(pixels))

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
        out_w = ctypes.c_uint32()
        out_h = ctypes.c_uint32()
        status = sillstone_image_rotate_size_raw(
            self._width, self._height, float(angle), 1 if expand else 0,
            ctypes.byref(out_w), ctypes.byref(out_h),
        )
        if status != 0:
            raise RuntimeError(f"Rotate failed with status {status}")
        out_size = (out_w.value, out_h.value)
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
        if mode not in MODE_TO_LAYOUT:
            raise ValueError(f"Unsupported mode {mode}; expected 'L', 'RGB', or 'RGBA'")
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
        return {
            "version": 3,
            "shape": shape,
            "typestr": "|u1",
            # The pixels are immutable ``bytes``: the buffer protocol hands NumPy a read-only view.
            "data": self._data,
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
        dtype = str(getattr(arr, "dtype", "uint8"))
        if dtype != "uint8":
            raise TypeError(f"Image.from_array needs a uint8 array, got dtype {dtype}")
        if len(shape) == 2:
            h, w = shape
            c = 1
            m = mode or "L"
        elif len(shape) == 3:
            h, w, c = shape
            if mode:
                m = mode
            else:
                m = "RGB" if c == 3 else ("RGBA" if c == 4 else "L")
        else:
            raise ValueError("Array must have 2 or 3 dimensions to convert to Image")
        if m not in CHANNELS:
            raise ValueError(f"Unsupported mode {m}; expected 'L', 'RGB', or 'RGBA'")
        if c != CHANNELS[m]:
            raise ValueError(f"Array with {c} channel(s) cannot be converted to mode {m} ({CHANNELS[m]} expected)")
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
            return self._buf[off]
        return tuple(self._buf[off : off + c])

    def putpixel(self, xy: Tuple[int, int], value: Union[int, Tuple[int, ...]]):
        """Sets the pixel value at a given coordinate; channel values are clamped to 0..255 like Pillow.

        ``L`` takes an int or a 1-tuple, ``RGB`` a 3- or 4-tuple (a fourth value is dropped),
        ``RGBA`` a 3-tuple (opaque) or 4-tuple.
        """
        x, y = xy
        if x < 0 or x >= self._width or y < 0 or y >= self._height:
            raise IndexError("pixel coordinate out of range")
        c = CHANNELS[self._mode]
        if isinstance(value, int):
            values = (value,)
        else:
            try:
                values = tuple(value)
            except TypeError:
                raise TypeError(
                    f"color must be an int or a tuple of ints, got {type(value).__name__}"
                ) from None
        if self._mode == "RGBA" and len(values) == 3:
            values += (255,)
        elif self._mode == "RGB" and len(values) == 4:
            values = values[:3]
        if len(values) != c:
            raise TypeError(f"{self._mode} pixel needs {c} channel value(s), got {len(values)}")
        pixel = bytes(min(255, max(0, int(v))) for v in values)
        off = (y * self._width + x) * c
        self._buf[off : off + c] = pixel
        self._snapshot = None
