"""Sillstone Python SDK: Zero-dependency multimedia, hashing, and array computing suite."""

from __future__ import annotations

import ctypes
from dataclasses import dataclass
from pathlib import Path
import struct

from ._generated import (
    LIB,
    sillstone_version_major_raw,
    sillstone_version_minor_raw,
    sillstone_version_patch_raw,
    sillstone_buffer_checksum_raw,
    sillstone_digest_sha256_raw,
    sillstone_digest_sha3_256_raw,
    sillstone_digest_sha3_512_raw,
    sillstone_digest_blake2b_512_raw,
    sillstone_digest_blake3_raw,
    sillstone_math_dst1_raw,
    sillstone_math_dst2_raw,
    sillstone_math_dst3_raw,
    sillstone_unicode_grapheme_count_raw,
    sillstone_unicode_char_category_raw,
    sillstone_image_phash_raw,
    sillstone_image_ahash_raw,
    sillstone_image_dhash_raw,
    sillstone_image_whash_raw,
    sillstone_image_colorhash_raw,
    sillstone_text_tlsh_raw,
    sillstone_text_tlsh_distance_raw,
    sillstone_curator_detect_raw,
    sillstone_curator_content_hash_raw,
    sillstone_curator_signature_raw,
    sillstone_curator_describe_raw,
    sillstone_curator_match_raw,
    sillstone_curator_free_raw,
)
from .image import Image, ImageFilter, Resampling, Transpose
from .array import Array, array, asarray, zeros, ones, arange, linspace, matmul, dot
__version__ = "0.3.0b1"

__all__ = [
    "Image",
    "ImageFilter",
    "Resampling",
    "Transpose",
    "Array",
    "array",
    "asarray",
    "zeros",
    "ones",
    "arange",
    "linspace",
    "matmul",
    "dot",
    "version",
    "buffer_checksum",
    "sha256",
    "sha3_256",
    "sha3_512",
    "blake2b_512",
    "blake3",
    "dst1",
    "dst2",
    "dst3",
    "grapheme_count",
    "char_category",
    "phash",
    "ahash",
    "dhash",
    "whash",
    "colorhash",
    "tlsh",
    "tlsh_distance",
    "detect",
    "content_hash",
    "signature",
    "describe",
    "match_signatures",
    "parse_signature",
    "parse_describe",
    "fnv1a64",
    "Detection",
    "Signature",
    "Description",
    "MatchResult",
    "FfiError",
    "FORMATS",
    "MODALITIES",
    "MATCH_TAGS",
    "STATUS_OK",
    "STATUS_INVALID",
    "STATUS_REJECTED",
]

#: Status codes
STATUS_OK = 0
STATUS_INVALID = -1
STATUS_REJECTED = -2

#: Format names by wire code (declaration order)
FORMATS = ("png", "jpeg", "bmp", "zip", "mp4", "mp3", "wav", "flac", "pdf", "gif", "unknown")

#: Modality names by wire code (declaration order)
MODALITIES = ("image", "audio", "text", "binary", "video")

#: Match-outcome tags by wire code (declaration order)
MATCH_TAGS = ("image", "audio", "text", "binary", "video")

#: Signature-stream tag bytes (declaration order)
SIGNATURE_TAGS = ("image", "audio", "text", "binary", "video")


class FfiError(Exception):
    """A non-zero status code came back from the cdylib."""

    def __init__(self, op: str, status: int) -> None:
        kind = {
            STATUS_INVALID: "invalid argument",
            STATUS_REJECTED: "input rejected",
        }.get(status, "unknown failure")
        super().__init__(f"{op} failed: {kind} (status {status})")
        self.status = status


@dataclass(frozen=True)
class Detection:
    """Container sniffing result."""
    format: str
    modality: str
    pending: bool


