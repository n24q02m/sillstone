# SPDX-License-Identifier: Apache-2.0
# Copyright (c) 2026 pith-hash / sillstone Contributors
"""Hex-exact conformance: the committed reference vectors through Python.

Every one of the 45 vectors in the repository-root reference.json is replayed
through Python ctypes and compared against every recorded field:
  - 13 detect codes
  - 14 tier-2 signature fold sets (pHash, audio peaks, text/video minhash,
    cdc chunk digests, frame hashes)
  - 7 tier-1 / tier-2 describe packets
  - 9 match-outcome slot vectors
  - 2 pinned refusal errors (cross-modality, non-AVC video)
"""

from __future__ import annotations

import hashlib
import json
import math
import os
import struct
import unittest
from pathlib import Path

from sillstone import (
    FORMATS,
    FfiError,
    MatchResult,
    MODALITIES,
    Description,
    Detection,
    Signature,
    content_hash,
    describe,
    detect,
    fnv1a64,
    match_signatures,
    signature,
)

REPO_ROOT = Path(__file__).resolve().parents[3]
REFERENCE = json.loads((REPO_ROOT / "reference.json").read_text(encoding="utf-8"))["vectors"]
BY_NAME = {v["name"]: v for v in REFERENCE}
FIXTURES = REPO_ROOT / "tests" / "fixtures"

MASK64 = (1 << 64) - 1


# ---------------------------------------------------------------------
# The deterministic inline corpus (transcribed from tools/gen-reference)
# ---------------------------------------------------------------------


class SplitMix64:
    """The 64-bit generator behind every inline recipe."""

    def __init__(self, seed: int) -> None:
        self.s = seed & MASK64

    def next_u64(self) -> int:
        self.s = (self.s + 0x9E3779B97F4A7C15) & MASK64
        z = self.s
        z = ((z ^ (z >> 30)) * 0xBF58476D1CE4E5B9) & MASK64
        z = ((z ^ (z >> 27)) * 0x94D049BB133111EB) & MASK64
        return z ^ (z >> 31)


def splitmix_bytes(seed: int, length: int) -> bytes:
    """One next_u64 per byte, low byte kept — the upstream recipe."""
    rng = SplitMix64(seed)
    return bytes(rng.next_u64() & 0xFF for _ in range(length))


def blob48(seed: int) -> bytes:
    """The 48 KiB pseudo-file recipe of the upstream facade suite."""
    return splitmix_bytes(seed, 48 * 1024)


VOCAB = (
    "hash", "image", "audio", "signal", "codec", "block", "frame", "sample", "stream",
    "vector", "median", "kernel", "window", "raster", "pixel", "tone", "hash", "luma",
    "delta", "chunk", "radix", "filter", "table", "edge",
)


def prose(seed: int, size: int) -> str:
    """Seeded vocabulary words, one space apart, truncated to `size`."""
    rng = SplitMix64(seed)
    out = ""
    while len(out) < size:
        out += VOCAB[rng.next_u64() % len(VOCAB)] + " "
    return out[:size]


def patterned_pixels(seed: int, w: int, h: int) -> bytes:
    """Smooth periodic RGB content with real low-frequency energy."""
    rng = SplitMix64(seed)
    fx = float(rng.next_u64() % 5 + 1)
    fy = float(rng.next_u64() % 5 + 1)
    p1 = (rng.next_u64() % 628) / 100.0
    p2 = (rng.next_u64() % 628) / 100.0
    px = bytearray()
    for y in range(h):
        for x in range(w):
            u = x / w
            v = y / h
            r = 127.5 + 120.0 * math.sin(u * fx * (2 * math.pi) + p1)
            g = 127.5 + 120.0 * math.sin(v * fy * (2 * math.pi) + p2)
            b = 127.5 + 120.0 * math.sin((u + v) * (fx + fy) + p1 + p2)
            px += bytes((int(r) & 0xFF, int(g) & 0xFF, int(b) & 0xFF))
    return bytes(px)


