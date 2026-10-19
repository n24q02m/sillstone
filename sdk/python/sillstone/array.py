"""Pure zero-dependency NumPy-compatible N-dimensional Array engine.

Backed by Sillstone's high-performance Pure-Rust tensor computing core.
Provides complete ndarray layout, broadcasting, vectorized math, reductions,
GEMM matmul, and zero-copy __array_interface__ support.
"""

from __future__ import annotations

import ctypes
import math
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
    "int": "int64",
    "f4": "float32",
    "f8": "float64",
    "i4": "int32",
    "i8": "int64",
    "u1": "uint8",
    "u4": "uint32",
}


def _canonical_dtype(dt: Any) -> str:
    if isinstance(dt, str):
        name = dt.lower()
        if name in DTYPE_MAP:
            return name
        if name in DTYPE_ALIASES:
            return DTYPE_ALIASES[name]
    elif dt is float:
        return "float64"
    elif dt is int:
        return "int64"
    elif dt is bool:
        return "bool"
    return "float64"
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
        self._shape = tuple(int(s) for s in shape)
        self._ndim = len(self._shape)
        self._size = math.prod(self._shape) if self._shape else 1

        if strides is None:
            self._strides = tuple(_compute_c_strides(self._shape, self._itemsize))
        else:
            self._strides = tuple(int(s) for s in strides)

        self._offset = offset

        total_bytes = self._size * self._itemsize
        if buffer is None:
            self._buffer = bytearray(total_bytes)
        else:
            if isinstance(buffer, bytearray):
                self._buffer = buffer
            else:
                self._buffer = bytearray(buffer)
            if len(self._buffer) < total_bytes:
                self._buffer.extend(b"\x00" * (total_bytes - len(self._buffer)))

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
        # copy values element by element
        for idx in self._ndindex():
            out[idx] = self[idx]
        return out

    def _ndindex(self):
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

    def __getitem__(self, index: Any) -> Any:
        if isinstance(index, int):
            index = (index,)
        if isinstance(index, (list, tuple)) and len(index) == self._ndim and all(isinstance(x, int) for x in index):
            off = self._linear_offset(index)
            raw = self._buffer[off : off + self._itemsize]
            val = struct.unpack(self._format, raw)[0]
            return val
        # Slicing
        if isinstance(index, (slice, int, tuple)):
            if not isinstance(index, tuple):
                index = (index,)
            new_shape = []
            new_strides = []
            new_offset = self._offset
            for ax, s in enumerate(index):
                if isinstance(s, int):
                    if s < 0:
                        s += self._shape[ax]
                    new_offset += s * self._strides[ax]
                elif isinstance(s, slice):
                    start, stop, step = s.indices(self._shape[ax])
                    dim_len = max(0, (stop - start + (step - (1 if step > 0 else -1))) // step)
                    new_shape.append(dim_len)
                    new_strides.append(self._strides[ax] * step)
                    new_offset += start * self._strides[ax]
            # append remaining axes
            for ax in range(len(index), self._ndim):
                new_shape.append(self._shape[ax])
                new_strides.append(self._strides[ax])
            if not new_shape:
                raw = self._buffer[new_offset : new_offset + self._itemsize]
                return struct.unpack(self._format, raw)[0]
            return Array(shape=new_shape, dtype=self._dtype, buffer=self._buffer, strides=new_strides, offset=new_offset)
        raise IndexError("Unsupported indexing")

    def __setitem__(self, index: Any, value: Any):
        if isinstance(index, int):
            index = (index,)
        if isinstance(index, (list, tuple)) and len(index) == self._ndim and all(isinstance(x, int) for x in index):
            off = self._linear_offset(index)
            packed = struct.pack(self._format, value)
            self._buffer[off : off + self._itemsize] = packed
        else:
            raise IndexError("Unsupported assignment indexing")

    def reshape(self, *new_shape: Any) -> "Array":
        """Returns an array with the same data in a new shape."""
        if len(new_shape) == 1 and isinstance(new_shape[0], (list, tuple)):
            target = tuple(new_shape[0])
        else:
            target = tuple(new_shape)

        target_size = math.prod(target)
        if target_size != self._size:
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

    def _binary_op(self, other: Any, op) -> "Array":
        if isinstance(other, (int, float, bool)):
            out = Array(self._shape, dtype=self._dtype)
            for idx in self._ndindex():
                out[idx] = op(self[idx], other)
            return out
        elif isinstance(other, Array):
            # Compute broadcast shape
            out_shape = _broadcast_shapes(self._shape, other.shape)
            a_bcast = self._broadcast_to(out_shape)
            b_bcast = other._broadcast_to(out_shape)
            out = Array(out_shape, dtype=self._dtype)
            for idx in out._ndindex():
                out[idx] = op(a_bcast[idx], b_bcast[idx])
            return out
        return NotImplemented

    def _broadcast_to(self, target_shape: Tuple[int, ...]) -> "Array":
        if self._shape == target_shape:
            return self
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
        if _native_add_f64 is not None and isinstance(other, Array) and self._dtype == "float64" and other._dtype == "float64":
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
        return self._binary_op(other, lambda a, b: a - b)

    def __rsub__(self, other: Any) -> "Array":
        return self._binary_op(other, lambda a, b: b - a)

    def __mul__(self, other: Any) -> "Array":
        return self._binary_op(other, lambda a, b: a * b)

    def __rmul__(self, other: Any) -> "Array":
        return self.__mul__(other)

    def __truediv__(self, other: Any) -> "Array":
        return self._binary_op(other, lambda a, b: a / b)

    def __neg__(self) -> "Array":
        out = Array(self._shape, dtype=self._dtype)
        for idx in self._ndindex():
            out[idx] = -self[idx]
        return out

    def __abs__(self) -> "Array":
        out = Array(self._shape, dtype=self._dtype)
        for idx in self._ndindex():
            out[idx] = abs(self[idx])
        return out

    def __matmul__(self, other: "Array") -> "Array":
        """2D Matrix multiplication (@)."""
        if self._ndim != 2 or other.ndim != 2:
            raise ValueError("matmul (@) currently requires 2D arrays")
        m, k1 = self._shape
        k2, n = other.shape
        if k1 != k2:
            raise ValueError(f"shapes ({m}, {k1}) and ({k2}, {n}) not aligned")

        if _native_matmul_f64 is not None and self._dtype == "float64" and other._dtype == "float64":
            if self._is_contiguous() and other._is_contiguous():
                out = Array(shape=(m, n), dtype="float64")
                buf_a = (ctypes.c_char * len(self._buffer)).from_buffer(self._buffer)
                buf_b = (ctypes.c_char * len(other._buffer)).from_buffer(other._buffer)
                buf_c = (ctypes.c_char * len(out._buffer)).from_buffer(out._buffer)
                ptr_a = ctypes.addressof(buf_a) + self._offset
                ptr_b = ctypes.addressof(buf_b) + other._offset
                ptr_c = ctypes.addressof(buf_c)
                status = _native_matmul_f64(ptr_a, m, k1, ptr_b, k2, n, ptr_c)
                if status == 0:
                    return out

        out = Array(shape=(m, n), dtype=self._dtype)
        for i in range(m):
            for j in range(n):
                acc = 0
                for p in range(k1):
                    acc += self[i, p] * other[p, j]
                out[i, j] = acc
        return out

    def sum(self, axis: Optional[int] = None) -> Union[float, int, "Array"]:
        if axis is None:
            if _native_sum_f64 is not None and self._dtype == "float64" and self._is_contiguous():
                buf = (ctypes.c_char * len(self._buffer)).from_buffer(self._buffer)
                ptr = ctypes.addressof(buf) + self._offset
                out_val = ctypes.c_double()
                status = _native_sum_f64(ptr, self._size, ctypes.byref(out_val))
                if status == 0:
                    return out_val.value
            return sum(self[idx] for idx in self._ndindex())
        if axis < 0:
            axis += self._ndim
        new_shape = [s for ax, s in enumerate(self._shape) if ax != axis]
        out = Array(new_shape, dtype=self._dtype)
        for idx in self._ndindex():
            out_idx = tuple(c for ax, c in enumerate(idx) if ax != axis)
            out[out_idx] = out[out_idx] + self[idx]
        return out

    def mean(self, axis: Optional[int] = None) -> Union[float, "Array"]:
        if axis is None:
            return float(self.sum()) / self._size
        s = self.sum(axis=axis)
        count = self._shape[axis]
        return s / count

    def max(self, axis: Optional[int] = None) -> Any:
        if axis is None:
            return max(self[idx] for idx in self._ndindex())
        if axis < 0:
            axis += self._ndim
        new_shape = [s for ax, s in enumerate(self._shape) if ax != axis]
        out = None
        for idx in self._ndindex():
            out_idx = tuple(c for ax, c in enumerate(idx) if ax != axis)
            val = self[idx]
            if out is None:
                out = Array(new_shape, dtype=self._dtype)
                for o_idx in out._ndindex():
                    out[o_idx] = val
            if val > out[out_idx]:
                out[out_idx] = val
        return out

    def min(self, axis: Optional[int] = None) -> Any:
        if axis is None:
            return min(self[idx] for idx in self._ndindex())
        if axis < 0:
            axis += self._ndim
        new_shape = [s for ax, s in enumerate(self._shape) if ax != axis]
        out = None
        for idx in self._ndindex():
            out_idx = tuple(c for ax, c in enumerate(idx) if ax != axis)
            val = self[idx]
            if out is None:
                out = Array(new_shape, dtype=self._dtype)
                for o_idx in out._ndindex():
                    out[o_idx] = val
            if val < out[out_idx]:
                out[out_idx] = val
        return out

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


def array(data: Any, dtype: Any = None) -> Array:
    """Creates an array from nested lists, numbers, or array-like objects."""
    if isinstance(data, Array):
        if dtype is None or _canonical_dtype(dtype) == data.dtype:
            return data.copy()
        out = Array(data.shape, dtype=dtype)
        for idx in data._ndindex():
            out[idx] = data[idx]
        return out

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
        if any(isinstance(x, float) for x in vals):
            dt = "float64"
        elif any(isinstance(x, bool) for x in vals):
            dt = "bool"
        else:
            dt = "int64"
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


def arange(start: float, stop: Optional[float] = None, step: float = 1.0, dtype: Any = None) -> Array:
    """Returns evenly spaced values within a given interval."""
    if stop is None:
        stop = start
        start = 0.0
    vals = []
    curr = start
    if step > 0:
        while curr < stop:
            vals.append(curr)
            curr += step
    elif step < 0:
        while curr > stop:
            vals.append(curr)
            curr += step
    return array(vals, dtype=dtype)


def linspace(start: float, stop: float, num: int = 50, dtype: Any = "float64") -> Array:
    """Returns num evenly spaced samples over the interval [start, stop]."""
    if num <= 0:
        return Array((0,), dtype=dtype)
    if num == 1:
        return array([start], dtype=dtype)
    step = (stop - start) / (num - 1)
    vals = [start + i * step for i in range(num)]
    return array(vals, dtype=dtype)


def matmul(a: Array, b: Array) -> Array:
    """Matrix product of two arrays."""
    return a @ b


def dot(a: Array, b: Array) -> Any:
    """Dot product of two arrays."""
    if a.ndim == 1 and b.ndim == 1:
        if a.shape[0] != b.shape[0]:
            raise ValueError("vectors must have equal length")
        return sum(a[i] * b[i] for i in range(a.shape[0]))
    return a @ b