@dataclass(frozen=True)
class Signature:
    """A tier-2 signature, re-expressed from the signature stream."""
    modality: str
    phash: int | None = None
    peak_count: int | None = None
    frames: int | None = None
    peaks: tuple[tuple[int, int], ...] | None = None
    peaks_bytes: bytes | None = None
    word_count: int | None = None
    words: tuple[int, ...] | None = None
    chunk_count: int | None = None
    digest_bytes: bytes | None = None
    frame_count: int | None = None
    width: int | None = None
    height: int | None = None
    duration_bits: int | None = None
    frame_hashes: tuple[int, ...] | None = None
    minhash: tuple[int, ...] | None = None


@dataclass(frozen=True)
class Description:
    """Both tiers plus facts, re-expressed from the describe stream."""
    format: str
    modality: str
    tier1: str
    signature: Signature
    facts: dict


@dataclass(frozen=True)
class MatchResult:
    """A match outcome, re-expressed from the slots."""
    tag: str
    slots: tuple[int, ...]
    matched: bool

    def slot_float(self, index: int) -> float:
        """Reinterprets slot `index` as an IEEE-754 double."""
        return struct.unpack("<d", struct.pack("<Q", self.slots[index]))[0]


def version() -> str:
    """Returns library version string."""
    return f"{sillstone_version_major_raw()}.{sillstone_version_minor_raw()}.{sillstone_version_patch_raw()}"


def buffer_checksum(data: bytes) -> int:
    """Computes standard 64-bit FNV-1a checksum over input bytes."""
    return int(sillstone_buffer_checksum_raw(data, len(data)))


def sha256(data: bytes) -> bytes:
    """Computes 32-byte SHA-256 digest."""
    out = ctypes.create_string_buffer(32)
    rc = sillstone_digest_sha256_raw(data, len(data), out)
    if rc != 0:
        raise FfiError("sha256", rc)
    return out.raw


def sha3_256(data: bytes) -> bytes:
    """Computes 32-byte SHA-3-256 digest."""
    out = ctypes.create_string_buffer(32)
    rc = sillstone_digest_sha3_256_raw(data, len(data), out)
    if rc != 0:
        raise FfiError("sha3_256", rc)
    return out.raw


def sha3_512(data: bytes) -> bytes:
    """Computes 64-byte SHA-3-512 digest."""
    out = ctypes.create_string_buffer(64)
    rc = sillstone_digest_sha3_512_raw(data, len(data), out)
    if rc != 0:
        raise FfiError("sha3_512", rc)
    return out.raw


def blake2b_512(data: bytes) -> bytes:
    """Computes 64-byte unkeyed BLAKE2b digest."""
    out = ctypes.create_string_buffer(64)
    rc = sillstone_digest_blake2b_512_raw(data, len(data), out)
    if rc != 0:
        raise FfiError("blake2b_512", rc)
    return out.raw


def blake3(data: bytes) -> bytes:
    """Computes 32-byte BLAKE3 digest."""
    out = ctypes.create_string_buffer(32)
    rc = sillstone_digest_blake3_raw(data, len(data), out)
    if rc != 0:
        raise FfiError("blake3", rc)
    return out.raw


def dst1(data: list[float]) -> list[float]:
    """Computes orthonormal DST-I over input float array."""
    c_in = (ctypes.c_double * len(data))(*data)
    c_out = (ctypes.c_double * len(data))()
    rc = sillstone_math_dst1_raw(c_in, len(data), c_out)
    if rc != 0:
        raise FfiError("dst1", rc)
    return list(c_out)


def dst2(data: list[float]) -> list[float]:
    """Computes orthonormal DST-II over input float array."""
    c_in = (ctypes.c_double * len(data))(*data)
    c_out = (ctypes.c_double * len(data))()
    rc = sillstone_math_dst2_raw(c_in, len(data), c_out)
    if rc != 0:
        raise FfiError("dst2", rc)
    return list(c_out)


def dst3(data: list[float]) -> list[float]:
    """Computes orthonormal DST-III over input float array."""
    c_in = (ctypes.c_double * len(data))(*data)
    c_out = (ctypes.c_double * len(data))()
    rc = sillstone_math_dst3_raw(c_in, len(data), c_out)
    if rc != 0:
        raise FfiError("dst3", rc)
    return list(c_out)


