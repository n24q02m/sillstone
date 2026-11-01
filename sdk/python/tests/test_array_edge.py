"""Edge-case and NumPy-parity tests for the Sillstone Array engine.

Each case documents NumPy behaviour that the pure-Python engine must match:
dtype resolution and promotion, bounds-checked indexing, slice assignment,
integer wrap-around, IEEE division, NaN-propagating reductions and the
creation helpers.
"""

import itertools
import math
import operator
import os
import sys
import unittest

sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), "..")))

from sillstone import Array, array, arange, dot, linspace, ones, zeros
from sillstone.array import DTYPE_MAP, AxisError, _canonical_dtype, _promote

try:
    import numpy as np
except ImportError:  # pragma: no cover - numpy is installed in CI
    np = None

NAN = float("nan")
BINARY_OPS = {"+": operator.add, "-": operator.sub, "*": operator.mul, "/": operator.truediv}


def same(a, b):
    """Equality that treats NaN as equal to NaN, recursing into nested lists."""
    if isinstance(a, list) and isinstance(b, list):
        return len(a) == len(b) and all(same(x, y) for x, y in zip(a, b))
    if isinstance(a, float) and isinstance(b, float) and math.isnan(a) and math.isnan(b):
        return True
    return a == b


class EdgeCase(unittest.TestCase):
    def assertSame(self, actual, expected):
        self.assertTrue(same(actual, expected), f"{actual!r} != {expected!r}")

    def assertMatches(self, arr, ref):
        """The Array equals the NumPy array in dtype, shape and every element."""
        self.assertEqual(arr.dtype, str(ref.dtype))
        self.assertEqual(arr.shape, ref.shape)
        self.assertSame(arr.tolist(), ref.tolist())


class TestDtypes(EdgeCase):
    def test_aliases_and_typestrings(self):
        for alias, name in [
            ("i1", "int8"), ("i2", "int16"), ("u2", "uint16"), ("u8", "uint64"),
            ("bool_", "bool"), ("?", "bool"), ("<f4", "float32"), ("|u1", "uint8"),
            ("<i8", "int64"), ("FLOAT64", "float64"), (float, "float64"), (int, "int64"), (bool, "bool"),
        ]:
            self.assertEqual(_canonical_dtype(alias), name, alias)

    def test_unknown_dtype_is_rejected(self):
        for bad in ("complex128", "datetime64", "nope", object, None, 3):
            with self.assertRaises(TypeError, msg=repr(bad)):
                Array((2,), dtype=bad)

    @unittest.skipIf(np is None, "numpy not installed")
    def test_numpy_dtype_objects(self):
        self.assertEqual(Array((1,), dtype=np.dtype("int32")).dtype, "int32")
        self.assertEqual(Array((1,), dtype=np.uint16).dtype, "uint16")
        self.assertEqual(Array((1,), dtype=np.bool_).dtype, "bool")

    @unittest.skipIf(np is None, "numpy not installed")
    def test_promotion_matches_numpy_for_every_pair(self):
        for a, b in itertools.product(DTYPE_MAP, repeat=2):
            self.assertEqual(_promote(a, b), str(np.promote_types(a, b)), (a, b))


