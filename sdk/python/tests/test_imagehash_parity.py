"""Bit-exact parity of the image hashes with `imagehash` (and so with Pillow).

Skipped when `imagehash`/`Pillow`/`numpy` are missing; CI installs them.
`phash` is compared on content whose DCT has no exactly-zero coefficients:
`imagehash` thresholds the floating-point noise of such coefficients, which
is defined by scipy's FFT rounding and is documented in docs/parity.md.
"""

import random
import unittest

import sillstone

try:
    import imagehash
    import numpy as np
    from PIL import Image as PILImage
except ImportError:  # pragma: no cover - optional oracle
    imagehash = None

LAYOUT = {"L": 0, "RGB": 2, "RGBA": 4}


def _bits(h):
    value = 0
    for b in h.hash.flatten():
        value = (value << 1) | int(bool(b))
    return value


def _image(rng, w, h, mode, kind):
    ch = 1 if mode == "L" else (3 if mode == "RGB" else 4)
    shape = (h, w) if ch == 1 else (h, w, ch)
    if kind == "noise":
        arr = rng.integers(0, 256, shape, dtype="uint8")
    else:  # smooth, photo-like: low-frequency field + ramp + fine noise
        base = rng.random((h // 8 + 2, w // 8 + 2, 3))
        small = PILImage.fromarray((base * 255).astype("uint8"))
        a = np.asarray(small.resize((w, h), PILImage.BICUBIC)).astype(int)
        a = np.clip(a + np.linspace(0, 60, w)[None, :, None] + rng.integers(-6, 7, (h, w, 3)), 0, 255)
        a = a.astype("uint8")
        if ch == 1:
            arr = np.asarray(PILImage.fromarray(a).convert("L"))
        elif ch == 3:
            arr = a
        else:
            arr = np.concatenate([a, rng.integers(0, 256, (h, w, 1), dtype="uint8")], axis=2)
    return PILImage.fromarray(np.ascontiguousarray(arr), mode)


@unittest.skipIf(imagehash is None, "imagehash oracle not installed")
class ImagehashParity(unittest.TestCase):
    def _compare(self, im, with_phash):
        w, h = im.size
        px = im.tobytes()
        layout = LAYOUT[im.mode]
        tag = f"{im.mode} {w}x{h}"
        self.assertEqual(sillstone.ahash(px, w, h, layout), _bits(imagehash.average_hash(im)), f"ahash {tag}")
        self.assertEqual(sillstone.dhash(px, w, h, layout), _bits(imagehash.dhash(im)), f"dhash {tag}")
        self.assertEqual(sillstone.whash(px, w, h, layout), _bits(imagehash.whash(im)), f"whash {tag}")
        self.assertEqual(sillstone.colorhash(px, w, h, layout), _bits(imagehash.colorhash(im)), f"colorhash {tag}")
        if with_phash:
            self.assertEqual(sillstone.phash(px, w, h, layout), _bits(imagehash.phash(im)), f"phash {tag}")

    def test_random_images_match_imagehash(self):
        rng = np.random.default_rng(20261025)
        pick = random.Random(20261025)
        for _ in range(60):
            w = pick.choice([8, 9, 17, 33, 64, 100, 129, 257, 500])
            h = pick.choice([8, 15, 32, 50, 64, 99, 128, 301])
            mode = pick.choice(["L", "RGB", "RGBA"])
            kind = pick.choice(["noise", "smooth"])
            self._compare(_image(rng, w, h, mode, kind), with_phash=w >= 32 and h >= 32)

    def test_extreme_aspect_ratios_exercise_vertical_first_resize(self):
        # Pillow runs the vertical pass first when the height shrinks by
        # more than twice the width's shrink; both orders must agree.
        rng = np.random.default_rng(7)
        for w, h in ((5, 300), (300, 5), (6, 300), (64, 1024), (1024, 64)):
            self._compare(_image(rng, w, h, "RGB", "noise"), with_phash=False)

    def test_tiny_images_match_imagehash_whash_ahash_dhash(self):
        rng = np.random.default_rng(3)
        for w, h in ((1, 1), (2, 3), (3, 2), (7, 7), (8, 8), (9, 8)):
            im = _image(rng, w, h, "RGB", "noise")
            px = im.tobytes()
            self.assertEqual(sillstone.ahash(px, w, h, 2), _bits(imagehash.average_hash(im)))
            self.assertEqual(sillstone.dhash(px, w, h, 2), _bits(imagehash.dhash(im)))
            self.assertEqual(sillstone.whash(px, w, h, 2), _bits(imagehash.whash(im)))


@unittest.skipIf(imagehash is None, "Pillow oracle not installed")
class PillowResizeParity(unittest.TestCase):
    def test_resize_is_pillow_exact(self):
        rng = np.random.default_rng(11)
        pick = random.Random(11)
        R = sillstone.Resampling
        filters = [
            (PILImage.LANCZOS, R.LANCZOS),
            (PILImage.BICUBIC, R.BICUBIC),
            (PILImage.BILINEAR, R.BILINEAR),
            (PILImage.NEAREST, R.NEAREST),
        ]
        for _ in range(60):
            w, h = pick.randint(1, 160), pick.randint(1, 160)
            ow, oh = pick.randint(1, 160), pick.randint(1, 160)
            mode = pick.choice(["L", "RGB"])
            shape = (h, w) if mode == "L" else (h, w, 3)
            im = PILImage.fromarray(rng.integers(0, 256, shape, dtype="uint8"), mode)
            ours_in = sillstone.Image.frombytes(mode, (w, h), im.tobytes())
            for pil_filter, our_filter in filters:
                expected = im.resize((ow, oh), pil_filter).tobytes()
                got = ours_in.resize((ow, oh), our_filter).tobytes()
                self.assertEqual(got, expected, f"{mode} {w}x{h}->{ow}x{oh} filter {our_filter}")


if __name__ == "__main__":
    unittest.main()
