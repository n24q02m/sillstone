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