class TestConstruction(EdgeCase):
    def test_invalid_layouts_are_rejected(self):
        with self.assertRaises(ValueError):
            Array((-1,))
        with self.assertRaises(ValueError):
            Array((2, 2), strides=(8,))
        with self.assertRaises(ValueError):
            Array((2,), offset=-1)
        with self.assertRaises(ValueError):
            Array((2,), "float64", buffer=b"\x00" * 4)
        with self.assertRaises(ValueError):
            Array((2,), "int8", buffer=b"\x01\x02", strides=(10,))
        with self.assertRaises(ValueError):
            Array((2,), "int8", buffer=b"\x01\x02", offset=1)
        with self.assertRaises(TypeError):
            Array((2.5,))

    def test_negative_strides_and_oversized_buffer_are_valid(self):
        rev = Array((3,), "int8", buffer=b"\x01\x02\x03", strides=(-1,), offset=2)
        self.assertEqual(rev.tolist(), [3, 2, 1])
        self.assertEqual(Array((1,), "int8", buffer=b"\x07\x08\x09").tolist(), [7])

    def test_zero_size_arrays(self):
        for shape in [(0,), (0, 3), (3, 0), (2, 0, 2)]:
            z = zeros(shape)
            self.assertEqual(z.size, 0)
            self.assertEqual(z.copy().shape, shape)
            self.assertEqual(z.tobytes(), b"")
        self.assertEqual(zeros((2, 0)).tolist(), [[], []])
        self.assertEqual(zeros((0, 3)).tolist(), [])

    def test_buffer_is_shared_with_caller_bytearray(self):
        raw = bytearray(8)
        view = Array((2,), "int32", buffer=raw)
        view[1] = 7
        self.assertEqual(raw[4], 7)


class TestIndexing(EdgeCase):
    def setUp(self):
        self.a = array([[1, 2, 3], [4, 5, 6]])

    def test_out_of_range_rows_raise_index_error(self):
        for bad in (2, 5, -3, -5):
            with self.assertRaises(IndexError, msg=bad):
                self.a[bad]
        with self.assertRaises(IndexError):
            self.a[0, 3]
        with self.assertRaises(IndexError):
            self.a[:, 7]

    def test_too_many_indices(self):
        with self.assertRaisesRegex(IndexError, "too many indices"):
            self.a[1, 2, 3]
        with self.assertRaises(IndexError):
            array(5)[0]

    def test_unsupported_index_kinds(self):
        for bad in (None, "x", [0, 1], 1.5, True, (0, None)):
            with self.assertRaises(IndexError, msg=repr(bad)):
                self.a[bad]

    def test_ellipsis(self):
        self.assertEqual(self.a[...].tolist(), [[1, 2, 3], [4, 5, 6]])
        self.assertEqual(self.a[..., 0].tolist(), [1, 4])
        self.assertEqual(self.a[1, ...].tolist(), [4, 5, 6])
        with self.assertRaises(IndexError):
            self.a[..., ...]

    def test_numpy_style_integer_scalars_are_accepted(self):
        class Idx:
            def __index__(self):
                return 1

        self.assertEqual(self.a[Idx(), Idx()], 5)
        self.assertEqual(self.a[Idx()].tolist(), [4, 5, 6])

    def test_slices(self):
        v = array([0, 1, 2, 3, 4])
        self.assertEqual(v[::3].tolist(), [0, 3])
        self.assertEqual(v[4:0:-2].tolist(), [4, 2])
        self.assertEqual(v[::-1].tolist(), [4, 3, 2, 1, 0])
        self.assertEqual(v[1:1].shape, (0,))
        self.assertEqual(v[10:].shape, (0,))
        self.assertEqual(self.a[0, 5:].shape, (0,))
        self.assertEqual(self.a[3:].shape, (0, 3))
        self.assertEqual(self.a[:, 1:].tolist(), [[2, 3], [5, 6]])

    def test_basic_slices_are_views(self):
        view = self.a[1]
        view[0] = 40
        self.assertEqual(self.a[1, 0], 40)

    def test_zero_dim_indexing(self):
        z = array(5)
        self.assertEqual(z[()], 5)
        z[()] = 8
        self.assertEqual(z.tolist(), 8)


