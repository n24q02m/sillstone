package io.github.sillstone;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.function.Executable;

/** The tensor API beyond the golden vectors: factories, argument checks, broadcasting maths and reductions. */
class TensorApiTest {
    private static Tensor t(int[] shape, double... data) {
        return new Tensor(shape, data);
    }

    private static void rejects(String message, Executable call) {
        assertEquals(message, assertThrows(IllegalArgumentException.class, call).getMessage());
    }

    @Test
    void shapesAreValidated() {
        rejects("shape must have at least one dimension", () -> new Tensor(null, new double[0]));
        rejects("shape must have at least one dimension", () -> Tensor.ones());
        rejects("every dimension must be positive, got -2", () -> Tensor.full(1.0, 3, -2));
        rejects("tensor is too large", () -> Tensor.zeros(65536, 65536));
    }

    @Test
    void factoriesFillEveryElement() {
        assertEquals(t(new int[] {2, 2}, 0, 0, 0, 0), Tensor.zeros(2, 2));
        assertEquals(t(new int[] {3}, 1, 1, 1), Tensor.ones(3));
        assertEquals(t(new int[] {1, 2}, 2.5, 2.5), Tensor.full(2.5, 1, 2));
        int[] shape = {2};
        Tensor z = Tensor.zeros(shape);
        shape[0] = 5;
        assertArrayEquals(new int[] {2}, z.getShape());
    }

    @Test
    void arangeAndLinspaceMatchNumpy() {
        assertArrayEquals(new double[] {5, 3, 1}, Tensor.arange(5, 0, -2).getData(), 0);
        assertArrayEquals(new double[] {0, 0.5, 1, 1.5}, Tensor.arange(0, 2, 0.5).getData(), 0);
        for (double step : new double[] {0, Double.NaN, Double.POSITIVE_INFINITY, Double.NEGATIVE_INFINITY}) {
            rejects("step must be a non-zero finite number", () -> Tensor.arange(0, 1, step));
        }
        rejects("arange produces no elements", () -> Tensor.arange(0, 0, 1));
        rejects("arange produces no elements", () -> Tensor.arange(0, 5, -1));
        rejects("arange produces no elements", () -> Tensor.arange(Double.NaN, 1, 1));

        assertArrayEquals(new double[] {0, 0.25, 0.5, 0.75, 1}, Tensor.linspace(0, 1, 5).getData(), 0);
        assertArrayEquals(new double[] {2}, Tensor.linspace(2, 3, 1).getData(), 0);
        // 0 + 3 * (0.9 / 3) is 0.8999999999999999; the last sample is the endpoint itself, as in NumPy.
        assertEquals(0.9, Tensor.linspace(0, 0.9, 4).getData()[3], 0);
        rejects("num must be positive", () -> Tensor.linspace(0, 1, 0));
    }

    @Test
    void toImageChecksRankAndChannels() {
        rejects("toImage needs a [h, w] or [h, w, channels] tensor", () -> Tensor.zeros(4).toImage());
        rejects("toImage needs a [h, w] or [h, w, channels] tensor", () -> Tensor.zeros(1, 1, 1, 1).toImage());
        rejects("channel count 2 has no image mode", () -> Tensor.zeros(1, 1, 2).toImage());

        Image gray = t(new int[] {1, 4}, -3.2, 300.6, 1.5, 2.49).toImage();
        assertEquals(Image.Mode.L, gray.getMode());
        assertEquals(4, gray.getWidth());
        assertEquals(1, gray.getHeight());
        assertArrayEquals(new byte[] {0, (byte) 255, 2, 2}, gray.toBytes());
        assertEquals(Image.Mode.RGBA, Tensor.full(9, 1, 1, 4).toImage().getMode());
    }

