"""Sillstone Python SDK."""

import ctypes
from ._generated import (
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
)

__version__ = "0.2.0b1"


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
        raise ValueError(f"sha256 failed with status {rc}")
    return out.raw


def sha3_256(data: bytes) -> bytes:
    """Computes 32-byte SHA-3-256 digest."""
    out = ctypes.create_string_buffer(32)
    rc = sillstone_digest_sha3_256_raw(data, len(data), out)
    if rc != 0:
        raise ValueError(f"sha3_256 failed with status {rc}")
    return out.raw


def sha3_512(data: bytes) -> bytes:
    """Computes 64-byte SHA-3-512 digest."""
    out = ctypes.create_string_buffer(64)
    rc = sillstone_digest_sha3_512_raw(data, len(data), out)
    if rc != 0:
        raise ValueError(f"sha3_512 failed with status {rc}")
    return out.raw


def blake2b_512(data: bytes) -> bytes:
    """Computes 64-byte unkeyed BLAKE2b digest."""
    out = ctypes.create_string_buffer(64)
    rc = sillstone_digest_blake2b_512_raw(data, len(data), out)
    if rc != 0:
        raise ValueError(f"blake2b_512 failed with status {rc}")
    return out.raw


def blake3(data: bytes) -> bytes:
    """Computes 32-byte BLAKE3 digest."""
    out = ctypes.create_string_buffer(32)
    rc = sillstone_digest_blake3_raw(data, len(data), out)
    if rc != 0:
        raise ValueError(f"blake3 failed with status {rc}")
    return out.raw

def dst1(data: list[float]) -> list[float]:
    """Computes orthonormal DST-I over input float array."""
    n = len(data)
    if n == 0:
        return []
    c_in = (ctypes.c_double * n)(*data)
    c_out = (ctypes.c_double * n)()
    rc = sillstone_math_dst1_raw(c_in, n, c_out)
    if rc != 0:
        raise ValueError(f"dst1 failed with status {rc}")
    return list(c_out)


def dst2(data: list[float]) -> list[float]:
    """Computes orthonormal DST-II over input float array."""
    n = len(data)
    if n == 0:
        return []
    c_in = (ctypes.c_double * n)(*data)
    c_out = (ctypes.c_double * n)()
    rc = sillstone_math_dst2_raw(c_in, n, c_out)
    if rc != 0:
        raise ValueError(f"dst2 failed with status {rc}")
    return list(c_out)


def dst3(data: list[float]) -> list[float]:
    """Computes orthonormal DST-III over input float array."""
    n = len(data)
    if n == 0:
        return []
    c_in = (ctypes.c_double * n)(*data)
    c_out = (ctypes.c_double * n)()
    rc = sillstone_math_dst3_raw(c_in, n, c_out)
    if rc != 0:
        raise ValueError(f"dst3 failed with status {rc}")
    return list(c_out)

def grapheme_count(text: str) -> int:
    """Counts UAX #29 grapheme clusters in a string."""
    b = text.encode("utf-8")
    count = ctypes.c_size_t(0)
    rc = sillstone_unicode_grapheme_count_raw(b, len(b), ctypes.byref(count))
    if rc != 0:
        raise ValueError(f"grapheme_count failed with status {rc}")
    return count.value


def char_category(char: str) -> int:
    """Returns general category tag for a single character."""
    if len(char) != 1:
        raise ValueError("char_category expects a 1-character string")
    cp = ord(char)
    cat = ctypes.c_char(b'\x00')
    rc = sillstone_unicode_char_category_raw(cp, ctypes.byref(cat))
    if rc != 0:
        raise ValueError(f"char_category failed with status {rc}")
    return ord(cat.value)