class TestAssignment(EdgeCase):
    def test_partial_and_slice_assignment_broadcasts(self):
        a = zeros((2, 3), dtype="int64")
        a[0] = 7
        a[1, 1:] = [8, 9]
        a[:, 0] = array([1, 2])
        self.assertEqual(a.tolist(), [[1, 7, 7], [2, 8, 9]])
        a[...] = 0
        self.assertEqual(a.tolist(), [[0, 0, 0], [0, 0, 0]])

    def test_incompatible_shape_is_rejected(self):
        a = zeros((2, 3))
        with self.assertRaises(ValueError):
            a[0] = [1, 2]
        with self.assertRaises(ValueError):
            a[:] = zeros((3, 2))

    def test_overlapping_assignment_reads_before_writing(self):
        v = array([1, 2, 3, 4])
        v[1:] = v[:-1]
        self.assertEqual(v.tolist(), [1, 1, 2, 3])

    def test_element_conversion_follows_numpy(self):
        i = array([0, 0], dtype="int64")
        i[0] = 1.9
        i[1] = -1.9
        self.assertEqual(i.tolist(), [1, -1])
        f = array([0.0])
        f[0] = 5
        self.assertEqual(f.tolist(), [5.0])
        b = array([False, False])
        b[0] = 5
        self.assertEqual(b.tolist(), [True, False])
        small = array([0], dtype="int8")
        with self.assertRaises(OverflowError):
            small[0] = 300
        with self.assertRaises(OverflowError):
            array([-1], dtype="uint8")
        with self.assertRaises(ValueError):
            i[0] = NAN
        with self.assertRaises(ValueError):
            f[0] = "x"
        with self.assertRaises(TypeError):
            i[0] = None

    def test_float32_overflow_becomes_infinity(self):
        f = zeros(1, dtype="float32")
        f[0] = 1e300
        self.assertEqual(f[0], math.inf)
        f[0] = -1e300
        self.assertEqual(f[0], -math.inf)


class TestShapeOps(EdgeCase):
    def setUp(self):
        self.a = array([[1, 2, 3], [4, 5, 6]])

    def test_reshape_infers_one_dimension(self):
        self.assertEqual(self.a.reshape(-1).shape, (6,))
        self.assertEqual(self.a.reshape(-1, 2).shape, (3, 2))
        self.assertEqual(self.a.reshape((3, -1)).shape, (3, 2))
        self.assertEqual(self.a.reshape(1, -1, 3).shape, (1, 2, 3))

    def test_reshape_errors(self):
        for bad in [(4, 2), (-1, -1), (-2, 3), (-1, 4), (0, -1)]:
            with self.assertRaises(ValueError, msg=bad):
                self.a.reshape(*bad)

    def test_reshape_of_views_copies_when_needed(self):
        self.assertEqual(self.a.T.reshape(6).tolist(), [1, 4, 2, 5, 3, 6])
        self.assertEqual(self.a[:, ::2].reshape(4).tolist(), [1, 3, 4, 6])
        self.assertEqual(array([1, 2, 3, 4, 5, 6])[2:].reshape(2, 2).tolist(), [[3, 4], [5, 6]])

    def test_transpose_validates_axes(self):
        self.assertEqual(self.a.transpose(-1, 0).shape, (3, 2))
        self.assertEqual(self.a.transpose([1, 0]).shape, (3, 2))
        with self.assertRaises(ValueError):
            self.a.transpose(0, 0)
        with self.assertRaises(AxisError):
            self.a.transpose(0, 5)
        with self.assertRaises(ValueError):
            self.a.transpose(0)

    def test_transpose_3d_and_0d(self):
        cube = array([[[1, 2], [3, 4]], [[5, 6], [7, 8]]])
        self.assertEqual(cube.transpose(2, 0, 1).tolist(), [[[1, 3], [5, 7]], [[2, 4], [6, 8]]])
        self.assertEqual(array(5).T.tolist(), 5)

    def test_copy_and_tobytes_of_views_are_exact(self):
        v = array([1, 2, 3, 4])
        self.assertEqual(v[::-1].tobytes(), array([4, 3, 2, 1]).tobytes())
        self.assertEqual(v[1:3].tobytes(), array([2, 3]).tobytes())
        self.assertEqual(self.a.T.copy().strides, (16, 8))
        self.assertEqual(self.a.T.tobytes(), array([[1, 4], [2, 5], [3, 6]]).tobytes())

    def test_copy_preserves_nan_payload_bytes(self):
        raw = b"\x01\x00\x00\x00\x00\x00\xf8\x7f"
        a = Array((1,), "float64", buffer=bytearray(raw))
        self.assertEqual(a.copy().tobytes(), raw)

    def test_len_and_iter(self):
        self.assertEqual(len(self.a), 2)
        self.assertEqual([row.tolist() for row in self.a], [[1, 2, 3], [4, 5, 6]])
        self.assertEqual(list(array([7, 8])), [7, 8])
        with self.assertRaises(TypeError):
            len(array(1))


