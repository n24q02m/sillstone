# NumPy Parity & N-Dimensional Array Architecture

`sillstone` provides a pure-Rust, zero-dependency implementation of N-dimensional strided arrays and tensor computing (`crates/tensor`), functioning as a high-performance alternative to NumPy.

## Architecture

1. **Zero Third-Party Dependencies**: Pure Rust (`core` + `alloc` only in kernel compute paths).
2. **Strided Memory Layout**: Complete support for `shape`, `strides`, and `offset`. Views (such as slicing and transposing) do not allocate or copy data buffers.
3. **NumPy-Compatible Broadcasting**: Multi-dimensional broadcasting rules following standard NumPy semantics (trailing dimensions aligned, dimensions equal or 1, stride-0 dimension expansion).
4. **Vectorized Math & Axis Reductions**: Vectorized elementwise arithmetic (`add`, `sub`, `mul`, `div`, `neg`, `abs`, `eq`, `ne`) and reductions (`sum`, `mean`, `max`, `min`, `argmax`, `argmin`) across arbitrary axes.
5. **High-Performance GEMM Matmul**: Cache-blocked matrix multiplication with 32x32 L1-friendly cache tiling.
6. **Zero-Copy Image-Tensor Bridge**: Bidirectional conversion between `Image` and `Tensor`/`Array` without buffer reallocation.
7. **Python Buffer Protocol**: Implements `__array_interface__` (v3) allowing direct, zero-copy consumption by external scientific tools when desired.

## Multi-Language SDK Availability

| Feature | Python (`sillstone.Array`) | Node.js (`sillstone.tensor`) | Go (`sillstone.Tensor`) | Java (`io.github.sillstone.Tensor`) |
| :--- | :---: | :---: | :---: | :---: |
| N-d Array Layout | ✓ | ✓ | ✓ | ✓ |
| Reshape & Transpose | ✓ | ✓ | ✓ | ✓ |
| GEMM Matmul (`@`) | ✓ | ✓ | ✓ | ✓ |
| Axis Reductions | ✓ | ✓ | ✓ | ✓ |
| Image Zero-Copy Bridge | ✓ | ✓ | ✓ | ✓ |
| `__array_interface__` | ✓ | N/A | N/A | N/A |
