"""Pure zero-dependency NumPy-compatible N-dimensional Array engine.

Backed by Sillstone's high-performance Pure-Rust tensor computing core.
Provides complete ndarray layout, broadcasting, vectorized math, reductions,
GEMM matmul, and zero-copy __array_interface__ support.
"""

from __future__ import annotations

import ctypes
import math
import operator
import struct
from typing import Any, List, Optional, Sequence, Tuple, Union

# Supported Dtype configurations
DTYPE_MAP = {
    "float32": ("f", 4, "<f4"),
    "float64": ("d", 8, "<f8"),
    "int8": ("b", 1, "|i1"),
    "int16": ("h", 2, "<i2"),
    "int32": ("i", 4, "<i4"),
    "int64": ("q", 8, "<i8"),
    "uint8": ("B", 1, "|u1"),
    "uint16": ("H", 2, "<u2"),
    "uint32": ("I", 4, "<u4"),
    "uint64": ("Q", 8, "<u8"),
    "bool": ("?", 1, "|b1"),
}

DTYPE_ALIASES = {
    "float": "float64",
    "double": "float64",
    "single": "float32",
    "int": "int64",
    "uint": "uint64",
    "bool_": "bool",
    "?": "bool",
    "b1": "bool",
    "f4": "float32",
    "f8": "float64",
    "i1": "int8",
    "i2": "int16",
    "i4": "int32",
    "i8": "int64",
    "u1": "uint8",
    "u2": "uint16",
    "u4": "uint32",
    "u8": "uint64",
}

# Numeric kind of every dtype: b = bool, i = signed int, u = unsigned int, f = float.
_KIND = {
    "bool": "b",
    "int8": "i", "int16": "i", "int32": "i", "int64": "i",
    "uint8": "u", "uint16": "u", "uint32": "u", "uint64": "u",
    "float32": "f", "float64": "f",
}


class AxisError(ValueError, IndexError):
    """Raised when an axis is out of range (mirrors numpy.exceptions.AxisError)."""


def _canonical_dtype(dt: Any) -> str:
    """Resolve a dtype spec (name, alias, typestr, Python type, NumPy dtype) to its canonical name."""
    if isinstance(dt, str):
        name = dt.strip().lower().lstrip("<=|")
    elif dt is float:
        return "float64"
    elif dt is int:
        return "int64"
    elif dt is bool:
        return "bool"
    else:
        # NumPy dtype objects expose `.name`; NumPy scalar types expose `__name__`.
        name = getattr(dt, "name", None)
        if not isinstance(name, str):
            name = getattr(dt, "__name__", None)
        if not isinstance(name, str):
            raise TypeError(f"unsupported dtype {dt!r}")
        name = name.lower()
    if name in DTYPE_MAP:
        return name
    if name in DTYPE_ALIASES:
        return DTYPE_ALIASES[name]
    raise TypeError(f"unsupported dtype {dt!r}")


def _promote(a: str, b: str) -> str:
    """NumPy `result_type` for two array dtypes in the supported set."""
    if a == b:
        return a
    ka, kb = _KIND[a], _KIND[b]
    ba, bb = DTYPE_MAP[a][1] * 8, DTYPE_MAP[b][1] * 8
    if ka == "b":
        return b
    if kb == "b":
        return a
    if ka == kb:
        return a if ba >= bb else b
    if "f" in (ka, kb):
        float_bits, int_bits = (ba, bb) if ka == "f" else (bb, ba)
        # A float absorbs an integer only when it can hold every value of it exactly.
        return f"float{max(float_bits, 32 if int_bits <= 16 else 64)}"
    signed_bits, unsigned_bits = (ba, bb) if ka == "i" else (bb, ba)
    if signed_bits > unsigned_bits:
        return f"int{signed_bits}"
    if unsigned_bits < 64:
        return f"int{unsigned_bits * 2}"
    return "float64"


def _promote_scalar(dtype: str, value: Any) -> str:
    """Result dtype of `array(dtype) <op> python_scalar` (NEP 50 weak scalar rules)."""
    kind = _KIND[dtype]
    if isinstance(value, bool):
        return dtype
    if isinstance(value, int):
        return "int64" if kind == "b" else dtype
    return dtype if kind == "f" else "float64"