class TestArithmetic(EdgeCase):
    def test_result_dtype_follows_numpy_promotion(self):
        ints = array([1, 2, 3])
        self.assertEqual((ints + 1.5).dtype, "float64")
        self.assertEqual((ints + 1).dtype, "int64")
        self.assertEqual((ints / 2).dtype, "float64")
        self.assertEqual((ints + array([0.5, 0.5, 0.5])).dtype, "float64")
        self.assertEqual((array([1.0]) + array([1])).dtype, "float64")
        f32 = array([1, 2], dtype="float32")
        self.assertEqual((f32 + 1.5).dtype, "float32")
        self.assertEqual((f32 + array([1, 2], dtype="int8")).dtype, "float32")
        self.assertEqual((f32 + array([1, 2], dtype="int32")).dtype, "float64")
        self.assertEqual((array([True]) + 1).dtype, "int64")
        self.assertEqual((array([True]) + True).dtype, "bool")
        self.assertEqual((array([1], dtype="int8") + array([1], dtype="uint8")).dtype, "int16")

    def test_values(self):
        ints = array([[1, 2], [3, 4]])
        self.assertEqual((ints + 1.5).tolist(), [[2.5, 3.5], [4.5, 5.5]])
        self.assertEqual((ints / 2).tolist(), [[0.5, 1.0], [1.5, 2.0]])
        self.assertEqual((ints / ints).tolist(), [[1.0, 1.0], [1.0, 1.0]])
        self.assertEqual((10 - ints).tolist(), [[9, 8], [7, 6]])
        self.assertEqual((3 * ints).tolist(), [[3, 6], [9, 12]])
        self.assertEqual((3 / array([1.0, 2.0])).tolist(), [3.0, 1.5])
        self.assertEqual((array([1, 2]) + array([[10], [20]])).tolist(), [[11, 12], [21, 22]])

    def test_division_by_zero_follows_ieee(self):
        self.assertSame((array([1.0, -1.0, 0.0]) / 0).tolist(), [math.inf, -math.inf, NAN])
        self.assertSame((array([1, -2, 0]) / array([0, 0, 0])).tolist(), [math.inf, -math.inf, NAN])
        self.assertSame((1 / array([0.0, -0.0])).tolist(), [math.inf, -math.inf])

    def test_integer_arithmetic_wraps_around(self):
        self.assertEqual((array([100], dtype="int8") + array([100], dtype="int8")).tolist(), [-56])
        self.assertEqual((-array([1], dtype="uint8")).tolist(), [255])
        self.assertEqual(abs(array([-128], dtype="int8")).tolist(), [-128])
        self.assertEqual((array([250], dtype="uint8") * 2).tolist(), [244])
        with self.assertRaises(OverflowError):
            array([1], dtype="int8") + 300

    def test_float32_results_are_rounded_to_float32(self):
        total = array([0.1], dtype="float32") + array([0.2], dtype="float32")
        self.assertEqual(total[0], struct_float32(0.1 + 0.2))
        big = array([3e38], dtype="float32") * 10
        self.assertEqual(big[0], math.inf)

    def test_bool_operators(self):
        flags = array([True, False])
        self.assertEqual((flags + flags).tolist(), [True, False])
        self.assertEqual((flags * flags).dtype, "bool")
        with self.assertRaises(TypeError):
            flags - flags
        with self.assertRaises(TypeError):
            True - flags
        with self.assertRaises(TypeError):
            -flags

    def test_unsupported_operands(self):
        a = array([1, 2])
        with self.assertRaises(TypeError):
            a + "x"
        with self.assertRaises(TypeError):
            a * None
        with self.assertRaises(TypeError):
            a @ 3
        with self.assertRaises(ValueError):
            a + array([1, 2, 3])

    def test_native_float64_add_with_views(self):
        base = array([1.0, 2.0, 3.0, 4.0])
        self.assertEqual((base[1:] + array([10.0, 20.0, 30.0])).tolist(), [12.0, 23.0, 34.0])
        self.assertEqual((base[::2] + base[1::2]).tolist(), [3.0, 7.0])
        self.assertEqual((zeros(0) + zeros(0)).shape, (0,))


