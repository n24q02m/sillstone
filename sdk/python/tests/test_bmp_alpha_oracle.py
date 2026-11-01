"""BMP encoder checked against Pillow as an independent decoder.

Our own decoder reading back our own encoder proves nothing about alpha: a file that
silently drops the fourth byte would still round-trip through a decoder that ignores it.
Here Pillow, which shares no code with this repository, decodes the files we write and
every pixel (alpha included) is compared.
"""

import io
import unittest

from PIL import Image as PilImage

from sillstone import Image as SillImage


def rgba_pixels(w: int, h: int) -> bytes:
    """Distinct R, G, B and an alpha ramp that visits 0 and 255 and values in between."""
    out = bytearray()
    for y in range(h):
        for x in range(w):
            i = y * w + x
            out += bytes([(x * 17 + 3) & 255, (y * 29 + 5) & 255, (i * 7 + 11) & 255, (i * 255) // max(w * h - 1, 1)])
    return bytes(out)


def save_bmp(im: SillImage) -> bytes:
    buf = io.BytesIO()
    im.save(buf, format="BMP")
    return buf.getvalue()


class TestBmpAgainstPillow(unittest.TestCase):
    def test_rgba_bmp_keeps_every_alpha_value_per_pillow(self):
        for w, h in ((16, 9), (1, 1), (3, 5), (256, 1)):
            px = rgba_pixels(w, h)
            if w == 256:  # every alpha value 0..255 exactly once
                px = b"".join(bytes([x, 255 - x, (x * 3) & 255, x]) for x in range(256))
            data = save_bmp(SillImage.frombytes("RGBA", (w, h), px))
            pil = PilImage.open(io.BytesIO(data))
            pil.load()
            self.assertEqual(pil.mode, "RGBA", f"{w}x{h}")
            self.assertEqual(pil.size, (w, h))
            got = pil.tobytes()
            alpha_bad = [i // 4 for i in range(3, len(px), 4) if got[i] != px[i]]
            self.assertEqual(alpha_bad, [], f"{w}x{h}: alpha differs at pixels {alpha_bad[:5]}")
            self.assertEqual(got, px, f"{w}x{h}: colour differs")

    def test_rgb_bmp_matches_pillow(self):
        w, h = 7, 5  # width not a multiple of 4: exercises row padding
        px = b"".join(rgba_pixels(w, h)[i : i + 3] for i in range(0, w * h * 4, 4))
        data = save_bmp(SillImage.frombytes("RGB", (w, h), px))
        pil = PilImage.open(io.BytesIO(data))
        pil.load()
        self.assertEqual((pil.mode, pil.size), ("RGB", (w, h)))
        self.assertEqual(pil.tobytes(), px)

    def test_own_rgba_bmp_reads_back_with_alpha(self):
        px = rgba_pixels(16, 9)
        data = save_bmp(SillImage.frombytes("RGBA", (16, 9), px))
        back = SillImage.open(data)
        self.assertEqual((back.mode, back.size), ("RGBA", (16, 9)))
        self.assertEqual(back.tobytes(), px)

    def test_top_down_bitfields_bmp_reads_like_pillow(self):
        w, h = 4, 3
        px = rgba_pixels(w, h)
        data = bytearray(save_bmp(SillImage.frombytes("RGBA", (w, h), px)))
        offset = int.from_bytes(data[10:14], "little")
        rows = [bytes(data[offset + r * w * 4 : offset + (r + 1) * w * 4]) for r in range(h)]
        data[offset:] = b"".join(reversed(rows))  # store top row first
        data[22:26] = (-h).to_bytes(4, "little", signed=True)  # negative height: top-down
        data = bytes(data)

        pil = PilImage.open(io.BytesIO(data))
        pil.load()
        self.assertEqual(pil.tobytes(), px)
        self.assertEqual(SillImage.open(data).tobytes(), px)

    def test_plain_32bit_bmp_alpha_byte_is_padding_like_pillow(self):
        # Pillow writes RGBA as BI_RGB 32-bit and reads it back as RGB: the fourth byte is padding.
        px = rgba_pixels(5, 4)
        buf = io.BytesIO()
        PilImage.frombytes("RGBA", (5, 4), px).save(buf, format="BMP")
        pil = PilImage.open(io.BytesIO(buf.getvalue()))
        pil.load()
        ours = SillImage.open(buf.getvalue())
        self.assertEqual((ours.mode, ours.size), (pil.mode, pil.size))
        self.assertEqual(ours.tobytes(), pil.tobytes())

    def test_unsupported_32bit_variants_are_refused(self):
        data = bytearray(save_bmp(SillImage.frombytes("RGBA", (2, 2), rgba_pixels(2, 2))))
        bad_compression = bytearray(data)
        bad_compression[30:34] = (1).to_bytes(4, "little")
        with self.assertRaises(NotImplementedError):
            SillImage.open(bytes(bad_compression))
        wide_mask = bytearray(data)
        wide_mask[54:58] = (0x0000_FFFF).to_bytes(4, "little")  # 16-bit wide red mask
        with self.assertRaises(NotImplementedError):
            SillImage.open(bytes(wide_mask))
        no_red = bytearray(data)
        no_red[54:58] = (0).to_bytes(4, "little")
        with self.assertRaises(ValueError):
            SillImage.open(bytes(no_red))
        with self.assertRaises(ValueError):
            SillImage.open(bytes(data[:-4]))


if __name__ == "__main__":
    unittest.main()
