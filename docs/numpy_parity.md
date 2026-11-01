# NumPy Parity & N-Dimensional Array Architecture

`sillstone` provides a pure-Rust, zero-dependency implementation of N-dimensional strided arrays and tensor computing (`crates/tensor`), functioning as a high-performance alternative to NumPy.

## Architecture

1. **Zero Third-Party Dependencies**: Pure Rust (`core` + `alloc` only in kernel compute paths).
2. **Strided Memory Layout**: Complete support for `shape`, `strides`, and `offset`. Views (such as slicing and transposing) do not allocate or copy data buffers.
3. **NumPy-Compatible Broadcasting**: Multi-dimensional broadcasting rules following standard NumPy semantics (trailing dimensions aligned, dimensions equal or 1, stride-0 dimension expansion).
4. **Vectorized Math & Axis Reductions**: Vectorized elementwise arithmetic (`add`, `sub`, `mul`, `div`, `neg`, `abs`, `eq`, `ne`) and reductions (`sum`, `mean`, `max`, `min`, `argmax`, `argmin`) across arbitrary axes. The Rust kernel provides `add`/`sub`/`mul`/`div`/`neg`/`eq`/`ne` and `sum`/`min`/`max`/`argmin`/`argmax` (plus `mean` for floats); `abs`, `pow` and `std` are implemented by the language SDKs. NaN propagates through `min`/`max`/`argmin`/`argmax`.
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

## Python SDK (`sillstone.Array`) behaviour and limits

The Python `Array` is a pure-Python strided engine (with the native kernel used for contiguous `float64` add and matmul when the shared library is present). It follows NumPy 2 semantics for the surface it implements:

- **dtypes**: `bool`, `int8`-`int64`, `uint8`-`uint64`, `float32`, `float64`, plus the usual aliases (`i4`, `<f8`, `bool_`, `float`, `int`, NumPy dtype objects). Any other dtype (`float16`, `complex*`, `datetime64`, `object`, ...) raises `TypeError`; it is never silently converted.
- **Promotion**: array-array results use NumPy's `result_type` table; Python scalars are weak (NEP 50), so `uint8_array + 1` stays `uint8` and an out-of-range scalar raises `OverflowError`.
- **Integer arithmetic wraps** (two's complement) like NumPy; `float32` results are rounded to `float32` and overflow to infinity; `x / 0` follows IEEE (`inf`/`nan`), never `ZeroDivisionError`. Boolean `-` and unary `-` raise `TypeError`.
- **Indexing**: integers (negative allowed), slices with any step, `...` and tuples of those, bounds-checked (`IndexError`). Basic indexing returns views; assignment broadcasts and reads the source before writing, so overlapping assignment is safe. Assigning floats to integer arrays truncates; assigning an out-of-range integer raises `OverflowError`.
- **Shape ops**: `reshape` accepts one `-1`; `transpose` validates axes (`ValueError` for repeats, `AxisError` for out-of-range).
- **Reductions**: `sum`, `mean`, `max`, `min`, `argmax`, `argmin` over `axis=None`, an int, or a negative int. `sum` of bool/int widens to `int64` (`uint64` for unsigned); `mean` of ints is `float64`; NaN propagates through `max`/`min`/`argmax`/`argmin`; an out-of-range axis raises `AxisError`; `max`/`min`/`argmax`/`argmin` of an empty array raise `ValueError`.
- **Matmul / `dot`**: 1-D and 2-D operands (`1-D @ 1-D` returns a scalar) with dtype promotion.
- **Creation**: `array`, `zeros`, `ones`, `arange` (count `ceil((stop - start) / step)`), `linspace` (last element is exactly `stop`).

Not implemented in the Python `Array`: elementwise comparison operators (`<`, `==`, ...), batched (N-d, N > 2) matmul, fancy / boolean-mask / `None` indexing, `float16` and complex dtypes.

### Known deviations from NumPy (intentional, not planned to change)

Python `Array`:

- `a[..., i]` that selects a single element returns a Python scalar; NumPy returns a 0-d array.
- Float `sum` / `mean` add in sequential order, NumPy adds pairwise, so the last digits can differ.
- Mixed integer/float arithmetic where an operand exceeds `2**53` in magnitude: Sillstone computes the exact integer result and converts to float afterwards, NumPy converts the operands to float first (for example `uint64 - int64` can give `1.0` here and `0.0` in NumPy).
- A float32 array times a Python float that overflows float32 (for example `1e300`): zero elements stay `0.0` here, NumPy converts the scalar to `inf` first and returns `nan`.

Python `Image`:

- `Image.from_array` takes `uint8` arrays only and raises `TypeError` for any other dtype; Pillow maps `uint16`, `int32` and `float32` arrays to the `I;16`, `I` and `F` modes, which Sillstone does not have. Two-channel arrays (`LA`) are refused.
- The array exposed through `__array_interface__` is read-only (the pixels are immutable `bytes`); change pixels with `putpixel`.

Rust `Tensor`:

- `mean` of an empty tensor returns an error; NumPy returns `nan` with a warning.
- Float `sum` is sequential; NumPy uses pairwise summation, so the last digits can differ.
