# sillstone-tensor

Pure-Rust, zero-dependency implementation of N-dimensional strided arrays and tensor computing (**NumPy replacement**).

## Features

- Zero third-party dependencies (`core` + `alloc` in core operations).
- Complete N-dimensional strided layout (`shape`, `strides`, `offset`).
- Arbitrary dimension reshaping, transposing, and strided slicing without memory copying.
- Full NumPy-compatible broadcasting semantics.
- Vectorized elementwise arithmetic (`add`, `sub`, `mul`, `div`, `neg`) and comparisons (`eq`, `ne`); `unary_op`/`binary_op` take any closure.
- Fast reduction operations along arbitrary axes (`sum`, `min`, `max`, `argmin`, `argmax` for every dtype; `mean` for `f32`/`f64`). NaN propagates through `min`/`max`/`argmin`/`argmax` wherever it sits. `pow`, `abs` and `std` live in the language SDKs, not in this `no_std` kernel.
- High-performance matrix multiplication (GEMM) with cache-tiling.
