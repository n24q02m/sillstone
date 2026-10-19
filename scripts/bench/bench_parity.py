"""Benchmark comparing Sillstone Pure-Rust engine vs Pillow (PIL) and NumPy."""

import os
import sys
import time

sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), "../../sdk/python")))

import numpy as np
from PIL import Image as PilImage, ImageFilter as PilImageFilter
from sillstone import Array, Image as SillImage, ImageFilter as SillImageFilter, Resampling


def bench_image():
    print("=" * 60)
    print("BENCHMARK: sillstone.Image vs PIL.Image")
    print("=" * 60)

    w, h = 512, 512
    # Create test image with non-trivial pattern
    pixels = bytearray(w * h * 3)
    for y in range(h):
        for x in range(w):
            idx = (y * w + x) * 3
            pixels[idx] = (x ^ y) & 0xFF
            pixels[idx + 1] = (x * 3 + y * 7) & 0xFF
            pixels[idx + 2] = (x * 11) & 0xFF

    pil_im = PilImage.frombytes("RGB", (w, h), bytes(pixels))
    sill_im = SillImage.frombytes("RGB", (w, h), bytes(pixels))

    # 1. Grayscale Conversion
    t0 = time.perf_counter()
    for _ in range(20):
        _ = pil_im.convert("L")
    t_pil_gray = (time.perf_counter() - t0) / 20 * 1000

    t0 = time.perf_counter()
    for _ in range(20):
        _ = sill_im.convert("L")
    t_sill_gray = (time.perf_counter() - t0) / 20 * 1000
    print(f"Convert RGB -> L (512x512):")
    print(f"  Pillow (PIL):     {t_pil_gray:.2f} ms")
    print(f"  Sillstone (Rust): {t_sill_gray:.2f} ms")

    # 2. Resampling Bilinear (512x512 -> 256x256)
    t0 = time.perf_counter()
    for _ in range(20):
        _ = pil_im.resize((256, 256), resample=PilImage.Resampling.BILINEAR)
    t_pil_resize = (time.perf_counter() - t0) / 20 * 1000

    t0 = time.perf_counter()
    for _ in range(20):
        _ = sill_im.resize((256, 256), resample=Resampling.BILINEAR)
    t_sill_resize = (time.perf_counter() - t0) / 20 * 1000
    print(f"\nResize Bilinear 512x512 -> 256x256:")
    print(f"  Pillow (PIL):     {t_pil_resize:.2f} ms")
    print(f"  Sillstone (Rust): {t_sill_resize:.2f} ms")

    # 3. 2D Convolution Blur (512x512)
    t0 = time.perf_counter()
    for _ in range(10):
        _ = pil_im.filter(PilImageFilter.BLUR)
    t_pil_blur = (time.perf_counter() - t0) / 10 * 1000

    t0 = time.perf_counter()
    for _ in range(10):
        _ = sill_im.filter(SillImageFilter.BLUR)
    t_sill_blur = (time.perf_counter() - t0) / 10 * 1000
    print(f"\n2D Convolution Blur (512x512):")
    print(f"  Pillow (PIL):     {t_pil_blur:.2f} ms")
    print(f"  Sillstone (Rust): {t_sill_blur:.2f} ms")


def bench_array():
    print("\n" + "=" * 60)
    print("BENCHMARK: sillstone.Array vs NumPy")
    print("=" * 60)

    n = 100_000
    np_a = np.ones(n, dtype=np.float64) * 2.5
    np_b = np.ones(n, dtype=np.float64) * 1.5

    raw_a = (np_a.tobytes())
    raw_b = (np_b.tobytes())
    sill_a = Array(shape=(n,), dtype="float64", buffer=raw_a)
    sill_b = Array(shape=(n,), dtype="float64", buffer=raw_b)

    # 1. Elementwise Add
    t0 = time.perf_counter()
    for _ in range(20):
        _ = np_a + np_b
    t_np_add = (time.perf_counter() - t0) / 20 * 1000

    t0 = time.perf_counter()
    for _ in range(20):
        _ = sill_a + sill_b
    t_sill_add = (time.perf_counter() - t0) / 20 * 1000
    print(f"Vectorized Add (100,000 elements):")
    print(f"  NumPy:            {t_np_add:.2f} ms")
    print(f"  Sillstone (Rust): {t_sill_add:.2f} ms")

    # 2. Reduction Sum
    t0 = time.perf_counter()
    for _ in range(50):
        _ = np_a.sum()
    t_np_sum = (time.perf_counter() - t0) / 50 * 1000

    t0 = time.perf_counter()
    for _ in range(50):
        _ = sill_a.sum()
    t_sill_sum = (time.perf_counter() - t0) / 50 * 1000
    print(f"\nSum Reduction (100,000 elements):")
    print(f"  NumPy:            {t_np_sum:.2f} ms")
    print(f"  Sillstone (Rust): {t_sill_sum:.2f} ms")

    # 3. 2D Matmul (128x128 @ 128x128)
    dim = 128
    np_mat1 = np.ones((dim, dim), dtype=np.float64) * 1.1
    np_mat2 = np.ones((dim, dim), dtype=np.float64) * 0.9

    sill_mat1 = Array(shape=(dim, dim), dtype="float64", buffer=np_mat1.tobytes())
    sill_mat2 = Array(shape=(dim, dim), dtype="float64", buffer=np_mat2.tobytes())

    t0 = time.perf_counter()
    for _ in range(20):
        _ = np_mat1 @ np_mat2
    t_np_matmul = (time.perf_counter() - t0) / 20 * 1000

    t0 = time.perf_counter()
    for _ in range(20):
        _ = sill_mat1 @ sill_mat2
    t_sill_matmul = (time.perf_counter() - t0) / 20 * 1000
    print(f"\n2D Matrix Multiplication (128x128 @ 128x128):")
    print(f"  NumPy:            {t_np_matmul:.2f} ms")
    print(f"  Sillstone (Rust): {t_sill_matmul:.2f} ms")


if __name__ == "__main__":
    bench_image()
    bench_array()
