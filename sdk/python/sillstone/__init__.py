"""Sillstone Python SDK."""

from ._generated import (
    sillstone_version_major_raw,
    sillstone_version_minor_raw,
    sillstone_version_patch_raw,
    sillstone_buffer_checksum_raw,
)

__version__ = "0.2.0b1"


def version() -> str:
    """Returns library version string."""
    return f"{sillstone_version_major_raw()}.{sillstone_version_minor_raw()}.{sillstone_version_patch_raw()}"


def buffer_checksum(data: bytes) -> int:
    """Computes standard 64-bit FNV-1a checksum over input bytes."""
    return int(sillstone_buffer_checksum_raw(data, len(data)))
