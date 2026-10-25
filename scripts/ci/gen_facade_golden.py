#!/usr/bin/env python3
"""Generate `tests/fixtures/facade/golden.json` from the Python SDK.

The Python SDK is the oracle for the Node and Java image/array facades: both load
this file and must reproduce every vector byte-for-byte (images) or within 1e-9
(tensors). `sdk/python/tests/test_facade_golden.py` regenerates the vectors in memory
and fails when the committed file drifted, so the oracle cannot silently change.

Usage: python scripts/ci/gen_facade_golden.py [--check]
"""

from __future__ import annotations

import ctypes
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "sdk" / "python"))

import sillstone  # noqa: E402
from sillstone._generated import (  # noqa: E402
    sillstone_image_adjust_raw,
    sillstone_image_qoi_decode_raw,
    sillstone_image_free_bytes_raw,
)
from sillstone.image import Image, ImageFilter, MODE_TO_LAYOUT, Resampling, Transpose  # noqa: E402

FIXTURE = ROOT / "tests" / "fixtures" / "facade" / "golden.json"


def _lcg_bytes(n: int, seed: int) -> bytes:
    out = bytearray()
    state = seed
    for _ in range(n):
        state = (state * 6364136223846793005 + 1442695040888963407) & 0xFFFFFFFFFFFFFFFF
        out.append((state >> 33) & 0xFF)
    return bytes(out)


def _img(image: Image) -> dict:
    return {"mode": image.mode, "width": image.width, "height": image.height, "hex": image.tobytes().hex()}


def _adjust(image: Image, op: int, factor: float) -> Image:
    out = ctypes.create_string_buffer(len(image.tobytes()))
    data = image.tobytes()
    status = sillstone_image_adjust_raw(
        data, len(data), image.width, image.height, MODE_TO_LAYOUT[image.mode], op, factor, out, len(data)
    )
    assert status == 0, status
    return Image(image.mode, image.size, out.raw)


def _qoi_decode(blob: bytes) -> Image:
    w, h, layout = ctypes.c_uint32(), ctypes.c_uint32(), ctypes.c_uint32()
    ptr, n = ctypes.c_void_p(), ctypes.c_size_t()
    status = sillstone_image_qoi_decode_raw(
        blob, len(blob), ctypes.byref(w), ctypes.byref(h), ctypes.byref(layout), ctypes.byref(ptr), ctypes.byref(n)
    )
    assert status == 0, status
    try:
        pixels = ctypes.string_at(ptr, n.value)
    finally:
        sillstone_image_free_bytes_raw(ptr, n.value)
    mode = {0: "L", 2: "RGB", 4: "RGBA"}[layout.value]
    return Image(mode, (w.value, h.value), pixels)


def _encode(image: Image, fmt: str) -> bytes:
    import io

    buf = io.BytesIO()
    image.save(buf, fmt)
    return buf.getvalue()


def build() -> dict:
    w, h = 9, 7
    base = Image("RGB", (w, h), _lcg_bytes(w * h * 3, 7))
    rgba = Image("RGBA", (6, 5), _lcg_bytes(6 * 5 * 4, 11))
    ops: dict[str, dict] = {}

    for name in ("NEAREST", "BOX", "BILINEAR", "BICUBIC", "LANCZOS"):
        ops[f"resize_{name.lower()}_5x4"] = _img(base.resize((5, 4), getattr(Resampling, name)))
    ops["resize_bicubic_14x11"] = _img(base.resize((14, 11), Resampling.BICUBIC))
    ops["resize_rgba_bilinear_3x8"] = _img(rgba.resize((3, 8), Resampling.BILINEAR))
    ops["crop_2_1_8_6"] = _img(base.crop((2, 1, 8, 6)))
    for name in ("FLIP_LEFT_RIGHT", "FLIP_TOP_BOTTOM", "ROTATE_90", "ROTATE_180", "ROTATE_270", "TRANSPOSE", "TRANSVERSE"):
        ops[f"transpose_{name.lower()}"] = _img(base.transpose(getattr(Transpose, name)))
    ops["rotate_30_bilinear"] = _img(base.rotate(30.0, Resampling.BILINEAR, False))
    ops["rotate_30_bilinear_expand"] = _img(base.rotate(30.0, Resampling.BILINEAR, True))
    ops["rotate_45_nearest_expand"] = _img(base.rotate(45.0, Resampling.NEAREST, True))
    ops["rotate_90"] = _img(base.rotate(90.0, Resampling.BILINEAR, False))
    ops["rotate_neg20_nearest"] = _img(base.rotate(-20.0, Resampling.NEAREST, False))
    for name in ("BLUR", "SHARPEN", "CONTOUR", "DETAIL", "EDGE_ENHANCE", "FIND_EDGES", "EMBOSS"):
        ops[f"filter_{name.lower()}"] = _img(base.filter(getattr(ImageFilter, name)))
    ops["convert_rgb_to_l"] = _img(base.convert("L"))
    ops["convert_rgb_to_rgba"] = _img(base.convert("RGBA"))
    ops["convert_rgba_to_rgb"] = _img(rgba.convert("RGB"))
    ops["brightness_1_5"] = _img(_adjust(base, 0, 1.5))
    ops["invert"] = _img(_adjust(base, 1, 0.0))

    qoi = _encode(base, "QOI")
    bmp = _encode(base, "BMP")
    qoi_rgba = _encode(rgba, "QOI")
    roundtrip = _qoi_decode(qoi_rgba)
    assert roundtrip.tobytes() == rgba.tobytes()

    hashes = {
        "phash": str(base.phash()),
        "ahash": str(base.ahash()),
        "dhash": str(base.dhash()),
        "whash": str(base.whash()),
        "colorhash": str(base.colorhash()),
    }

    a = sillstone.array([[1.5, -2.0, 3.0, 0.25], [4.0, 5.5, -6.0, 7.0], [0.125, 8.0, 9.0, -1.0]], dtype="float64")
    b = sillstone.array([[1.0, 2.0], [-3.0, 0.5], [2.5, 4.0], [0.0, -1.5]], dtype="float64")
    a2 = sillstone.array([[0.5, 0.5, 0.5, 0.5], [1.0, 2.0, 3.0, 4.0], [-1.0, -1.0, -1.0, -1.0]], dtype="float64")
    tensor = {
        "a": {"shape": list(a.shape), "data": [x for row in a.tolist() for x in row]},
        "b": {"shape": list(b.shape), "data": [x for row in b.tolist() for x in row]},
        "a2": {"shape": list(a2.shape), "data": [x for row in a2.tolist() for x in row]},
        "matmul": [x for row in (a @ b).tolist() for x in row],
        "add": [x for row in (a + a2).tolist() for x in row],
        "sum": float(a.sum()),
    }

    return {
        "image": _img(base),
        "image_rgba": _img(rgba),
        "ops": ops,
        "qoi_rgb": qoi.hex(),
        "bmp_rgb": bmp.hex(),
        "qoi_rgba": qoi_rgba.hex(),
        "hashes": hashes,
        "tensor": tensor,
    }


def main() -> int:
    text = json.dumps(build(), indent=1, sort_keys=True) + "\n"
    if "--check" in sys.argv:
        if not FIXTURE.exists() or FIXTURE.read_text(encoding="utf-8") != text:
            print(f"facade golden vectors drifted: {FIXTURE}", file=sys.stderr)
            return 1
        print("facade golden vectors in sync")
        return 0
    FIXTURE.parent.mkdir(parents=True, exist_ok=True)
    FIXTURE.write_text(text, encoding="utf-8", newline="\n")
    print(f"wrote {FIXTURE}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