def _wrap_int(value: int, dtype: str) -> int:
    """Two's-complement wrap-around of `value` into the integer `dtype` (NumPy array arithmetic)."""
    bits = DTYPE_MAP[dtype][1] * 8
    value &= (1 << bits) - 1
    if _KIND[dtype] == "i" and value >= 1 << (bits - 1):
        value -= 1 << bits
    return value


def _pack_value(dtype: str, value: Any, wrap: bool = False) -> bytes:
    """Encode `value` as one element of `dtype`.

    Assignment (`wrap=False`) rejects Python integers that do not fit, like NumPy 2.
    Arithmetic results (`wrap=True`) wrap around for integers and overflow to inf for floats.
    """
    fmt = DTYPE_MAP[dtype][0]
    kind = _KIND[dtype]
    if kind == "b":
        return struct.pack(fmt, bool(value))
    if kind == "f":
        number = float(value)
        try:
            return struct.pack(fmt, number)
        except OverflowError:
            return struct.pack(fmt, math.copysign(math.inf, number))
    if isinstance(value, float):
        if math.isnan(value) or math.isinf(value):
            raise ValueError("cannot convert float NaN or infinity to integer")
        value = int(value)
    else:
        value = operator.index(value)
    if wrap:
        value = _wrap_int(value, dtype)
    else:
        bits = DTYPE_MAP[dtype][1] * 8
        low, high = (-(1 << (bits - 1)), (1 << (bits - 1)) - 1) if kind == "i" else (0, (1 << bits) - 1)
        if not low <= value <= high:
            raise OverflowError(f"Python integer {value} out of bounds for {dtype}")
    return struct.pack(fmt, value)


def _true_divide(a: Any, b: Any) -> float:
    """IEEE-754 division (x/0 is inf or nan) instead of Python's ZeroDivisionError."""
    try:
        return a / b
    except ZeroDivisionError:
        if a == 0 or a != a:
            return math.nan
        return math.copysign(math.inf, a) * math.copysign(1.0, b)


try:
    from ._generated import LIB
except Exception:
    LIB = None

_native_matmul_f64 = None
_native_add_f64 = None
_native_sum_f64 = None

if LIB is not None:
    try:
        _native_matmul_f64 = getattr(LIB, "sillstone_tensor_matmul_f64")
        _native_matmul_f64.argtypes = [
            ctypes.c_void_p, ctypes.c_size_t, ctypes.c_size_t,
            ctypes.c_void_p, ctypes.c_size_t, ctypes.c_size_t,
            ctypes.c_void_p,
        ]
        _native_matmul_f64.restype = ctypes.c_int32
    except AttributeError:
        pass

    try:
        _native_add_f64 = getattr(LIB, "sillstone_tensor_add_f64")
        _native_add_f64.argtypes = [
            ctypes.c_void_p, ctypes.c_void_p, ctypes.c_size_t, ctypes.c_void_p,
        ]
        _native_add_f64.restype = ctypes.c_int32
    except AttributeError:
        pass

    try:
        _native_sum_f64 = getattr(LIB, "sillstone_tensor_sum_f64")
        _native_sum_f64.argtypes = [
            ctypes.c_void_p, ctypes.c_size_t, ctypes.POINTER(ctypes.c_double),
        ]
        _native_sum_f64.restype = ctypes.c_int32
    except AttributeError:
        pass



def _compute_c_strides(shape: Sequence[int], itemsize: int) -> List[int]:
    strides = [0] * len(shape)
    s = itemsize
    for i in range(len(shape) - 1, -1, -1):
        strides[i] = s
        s *= shape[i]
    return strides