    @Test
    void getReadsRowMajorAndChecksEveryAxis() {
        Tensor m = Tensor.arange(0, 24, 1).reshape(new int[] {2, 3, 4});
        assertEquals(23, m.get(1, 2, 3), 0);
        assertEquals(6, m.get(0, 1, 2), 0);
        rejects("expected 3 coordinates", () -> m.get(1, 2));
        assertEquals("index 3 out of range for axis 1",
            assertThrows(IndexOutOfBoundsException.class, () -> m.get(0, 3, 0)).getMessage());
        assertEquals("index -1 out of range for axis 0",
            assertThrows(IndexOutOfBoundsException.class, () -> m.get(-1, 0, 0)).getMessage());
    }

    @Test
    void sumOfOppositeInfinitiesIsNanAsInNumpy() {
        // numpy.sum([inf, -inf]) is nan: IEEE addition, not a refusal.
        Tensor t = t(new int[] {2}, Double.POSITIVE_INFINITY, Double.NEGATIVE_INFINITY);
        assertTrue(Double.isNaN(t.sum()));
        assertTrue(Double.isNaN(t.mean()));
        assertTrue(Double.isNaN(t(new int[] {2}, 1, Double.NaN).sum()));
        assertEquals(2.5, t(new int[] {4}, 1, 2, 3, 4).mean(), 0);
    }

    @Test
    void reshapeInfersOneDimension() {
        Tensor v = Tensor.arange(0, 12, 1);
        assertArrayEquals(new int[] {3, 4}, v.reshape(new int[] {-1, 4}).getShape());
        assertArrayEquals(new int[] {2, 2, 3}, v.reshape(new int[] {2, -1, 3}).getShape());
        assertArrayEquals(v.getData(), v.reshape(new int[] {3, -1}).getData(), 0);
        rejects("only one dimension can be inferred", () -> v.reshape(new int[] {-1, -1}));
        rejects("Cannot reshape with differing element count", () -> v.reshape(new int[] {-1, 5}));
        rejects("Cannot reshape with differing element count", () -> v.reshape(new int[] {0, -1}));
        rejects("Cannot reshape with differing element count", () -> v.reshape(new int[] {5, 2}));
        rejects("every dimension must be positive, got 0", () -> v.reshape(new int[] {12, 0}));
    }

    @Test
    void transposeRejectsAnythingButAPermutation() {
        Tensor m = Tensor.arange(0, 6, 1).reshape(new int[] {2, 3});
        assertEquals(m, m.transpose(0, 1));
        for (int[] axes : new int[][] {{0}, {0, 0}, {0, 2}, {-1, 0}}) {
            rejects("axes are not a permutation of 2 dimensions", () -> m.transpose(axes));
        }
    }

    @Test
    void elementwiseOpsBroadcastEitherWay() {
        Tensor m = t(new int[] {2, 3}, 1, 2, 3, 4, 5, 6);
        Tensor row = t(new int[] {3}, 10, 20, 30);
        Tensor col = t(new int[] {2, 1}, 2, 4);
        assertArrayEquals(new double[] {-9, -18, -27, -6, -15, -24}, m.subtract(row).getData(), 0);
        assertArrayEquals(new double[] {9, 18, 27, 6, 15, 24}, row.subtract(m).getData(), 0);
        assertArrayEquals(new double[] {2, 4, 6, 16, 20, 24}, m.multiply(col).getData(), 0);
        assertArrayEquals(new double[] {0.5, 1, 1.5, 1, 1.25, 1.5}, m.divide(col).getData(), 0);
        assertArrayEquals(new double[] {11, 12, 13, 14, 15, 16}, m.add(10.0).getData(), 0);
        assertArrayEquals(new double[] {-1, -2, -3, -4, -5, -6}, m.multiply(-1.0).getData(), 0);
        assertArrayEquals(new int[] {2, 3}, m.multiply(-1.0).getShape());
        assertEquals(Double.POSITIVE_INFINITY, t(new int[] {1}, 1).divide(t(new int[] {1}, 0)).getData()[0], 0);
        rejects("shapes [2, 3] and [2] cannot be broadcast together", () -> m.subtract(t(new int[] {2}, 1, 2)));

        Tensor squared = m.map(x -> x * x);
        assertArrayEquals(new int[] {2, 3}, squared.getShape());
        assertArrayEquals(new double[] {1, 4, 9, 16, 25, 36}, squared.getData(), 0);
    }