def struct_float32(value):
    import struct

    return struct.unpack("f", struct.pack("f", value))[0]


class TestMatmul(EdgeCase):
    def test_one_and_two_dimensional_products(self):
        m = array([[1, 2, 3], [4, 5, 6]])
        v3 = array([1, 0, -1])
        v2 = array([2, 3])
        self.assertEqual((m @ v3).tolist(), [-2, -2])
        self.assertEqual((v2 @ m).tolist(), [14, 19, 24])
        self.assertEqual(v3 @ v3, 2)
        self.assertEqual(dot(v3, v3), 2)
        self.assertEqual(dot(m, v3).tolist(), [-2, -2])
        self.assertEqual(dot(m, m.T).tolist(), [[14, 32], [32, 77]])

    def test_errors(self):
        m = array([[1, 2, 3], [4, 5, 6]])
        with self.assertRaisesRegex(ValueError, "not aligned"):
            m @ m
        with self.assertRaisesRegex(ValueError, "not aligned"):
            dot(array([1, 2, 3]), array([4, 5]))
        with self.assertRaises(ValueError):
            array(5) @ array(5)
        with self.assertRaises(ValueError):
            zeros((2, 2, 2)) @ zeros((2, 2, 2))

    def test_mixed_dtypes_promote(self):
        out = array([[1, 2]]) @ array([[1.5], [2.5]])
        self.assertEqual(out.dtype, "float64")
        self.assertEqual(out.tolist(), [[6.5]])
        wrapped = array([[100]], dtype="int8") @ array([[2]], dtype="int8")
        self.assertEqual(wrapped.tolist(), [[-56]])

    def test_float64_native_path_with_views_and_empty_dims(self):
        a = array([[1.0, 2.0], [3.0, 4.0], [5.0, 6.0]])
        eye = array([[1.0, 0.0], [0.0, 1.0]])
        self.assertEqual((a[1:] @ eye).tolist(), [[3.0, 4.0], [5.0, 6.0]])
        self.assertEqual((a.T @ a).tolist(), [[35.0, 44.0], [44.0, 56.0]])
        self.assertEqual((zeros((2, 0)) @ zeros((0, 3))).tolist(), [[0.0] * 3] * 2)
        self.assertEqual((zeros((0, 2)) @ zeros((2, 3))).shape, (0, 3))


