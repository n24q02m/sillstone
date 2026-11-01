#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Generates the Pillow golden vectors in ``pillow_golden.jsonl``.

Pillow and numpy are the *oracle*: they are development-time inputs of this
script only. Nothing in the shipped crates or SDKs depends on them, and the
Rust / Python tests read the committed JSONL, not a live Pillow.

Every line is one flat JSON object whose values are strings, numbers or null:

    {"id", "op", "mode", "w", "h", "ow", "oh", "filter", "gap", "to",
     "input", "expected"}

``input`` / ``expected`` are lowercase hex of the raw interleaved samples
(``I;16`` little-endian, ``F`` as little-endian ``f32``). ``op`` is one of
``resize`` (optionally with ``gap`` = ``reducing_gap``), ``reduce``
(``ow`` / ``oh`` carry the integer factors) and ``convert`` (``to`` = target
mode). The first line is a ``_provenance`` object with the oracle versions.

Usage:
    pip install -r scripts/golden/requirements.txt
    python scripts/golden/gen_pillow_golden.py
"""

from __future__ import annotations

import json
import random
import sys
from pathlib import Path

import numpy as np
import PIL
from PIL import Image

OUT = (
    Path(__file__).resolve().parents[2]
    / "crates/image/tests/fixtures/pillow_golden/pillow_golden.jsonl"
)

R = Image.Resampling
FILTERS = {
    "nearest": R.NEAREST,
    "box": R.BOX,
    "bilinear": R.BILINEAR,
    "bicubic": R.BICUBIC,
    "lanczos": R.LANCZOS,
}
CHANNELS = {"L": 1, "LA": 2, "RGB": 3, "RGBA": 4, "CMYK": 4}
KEYS = ["id", "op", "mode", "w", "h", "ow", "oh", "filter", "gap", "to", "input", "expected"]


def noise(rng: random.Random, n: int) -> bytes:
    return bytes(rng.randrange(256) for _ in range(n))


def alpha_stress(rng: random.Random, w: int, h: int, mode: str) -> bytes:
    """RGBA / LA content whose alpha cycles through awkward values."""
    ch = CHANNELS[mode]
    alphas = [0, 1, 2, 64, 127, 128, 200, 253, 254, 255]
    out = bytearray()
    for _ in range(w * h):
        for _c in range(ch - 1):
            out.append(rng.randrange(256))
        out.append(rng.choice(alphas))
    return bytes(out)


def content(rng: random.Random, mode: str, w: int, h: int) -> bytes:
    if mode in ("LA", "RGBA"):
        return alpha_stress(rng, w, h, mode)
    return noise(rng, w * h * CHANNELS[mode])


def make_image(mode: str, w: int, h: int, raw: bytes) -> Image.Image:
    if mode == "I;16":
        return Image.fromarray(np.frombuffer(raw, dtype="<u2").reshape(h, w).copy())
    if mode == "F":
        return Image.fromarray(np.frombuffer(raw, dtype="<f4").reshape(h, w).copy(), "F")
    return Image.frombytes(mode, (w, h), raw)


def to_bytes(im: Image.Image) -> bytes:
    if im.mode == "I;16":
        return np.asarray(im).astype("<u2").tobytes()
    if im.mode == "F":
        return np.asarray(im).astype("<f4").tobytes()
    return im.tobytes()


def rec(**kw) -> dict:
    base = dict(w=None, h=None, ow=None, oh=None, filter=None, gap=None, to=None)
    base.update(kw)
    return base


def main() -> int:
    rng = random.Random(0x51115701)
    cases: list[dict] = []

    # ------------------------------------------------------------ resize
    sizes = [
        (17, 13, 9, 20),  # mixed down / up
        (31, 29, 8, 7),  # strong downscale
        (8, 8, 21, 19),  # upscale
        (40, 12, 7, 30),  # width shrinks, height grows
        (64, 64, 8, 8),  # 8x (vertical-first rule needs dh > 2 * dw)
        (5, 50, 5, 9),  # one axis unchanged
        (6, 40, 3, 10),  # dh > 2 * dw -> vertical pass first
    ]
    for mode in ("L", "LA", "RGB", "RGBA"):
        for (w, h, ow, oh) in sizes:
            raw = content(rng, mode, w, h)
            src = make_image(mode, w, h, raw)
            for fname, f in FILTERS.items():
                out = src.resize((ow, oh), f)
                cases.append(rec(
                    id=f"resize/{mode}/{fname}/{w}x{h}->{ow}x{oh}", op="resize",
                    mode=mode, w=w, h=h, ow=ow, oh=oh, filter=fname,
                    input=raw.hex(), expected=to_bytes(out).hex(),
                ))

    # Alpha corner cases: fully transparent / opaque / near-opaque pixels.
    for mode in ("LA", "RGBA"):
        ch = CHANNELS[mode]
        w, h, ow, oh = 12, 10, 5, 4
        raw = bytearray()
        for i in range(w * h):
            a = [0, 255, 1, 254, 128][i % 5]
            raw += noise(rng, ch - 1) + bytes([a])
        raw = bytes(raw)
        src = make_image(mode, w, h, raw)
        for fname in ("box", "bilinear", "bicubic", "lanczos"):
            out = src.resize((ow, oh), FILTERS[fname])
            cases.append(rec(
                id=f"resize/{mode}/{fname}/alpha-corners", op="resize", mode=mode,
                w=w, h=h, ow=ow, oh=oh, filter=fname,
                input=raw.hex(), expected=to_bytes(out).hex(),
            ))

    # 16-bit and float rasters (Pillow convolves them in f64).
    for (w, h, ow, oh) in [(9, 7, 4, 3), (31, 17, 8, 5), (8, 8, 20, 13), (40, 12, 7, 30)]:
        a16 = noise(rng, w * h * 2)
        af = np.array(
            [rng.uniform(-1000.0, 70000.0) for _ in range(w * h)], dtype="<f4"
        ).tobytes()
        for mode, raw in (("I;16", a16), ("F", af)):
            src = make_image(mode, w, h, raw)
            for fname, f in FILTERS.items():
                out = src.resize((ow, oh), f)
                cases.append(rec(
                    id=f"resize/{mode}/{fname}/{w}x{h}->{ow}x{oh}", op="resize",
                    mode=mode, w=w, h=h, ow=ow, oh=oh, filter=fname,
                    input=raw.hex(), expected=to_bytes(out).hex(),
                ))

    # ------------------------------------------------------- reducing_gap
    for mode in ("L", "RGB", "RGBA"):
        for (w, h, ow, oh, gap) in [
            (67, 53, 9, 7, 2.0),
            (100, 41, 12, 11, 3.0),
            (33, 95, 8, 8, 2.0),
            (64, 64, 5, 5, 1.5),
        ]:
            raw = content(rng, mode, w, h)
            src = make_image(mode, w, h, raw)
            for fname in ("box", "bilinear", "bicubic", "lanczos"):
                out = src.resize((ow, oh), FILTERS[fname], reducing_gap=gap)
                cases.append(rec(
                    id=f"resize-gap/{mode}/{fname}/{w}x{h}->{ow}x{oh}/g{gap}",
                    op="resize", mode=mode, w=w, h=h, ow=ow, oh=oh, filter=fname,
                    gap=gap, input=raw.hex(), expected=to_bytes(out).hex(),
                ))

    # Image.reduce with integer factors, including a partial last block.
    for mode in ("L", "RGB", "RGBA"):
        for (w, h, fx, fy) in [
            (10, 9, 2, 2), (11, 7, 3, 2), (13, 13, 4, 5),
            (9, 8, 1, 3), (8, 9, 3, 1), (20, 20, 7, 7),
        ]:
            raw = content(rng, mode, w, h)
            out = make_image(mode, w, h, raw).reduce((fx, fy))
            cases.append(rec(
                id=f"reduce/{mode}/{w}x{h}/f{fx}x{fy}", op="reduce", mode=mode,
                w=w, h=h, ow=fx, oh=fy, input=raw.hex(), expected=to_bytes(out).hex(),
            ))

    # Tall images (height > 100 * width) take Pillow's two-step resize.
    for mode in ("L", "RGB"):
        for (w, h, ow, oh) in [(3, 400, 2, 20), (2, 305, 5, 9)]:
            raw = content(rng, mode, w, h)
            src = make_image(mode, w, h, raw)
            for fname in ("box", "bilinear", "bicubic", "lanczos"):
                out = src.resize((ow, oh), FILTERS[fname])
                cases.append(rec(
                    id=f"resize-tall/{mode}/{fname}/{w}x{h}->{ow}x{oh}", op="resize",
                    mode=mode, w=w, h=h, ow=ow, oh=oh, filter=fname,
                    input=raw.hex(), expected=to_bytes(out).hex(),
                ))
    raw = content(rng, "L", 3, 400)
    out = make_image("L", 3, 400, raw).resize((2, 20), R.BICUBIC, reducing_gap=2.0)
    cases.append(rec(
        id="resize-tall-gap/L/bicubic/3x400->2x20/g2.0", op="resize", mode="L",
        w=3, h=400, ow=2, oh=20, filter="bicubic", gap=2.0,
        input=raw.hex(), expected=to_bytes(out).hex(),
    ))

    # Premultiply, exhaustively: every (colour, alpha) pair of an LA image,
    # duplicated horizontally so reduce((2, 1)) averages it to itself and the
    # result is exactly unpremultiply(premultiply(pixel)).
    raw = bytearray()
    for a in range(256):
        for c in range(256):
            raw += bytes([c, a, c, a])
    raw = bytes(raw)
    out = make_image("LA", 512, 256, raw).reduce((2, 1))
    cases.append(rec(
        id="reduce/LA/premultiply-exhaustive", op="reduce", mode="LA",
        w=512, h=256, ow=2, oh=1, input=raw.hex(), expected=to_bytes(out).hex(),
    ))

    # ------------------------------------------------------------ convert
    def conv(mode_from: str, mode_to: str, w: int, h: int, raw: bytes, tag: str) -> None:
        out = make_image(mode_from, w, h, raw).convert(mode_to)
        cases.append(rec(
            id=f"convert/{mode_from}->{mode_to}/{tag}", op="convert", mode=mode_from,
            w=w, h=h, to=mode_to, input=raw.hex(), expected=to_bytes(out).hex(),
        ))

    # The two luma formulas differ on a few percent of colours, so the
    # lattice mixes dense low / mid / high values with random noise.
    lat = [0, 1, 2, 3, 7, 8, 63, 64, 85, 127, 128, 129, 170, 191, 192, 250, 253, 254, 255]
    rgb = bytes(c for r in lat for g in lat for b in lat[::3] for c in (r, g, b))
    n_px = len(rgb) // 3
    noise_rgb = noise(rng, 3 * 1024)
    noise_rgba = noise(rng, 4 * 1024)
    conv("RGB", "L", n_px, 1, rgb, "lattice")
    conv("RGB", "L", 1024, 1, noise_rgb, "noise")
    conv("RGBA", "L", 1024, 1, noise_rgba, "noise")
    conv("RGBA", "RGB", 1024, 1, noise_rgba, "noise")
    conv("RGB", "RGBA", 1024, 1, noise_rgb, "noise")
    conv("L", "RGB", 256, 1, bytes(range(256)), "ramp")
    conv("L", "RGBA", 256, 1, bytes(range(256)), "ramp")
    conv("RGB", "CMYK", n_px, 1, rgb, "lattice")
    conv("RGB", "CMYK", 1024, 1, noise_rgb, "noise")
    conv("CMYK", "RGB", 1024, 1, noise(rng, 4 * 1024), "noise")
    cmyk_lat = bytes(
        c for a in lat for b in lat[::3] for d in lat[::3] for k in lat[::4] for c in (a, b, d, k)
    )
    conv("CMYK", "RGB", len(cmyk_lat) // 4, 1, cmyk_lat, "lattice")

    meta = {
        "_provenance": {
            "generator": "scripts/golden/gen_pillow_golden.py",
            "pillow": PIL.__version__,
            "numpy": np.__version__,
            "python": sys.version.split()[0],
            "cases": len(cases),
        }
    }
    OUT.parent.mkdir(parents=True, exist_ok=True)
    with OUT.open("w", encoding="utf-8", newline="\n") as fh:
        fh.write(json.dumps(meta, separators=(",", ":")) + "\n")
        for c in cases:
            fh.write(json.dumps({k: c[k] for k in KEYS}, separators=(",", ":")) + "\n")
    print(f"wrote {len(cases)} cases to {OUT} ({OUT.stat().st_size} bytes)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
