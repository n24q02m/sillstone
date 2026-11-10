#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright (c) 2026 Sillstone Contributors
"""Prove an installed `sillstone` package can reach its compiled library.

The source distribution carries the Rust workspace and no binary: installing it
compiles the cdylib on the machine doing the install. Every check below crosses
the ctypes boundary, because the failure this guards against — the archive
installs, `import sillstone` succeeds, the first native call dies — is invisible
to an import-only smoke test. Values are pinned to published constants (FIPS
180-4, the FNV-1a offset basis, the definition of DST-I) so a cdylib built from
the wrong sources fails here instead of in a user's pipeline.

Run against any interpreter that can import sillstone:

    python scripts/ci/smoke-python.py
"""
from __future__ import annotations

import math
import pathlib
import sys

CDYLIB_SUFFIXES = (".so", ".dylib", ".dll")

#: SHA-256("abc"), FIPS 180-4 / RFC 6234 appendix B.1.
SHA256_ABC = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"

#: FNV-1a 64-bit offset basis, the checksum of the empty input.
FNV1A64_EMPTY = 14695981039346656037

FAILURES: list[str] = []


def check(name: str, condition: bool, detail: str = "") -> None:
    if condition:
        print(f"ok   {name}")
        return
    FAILURES.append(f"{name}{': ' + detail if detail else ''}")
    print(f"FAIL {name}{': ' + detail if detail else ''}")


def main() -> int:
    import sillstone

    package = pathlib.Path(sillstone.__file__).resolve().parent
    libraries = sorted(p.name for p in package.iterdir() if p.suffix in CDYLIB_SUFFIXES)
    check(
        "package carries a compiled library",
        bool(libraries),
        f"no {'/'.join(CDYLIB_SUFFIXES)} file beside {package}",
    )

    version = sillstone.version()
    check(
        "library reports the package version",
        sillstone.__version__.startswith(version) and version.count(".") == 2,
        f"version()={version!r} __version__={sillstone.__version__!r}",
    )

    digest = sillstone.sha256(b"abc").hex()
    check("sha256 matches FIPS 180-4", digest == SHA256_ABC, f"got {digest}")

    for name, value in (
        ("sha3_256", sillstone.sha3_256(b"abc")),
        ("sha3_512", sillstone.sha3_512(b"abc")),
        ("blake2b_512", sillstone.blake2b_512(b"abc")),
        ("blake3", sillstone.blake3(b"abc")),
    ):
        check(f"{name} returns its full width", len(value) == (64 if name.endswith("512") else 32), f"{len(value)} bytes")

    checksum = sillstone.buffer_checksum(b"")
    check("fnv1a64 matches the offset basis", checksum == FNV1A64_EMPTY, f"got {checksum}")

    # An orthonormal DST-I is its own inverse: a wrong kernel or a transposed
    # buffer breaks the round trip even though both calls still return floats.
    signal = [1.0, 2.0, 3.0, 4.0]
    transformed = sillstone.dst1(signal)
    restored = sillstone.dst1(transformed)
    check(
        "dst1 round-trips",
        len(transformed) == len(signal)
        and all(math.isclose(got, want, rel_tol=1e-12, abs_tol=1e-12) for got, want in zip(restored, signal)),
        f"{restored} != {signal}",
    )

    total = sillstone.array([[1.0, 2.0], [3.0, 4.0]]).sum()
    check("tensor sum crosses the ABI", total == 10.0, f"got {total!r}")

    gradient = bytes(range(64))
    first = sillstone.phash(gradient, 8, 8)
    check(
        "perceptual hash is a deterministic 64-bit value",
        isinstance(first, int) and 0 <= first < 2**64 and first == sillstone.phash(gradient, 8, 8),
        f"got {first!r}",
    )

    detection = sillstone.detect(b"\x89PNG\r\n\x1a\n" + b"\x00" * 8)
    check("container sniffing reads the magic", detection.format == "png", f"got {detection.format!r}")

    if FAILURES:
        print(f"\n{len(FAILURES)} check(s) failed against {package}", file=sys.stderr)
        return 1
    print(f"\nsillstone {sillstone.version()} answered every native call ({libraries[0]})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