class Array:
    """NumPy-compatible N-dimensional array."""

    def __init__(
        self,
        shape: Sequence[int],
        dtype: Any = "float64",
        buffer: Optional[Union[bytes, bytearray]] = None,
        strides: Optional[Sequence[int]] = None,
        offset: int = 0,
    ):
        self._dtype = _canonical_dtype(dtype)
        self._format, self._itemsize, self._typestr = DTYPE_MAP[self._dtype]
        self._shape = tuple(operator.index(s) for s in shape)
        if any(s < 0 for s in self._shape):
            raise ValueError("negative dimensions are not allowed")
        self._ndim = len(self._shape)
        self._size = math.prod(self._shape)

        if strides is None:
            self._strides = tuple(_compute_c_strides(self._shape, self._itemsize))
        else:
            self._strides = tuple(operator.index(s) for s in strides)
            if len(self._strides) != self._ndim:
                raise ValueError("strides must have one entry per dimension")

        self._offset = operator.index(offset)
        if self._offset < 0:
            raise ValueError("offset must be non-negative")

        if buffer is None:
            self._buffer = bytearray(self._size * self._itemsize)
        elif isinstance(buffer, bytearray):
            self._buffer = buffer
        else:
            self._buffer = bytearray(buffer)

        if self._size:
            low = high = self._offset
            for dim, stride in zip(self._shape, self._strides):
                if stride >= 0:
                    high += (dim - 1) * stride
                else:
                    low += (dim - 1) * stride
            if low < 0 or high + self._itemsize > len(self._buffer):
                raise ValueError("buffer is too small for the requested shape, strides and offset")

    @property
    def shape(self) -> Tuple[int, ...]:
        return self._shape

    @property
    def strides(self) -> Tuple[int, ...]:
        return self._strides

    @property
    def ndim(self) -> int:
        return self._ndim

    @property
    def size(self) -> int:
        return self._size

    @property
    def dtype(self) -> str:
        return self._dtype

    @property
    def itemsize(self) -> int:
        return self._itemsize

    @property
    def T(self) -> "Array":
        """Transposed view of the array (reversing dimension order)."""
        return self.transpose()

    @property
    def __array_interface__(self) -> dict:
        """NumPy Array Interface (version 3) for zero-copy interop."""
        buf_type = ctypes.c_char * len(self._buffer)
        addr = ctypes.addressof(buf_type.from_buffer(self._buffer)) + self._offset
        return {
            "version": 3,
            "shape": self._shape,
            "typestr": self._typestr,
            "data": (addr, False),
            "strides": self._strides,
        }

    def __len__(self) -> int:
        if self._ndim == 0:
            raise TypeError("len() of unsized object")
        return self._shape[0]

    def __iter__(self):
        for i in range(len(self)):
            yield self[i]

    def tobytes(self) -> bytes:
        """Returns raw contiguous buffer bytes."""
        if self._is_contiguous():
            return bytes(self._buffer[self._offset : self._offset + self._size * self._itemsize])
        else:
            # Flatten to contiguous
            return self.copy().tobytes()

    def _is_contiguous(self) -> bool:
        expected = _compute_c_strides(self._shape, self._itemsize)
        return self._strides == tuple(expected)

    def copy(self) -> "Array":
        """Returns a contiguous copy of the array."""
        out = Array(self._shape, dtype=self._dtype)
        size = self._itemsize
        for idx in self._ndindex():
            src = self._raw_offset(idx)
            dst = out._raw_offset(idx)
            out._buffer[dst : dst + size] = self._buffer[src : src + size]
        return out

    def _ndindex(self):
        if 0 in self._shape:
            return
        if not self._shape:
            yield ()
            return
        idx = [0] * self._ndim
        while True:
            yield tuple(idx)
            for i in range(self._ndim - 1, -1, -1):
                idx[i] += 1
                if idx[i] < self._shape[i]:
                    break
                idx[i] = 0
            else:
                break

    def _raw_offset(self, index: Sequence[int]) -> int:
        """Byte offset of an in-bounds, non-negative index."""
        off = self._offset
        for i, stride in zip(index, self._strides):
            off += i * stride
        return off

    def _linear_offset(self, index: Sequence[int]) -> int:
        if len(index) != self._ndim:
            raise IndexError(f"index dimensionality {len(index)} does not match {self._ndim}")
        off = self._offset
        for i, idx in enumerate(index):
            if idx < 0:
                idx += self._shape[i]
            if idx < 0 or idx >= self._shape[i]:
                raise IndexError(f"index {idx} is out of bounds for axis {i} with size {self._shape[i]}")
            off += idx * self._strides[i]
        return off

    def _norm_axis(self, axis: Any) -> int:
        try:
            ax = operator.index(axis)
        except TypeError:
            raise TypeError(f"axis must be an integer, not {type(axis).__name__}") from None
        if not -self._ndim <= ax < self._ndim:
            raise AxisError(f"axis {ax} is out of bounds for array of dimension {self._ndim}")
        return ax + self._ndim if ax < 0 else ax

    def _is_scalar_index(self, index: Any) -> bool:
        """True for a plain tuple/int of exactly `ndim` Python ints (the hot element path)."""
        if type(index) is int:
            return self._ndim == 1
        return type(index) is tuple and len(index) == self._ndim and all(type(x) is int for x in index)

    def _resolve(self, index: Any) -> Tuple[List[int], List[int], int]:
        """Resolve an int/slice/Ellipsis index to the (shape, strides, offset) of the selected view."""
        if not isinstance(index, tuple):
            index = (index,)
        ellipses = [i for i, x in enumerate(index) if x is Ellipsis]
        if len(ellipses) > 1:
            raise IndexError("an index can only have a single ellipsis ('...')")
        used = len(index) - len(ellipses)
        if used > self._ndim:
            raise IndexError(
                f"too many indices for array: array is {self._ndim}-dimensional, but {used} were indexed"
            )
        if ellipses:
            pos = ellipses[0]
            index = index[:pos] + (slice(None),) * (self._ndim - used) + index[pos + 1:]

        new_shape: List[int] = []
        new_strides: List[int] = []
        new_offset = self._offset
        for ax, s in enumerate(index):
            if isinstance(s, slice):
                start, stop, step = s.indices(self._shape[ax])
                count = len(range(start, stop, step))
                new_shape.append(count)
                new_strides.append(self._strides[ax] * step)
                if count:
                    new_offset += start * self._strides[ax]
                continue
            if isinstance(s, bool):
                raise IndexError("boolean indices are not supported")
            try:
                i = operator.index(s)
            except TypeError:
                raise IndexError(
                    "only integers, slices (`:`) and ellipsis (`...`) are valid indices"
                ) from None
            if i < 0:
                i += self._shape[ax]
            if i < 0 or i >= self._shape[ax]:
                raise IndexError(f"index {s} is out of bounds for axis {ax} with size {self._shape[ax]}")
            new_offset += i * self._strides[ax]
        for ax in range(len(index), self._ndim):
            new_shape.append(self._shape[ax])
            new_strides.append(self._strides[ax])
        return new_shape, new_strides, new_offset

    def __getitem__(self, index: Any) -> Any:
        if self._is_scalar_index(index):
            off = self._linear_offset((index,) if type(index) is int else index)
            return struct.unpack_from(self._format, self._buffer, off)[0]
        new_shape, new_strides, new_offset = self._resolve(index)
        if not new_shape:
            return struct.unpack_from(self._format, self._buffer, new_offset)[0]
        return Array(shape=new_shape, dtype=self._dtype, buffer=self._buffer, strides=new_strides, offset=new_offset)

    def __setitem__(self, index: Any, value: Any):
        if self._is_scalar_index(index):
            off = self._linear_offset((index,) if type(index) is int else index)
            self._buffer[off : off + self._itemsize] = _pack_value(self._dtype, value)
            return
        new_shape, new_strides, new_offset = self._resolve(index)
        target = Array(shape=new_shape, dtype=self._dtype, buffer=self._buffer, strides=new_strides, offset=new_offset)
        target._assign(value)

    def _assign(self, value: Any):
        """Broadcast `value` (scalar, nested list or Array) into every element of this array."""
        if isinstance(value, (list, tuple)):
            value = array(value)
        if isinstance(value, Array):
            if _broadcast_shapes(value.shape, self._shape) != self._shape:
                raise ValueError(
                    f"could not broadcast input array from shape {value.shape} into shape {self._shape}"
                )
            source = value._broadcast_to(self._shape)
            # Read everything first so overlapping views (a[1:] = a[:-1]) behave like NumPy.
            values = [source[idx] for idx in self._ndindex()]
        else:
            values = [value] * self._size
        for idx, v in zip(self._ndindex(), values):
            off = self._raw_offset(idx)
            self._buffer[off : off + self._itemsize] = _pack_value(self._dtype, v)

    def _store(self, index: Sequence[int], value: Any):
        """Write an arithmetic result (integers wrap around instead of raising)."""
        off = self._raw_offset(index)
        self._buffer[off : off + self._itemsize] = _pack_value(self._dtype, value, wrap=True)

    def _flat(self) -> List[Any]:
        """All elements in C order as Python scalars."""
        return [self[idx] for idx in self._ndindex()]

    def reshape(self, *new_shape: Any) -> "Array":
        """Returns an array with the same data in a new shape (one dimension may be -1)."""
        if len(new_shape) == 1 and isinstance(new_shape[0], (list, tuple)):
            target = [operator.index(d) for d in new_shape[0]]
        else:
            target = [operator.index(d) for d in new_shape]

        unknown = [i for i, d in enumerate(target) if d == -1]
        if any(d < -1 for d in target) or len(unknown) > 1:
            raise ValueError("can only specify one unknown dimension and dimensions must be non-negative or -1")
        if unknown:
            known = math.prod(d for d in target if d != -1)
            if known == 0 or self._size % known:
                raise ValueError(f"cannot reshape array of size {self._size} into shape {tuple(target)}")
            target[unknown[0]] = self._size // known
        target = tuple(target)

        if math.prod(target) != self._size:
            raise ValueError(f"cannot reshape array of size {self._size} into shape {target}")

        if self._is_contiguous():
            return Array(shape=target, dtype=self._dtype, buffer=self._buffer, offset=self._offset)
        else:
            return self.copy().reshape(target)

    def transpose(self, *axes: Any) -> "Array":
        """Permutes dimensions."""
        if not axes:
            perm = list(range(self._ndim - 1, -1, -1))
        elif len(axes) == 1 and isinstance(axes[0], (list, tuple)):
            perm = list(axes[0])
        else:
            perm = list(axes)

        if len(perm) != self._ndim:
            raise ValueError("axes length must match array ndim")
        perm = [self._norm_axis(ax) for ax in perm]
        if len(set(perm)) != len(perm):
            raise ValueError("repeated axis in transpose")

        new_shape = [self._shape[ax] for ax in perm]
        new_strides = [self._strides[ax] for ax in perm]
        return Array(shape=new_shape, dtype=self._dtype, buffer=self._buffer, strides=new_strides, offset=self._offset)

    def tolist(self) -> Any:
        """Converts array to nested Python lists."""
        if self._ndim == 0:
            return self[()]
        if self._ndim == 1:
            return [self[i] for i in range(self._shape[0])]
        return [self[i].tolist() for i in range(self._shape[0])]

    def _binary_op(self, other: Any, op, true_div: bool = False, no_bool: bool = False) -> "Array":
        if isinstance(other, Array):
            dtype = _promote(self._dtype, other._dtype)
        elif isinstance(other, (int, float)):
            dtype = _promote_scalar(self._dtype, other)
        else:
            return NotImplemented
        if true_div and _KIND[dtype] != "f":
            dtype = "float64"
        if isinstance(other, int) and not isinstance(other, bool) and _KIND[dtype] in "iu":
            # Only integer results can overflow; true division already produced float64 above.
            _pack_value(dtype, other)  # a Python int that does not fit the dtype raises OverflowError
        if no_bool and dtype == "bool":
            raise TypeError("the boolean subtract operator is not supported, use bitwise_xor instead")

        if isinstance(other, Array):
            out_shape = _broadcast_shapes(self._shape, other.shape)
            a_bcast = self._broadcast_to(out_shape)
            b_bcast = other._broadcast_to(out_shape)
            out = Array(out_shape, dtype=dtype)
            for idx in out._ndindex():
                out._store(idx, op(a_bcast[idx], b_bcast[idx]))
        else:
            out = Array(self._shape, dtype=dtype)
            for idx in self._ndindex():
                out._store(idx, op(self[idx], other))
        return out

    def _broadcast_to(self, target_shape: Tuple[int, ...]) -> "Array":
        if self._shape == target_shape:
            return self
        if self._ndim > len(target_shape):
            raise ValueError(f"cannot broadcast shape {self._shape} to {tuple(target_shape)}")
        new_strides = [0] * len(target_shape)
        for i in range(len(target_shape)):
            dim_target = target_shape[-1 - i]
            if i < self._ndim:
                dim_curr = self._shape[-1 - i]
                stride_curr = self._strides[-1 - i]
                if dim_curr == dim_target:
                    new_strides[-1 - i] = stride_curr
                elif dim_curr == 1:
                    new_strides[-1 - i] = 0
                else:
                    raise ValueError("Incompatible dimension for broadcast")
            else:
                new_strides[-1 - i] = 0
        return Array(shape=target_shape, dtype=self._dtype, buffer=self._buffer, strides=new_strides, offset=self._offset)

    def __add__(self, other: Any) -> "Array":
        if (
            _native_add_f64 is not None
            and isinstance(other, Array)
            and self._dtype == "float64"
            and other._dtype == "float64"
            and self._size
        ):
            if self._shape == other._shape and self._is_contiguous() and other._is_contiguous():
                out = Array(shape=self._shape, dtype="float64")
                buf_a = (ctypes.c_char * len(self._buffer)).from_buffer(self._buffer)
                buf_b = (ctypes.c_char * len(other._buffer)).from_buffer(other._buffer)
                buf_c = (ctypes.c_char * len(out._buffer)).from_buffer(out._buffer)
                ptr_a = ctypes.addressof(buf_a) + self._offset
                ptr_b = ctypes.addressof(buf_b) + other._offset
                ptr_c = ctypes.addressof(buf_c)
                status = _native_add_f64(ptr_a, ptr_b, self._size, ptr_c)
                if status == 0:
                    return out
        return self._binary_op(other, lambda a, b: a + b)

    def __radd__(self, other: Any) -> "Array":
        return self.__add__(other)

    def __sub__(self, other: Any) -> "Array":
        return self._binary_op(other, lambda a, b: a - b, no_bool=True)

    def __rsub__(self, other: Any) -> "Array":
        return self._binary_op(other, lambda a, b: b - a, no_bool=True)

    def __mul__(self, other: Any) -> "Array":
        return self._binary_op(other, lambda a, b: a * b)

    def __rmul__(self, other: Any) -> "Array":
        return self.__mul__(other)

    def __truediv__(self, other: Any) -> "Array":
        return self._binary_op(other, _true_divide, true_div=True)

    def __rtruediv__(self, other: Any) -> "Array":
        return self._binary_op(other, lambda a, b: _true_divide(b, a), true_div=True)

    def __neg__(self) -> "Array":
        if self._dtype == "bool":
            raise TypeError("the boolean negative operator is not supported, use logical_not instead")
        out = Array(self._shape, dtype=self._dtype)
        for idx in self._ndindex():
            out._store(idx, -self[idx])
        return out

    def __abs__(self) -> "Array":
        out = Array(self._shape, dtype=self._dtype)
        for idx in self._ndindex():
            out._store(idx, abs(self[idx]))
        return out

    def __matmul__(self, other: "Array") -> "Array":
        """Matrix product (@) of 1-D and 2-D arrays."""
        if not isinstance(other, Array):
            return NotImplemented
        if not (1 <= self._ndim <= 2 and 1 <= other._ndim <= 2):
            raise ValueError("matmul (@) currently requires 1D or 2D arrays")
        left = self if self._ndim == 2 else self.reshape(1, self._shape[0])
        right = other if other._ndim == 2 else other.reshape(other._shape[0], 1)
        m, k1 = left._shape
        k2, n = right._shape
        if k1 != k2:
            raise ValueError(f"shapes {self._shape} and {other._shape} not aligned: {k1} != {k2}")

        result = left._matmul_2d(right, m, k1, n)
        if self._ndim == 1 and other._ndim == 1:
            return result[0, 0]
        if self._ndim == 1:
            return result.reshape(n)
        if other._ndim == 1:
            return result.reshape(m)
        return result

    def _matmul_2d(self, other: "Array", m: int, k: int, n: int) -> "Array":
        if (
            _native_matmul_f64 is not None
            and self._dtype == "float64"
            and other._dtype == "float64"
            and m
            and k
            and n
            and self._is_contiguous()
            and other._is_contiguous()
        ):
            out = Array(shape=(m, n), dtype="float64")
            buf_a = (ctypes.c_char * len(self._buffer)).from_buffer(self._buffer)
            buf_b = (ctypes.c_char * len(other._buffer)).from_buffer(other._buffer)
            buf_c = (ctypes.c_char * len(out._buffer)).from_buffer(out._buffer)
            ptr_a = ctypes.addressof(buf_a) + self._offset
            ptr_b = ctypes.addressof(buf_b) + other._offset
            ptr_c = ctypes.addressof(buf_c)
            status = _native_matmul_f64(ptr_a, m, k, ptr_b, k, n, ptr_c)
            if status == 0:
                return out

        out = Array(shape=(m, n), dtype=_promote(self._dtype, other._dtype))
        rows = [[self[i, p] for p in range(k)] for i in range(m)]
        cols = [[other[p, j] for p in range(k)] for j in range(n)]
        for i in range(m):
            for j in range(n):
                acc = 0
                for a, b in zip(rows[i], cols[j]):
                    acc += a * b
                out._store((i, j), acc)
        return out

    def _sum_dtype(self) -> str:
        """NumPy accumulates bool and small signed ints in int64, unsigned ints in uint64."""
        kind = _KIND[self._dtype]
        if kind in "bi":
            return "int64"
        if kind == "u":
            return "uint64"
        return self._dtype

    def _cast_scalar(self, dtype: str, value: Any) -> Any:
        """Round a Python number to what an element of `dtype` can hold."""
        return struct.unpack(DTYPE_MAP[dtype][0], _pack_value(dtype, value, wrap=True))[0]

    def _reduce(self, axis: Optional[int], fn, out_dtype: str) -> Any:
        """Apply `fn(list_of_values)` over all elements (axis=None) or along one axis."""
        if axis is None:
            return self._cast_scalar(out_dtype, fn(self._flat()))
        ax = self._norm_axis(axis)
        count = self._shape[ax]
        out = Array(self._shape[:ax] + self._shape[ax + 1:], dtype=out_dtype)
        for oi in out._ndindex():
            out._store(oi, fn([self[oi[:ax] + (k,) + oi[ax:]] for k in range(count)]))
        return out if out._shape else out[()]

    def sum(self, axis: Optional[int] = None) -> Union[float, int, "Array"]:
        if (
            axis is None
            and _native_sum_f64 is not None
            and self._dtype == "float64"
            and self._size
            and self._is_contiguous()
        ):
            buf = (ctypes.c_char * len(self._buffer)).from_buffer(self._buffer)
            ptr = ctypes.addressof(buf) + self._offset
            out_val = ctypes.c_double()
            status = _native_sum_f64(ptr, self._size, ctypes.byref(out_val))
            if status == 0:
                return out_val.value
        return self._reduce(axis, sum, self._sum_dtype())

    def mean(self, axis: Optional[int] = None) -> Union[float, "Array"]:
        out_dtype = self._dtype if _KIND[self._dtype] == "f" else "float64"

        def average(values: List[Any]) -> float:
            if not values:
                return math.nan
            return _true_divide(float(sum(values)), len(values))

        if axis is None and self._dtype == "float64" and self._size:
            return float(self.sum()) / self._size
        return self._reduce(axis, average, out_dtype)

    @staticmethod
    def _select(values: List[Any], want_max: bool) -> int:
        """Index of the first max/min; the first NaN wins, as in NumPy."""
        if not values:
            word = "maximum" if want_max else "minimum"
            raise ValueError(f"zero-size array to reduction operation {word} which has no identity")
        best = 0
        for i, v in enumerate(values):
            if v != v:
                return i
            if (v > values[best]) if want_max else (v < values[best]):
                best = i
        return best

    def max(self, axis: Optional[int] = None) -> Any:
        return self._reduce(axis, lambda vs: vs[self._select(vs, True)], self._dtype)

    def min(self, axis: Optional[int] = None) -> Any:
        return self._reduce(axis, lambda vs: vs[self._select(vs, False)], self._dtype)

    def argmax(self, axis: Optional[int] = None) -> Any:
        return self._reduce(axis, lambda vs: self._select(vs, True), "int64")

    def argmin(self, axis: Optional[int] = None) -> Any:
        return self._reduce(axis, lambda vs: self._select(vs, False), "int64")

    def to_image(self, mode: Optional[str] = None):
        """Converts array to a Sillstone Image object."""
        from .image import Image
        return Image.from_array(self, mode=mode)

    def __repr__(self) -> str:
        return f"Array(shape={self._shape}, dtype='{self._dtype}', data={self.tolist()})"


