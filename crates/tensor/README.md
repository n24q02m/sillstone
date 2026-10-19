# sillstone-tensor

Pure-Rust, zero-dependency implementation of N-dimensional strided arrays and tensor computing (**NumPy replacement**).

## Features

- Zero third-party dependencies (`core` + `alloc` in core operations).
- Complete N-dimensional strided layout (`shape`, `strides`, `offset`).
- Arbitrary dimension reshaping, transposing, and strided slicing without memory copying.
- Full NumPy-compatible broadcasting semantics.
- Vectorized elementwise arithmetic (`add`, `sub`, `mul`, `div`, `pow`, `neg`, `abs`).
- Fast reduction operations along arbitrary axes (`sum`, `mean`, `std`, `min`, `max`, `argmin`, `argmax`).
- High-performance matrix multiplication (GEMM) with cache-tiling.
