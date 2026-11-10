#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Generates the Zstandard decoder vectors in ``zstd_vectors.jsonl``.

``zstandard`` (the Python binding over the reference ``libzstd``) is the
*oracle*: it is a development-time input of this script only. Nothing in the
shipped crates or SDKs depends on it, and the Rust test replays the committed
JSONL, not a live libzstd.

Every case is one line of flat JSON; the first line carries ``_provenance``.
Fields:

    id            stable case name
    level         compression level passed to the oracle
    checksum      1 when the frame carries a content checksum
    content_size  1 when the frame declares its content size
    window_log    windowLog override (0 = the level's default)
    ldm           1 when long-distance matching was enabled
    expect        "ok" or "err"
    err           expected error kind for "err" cases: truncated,
                  invalid_magic, bad_value or unsupported
    plain_len     produced byte count ("ok" cases)
    plain         lowercase hex of the produced bytes, empty above 4096
    plain_sha256  SHA-256 of the produced bytes ("ok" cases)
    frame         lowercase hex of the frame handed to the decoder

The generator refuses to write a case whose expectation the oracle does not
agree with: an ``ok`` case must decode through ``ZstdDecompressor.decompress``
with a matching digest, and an ``err`` case must raise there (libzstd's
one-shot API is strict about trailing bytes, truncated trailers and checksum
mismatches, which is the contract the decoder is held to).

Usage:
    pip install -r scripts/golden/requirements.txt
    python scripts/golden/gen_zstd_vectors.py           # rewrite the vectors
    python scripts/golden/gen_zstd_vectors.py --check   # verify byte-identical
"""

from __future__ import annotations

import argparse
import hashlib
import json
import random
import shutil
import subprocess
import sys
from pathlib import Path

import zstandard as zstd

OUT = (
    Path(__file__).resolve().parents[2]
    / "crates/inflate/tests/fixtures/zstd_vectors.jsonl"
)
SEED = 0x5A17_2026
MAX_INLINE_PLAIN = 1024
MAGIC = bytes.fromhex("28b52ffd")

WORDS = (
    "the of and to in a is that for it as was with be by on not he this are or "
    "his from at which but have an had they you were their one all we can her "
    "has there been if more when will would who so no sillstone zstandard "
    "finite state entropy huffman literal sequence offset match window frame "
    "checksum dictionary skippable meta block"
).split()


def text(rng: random.Random, n: int) -> bytes:
    out = bytearray()
    while len(out) < n:
        r = rng.randrange(1000)
        idx = min(len(WORDS) - 1, (r * r) // 1000 * len(WORDS) // 1000)
        out += WORDS[idx].encode()
        out += b"\n" if rng.randrange(12) == 0 else b" "
    return bytes(out[:n])


def noise(rng: random.Random, n: int) -> bytes:
    return bytes(rng.getrandbits(8) for _ in range(n))


def blob(rng: random.Random, n: int) -> bytes:
    """Structured binary: counters, small codes and zero padding."""
    out = bytearray()
    counter = rng.getrandbits(32)
    while len(out) < n:
        for _ in range(rng.randrange(1, 40)):
            out += counter.to_bytes(4, "little")
            out += rng.randrange(50).to_bytes(2, "little")
            out += rng.randrange(4).to_bytes(1, "little") + b"\0"
            counter = (counter + rng.randrange(1, 4)) & 0xFFFFFFFF
    return bytes(out[:n])


def phrase_repeats(rng: random.Random, n: int) -> bytes:
    """Phrases repeated at a handful of fixed distances (repeat offsets)."""
    phrases = [noise(rng, 11), noise(rng, 24), noise(rng, 37)]
    out = bytearray()
    while len(out) < n:
        out += phrases[rng.randrange(len(phrases))]
        out += b"." if rng.randrange(3) else b"-"
    return bytes(out[:n])


def long_distance(rng: random.Random, n: int, prefix: int) -> bytes:
    """A unique prefix, filler, then copies of the prefix at far distances."""
    head = noise(rng, prefix)
    out = bytearray(head)
    out += text(rng, max(0, n // 3 - len(out)))
    while len(out) < n:
        out += head[rng.randrange(0, prefix // 2) :]
        out += text(rng, min(rng.randrange(200, 4000), n - len(out)))
    return bytes(out[:n])


def frame(plain: bytes, *, level: int, checksum: bool, content_size: bool,
          window_log: int = 0, ldm: bool = False) -> bytes:
    params = zstd.ZstdCompressionParameters.from_level(
        level,
        window_log=window_log,
        enable_ldm=ldm,
        write_checksum=checksum,
        write_content_size=content_size,
        write_dict_id=False,
    )
    return zstd.ZstdCompressor(compression_params=params).compress(plain)


def oracle_all(data: bytes) -> bytes:
    """Decodes every frame in ``data`` (the one-shot API stops after the first)."""
    with zstd.ZstdDecompressor().stream_reader(data) as reader:
        return reader.read()


def oracle_ok(data: bytes) -> bool:
    try:
        zstd.ZstdDecompressor().decompress(data, max_output_size=1 << 28)
        return True
    except Exception:
        return False


def skippable(payload: bytes) -> bytes:
    return bytes.fromhex("502a4d18") + len(payload).to_bytes(4, "little") + payload


def block_header(last: bool, kind: int, size: int) -> bytes:
    hdr = int(last) | (kind << 1) | (size << 3)
    return hdr.to_bytes(3, "little")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true",
                    help="compare with the committed file instead of writing it")
    args = ap.parse_args()

    rng = random.Random(SEED)
    cases: list[dict[str, object]] = []

    def ok(case_id: str, plain: bytes, *, level: int, checksum: bool,
           content_size: bool, window_log: int = 0, ldm: bool = False,
           payload: bytes | None = None) -> None:
        data = payload if payload is not None else frame(
            plain, level=level, checksum=checksum, content_size=content_size,
            window_log=window_log, ldm=ldm)
        # The oracle has to reproduce the plaintext through the same stream the
        # decoder is handed: that is also what proves the hand-assembled
        # multi-frame and skippable cases say what they claim.
        assert oracle_all(data) == plain, (
            f"{case_id}: the oracle does not reproduce the plaintext")
        cases.append({
            "id": case_id,
            "level": level,
            "checksum": int(checksum),
            "content_size": int(content_size),
            "window_log": window_log,
            "ldm": int(ldm),
            "expect": "ok",
            "err": "",
            "plain_len": len(plain),
            "plain": plain.hex() if len(plain) <= MAX_INLINE_PLAIN else "",
            "plain_sha256": hashlib.sha256(plain).hexdigest(),
            "frame": data.hex(),
        })

    def err(case_id: str, data: bytes, kind: str, *, level: int = 0,
            checksum: bool = False, content_size: bool = False,
            window_log: int = 0, ldm: bool = False,
            cli_only: bool = False) -> None:
        # ``cli_only`` marks a rule libzstd's one-shot API is lenient about
        # but its own CLI enforces (bytes after the last frame); the reference
        # CLI is consulted instead of the Python binding when it is on PATH.
        if cli_only:
            zstd_bin = shutil.which("zstd")
            if zstd_bin is not None:
                res = subprocess.run([zstd_bin, "-d", "-c", "-q"], input=data,
                                     capture_output=True)
                assert res.returncode != 0, f"{case_id}: the reference CLI accepts this input"
        else:
            assert not oracle_ok(data), f"{case_id}: the oracle accepts this input"
        cases.append({
            "id": case_id,
            "level": level,
            "checksum": int(checksum),
            "content_size": int(content_size),
            "window_log": window_log,
            "ldm": int(ldm),
            "expect": "err",
            "err": kind,
            "plain_len": 0,
            "plain": "",
            "plain_sha256": "",
            "frame": data.hex(),
        })

    # Empty frames: one byte of frame content size, one block header of size 0.
    for level, checksum, content_size in ((1, True, True), (19, False, False), (22, True, False)):
        ok(f"empty-l{level}-{'ck' if checksum else 'nock'}", b"",
           level=level, checksum=checksum, content_size=content_size)

    # Raw (stored) blocks: incompressible payloads at a low level.
    for size, level, checksum, content_size in (
        (1, 1, True, True),
        (3, 3, False, True),
        (17, 1, True, False),
        (300, 9, False, False),
    ):
        ok(f"raw-{size}-l{level}", noise(rng, size), level=level,
           checksum=checksum, content_size=content_size)

    # RLE blocks: one byte repeated far past the block size, so the block is a
    # single repeated byte rather than a stored or entropy-coded one.
    for size, level in ((31, 5), (200_000, 1)):
        ok(f"rle-{size}-l{level}", b"\xff" * size, level=level,
           checksum=True, content_size=True)

    # Compressed blocks: text at several levels. Above 16 KiB of literals the
    # literals section needs its widest header, and a block building enough
    # sequences exercises the 2-byte sequence count.
    corpus = text(rng, 18_000)
    for level in (1, 3, 12, 19):
        ok(f"text-18k-l{level}", corpus, level=level, checksum=level % 2 == 1,
           content_size=True)

    # Binary structure at a high level.
    ok("blob-10k-l12", blob(rng, 10_000), level=12, checksum=True, content_size=True)

    # A 1 KiB window forces dozens of blocks, so the second and later blocks
    # reuse the Huffman tree and the FSE tables (treeless / repeat modes).
    ok("text-18k-window10", text(rng, 18_000), level=9, checksum=True,
       content_size=True, window_log=10)

    # Long matches: a unique 20 KiB prefix copied back after 120 KiB, so the
    # offsets land past the 128 KiB block maximum and inside a 128 MiB window.
    ok("long-ldm-l19", long_distance(rng, 160_000, 20_000), level=19,
       checksum=True, content_size=True, window_log=27, ldm=True)

    # Repeat offsets: short phrases at three fixed distances.
    ok("repeats-8k-l9", phrase_repeats(rng, 8_000), level=9, checksum=True,
       content_size=True)
    ok("repeats-64k-l12", phrase_repeats(rng, 64_000), level=12, checksum=False,
       content_size=False)

    # Multi-frame input: every frame decodes in sequence, into one buffer.
    plain_two = text(rng, 3000) + noise(rng, 2000)
    two = (frame(plain_two[:3000], level=3, checksum=True, content_size=True)
           + frame(plain_two[3000:], level=1, checksum=False, content_size=True))
    # The flags record the frame the case is named for; "checksum" is only 1
    # when every data frame in the stream carries one.
    ok("multi-frame-2", plain_two, level=3, checksum=False, content_size=True,
       payload=two)

    plain_three = text(rng, 2500) + blob(rng, 1500) + text(rng, 1500)
    three = (frame(plain_three[:2500], level=9, checksum=False, content_size=True)
             + frame(plain_three[2500:4000], level=3, checksum=True, content_size=False)
             + frame(plain_three[4000:], level=19, checksum=True, content_size=True))
    ok("multi-frame-3-mixed", plain_three, level=9, checksum=False,
       content_size=True, payload=three)

    # Skippable frames: zero-length and non-empty payloads around data frames.
    skip_plain = text(rng, 1200)
    ok("skippable-empty", skip_plain, level=3, checksum=True, content_size=True,
       payload=skippable(b"") + frame(skip_plain, level=3, checksum=True,
                                      content_size=True))
    skip_two = text(rng, 900)
    ok("skippable-around-frame", skip_two, level=3, checksum=True,
       content_size=True,
       payload=skippable(b"skip!") + frame(skip_two, level=3, checksum=True,
                                           content_size=True)
       + skippable(b"\x00" * 9))

    # A frame whose content size is absent still decodes to the right bytes.
    ok("no-content-size", text(rng, 5000), level=9, checksum=True,
       content_size=False)

    # A sweep across levels, sizes, windows and content classes: the shape the
    # random differential runs take, reduced to a committed, reproducible set.
    # The incompressible class stays small so the fixture stays small; the
    # randomised corpus in tests/zstd_oracle.rs covers the large noisy inputs.
    classes = (text, phrase_repeats, blob, noise)
    for i in range(62):
        cls = classes[i % len(classes)]
        size = (i * 37 + 11) % 2500
        if cls is noise:
            size = size % 300
        level = 1 + (i * 7) % 22
        window_log = (0, 0, 10, 17, 27)[i % 5]
        ok(f"sweep-{i:02d}-{cls.__name__}-l{level}-w{window_log}", cls(rng, size),
           level=level, checksum=bool(i % 2), content_size=bool(i % 3),
           window_log=window_log, ldm=window_log == 27 and i % 3 == 0)

    # Fail-closed cases. Each of these must be an error, never output.
    checksummed = frame(text(rng, 4000), level=3, checksum=True, content_size=True)
    bad = bytearray(checksummed)
    bad[-1] ^= 0x80
    err("err-checksum-mismatch", bytes(bad), "bad_value", level=3,
        checksum=True, content_size=True)

    err("err-empty-input", b"", "truncated")
    err("err-magic-only", MAGIC, "truncated")
    err("err-bad-magic", bytes.fromhex("28b52ffc") + b"\x00\x00", "invalid_magic")
    err("err-trailing-garbage", frame(b"abc", level=1, checksum=False,
                                      content_size=True) + b"\x01\x02\x03\x04",
        "invalid_magic", level=1, content_size=True, cli_only=True)

    stored = frame(noise(rng, 800), level=1, checksum=False, content_size=True)
    err("err-truncated-block-body", stored[:-100], "truncated", level=1,
        content_size=True)
    err("err-truncated-checksum", checksummed[:-4], "truncated", level=3,
        checksum=True, content_size=True)
    err("err-truncated-window-descriptor", frame(b"", level=3, checksum=False,
                                                 content_size=False)[:5],
        "truncated", level=3)

    header = MAGIC + bytes([0x00, 0x00])
    err("err-reserved-block-type", header + block_header(True, 3, 0), "bad_value")
    err("err-reserved-descriptor-bit", MAGIC + bytes([0x08, 0x00])
        + block_header(True, 0, 0), "bad_value")
    err("err-dictionary-id", MAGIC + bytes([0x01, 0x00, 0x2A])
        + block_header(True, 0, 0), "unsupported")
    err("err-block-larger-than-window", MAGIC + bytes([0x00, 0x00])
        + block_header(True, 0, 1500) + b"\x00" * 1500, "bad_value")
    err("err-content-size-mismatch", MAGIC + bytes([0x20, 0x05])
        + block_header(True, 0, 3) + b"abc", "bad_value")
    err("err-skippable-truncated", bytes.fromhex("502a4d18") + (9).to_bytes(4, "little")
        + b"\x00", "truncated")

    provenance = {
        "_provenance": "scripts/golden/gen_zstd_vectors.py",
        "oracle": f"zstandard {zstd.__version__}",
        "libzstd": "%d.%d.%d" % zstd.ZSTD_VERSION,
        "seed": hex(SEED),
        "cases": str(len(cases)),
        "regenerate": "python scripts/golden/gen_zstd_vectors.py",
        "check": "python scripts/golden/gen_zstd_vectors.py --check",
        "note": "dev-time oracle only; the Rust test replays these lines",
    }
    lines = [json.dumps(provenance, separators=(",", ":"), sort_keys=True)]
    for case in cases:
        lines.append(json.dumps(case, separators=(",", ":"), sort_keys=True))
    rendered = "\n".join(lines) + "\n"
    raw = rendered.encode()

    if args.check:
        current = OUT.read_bytes() if OUT.exists() else b""
        if current != raw:
            print(f"{OUT} is stale: regenerate it", file=sys.stderr)
            return 1
        print(f"{OUT} is byte-identical ({len(cases)} cases)")
        return 0

    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_bytes(raw)
    print(f"wrote {len(cases)} cases, {len(raw)} bytes to {OUT}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