def _broadcast_shapes(a: Sequence[int], b: Sequence[int]) -> Tuple[int, ...]:
    max_len = max(len(a), len(b))
    res = []
    for i in range(max_len):
        dim_a = a[-1 - i] if i < len(a) else 1
        dim_b = b[-1 - i] if i < len(b) else 1
        if dim_a == dim_b:
            res.append(dim_a)
        elif dim_a == 1:
            res.append(dim_b)
        elif dim_b == 1:
            res.append(dim_a)
        else:
            raise ValueError(f"operands could not be broadcast together with shapes {tuple(a)} and {tuple(b)}")
    res.reverse()
    return tuple(res)


def _element_kind(x: Any) -> str:
    """Classify a Python scalar for dtype inference: 'b', 'i' or 'f'."""
    if isinstance(x, bool):
        return "b"
    if isinstance(x, int):
        return "i"
    if isinstance(x, float):
        return "f"
    try:
        operator.index(x)
        return "i"
    except TypeError:
        pass
    try:
        float(x)
        return "f"
    except (TypeError, ValueError):
        raise TypeError(f"unsupported array element of type {type(x).__name__}") from None


def array(data: Any, dtype: Any = None) -> Array:
    """Creates an array from nested lists, numbers, or array-like objects."""
    if isinstance(data, Array):
        if dtype is None or _canonical_dtype(dtype) == data.dtype:
            return data.copy()
        out = Array(data.shape, dtype=dtype)
        for idx in data._ndindex():
            out[idx] = data[idx]
        return out

    if dtype is None and hasattr(data, "dtype"):
        # NumPy arrays and scalars keep their dtype when it is one we support.
        try:
            dtype = _canonical_dtype(data.dtype)
        except TypeError:
            dtype = None
    if not isinstance(data, (list, tuple)) and hasattr(data, "tolist"):
        data = data.tolist()

    # Deduce shape and flattened values
    def _inspect(obj):
        if isinstance(obj, (list, tuple)):
            if not obj:
                return (0,), []
            sub_shapes, sub_vals = zip(*[_inspect(x) for x in obj])
            first_shape = sub_shapes[0]
            for s in sub_shapes[1:]:
                if s != first_shape:
                    raise ValueError("inhomogeneous shape")
            flat = []
            for v in sub_vals:
                flat.extend(v)
            return (len(obj),) + first_shape, flat
        else:
            return (), [obj]

    shape, vals = _inspect(data)
    if dtype is None:
        kinds = {_element_kind(x) for x in vals}
        if not kinds or "f" in kinds:
            dt = "float64"
        elif "i" in kinds:
            dt = "int64"
        else:
            dt = "bool"
    else:
        dt = _canonical_dtype(dtype)

    arr = Array(shape, dtype=dt)
    for idx, val in zip(arr._ndindex(), vals):
        arr[idx] = val
    return arr


