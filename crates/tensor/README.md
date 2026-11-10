# sillstone-tensor

Pure-Rust, zero-dependency implementation of N-dimensional strided arrays and tensor computing (**NumPy replacement**).

## Features

- Zero third-party dependencies (`core` + `alloc` in core operations).
- Complete N-dimensional strided layout (`shape`, `strides`, `offset`).
- Arbitrary dimension reshaping, transposing, and strided slicing without memory copying.
- Full NumPy-compatible broadcasting semantics.
- Vectorized elementwise arithmetic (`add`, `sub`, `mul`, `div`, `neg`) and comparisons (`eq`, `ne`); `unary_op`/`binary_op` take any closure.
- Fast reduction operations along arbitrary axes (`sum`, `min`, `max`, `argmin`, `argmax` for every dtype; `mean`, `var` and `std` for the four float widths). NaN propagates through `min`/`max`/`argmin`/`argmax` wherever it sits.
- Elementwise elementary functions on `f32`/`f64`/`float16`/`bfloat16`: `sqrt`, `exp`, `ln`, `log2`, `log10`, `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `pow`/`powf` and `atan2`, with broadcasting. Results keep the element type — there is no implicit promotion, so integer input has to `astype` first.
- Linear algebra: cache-tiled GEMM (`matmul`), `dot` for the four vector/matrix shape combinations, `norm`/`norm_p`, `lu`, `det`, `inv` and `solve` (vector and matrix right-hand sides).
- Shape manipulation: `concat`, `stack` and `pad` (constant mode), all of which accept strided views.
- `astype` across every scalar pair the crate holds, including `float16`/`bfloat16` with round-to-nearest-even, saturation at the integer bounds and IEEE overflow to infinity.
- Payload ownership: `from_vec` takes a `Vec` without copying and `into_data` hands it back when the tensor is the sole owner of a contiguous buffer; `from_arc` shares a buffer that is already behind an `Arc`.
- High-performance matrix multiplication (GEMM) with cache-tiling.

## NumPy parity

`tests/oracle/generate.py` renders the expected values with NumPy into
`tests/oracle/vectors.json`, and `tests/numpy_oracle.rs` replays them through
this crate. Elementwise results are within a couple of
`EPSILON * max(|expected|, 1)` units, exact where the contract is exact
(`astype`, `concat`/`stack`/`pad`, half-precision bit patterns), and the
recorded worst case per operation is printed by

```console
cargo test -p sillstone-tensor --test numpy_oracle -- --nocapture
```

Known deviations, all deliberate: an empty reduction is an `Err` rather than a
`NaN` plus a `RuntimeWarning`; float-to-integer casts saturate (Rust `as`
semantics) instead of being undefined; `norm_p` on a matrix is the elementwise
p-norm of the flattened buffer, not the induced operator norm.