def bmp24(w: int, h: int, rgb: bytes) -> bytes:
    """Bottom-up 24-bit BMP — the suite's deterministic fixture encoder."""
    assert len(rgb) == w * h * 3
    row = ((w * 3 + 3) // 4) * 4
    img = row * h
    f = bytearray()
    f += b"BM"
    f += struct.pack("<I", 54 + img)
    f += b"\x00\x00\x00\x00"
    f += struct.pack("<I", 54)
    f += struct.pack("<I", 40)
    f += struct.pack("<ii", w, h)
    f += struct.pack("<hh", 1, 24)
    f += b"\x00" * 24
    for y in range(h - 1, -1, -1):
        for x in range(w):
            i = (y * w + x) * 3
            f += bytes((rgb[i + 2], rgb[i + 1], rgb[i]))
        f += b"\x00" * (row - w * 3)
    return bytes(f)


def fixture(name: str) -> bytes:
    p = FIXTURES / name
    if not p.is_file():
        raise FileNotFoundError(f"Fixture {p} is missing")
    return p.read_bytes()


# Pinned payloads constructed identically to tools/gen-reference:
PNG48 = fixture("phash_rgb8_48x40.png")
GRAY40 = fixture("phash_gray8_40x32.png")
PNG_BASE = fixture("base_444.png")
JPG_BASE = fixture("base_444.jpg")
WAV = fixture("tone.wav")
FLAC = fixture("tone.flac")
MP3 = fixture("l3_short.mp3")
MP4 = fixture("a_64x48.mp4")
MOV = fixture("a_64x48.mov")
MP4V = fixture("e_mp4v.mp4")
PDF = fixture("text_page.pdf")

BMP_SYNTH = bmp24(48, 40, patterned_pixels(0xBEEF_0001, 48, 40))
BLOB = blob48(0xD15E_5EED)
BLOB_SHIFTED = b"\xaa" + blob48(0xD15E_5EED)
BLOB_OTHER = blob48(0xBADC_0FFE)
PROSE_A = prose(0xD1, 4096).encode()
PROSE_B = prose(0xD2, 4096).encode()


def sha256_hex(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def le_words(words) -> bytes:
    """The canonical word fold: little-endian words concatenated."""
    return b"".join(struct.pack("<Q", w & MASK64) for w in words)


MATCH_INPUTS = {
    "match-image-self": (PNG48, PNG48, "image"),
    "match-image-distinct": (PNG48, GRAY40, "image"),
    "match-image-png-vs-jpeg-same-pixels": (PNG_BASE, JPG_BASE, "image"),
    "match-audio-self": (WAV, WAV, "audio"),
    "match-text-self": (PROSE_A, PROSE_A, "text"),
    "match-text-distinct": (PROSE_A, PROSE_B, "text"),
    "match-binary-near-duplicate": (BLOB, BLOB_SHIFTED, "binary"),
    "match-binary-unrelated": (BLOB, BLOB_OTHER, "binary"),
    "match-video-mp4-vs-mov": (MP4, MOV, "video"),
}


class TestReferenceVectors(unittest.TestCase):
    def test_detect_vectors(self):
        cases = [
            ("detect-png", PNG48),
            ("detect-jpeg", JPG_BASE),
            ("detect-bmp", BMP_SYNTH),
            ("detect-wav", WAV),
            ("detect-flac", FLAC),
            ("detect-mp3", MP3),
            ("detect-mp4", MP4),
            ("detect-mov", MOV),
            ("detect-pdf", PDF),
            ("detect-zip", b"PK\x03\x04\x14\x00\x00\x00\x00\x00"),
            ("detect-gif", b"GIF89a\x01\x00\x01\x00"),
            ("detect-text", b"just some words"),
            ("detect-binary", b"\x00\xff\x02\xfe"),
        ]
        for name, data in cases:
            with self.subTest(vector=name):
                rec = BY_NAME[name]
                d: Detection = detect(data)
                self.assertEqual(d.format, rec["format"])
                self.assertEqual(d.modality, rec["modality"])
                self.assertEqual(d.pending, rec["pending"])
                self.assertIn(FORMATS.index(d.format), range(len(FORMATS)))
                self.assertIn(MODALITIES.index(d.modality), range(len(MODALITIES)))

    def assert_signature_matches(self, sig: Signature, rec: dict):
        self.assertEqual(sig.modality, rec["modality"])
        if sig.modality == "image":
            self.assertEqual(sig.phash, int(rec["phash"], 16))
        elif sig.modality == "audio":
            self.assertEqual(sig.peak_count, rec["peak_count"])
            self.assertEqual(sig.frames, rec["frames"])
            self.assertEqual(fnv1a64(sig.peaks_bytes), int(rec["peaks_fnv1a64"], 16))
            self.assertEqual(sha256_hex(sig.peaks_bytes), rec["peaks_sha256"])
        elif sig.modality == "text":
            self.assertEqual(sig.word_count, rec["minhash_words"])
            fold = le_words(sig.words)
            self.assertEqual(len(fold), sig.word_count)
            self.assertEqual(fnv1a64(fold), int(rec["minhash_fnv1a64"], 16))
        elif sig.modality == "binary":
            self.assertEqual(sig.chunk_count, rec["chunk_count"])
            self.assertEqual(fnv1a64(sig.digest_bytes), int(rec["chunks_fnv1a64"], 16))
            self.assertEqual(sha256_hex(sig.digest_bytes), rec["chunks_sha256"])
        else:  # video
            self.assertEqual(sig.frame_count, rec["frame_count"])
            frames_fold = le_words(sig.frame_hashes)
            self.assertEqual(len(frames_fold), sig.frame_count)
            self.assertEqual(fnv1a64(frames_fold), int(rec["frames_fnv1a64"], 16))
            self.assertEqual(sha256_hex(frames_fold), rec["frames_sha256"])
            self.assertIsNotNone(sig.minhash)
            minhash_fold = le_words(sig.minhash)
            self.assertEqual(len(minhash_fold), rec["minhash_words"])
            self.assertEqual(fnv1a64(minhash_fold), int(rec["minhash_fnv1a64"], 16))
            self.assertEqual(sig.width, rec["width"])
            self.assertEqual(sig.height, rec["height"])
            self.assertEqual(f"{sig.duration_bits:016x}", rec["duration_bits"])

    def test_signature_vectors(self):
        cases = [
            ("sig-image-phash_rgb8_48x40", PNG48),
            ("sig-image-phash_gray8_40x32", GRAY40),
            ("sig-image-base_444-png", PNG_BASE),
            ("sig-image-base_444-jpeg", JPG_BASE),
            ("sig-image-bmp-patterned-48x40", BMP_SYNTH),
            ("sig-audio-tone-wav", WAV),
            ("sig-audio-tone-flac", FLAC),
            ("sig-audio-l3_short-mp3", MP3),
            ("sig-text-prose-4kib", PROSE_A),
            ("sig-binary-splitmix64-48kib", BLOB),
            ("sig-binary-splitmix64-48kib-prefix-insert", BLOB_SHIFTED),
            ("sig-binary-splitmix64-48kib-unrelated", BLOB_OTHER),
            ("sig-video-a_64x48-mp4", MP4),
            ("sig-pdf-text_page", PDF),
        ]
        for name, data in cases:
            with self.subTest(vector=name):
                self.assert_signature_matches(signature(data), BY_NAME[name])

    def test_describe_vectors(self):
        cases = [
            ("describe-image-phash_rgb8_48x40", PNG48),
            ("describe-image-bmp-patterned-48x40", BMP_SYNTH),
            ("describe-audio-tone-wav", WAV),
            ("describe-text-prose-4kib", PROSE_A),
            ("describe-binary-splitmix64-48kib", BLOB),
            ("describe-video-a_64x48-mp4", MP4),
            ("describe-pdf-text_page", PDF),
        ]
        for name, data in cases:
            with self.subTest(vector=name):
                rec = BY_NAME[name]
                d: Description = describe(data)
                self.assertEqual(d.format, rec["format"])
                self.assertEqual(d.modality, rec["modality"])
                self.assertEqual(d.tier1, rec["tier1"])
                self.assert_signature_matches(d.signature, rec)
                facts = d.facts
                if d.modality == "image":
                    self.assertEqual(facts["width"], rec["facts"]["width"])
                    self.assertEqual(facts["height"], rec["facts"]["height"])
                    self.assertEqual(facts["keypoints"], rec["facts"]["keypoints"])
                elif d.modality == "audio":
                    self.assertEqual(facts["sample_rate"], rec["facts"]["sample_rate"])
                    self.assertEqual(facts["channels"], rec["facts"]["channels"])
                    self.assertEqual(facts["frames"], rec["facts"]["frames"])
                    self.assertEqual(facts["peaks"], rec["facts"]["peaks"])
                elif d.modality == "text":
                    self.assertEqual(facts["words"], rec["facts"]["words"])
                    self.assertEqual(facts["canonical_len"], rec["facts"]["canonical_len"])
                elif d.modality == "binary":
                    self.assertEqual(facts["len"], rec["facts"]["len"])
                    self.assertEqual(facts["chunks"], rec["facts"]["chunks"])
                else:  # video
                    self.assertEqual(facts["width"], rec["facts"]["width"])
                    self.assertEqual(facts["height"], rec["facts"]["height"])
                    self.assertEqual(f"{facts['duration_bits']:016x}", rec["facts"]["duration_bits"])
                    self.assertEqual(facts["frames_sampled"], rec["facts"]["frames_sampled"])

    def test_match_vectors(self):
        for name in sorted(MATCH_INPUTS):
            with self.subTest(vector=name):
                a, b, kind = MATCH_INPUTS[name]
                rec = BY_NAME[name]
                r: MatchResult = match_signatures(a, b)
                self.assertEqual(r.tag, kind)
                self.assertEqual(r.matched, rec["matched"])
                if kind == "image":
                    self.assertEqual(r.slots[0], rec["hamming"])
                elif kind == "audio":
                    if "votes" in rec and rec["votes"]:
                        self.assertEqual(r.slots[0], rec["votes"])
                        self.assertEqual(r.slots[1], rec["delta_t"] & MASK64)
                    else:
                        self.assertEqual(r.slots[0], 0)
                elif kind == "video":
                    self.assertEqual(f"{r.slots[0]:016x}", rec["score_bits"])
                    self.assertEqual(f"{r.slots[1]:016x}", rec["minhash_jaccard_bits"])
                else:
                    self.assertEqual(f"{r.slots[0]:016x}", rec["jaccard_bits"])

    def test_match_cross_modality_is_pinned_refusal(self):
        rec = BY_NAME["match-error-cross-modality"]
        with self.assertRaises(FfiError) as ctx:
            match_signatures(PNG48, PROSE_A)
        self.assertEqual(ctx.exception.status, -2)

    def test_error_video_non_avc_is_pinned_refusal(self):
        rec = BY_NAME["error-video-non-avc"]
        with self.assertRaises(FfiError) as ctx:
            signature(MP4V)
        self.assertEqual(ctx.exception.status, -2)

    def test_corrupt_image_is_rejected(self):
        corrupt = b"\x89PNG\r\n\x1a\ngarbage"
        with self.assertRaises(FfiError) as ctx:
            signature(corrupt)
        self.assertEqual(ctx.exception.status, -2)

        with self.assertRaises(FfiError) as ctx:
            describe(corrupt)
        self.assertEqual(ctx.exception.status, -2)

        with self.assertRaises(FfiError) as ctx:
            content_hash(corrupt)
        self.assertEqual(ctx.exception.status, -2)

    def test_malformed_pdf_is_rejected(self):
        with self.assertRaises(FfiError) as ctx:
            signature(b"%PDF-1.4 garbage not a pdf")
        self.assertEqual(ctx.exception.status, -2)

    def test_content_hash_literal_pin(self):
        self.assertEqual(
            content_hash(PNG48).hex(),
            "1f7637f88725b1f3c1b60fb5559486349298b2a5525bac5fb1df196651e0cb74",
        )


if __name__ == "__main__":
    unittest.main()