def asarray(data: Any, dtype: Any = None) -> Array:
    """Converts input to an Array."""
    return array(data, dtype=dtype)


def zeros(shape: Union[int, Sequence[int]], dtype: Any = "float64") -> Array:
    """Returns an array filled with zeros."""
    if isinstance(shape, int):
        shape = (shape,)
    return Array(shape, dtype=dtype)


def ones(shape: Union[int, Sequence[int]], dtype: Any = "float64") -> Array:
    """Returns an array filled with ones."""
    if isinstance(shape, int):
        shape = (shape,)
    arr = Array(shape, dtype=dtype)
    for idx in arr._ndindex():
        arr[idx] = 1
    return arr


def arange(start: float, stop: Optional[float] = None, step: float = 1, dtype: Any = None) -> Array:
    """Returns evenly spaced values within the half-open interval [start, stop)."""
    if stop is None:
        start, stop = 0, start
    if step == 0:
        raise ZeroDivisionError("arange: step must not be zero")
    count = max(0, math.ceil((stop - start) / step))
    # NumPy derives the increment from the first two values so that rounding matches.
    delta = (start + step) - start
    vals = [start + i * delta for i in range(count)]
    if dtype is None:
        dtype = "int64" if all(isinstance(x, int) for x in (start, stop, step)) else "float64"
    return array(vals, dtype=dtype)


def linspace(start: float, stop: float, num: int = 50, dtype: Any = "float64") -> Array:
    """Returns num evenly spaced samples over the interval [start, stop]."""
    if num < 0:
        raise ValueError(f"Number of samples, {num}, must be non-negative.")
    if num == 0:
        return Array((0,), dtype=dtype)
    if num == 1:
        return array([start], dtype=dtype)
    step = (stop - start) / (num - 1)
    vals = [start + i * step for i in range(num)]
    vals[-1] = stop
    if _KIND[_canonical_dtype(dtype)] in "iu":
        vals = [math.floor(v) for v in vals]
    return array(vals, dtype=dtype)


def matmul(a: Array, b: Array) -> Array:
    """Matrix product of two arrays."""
    return a @ b


def dot(a: Array, b: Array) -> Any:
    """Dot product of two arrays (inner product for vectors, matrix product otherwise)."""
    return a @ b