    @Test
    void matmulPromotesVectorsLikeNumpy() {
        Tensor m = t(new int[] {2, 3}, 1, 2, 3, 4, 5, 6);
        Tensor v3 = t(new int[] {3}, 1, 0, -1);
        Tensor v2 = t(new int[] {2}, 1, 1);
        assertEquals(t(new int[] {2}, -2, -2), m.matmul(v3));
        assertEquals(t(new int[] {3}, 5, 7, 9), v2.matmul(m));
        // NumPy gives a 0-d scalar for vector @ vector; the tensor keeps one element of shape [1].
        assertEquals(t(new int[] {1}, 2), v3.matmul(v3));
        rejects("Matmul requires 1D or 2D tensors", () -> Tensor.zeros(2, 2, 2).matmul(m));
        rejects("Matmul requires 1D or 2D tensors", () -> m.matmul(Tensor.zeros(2, 2, 2)));
        rejects("Inner matrix dimensions must agree", () -> m.matmul(v2));
        rejects("dot needs 1D tensors; use matmul for matrices", () -> m.dot(v3));
        rejects("dot needs 1D tensors; use matmul for matrices", () -> v3.dot(m));
    }

    @Test
    void axisReductionsAndNanPropagation() {
        Tensor m = t(new int[] {2, 3}, 3, -1, 4, 1, 5, -9);
        assertEquals(t(new int[] {3}, 1, -1, -9), m.min(0));
        assertEquals(t(new int[] {2}, 4, 5), m.max(1));
        assertEquals(t(new int[] {2}, -1, -9), m.min(-1));
        assertEquals(t(new int[] {3}, 3, 5, 4), m.max(-2));
        assertEquals(-9, m.min(), 0);
        assertEquals(5, m.max(), 0);
        // NumPy reduces a vector along its only axis to a 0-d scalar; the tensor keeps shape [1].
        assertEquals(t(new int[] {1}, 10), t(new int[] {3}, 2, 7, 1).sum(0));
        rejects("axis 2 is out of range for 2 dimensions", () -> m.sum(2));
        rejects("axis -3 is out of range for 2 dimensions", () -> m.mean(-3));

        Tensor withNan = t(new int[] {2, 2}, 1, Double.NaN, 3, 4);
        assertArrayEquals(new double[] {1, Double.NaN}, withNan.min(0).getData(), 0);
        assertArrayEquals(new double[] {Double.NaN, 4}, withNan.max(1).getData(), 0);
        assertTrue(Double.isNaN(withNan.min()));
        assertTrue(Double.isNaN(withNan.max()));
    }

    @Test
    void allCloseNeedsTheSameShapeAndAnAbsoluteTolerance() {
        Tensor a = t(new int[] {2}, 1, 2);
        assertTrue(a.allClose(t(new int[] {2}, 1.05, 1.95), 0.1));
        assertFalse(a.allClose(t(new int[] {2}, 1.2, 2), 0.1));
        assertFalse(a.allClose(t(new int[] {1, 2}, 1, 2), 0.1));
        assertFalse(t(new int[] {1}, Double.NaN).allClose(t(new int[] {1}, Double.NaN), 1));
    }

    @Test
    void valueSemanticsAndDefensiveCopies() {
        double[] data = {1, 2};
        int[] shape = {2};
        Tensor a = new Tensor(shape, data);
        data[0] = 9;
        shape[0] = 1;
        Tensor b = t(new int[] {2}, 1, 2);
        assertEquals(b, a);
        assertEquals(b.hashCode(), a.hashCode());
        assertNotEquals(a, t(new int[] {1, 2}, 1, 2));
        assertNotEquals(a, t(new int[] {2}, 1, 3));
        assertNotEquals(a, (Object) "Tensor[2][1.0, 2.0]");
        assertEquals("Tensor[2][1.0, 2.0]", a.toString());
        a.getData()[0] = 7;
        a.getShape()[0] = 7;
        assertEquals(b, a);
    }
}