class TestReductions(EdgeCase):
    def setUp(self):
        self.a = array([[1, 5, 2], [6, 3, 4]])

    def test_sum_dtype_and_axes(self):
        self.assertEqual(self.a.sum(), 21)
        self.assertEqual(self.a.sum(axis=0).tolist(), [7, 8, 6])
        self.assertEqual(self.a.sum(axis=-1).tolist(), [8, 13])
        self.assertEqual(array([1, 2, 3]).sum(axis=0), 6)
        self.assertEqual(array(5).sum(), 5)
        flags = array([[True, True, False]])
        self.assertEqual(flags.sum(), 2)
        self.assertEqual(flags.sum(axis=1).tolist(), [2])
        self.assertEqual(flags.sum(axis=1).dtype, "int64")
        small = array([[100, 100]], dtype="int8")
        self.assertEqual(small.sum(), 200)
        self.assertEqual(small.sum(axis=1).tolist(), [200])
        self.assertEqual(array([[200, 200]], dtype="uint8").sum(axis=1).dtype, "uint64")
        self.assertEqual(array([0.5, 0.25], dtype="float32").sum(), 0.75)

    def test_axis_out_of_range(self):
        for call in (self.a.sum, self.a.mean, self.a.max, self.a.min, self.a.argmax, self.a.argmin):
            for bad in (2, 5, -3):
                with self.assertRaises(AxisError, msg=(call.__name__, bad)):
                    call(axis=bad)
        with self.assertRaises(TypeError):
            self.a.sum(axis=0.5)

    def test_empty_reductions(self):
        self.assertEqual(zeros(0).sum(), 0.0)
        self.assertEqual(zeros((0, 3)).sum(axis=0).tolist(), [0.0, 0.0, 0.0])
        self.assertEqual(zeros((3, 0)).sum(axis=1).tolist(), [0.0, 0.0, 0.0])
        self.assertTrue(math.isnan(zeros(0).mean()))
        self.assertTrue(all(math.isnan(x) for x in zeros((0, 2)).mean(axis=0).tolist()))
        for call in (zeros(0).max, zeros(0).min, zeros((0, 3)).max, zeros((0, 3)).argmax):
            with self.assertRaisesRegex(ValueError, "zero-size|empty"):
                call()
        with self.assertRaises(ValueError):
            zeros((0, 3)).max(axis=0)
        self.assertEqual(zeros((3, 0)).max(axis=0).shape, (0,))

    def test_mean(self):
        self.assertAlmostEqual(self.a.mean(), 3.5)
        self.assertEqual(self.a.mean(axis=0).tolist(), [3.5, 4.0, 3.0])
        self.assertEqual(self.a.mean(axis=0).dtype, "float64")
        self.assertEqual(self.a.mean(axis=-1).tolist(), [8 / 3, 13 / 3])
        self.assertEqual(array([[1.0, 2.0], [3.0, 4.0]]).mean(axis=0).tolist(), [2.0, 3.0])
        self.assertEqual(array([1, 2], dtype="float32").mean(), 1.5)
        self.assertEqual(array([1, 2], dtype="float32").mean(axis=0), 1.5)

    def test_max_min_propagate_nan(self):
        v = array([1.0, NAN, 2.0])
        self.assertTrue(math.isnan(v.max()))
        self.assertTrue(math.isnan(v.min()))
        self.assertTrue(math.isnan(array([NAN, 1.0]).max()))
        m = array([[1.0, NAN], [2.0, 3.0]])
        self.assertSame(m.max(axis=0).tolist(), [2.0, NAN])
        self.assertSame(m.min(axis=0).tolist(), [1.0, NAN])
        self.assertSame(m.max(axis=1).tolist(), [NAN, 3.0])
        self.assertEqual(v.argmax(), 1)
        self.assertEqual(v.argmin(), 1)

    def test_max_min_arg(self):
        self.assertEqual((self.a.max(), self.a.min()), (6, 1))
        self.assertEqual(self.a.max(axis=1).tolist(), [5, 6])
        self.assertEqual(self.a.min(axis=0).tolist(), [1, 3, 2])
        self.assertEqual(array([1, 3, 2]).max(axis=0), 3)
        self.assertEqual(array([1, 3, 3, 1]).argmax(), 1)
        self.assertEqual(array([2, 1, 1, 2]).argmin(), 1)
        self.assertEqual(self.a.argmax(), 3)
        self.assertEqual(self.a.argmin(), 0)
        self.assertEqual(self.a.argmax(axis=1).tolist(), [1, 0])
        self.assertEqual(self.a.argmin(axis=0).tolist(), [0, 1, 0])
        self.assertEqual(self.a.argmax(axis=0).dtype, "int64")