def grapheme_count(text: str) -> int:
    """Counts UAX #29 grapheme clusters in a string."""
    raw = text.encode("utf-8")
    count = ctypes.c_size_t()
    rc = sillstone_unicode_grapheme_count_raw(raw, len(raw), ctypes.byref(count))
    if rc != 0:
        raise FfiError("grapheme_count", rc)
    return count.value


def char_category(char: str) -> int:
    """Returns general category tag for a single character."""
    cp = ord(char[0])
    cat = ctypes.c_uint8()
    rc = sillstone_unicode_char_category_raw(cp, ctypes.byref(cat))
    if rc != 0:
        raise FfiError("char_category", rc)
    return cat.value


def phash(data: bytes, width: int, height: int, layout: int = 0) -> int:
    """64-bit pHash, bit-exact with `imagehash.phash` (hash_size=8)."""
    out = ctypes.c_uint64()
    rc = sillstone_image_phash_raw(data, len(data), width, height, layout, ctypes.byref(out))
    if rc != 0:
        raise FfiError("phash", rc)
    return out.value


def ahash(data: bytes, width: int, height: int, layout: int = 0) -> int:
    """64-bit aHash, bit-exact with `imagehash.average_hash`."""
    out = ctypes.c_uint64()
    rc = sillstone_image_ahash_raw(data, len(data), width, height, layout, ctypes.byref(out))
    if rc != 0:
        raise FfiError("ahash", rc)
    return out.value


def dhash(data: bytes, width: int, height: int, layout: int = 0) -> int:
    """64-bit dHash, bit-exact with `imagehash.dhash`."""
    out = ctypes.c_uint64()
    rc = sillstone_image_dhash_raw(data, len(data), width, height, layout, ctypes.byref(out))
    if rc != 0:
        raise FfiError("dhash", rc)
    return out.value


def whash(data: bytes, width: int, height: int, layout: int = 0, level: int = 0) -> int:
    """64-bit wHash, bit-exact with `imagehash.whash`.

    `level` is log2(image_scale); 0 (default) is imagehash's default
    2**floor(log2(min(width, height))), otherwise 3..=15.
    """
    out = ctypes.c_uint64()
    rc = sillstone_image_whash_raw(data, len(data), width, height, layout, level, ctypes.byref(out))
    if rc != 0:
        raise FfiError("whash", rc)
    return out.value


def colorhash(data: bytes, width: int, height: int, layout: int = 0, bins: int = 3) -> int:
    """64-bit colorhash, bit-exact with `imagehash.colorhash(binbits=bins)`.

    `bins` is binbits: 0 or 3 (default) gives imagehash's 42-bit hash, 1..=4
    selects 14*bins bits.
    """
    out = ctypes.c_uint64()
    rc = sillstone_image_colorhash_raw(data, len(data), width, height, layout, bins, ctypes.byref(out))
    if rc != 0:
        raise FfiError("colorhash", rc)
    return out.value


def tlsh(data: bytes) -> str:
    """Computes 70-hex-char TrendMicro Locality Sensitive Hash string."""
    out = ctypes.create_string_buffer(72)
    rc = sillstone_text_tlsh_raw(data, len(data), out)
    if rc != 0:
        raise FfiError("tlsh", rc)
    return out.raw.decode("ascii", errors="replace")


def tlsh_distance(hash_a: str | bytes, hash_b: str | bytes) -> int:
    """Computes TLSH distance between two 70-hex-char hashes."""
    a_bytes = hash_a.encode("ascii") if isinstance(hash_a, str) else hash_a
    b_bytes = hash_b.encode("ascii") if isinstance(hash_b, str) else hash_b
    out = ctypes.c_uint32()
    rc = sillstone_text_tlsh_distance_raw(a_bytes, b_bytes, ctypes.byref(out))
    if rc != 0:
        raise FfiError("tlsh_distance", rc)
    return out.value


