"""Probe which floating-point recipe the installed `pywt` uses for Haar.

`imagehash.whash` thresholds Haar wavelet coefficients at their median, so a
last-bit difference between two builds of PyWavelets can flip a bit of the
hash. x86-64 wheels are compiled without fused multiply-add; arm64 compilers
contract `sum += a * b` into one rounding. This prints, for 1-D and 2-D
transforms on random data, the share of coefficients each candidate recipe
reproduces bit for bit.

Usage: python scripts/bench/probe_pywt_fma.py   (needs pywavelets, numpy; Python >= 3.13)
"""

import math
import platform

import numpy as np
import pywt

C = 0.7071067811865476


def plain_dwt(x):
    x0, x1 = x[0::2], x[1::2]
    return C * x1 + C * x0, -C * x1 + C * x0


def fused_dwt(x):
    # sum = x1 * f0; sum = fma(x0, f1, sum)
    x0, x1 = x[0::2], x[1::2]
    lo = np.array([math.fma(a, C, C * b) for a, b in zip(x0, x1)])
    hi = np.array([math.fma(a, C, -C * b) for a, b in zip(x0, x1)])
    return lo, hi


def plain_idwt(lo, hi):
    out = np.empty(lo.size * 2)
    out[0::2] = C * lo + C * hi
    out[1::2] = C * lo - C * hi
    return out


def fused_idwt_a(lo, hi):
    out = np.empty(lo.size * 2)
    out[0::2] = [math.fma(C, h, C * l) for l, h in zip(lo, hi)]
    out[1::2] = [math.fma(-C, h, C * l) for l, h in zip(lo, hi)]
    return out


def fused_idwt_b(lo, hi):
    out = np.empty(lo.size * 2)
    out[0::2] = [math.fma(C, l, C * h) for l, h in zip(lo, hi)]
    out[1::2] = [math.fma(C, l, -C * h) for l, h in zip(lo, hi)]
    return out


def share(a, b):
    return float(np.mean(a == b))


def main():
    rng = np.random.default_rng(1)
    x = np.concatenate([rng.random(4096) * 255, rng.integers(0, 256, 4096).astype(np.float64)])
    lo, hi = pywt.dwt(x, "haar")
    print(f"platform {platform.platform()} {platform.machine()} pywt {pywt.__version__}")
    for name, fn in (("plain", plain_dwt), ("fused", fused_dwt)):
        l2, h2 = fn(x)
        print(f"dwt   {name:<8} lo {share(lo, l2):.4f} hi {share(hi, h2):.4f}")
    rec = pywt.idwt(lo, hi, "haar")
    for name, fn in (("plain", plain_idwt), ("fused_a", fused_idwt_a), ("fused_b", fused_idwt_b)):
        print(f"idwt  {name:<8} {share(rec, fn(lo, hi)):.4f}")


if __name__ == "__main__":
    main()