class TestCreation(EdgeCase):
    def test_array_infers_dtype_like_numpy(self):
        self.assertEqual(array([True, False]).dtype, "bool")
        self.assertEqual(array([True, 2]).dtype, "int64")
        self.assertEqual(array([True, 2.5]).dtype, "float64")
        self.assertEqual(array([1, 2.5]).dtype, "float64")
        self.assertEqual(array([]).dtype, "float64")
        self.assertEqual(array([]).shape, (0,))
        self.assertEqual(array([[], []]).shape, (2, 0))
        self.assertEqual(array(5).shape, ())
        with self.assertRaises(ValueError):
            array([[1], [1, 2]])
        with self.assertRaises(TypeError):
            array([object()])
        with self.assertRaises(TypeError):
            array(["a"])

    def test_array_casts_to_requested_dtype(self):
        self.assertEqual(array([1.7, -2.2], dtype="int64").tolist(), [1, -2])
        self.assertEqual(array(array([1, 2, 3]), dtype="float32").dtype, "float32")
        self.assertEqual(array([1, 2], dtype=float).tolist(), [1.0, 2.0])
        with self.assertRaises(OverflowError):
            array([300], dtype="int8")

    def test_array_copies_arrays(self):
        src = array([1, 2])
        dup = array(src)
        dup[0] = 9
        self.assertEqual(src.tolist(), [1, 2])

    def test_zeros_and_ones(self):
        self.assertEqual(zeros(()).shape, ())
        self.assertEqual(ones((2, 2), dtype="int8").tolist(), [[1, 1], [1, 1]])
        self.assertEqual(ones(2, dtype="bool").tolist(), [True, True])

    def test_arange(self):
        self.assertEqual((arange(5).dtype, arange(5).tolist()), ("int64", [0, 1, 2, 3, 4]))
        self.assertEqual(arange(1, 4).dtype, "int64")
        self.assertEqual(arange(5, 0, -2).tolist(), [5, 3, 1])
        self.assertEqual(arange(3, 3).shape, (0,))
        self.assertEqual(arange(0, 1, 0.25).tolist(), [0.0, 0.25, 0.5, 0.75])
        self.assertEqual(arange(0.0, 3.0).dtype, "float64")
        self.assertEqual(arange(3, dtype="float32").dtype, "float32")
        with self.assertRaises(ZeroDivisionError):
            arange(0, 5, 0)

    def test_linspace(self):
        self.assertEqual(linspace(0, 1, 5).tolist(), [0.0, 0.25, 0.5, 0.75, 1.0])
        self.assertEqual(linspace(3, 9, 1).tolist(), [3.0])
        self.assertEqual(linspace(0, 1, 0).shape, (0,))
        self.assertEqual(linspace(0, 10, 4, dtype="int64").tolist(), [0, 3, 6, 10])
        with self.assertRaisesRegex(ValueError, "non-negative"):
            linspace(0, 1, -1)


class TestInterop(EdgeCase):
    def test_repr(self):
        self.assertEqual(repr(array([1, 2])), "Array(shape=(2,), dtype='int64', data=[1, 2])")

    @unittest.skipIf(np is None, "numpy not installed")
    def test_numpy_input_keeps_dtype(self):
        src = np.arange(6, dtype="int16").reshape(2, 3)
        self.assertEqual(array(src).dtype, "int16")
        self.assertEqual(array(src).tolist(), src.tolist())
        self.assertEqual(array(np.float32(2.5)).dtype, "float32")
        self.assertEqual(array(np.array([1.5, 2.5], dtype="float16")).dtype, "float64")

    @unittest.skipIf(np is None, "numpy not installed")
    def test_views_round_trip_through_array_interface(self):
        a = array([[1.0, 2.0], [3.0, 4.0]])
        self.assertEqual(np.asarray(a[1:]).tolist(), [[3.0, 4.0]])
        self.assertEqual(np.asarray(a.T).tolist(), [[1.0, 3.0], [2.0, 4.0]])
        self.assertEqual(np.asarray(a[:, ::-1]).tolist(), [[2.0, 1.0], [4.0, 3.0]])