def fnv1a64(data: bytes) -> int:
    """The 64-bit FNV-1a fold the reference sub-digests use."""
    h = 0xCBF29CE484222325
    for b in data:
        h ^= b
        h = (h * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
    return h


def detect(data: bytes) -> Detection:
    """Sniffs container and modality without full decode."""
    fmt = ctypes.c_uint32()
    mod = ctypes.c_uint32()
    pending = ctypes.c_uint32()
    rc = sillstone_curator_detect_raw(
        data, len(data), ctypes.byref(fmt), ctypes.byref(mod), ctypes.byref(pending)
    )
    if rc != 0:
        raise FfiError("curator_detect", rc)
    return Detection(
        format=FORMATS[fmt.value],
        modality=MODALITIES[mod.value],
        pending=pending.value == 1,
    )


def content_hash(data: bytes) -> bytes:
    """Computes 32-byte tier-1 content digest."""
    buf = (ctypes.c_ubyte * 32)()
    out_len = ctypes.c_size_t()
    rc = sillstone_curator_content_hash_raw(data, len(data), buf, len(buf), ctypes.byref(out_len))
    if rc != 0:
        raise FfiError("curator_content_hash", rc)
    return bytes(buf)


def _stream_op(raw_fn, data: bytes) -> bytes:
    out = ctypes.c_void_p()
    out_len = ctypes.c_size_t()
    rc = raw_fn(data, len(data), ctypes.byref(out), ctypes.byref(out_len))
    if rc != 0:
        raise FfiError(raw_fn.__name__, rc)
    try:
        return ctypes.string_at(out, out_len.value)
    finally:
        sillstone_curator_free_raw(out, out_len.value)


def signature(data: bytes) -> Signature:
    """Computes tier-2 signature stream and parses it into a Signature object."""
    return parse_signature(_stream_op(sillstone_curator_signature_raw, data))


def describe(data: bytes) -> Description:
    """Computes both tiers and parses the describe stream."""
    return parse_describe(_stream_op(sillstone_curator_describe_raw, data))


def match_signatures(a: bytes, b: bytes) -> MatchResult:
    """Scores two raw inputs across modalities."""
    tag = ctypes.c_uint32()
    slots = (ctypes.c_uint64 * 4)()
    matched = ctypes.c_uint32()
    rc = sillstone_curator_match_raw(
        a, len(a), b, len(b), ctypes.byref(tag), slots, len(slots), ctypes.byref(matched)
    )
    if rc != 0:
        raise FfiError("curator_match", rc)
    return MatchResult(
        tag=MATCH_TAGS[tag.value],
        slots=tuple(int(s) for s in slots),
        matched=matched.value == 1,
    )

def parse_signature(raw: bytes) -> Signature:
    """Re-expresses the canonical signature stream as a Signature."""
    if len(raw) < 1:
        raise ValueError("signature stream is empty")
    tag = raw[0]
    if tag >= len(SIGNATURE_TAGS):
        raise ValueError(f"unknown signature tag {tag}")
    modality = SIGNATURE_TAGS[tag]
    if modality == "image":
        if len(raw) != 9:
            raise ValueError("image stream is not tag + u64")
        return Signature(modality=modality, phash=int.from_bytes(raw[1:9], "big"))
    if modality == "audio":
        if len(raw) < 13:
            raise ValueError("audio stream is shorter than its 13-byte header")
        peak_count = int.from_bytes(raw[1:9], "big")
        frames = int.from_bytes(raw[9:13], "big")
        payload = raw[13:]
        if peak_count != len(payload) or len(payload) % 6 != 0:
            raise ValueError("audio peak count disagrees with the payload")
        peaks = tuple(
            (
                int.from_bytes(payload[i : i + 4], "little"),
                int.from_bytes(payload[i + 4 : i + 6], "little"),
            )
            for i in range(0, len(payload), 6)
        )
        return Signature(
            modality=modality,
            peak_count=peak_count,
            frames=frames,
            peaks=peaks,
            peaks_bytes=payload,
        )
    if modality == "text":
        if len(raw) < 9:
            raise ValueError("text stream is shorter than its 9-byte header")
        word_count = int.from_bytes(raw[1:9], "big")
        payload = raw[9:]
        if word_count != len(payload) or len(payload) % 8 != 0:
            raise ValueError("text word count disagrees with the payload")
        words = tuple(int.from_bytes(payload[i : i + 8], "big") for i in range(0, len(payload), 8))
        return Signature(modality=modality, word_count=word_count, words=words)
    if modality == "binary":
        if len(raw) < 9:
            raise ValueError("binary stream is shorter than its 9-byte header")
        chunk_count = int.from_bytes(raw[1:9], "big")
        payload = raw[9:]
        if chunk_count != len(payload) or len(payload) % 32 != 0:
            raise ValueError("binary chunk count disagrees with the payload")
        return Signature(modality=modality, chunk_count=chunk_count, digest_bytes=payload)
    # video
    if len(raw) < 18:
        raise ValueError("video stream is shorter than its 18-byte header")
    frame_count = int.from_bytes(raw[1:5], "big")
    width = int.from_bytes(raw[5:9], "big")
    height = int.from_bytes(raw[9:13], "big")
    duration_bits = int.from_bytes(raw[13:21], "big")
    rest = raw[21:]
    if frame_count > len(rest) or frame_count % 8 != 0:
        raise ValueError("video frame count disagrees with the payload")
    frames_payload = rest[:frame_count]
    minhash_payload = rest[frame_count:]
    if len(minhash_payload) % 8 != 0:
        raise ValueError("video minhash payload is not whole words")
    return Signature(
        modality=modality,
        frame_count=frame_count,
        width=width,
        height=height,
        duration_bits=duration_bits,
        frame_hashes=tuple(
            int.from_bytes(frames_payload[i : i + 8], "little")
            for i in range(0, len(frames_payload), 8)
        ),
        minhash=tuple(
            int.from_bytes(minhash_payload[i : i + 8], "little")
            for i in range(0, len(minhash_payload), 8)
        ),
    )


def parse_describe(raw: bytes) -> Description:
    """Re-expresses the canonical describe stream as a Description."""
    if len(raw) < 35:
        raise ValueError("describe stream is shorter than its 35-byte header")
    fmt = raw[0]
    mod = raw[1]
    if fmt >= len(FORMATS) or mod >= len(MODALITIES):
        raise ValueError(f"unknown describe codes {fmt}/{mod}")
    tier1 = raw[2:34].hex()
    modality = MODALITIES[mod]
    sig_end = len(raw) - _facts_len(modality)
    sig = parse_signature(raw[34:sig_end])
    facts_raw = raw[sig_end:]
    facts: dict = {}
    if modality == "image":
        _need(facts_raw, 16)
        facts = {"width": _be32(facts_raw, 0), "height": _be32(facts_raw, 4), "keypoints": _be64(facts_raw, 8)}
    elif modality == "audio":
        _need(facts_raw, 18)
        facts = {
            "sample_rate": _be32(facts_raw, 0),
            "channels": int.from_bytes(facts_raw[4:6], "big"),
            "frames": _be32(facts_raw, 6),
            "peaks": _be64(facts_raw, 10),
        }
    elif modality == "text":
        _need(facts_raw, 16)
        facts = {"words": _be64(facts_raw, 0), "canonical_len": _be64(facts_raw, 8)}
    elif modality == "binary":
        _need(facts_raw, 16)
        facts = {"len": _be64(facts_raw, 0), "chunks": _be64(facts_raw, 8)}
    else:
        _need(facts_raw, 24)
        facts = {
            "width": _be32(facts_raw, 0),
            "height": _be32(facts_raw, 4),
            "duration_bits": _be64(facts_raw, 8),
            "frames_sampled": _be64(facts_raw, 16),
        }
    return Description(format=FORMATS[fmt], modality=modality, tier1=tier1, signature=sig, facts=facts)


def _be32(raw: bytes, off: int) -> int:
    return int.from_bytes(raw[off : off + 4], "big")


def _be64(raw: bytes, off: int) -> int:
    return int.from_bytes(raw[off : off + 8], "big")


def _need(raw: bytes, n: int) -> None:
    if len(raw) != n:
        raise ValueError(f"facts block is {len(raw)} bytes, want {n}")


def _facts_len(modality: str) -> int:
    if modality == "audio":
        return 18
    if modality == "video":
        return 24
    return 16
