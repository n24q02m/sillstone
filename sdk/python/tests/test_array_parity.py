"""Unit tests for Sillstone NumPy-compatible Array engine and zero-copy Image bridge."""

import os
import sys
import unittest

sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), "..")))

from sillstone import Array, Image, array, asarray, zeros, ones, arange, linspace, matmul, dot

class TestArrayEngine(unittest.TestCase):
    def test_creation_and_indexing(self):
        a = array([[1, 2, 3], [4, 5, 6]])
        self.assertEqual(a.shape, (2, 3))
        self.assertEqual(a.ndim, 2)
        self.assertEqual(a.size, 6)
        self.assertEqual(a[0, 0], 1)
        self.assertEqual(a[0, 2], 3)
        self.assertEqual(a[1, 1], 5)

        # Assignment
        a[1, 1] = 99
        self.assertEqual(a[1, 1], 99)

    def test_reshaping_and_transpose(self):
        a = array([[1, 2, 3], [4, 5, 6]])
        r = a.reshape(3, 2)
        self.assertEqual(r.shape, (3, 2))
        self.assertEqual(r.tolist(), [[1, 2], [3, 4], [5, 6]])

        t = a.T
        self.assertEqual(t.shape, (3, 2))
        self.assertEqual(t[0, 0], 1)
        self.assertEqual(t[0, 1], 4)
        self.assertEqual(t[2, 0], 3)
        self.assertEqual(t[2, 1], 6)

    def test_broadcasting_and_arithmetic(self):
        a = array([[10, 20, 30], [40, 50, 60]])
        b = array([1, 2, 3])
        c = a + b
        self.assertEqual(c.shape, (2, 3))
        self.assertEqual(c.tolist(), [[11, 22, 33], [41, 52, 63]])

        # Scalar multiplication
        d = b * 10
        self.assertEqual(d.tolist(), [10, 20, 30])

    def test_matrix_multiplication(self):
        # [2, 3] @ [3, 2] -> [2, 2]
        a = array([[1, 2, 3], [4, 5, 6]])
        b = array([[7, 8], [9, 1], [2, 3]])
        c = a @ b
        self.assertEqual(c.shape, (2, 2))
        self.assertEqual(c.tolist(), [[31, 19], [85, 55]])
        self.assertEqual(matmul(a, b).tolist(), c.tolist())

    def test_reductions(self):
        a = array([[1.0, 5.0, 2.0], [6.0, 3.0, 4.0]])
        self.assertEqual(a.sum(), 21.0)
        self.assertAlmostEqual(a.mean(), 3.5)

        sum_ax0 = a.sum(axis=0)
        self.assertEqual(sum_ax0.shape, (3,))
        self.assertEqual(sum_ax0.tolist(), [7.0, 8.0, 6.0])

        max_ax1 = a.max(axis=1)
        self.assertEqual(max_ax1.shape, (2,))
        self.assertEqual(max_ax1.tolist(), [5.0, 6.0])

    def test_image_array_bridge(self):
        # Create an RGB image
        img = Image.new("RGB", (10, 8), (255, 128, 64))
        arr = img.to_array()
        self.assertEqual(arr.shape, (8, 10, 3))
        self.assertEqual(arr.dtype, "uint8")
        self.assertEqual(arr[0, 0, 0], 255)
        self.assertEqual(arr[0, 0, 1], 128)
        self.assertEqual(arr[0, 0, 2], 64)

        # Convert back to image
        restored = arr.to_image()
        self.assertEqual(restored.size, (10, 8))
        self.assertEqual(restored.mode, "RGB")
        self.assertEqual(restored.getpixel((0, 0)), (255, 128, 64))

    def test_array_interface_interop(self):
        a = zeros((4, 4), dtype="float32")
        iface = a.__array_interface__
        self.assertEqual(iface["version"], 3)
        self.assertEqual(iface["shape"], (4, 4))
        self.assertEqual(iface["typestr"], "<f4")
        self.assertIsInstance(iface["data"][0], int)
        self.assertFalse(iface["data"][1])


if __name__ == "__main__":
    unittest.main()