@unittest.skipIf(np is None, "numpy not installed")
class TestNumpyParity(EdgeCase):
    """Randomised-shape sweeps that compare directly against NumPy."""

    def numpy_cube(self, dtype="int64"):
        return np.arange(24, dtype=dtype).reshape(2, 3, 4) - 7

    def test_reductions_along_every_axis(self):
        for dtype in ("int64", "int8", "float64", "float32", "uint8"):
            ref = np.abs(self.numpy_cube()).astype(dtype)
            arr = array(ref.tolist(), dtype=dtype)
            for axis in (None, 0, 1, 2, -1, -3):
                for name in ("sum", "max", "min", "argmax", "argmin", "mean"):
                    got = getattr(arr, name)(axis=axis)
                    want = getattr(ref, name)(axis=axis)
                    if isinstance(got, Array):
                        self.assertMatches(got, np.asarray(want))
                    else:
                        self.assertAlmostEqual(float(got), float(want), places=5, msg=(dtype, name, axis))

    def test_elementwise_operators_match_numpy(self):
        left = np.array([[1, 2, 3], [4, 5, 6]])
        for ldt, rdt in [("int64", "float64"), ("int8", "uint8"), ("float32", "int16"), ("int32", "float32")]:
            a, b = left.astype(ldt), (left[:1] + 1).astype(rdt)
            sa, sb = array(a.tolist(), dtype=ldt), array(b.tolist(), dtype=rdt)
            for op in BINARY_OPS.values():
                self.assertMatches(op(sa, sb), op(a, b))

    def test_scalar_operators_match_numpy(self):
        for dtype in ("int64", "int16", "uint8", "float32", "float64"):
            ref = np.array([1, 2, 3], dtype=dtype)
            arr = array([1, 2, 3], dtype=dtype)
            for scalar in (2, 1.5, True):
                for sym in ("+", "*", "/"):
                    op = BINARY_OPS[sym]
                    self.assertMatches(op(arr, scalar), op(ref, scalar))
                    self.assertMatches(op(scalar, arr), op(scalar, ref))

    def test_creation_matches_numpy(self):
        for args in [(5,), (1, 4), (5, 0, -2), (0, 1, 0.1), (0, 1, 0.3), (-1.5, 2.5, 0.5), (3, 3)]:
            self.assertMatches(arange(*args), np.arange(*args))
        for start, stop, num in [(0, 1, 7), (-3, 5, 11), (2, 2, 4), (0, 1, 50)]:
            self.assertMatches(linspace(start, stop, num), np.linspace(start, stop, num))
        self.assertMatches(linspace(-1.5, 7.5, 6, dtype="int64"), np.linspace(-1.5, 7.5, 6, dtype="int64"))

    def test_indexing_matches_numpy(self):
        ref = self.numpy_cube()
        arr = array(ref.tolist())
        for key in [
            (1,), (slice(None), 2), (0, slice(1, None), slice(None, None, 2)), (Ellipsis, 3),
            (slice(None, None, -1), slice(None), slice(2, 0, -1)), (-1, -1, -1), (slice(5, 9),),
            (1, Ellipsis, slice(1, 3)), (slice(None), slice(None), slice(None)),
        ]:
            want = ref[key]
            got = arr[key]
            if isinstance(got, Array):
                self.assertMatches(got, np.asarray(want))
            else:
                self.assertEqual(got, want)

    def test_matmul_matches_numpy(self):
        rng = np.random.default_rng(7)
        for dtype in ("float64", "float32", "int64"):
            for sa, sb in [((3, 4), (4, 2)), ((4,), (4, 2)), ((3, 4), (4,)), ((4,), (4,)), ((1, 1), (1, 1))]:
                a = (rng.integers(-5, 5, sa)).astype(dtype)
                b = (rng.integers(-5, 5, sb)).astype(dtype)
                want = a @ b
                got = array(a.tolist(), dtype=dtype) @ array(b.tolist(), dtype=dtype)
                if isinstance(got, Array):
                    self.assertMatches(got, want)
                else:
                    self.assertEqual(got, want.item())


if __name__ == "__main__":
    unittest.main()
