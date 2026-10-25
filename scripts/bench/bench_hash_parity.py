"""Parity + speed benchmark: sillstone vs hashlib and imagehash.

Dev-only oracle comparison (hashlib is stdlib; imagehash/pillow/numpy are NOT shipped
dependencies). Digests must be bit-identical to hashlib. Perceptual hashes are timed
against imagehash and compared bit-for-bit (Hamming distance over the 64-bit value,
MSB-first row-major, the order imagehash's ``__int__``-style flattening uses).

Usage:
    python scripts/bench/bench_hash_parity.py [--json out.json] [--quick] [--repeats N]

Exit status is non-zero when any digest differs from hashlib or any image hash
differs from imagehash.
"""

import argparse
import hashlib
import json
import os
import platform
import sys
import time

sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), "../../sdk/python")))

import sillstone  # noqa: E402

DIGESTS = [
    ("sha256", hashlib.sha256, sillstone.sha256),
    ("sha3_256", hashlib.sha3_256, sillstone.sha3_256),
    ("sha3_512", hashlib.sha3_512, sillstone.sha3_512),
    ("blake2b_512", hashlib.blake2b, sillstone.blake2b_512),
]


def best_of(fn, min_seconds=0.15, repeats=5):
    """Median-of-best timing: seconds per call."""
    calls = 1
    while True:
        t0 = time.perf_counter()
        for _ in range(calls):
            fn()
        dt = time.perf_counter() - t0
        if dt >= min_seconds or calls >= 1 << 20:
            break
        calls *= 2
    samples = []
    for _ in range(repeats):
        t0 = time.perf_counter()
        for _ in range(calls):
            fn()
        samples.append((time.perf_counter() - t0) / calls)
    samples.sort()
    return samples[len(samples) // 2]


def pattern(n):
    block = bytes((i * 131 + 17) & 0xFF for i in range(4096))
    return (block * (n // len(block) + 1))[:n]


def bench_digests(sizes):
    rows = []
    for name, ref, ours in DIGESTS:
        for size in sizes:
            data = pattern(size)
            ok = ref(data).digest() == ours(data)
            t_ref = best_of(lambda: ref(data).digest())
            t_our = best_of(lambda: ours(data))
            rows.append(
                {
                    "algo": name,
                    "bytes": size,
                    "parity": ok,
                    "hashlib_s": t_ref,
                    "sillstone_s": t_our,
                    "speed_ratio": t_ref / t_our,  # >1: sillstone faster
                    "sillstone_MBps": size / t_our / 1e6,
                    "hashlib_MBps": size / t_ref / 1e6,
                }
            )
    return rows


def image_pixels(w, h):
    px = bytearray(w * h * 3)
    for y in range(h):
        row = y * w * 3
        for x in range(w):
            i = row + x * 3
            px[i] = (x ^ y) & 0xFF
            px[i + 1] = (x * 3 + y * 7) & 0xFF
            px[i + 2] = (x * 11 + (y >> 2)) & 0xFF
    return bytes(px)


def bits_to_int(arr):
    value = 0
    for b in arr.flatten():
        value = (value << 1) | int(bool(b))
    return value


def bench_image_hashes(sizes):
    try:
        import imagehash
        from PIL import Image
    except ImportError as exc:  # pragma: no cover - dev tool
        print(f"imagehash/pillow unavailable ({exc}); skipping image-hash section")
        return []
    pairs = [
        ("ahash", imagehash.average_hash, sillstone.ahash),
        ("dhash", imagehash.dhash, sillstone.dhash),
        ("phash", imagehash.phash, sillstone.phash),
        ("whash", imagehash.whash, sillstone.whash),
    ]
    rows = []
    for w, h in sizes:
        px = image_pixels(w, h)
        pil = Image.frombytes("RGB", (w, h), px)
        for name, ref, ours in pairs:
            ref_val = bits_to_int(ref(pil).hash)
            our_val = ours(px, w, h, 2)
            hamming = bin(ref_val ^ our_val).count("1")
            t_ref = best_of(lambda: ref(pil), min_seconds=0.1, repeats=3)
            t_our = best_of(lambda: ours(px, w, h, 2), min_seconds=0.1, repeats=3)
            rows.append(
                {
                    "algo": name,
                    "size": f"{w}x{h}",
                    "hamming_vs_imagehash": hamming,
                    "imagehash_s": t_ref,
                    "sillstone_s": t_our,
                    "speed_ratio": t_ref / t_our,
                }
            )
        # colorhash: imagehash returns 14*binbits bits, compare when widths are equal
        ref_c = imagehash.colorhash(pil)
        our_c = sillstone.colorhash(px, w, h, 2)
        t_ref = best_of(lambda: imagehash.colorhash(pil), min_seconds=0.1, repeats=3)
        t_our = best_of(lambda: sillstone.colorhash(px, w, h, 2), min_seconds=0.1, repeats=3)
        rows.append(
            {
                "algo": "colorhash",
                "size": f"{w}x{h}",
                "hamming_vs_imagehash": bin(bits_to_int(ref_c.hash) ^ our_c).count("1"),
                "imagehash_s": t_ref,
                "sillstone_s": t_our,
                "speed_ratio": t_ref / t_our,
            }
        )
    return rows


def summarize(runs, section, keys):
    """Per-row median/min/max of `speed_ratio` over the repeated runs."""
    table = {}
    for run in runs:
        for r in run[section]:
            table.setdefault(tuple(r[k] for k in keys), []).append(r["speed_ratio"])
    out = []
    for key, ratios in table.items():
        ratios.sort()
        row = dict(zip(keys, key))
        row.update(
            {
                "runs": len(ratios),
                "ratio_median": ratios[len(ratios) // 2],
                "ratio_min": ratios[0],
                "ratio_max": ratios[-1],
            }
        )
        out.append(row)
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--json")
    ap.add_argument("--quick", action="store_true")
    ap.add_argument("--repeats", type=int, default=1, help="full-suite repetitions (CI uses >= 5)")
    args = ap.parse_args()
    sizes = [64, 4096, 1 << 20] if args.quick else [64, 4096, 1 << 20, 16 << 20]
    img_sizes = [(256, 256)] if args.quick else [(256, 256), (1024, 1024)]

    runs = [
        {"digests": bench_digests(sizes), "image_hashes": bench_image_hashes(img_sizes)}
        for _ in range(args.repeats)
    ]
    result = {
        "sillstone_version": sillstone.version(),
        "python": platform.python_version(),
        "platform": platform.platform(),
        "machine": platform.machine(),
        "repeats": args.repeats,
        "runs": runs,
        "digest_summary": summarize(runs, "digests", ("algo", "bytes")),
        "image_summary": summarize(runs, "image_hashes", ("algo", "size")),
    }

    print(f"sillstone {result['sillstone_version']} on {result['platform']} ({result['machine']}), {args.repeats} run(s)")
    print(f"{'digest':<12}{'bytes':>10}{'ratio med':>11}{'min':>8}{'max':>8}")
    for r in result["digest_summary"]:
        print(f"{r['algo']:<12}{r['bytes']:>10}{r['ratio_median']:>11.2f}{r['ratio_min']:>8.2f}{r['ratio_max']:>8.2f}")
    print(f"\n{'imagehash':<12}{'size':>12}{'ratio med':>11}{'min':>8}{'max':>8}")
    for r in result["image_summary"]:
        print(f"{r['algo']:<12}{r['size']:>12}{r['ratio_median']:>11.2f}{r['ratio_min']:>8.2f}{r['ratio_max']:>8.2f}")
    if args.json:
        with open(args.json, "w", encoding="utf-8") as fh:
            json.dump(result, fh, indent=2)
    digest_ok = all(r["parity"] for run in runs for r in run["digests"])
    # Every perceptual hash of this smooth synthetic image must match imagehash bit for bit.
    image_ok = all(r["hamming_vs_imagehash"] == 0 for run in runs for r in run["image_hashes"])
    return 0 if digest_ok and image_ok else 1


if __name__ == "__main__":
    sys.exit(main())
