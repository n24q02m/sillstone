package io.github.sillstone;

import java.util.Arrays;
import java.util.function.DoubleBinaryOperator;
import java.util.function.DoubleUnaryOperator;

/**
 * N-dimensional float64 tensor: one contiguous row-major {@code double[]}. Matrix products,
 * same-shape addition and full sums run in the native core; broadcasting, axis reductions and
 * elementwise maths are plain Java over the same buffer. Empty (zero-sized) tensors are not
 * supported. A {@code Tensor} is immutable.
 */
public final class Tensor {
    private final int[] shape;
    private final double[] data;

    /**
     * @param shape positive dimensions
     * @param data row-major values; length must equal the product of {@code shape} (copied)
     */
    public Tensor(int[] shape, double[] data) {
        int size = checkShape(shape);
        if (data.length != size) {
            throw new IllegalArgumentException("Data length does not match shape dimensions");
        }
        this.shape = Arrays.copyOf(shape, shape.length);
        this.data = Arrays.copyOf(data, data.length);
    }

    private Tensor(int[] shape, double[] data, boolean owned) {
        this.shape = shape;
        this.data = data;
    }

    private static int checkShape(int[] shape) {
        if (shape == null || shape.length == 0) {
            throw new IllegalArgumentException("shape must have at least one dimension");
        }
        long size = 1;
        for (int dim : shape) {
            if (dim < 1) {
                throw new IllegalArgumentException("every dimension must be positive, got " + dim);
            }
            size *= dim;
            if (size > Integer.MAX_VALUE - 8) {
                throw new IllegalArgumentException("tensor is too large");
            }
        }
        return (int) size;
    }

    private static int[] strides(int[] shape) {
        int[] st = new int[shape.length];
        int s = 1;
        for (int i = shape.length - 1; i >= 0; i--) {
            st[i] = s;
            s *= shape[i];
        }
        return st;
    }

    /** A tensor of zeros. */
    public static Tensor zeros(int... shape) {
        return new Tensor(Arrays.copyOf(shape, shape.length), new double[checkShape(shape)], true);
    }

    /** A tensor of ones. */
    public static Tensor ones(int... shape) {
        return full(1.0, shape);
    }

    /** A tensor filled with {@code value}. */
    public static Tensor full(double value, int... shape) {
        double[] d = new double[checkShape(shape)];
        Arrays.fill(d, value);
        return new Tensor(Arrays.copyOf(shape, shape.length), d, true);
    }

    /** {@code [start, stop)} stepping by {@code step}, like {@code numpy.arange}. */
    public static Tensor arange(double start, double stop, double step) {
        if (step == 0 || Double.isNaN(step) || Double.isInfinite(step)) {
            throw new IllegalArgumentException("step must be a non-zero finite number");
        }
        double count = Math.ceil((stop - start) / step);
        if (!(count >= 1)) {
            throw new IllegalArgumentException("arange produces no elements");
        }
        int n = (int) count;
        double[] d = new double[n];
        for (int i = 0; i < n; i++) {
            d[i] = start + i * step;
        }
        return new Tensor(new int[] {n}, d, true);
    }

    /** {@code num} evenly spaced values over {@code [start, stop]}, like {@code numpy.linspace}. */
    public static Tensor linspace(double start, double stop, int num) {
        if (num < 1) {
            throw new IllegalArgumentException("num must be positive");
        }
        double[] d = new double[num];
        double step = num > 1 ? (stop - start) / (num - 1) : 0.0;
        for (int i = 0; i < num; i++) {
            d[i] = start + i * step;
        }
        if (num > 1) {
            d[num - 1] = stop;
        }
        return new Tensor(new int[] {num}, d, true);
    }

    /** {@code [height, width, channels]} view of an image's samples as float64 (a copy). */
    public static Tensor fromImage(Image image) {
        byte[] px = image.toBytes();
        double[] d = new double[px.length];
        for (int i = 0; i < px.length; i++) {
            d[i] = px[i] & 0xff;
        }
        return new Tensor(new int[] {image.getHeight(), image.getWidth(), image.getMode().channels()}, d, true);
    }

    /** Rounds, clamps to 0..255 and wraps a {@code [h, w]} or {@code [h, w, c]} tensor as an image. */
    public Image toImage() {
        if (shape.length != 2 && shape.length != 3) {
            throw new IllegalArgumentException("toImage needs a [h, w] or [h, w, channels] tensor");
        }
        int ch = shape.length == 2 ? 1 : shape[2];
        Image.Mode mode = ch == 1 ? Image.Mode.L : ch == 3 ? Image.Mode.RGB : ch == 4 ? Image.Mode.RGBA : null;
        if (mode == null) {
            throw new IllegalArgumentException("channel count " + ch + " has no image mode");
        }
        byte[] px = new byte[data.length];
        for (int i = 0; i < px.length; i++) {
            px[i] = (byte) Math.min(255L, Math.max(0L, Math.round(data[i])));
        }
        return new Image(mode, shape[1], shape[0], px);
    }

    /** The dimensions (a copy). */
    public int[] getShape() {
        return Arrays.copyOf(shape, shape.length);
    }

    /** The row-major values (a copy). */
    public double[] getData() {
        return Arrays.copyOf(data, data.length);
    }

    /** Number of dimensions. */
    public int getNdim() {
        return shape.length;
    }

    /** Number of elements. */
    public int getSize() {
        return data.length;
    }

    /** The element at {@code index}, one coordinate per dimension. */
    public double get(int... index) {
        if (index.length != shape.length) {
            throw new IllegalArgumentException("expected " + shape.length + " coordinates");
        }
        int off = 0;
        int[] st = strides(shape);
        for (int i = 0; i < index.length; i++) {
            if (index[i] < 0 || index[i] >= shape[i]) {
                throw new IndexOutOfBoundsException("index " + index[i] + " out of range for axis " + i);
            }
            off += index[i] * st[i];
        }
        return data[off];
    }

    /** Sum of all elements, computed by the native core. */
    public double sum() {
        double v = Generated.sillstone_tensor_sum_f64(data);
        if (Double.isNaN(v) && !containsNaN()) {
            throw new SillstoneFfiException("sillstone_tensor_sum_f64", -2);
        }
        return v;
    }

    private boolean containsNaN() {
        for (double v : data) {
            if (Double.isNaN(v)) {
                return true;
            }
        }
        return false;
    }

    /** New shape over the same elements; one dimension may be {@code -1}. */
    public Tensor reshape(int[] newShape) {
        int[] target = Arrays.copyOf(newShape, newShape.length);
        int infer = -1;
        long known = 1;
        for (int i = 0; i < target.length; i++) {
            if (target[i] == -1) {
                if (infer != -1) {
                    throw new IllegalArgumentException("only one dimension can be inferred");
                }
                infer = i;
            } else {
                known *= target[i];
            }
        }
        if (infer != -1) {
            if (known < 1 || data.length % known != 0) {
                throw new IllegalArgumentException("Cannot reshape with differing element count");
            }
            target[infer] = (int) (data.length / known);
        }
        if (checkShape(target) != data.length) {
            throw new IllegalArgumentException("Cannot reshape with differing element count");
        }
        return new Tensor(target, data);
    }

    /** Permutes axes; reverses them when {@code axes} is empty. */
    public Tensor transpose(int... axes) {
        int nd = shape.length;
        int[] perm = axes.length == 0 ? new int[nd] : axes;
        if (axes.length == 0) {
            for (int i = 0; i < nd; i++) {
                perm[i] = nd - 1 - i;
            }
        }
        boolean[] seen = new boolean[nd];
        if (perm.length != nd) {
            throw new IllegalArgumentException("axes are not a permutation of " + nd + " dimensions");
        }
        for (int a : perm) {
            if (a < 0 || a >= nd || seen[a]) {
                throw new IllegalArgumentException("axes are not a permutation of " + nd + " dimensions");
            }
            seen[a] = true;
        }
        int[] outShape = new int[nd];
        int[] inStr = strides(shape);
        int[] permStr = new int[nd];
        for (int i = 0; i < nd; i++) {
            outShape[i] = shape[perm[i]];
            permStr[i] = inStr[perm[i]];
        }
        double[] out = new double[data.length];
        int[] idx = new int[nd];
        int src = 0;
        for (int i = 0; i < out.length; i++) {
            out[i] = data[src];
            for (int d = nd - 1; d >= 0; d--) {
                idx[d]++;
                src += permStr[d];
                if (idx[d] < outShape[d]) {
                    break;
                }
                src -= permStr[d] * outShape[d];
                idx[d] = 0;
            }
        }
        return new Tensor(outShape, out, true);
    }

    private int[] broadcastStrides(int[] target) {
        int[] own = strides(shape);
        int lead = target.length - shape.length;
        int[] out = new int[target.length];
        for (int i = 0; i < target.length; i++) {
            out[i] = i < lead || shape[i - lead] == 1 ? 0 : own[i - lead];
        }
        return out;
    }

    private Tensor zip(Tensor rhs, DoubleBinaryOperator fn) {
        int n = Math.max(shape.length, rhs.shape.length);
        int[] out = new int[n];
        for (int i = 0; i < n; i++) {
            int da = i < n - shape.length ? 1 : shape[i - (n - shape.length)];
            int db = i < n - rhs.shape.length ? 1 : rhs.shape[i - (n - rhs.shape.length)];
            if (da != db && da != 1 && db != 1) {
                throw new IllegalArgumentException(
                    "shapes " + Arrays.toString(shape) + " and " + Arrays.toString(rhs.shape) + " cannot be broadcast together");
            }
            out[i] = Math.max(da, db);
        }
        double[] res = new double[checkShape(out)];
        int[] ls = broadcastStrides(out);
        int[] rs = rhs.broadcastStrides(out);
        int[] idx = new int[n];
        int a = 0;
        int b = 0;
        for (int i = 0; i < res.length; i++) {
            res[i] = fn.applyAsDouble(data[a], rhs.data[b]);
            for (int d = n - 1; d >= 0; d--) {
                idx[d]++;
                a += ls[d];
                b += rs[d];
                if (idx[d] < out[d]) {
                    break;
                }
                a -= ls[d] * out[d];
                b -= rs[d] * out[d];
                idx[d] = 0;
            }
        }
        return new Tensor(out, res, true);
    }

    private static Tensor scalar(double v) {
        return new Tensor(new int[] {1}, new double[] {v}, true);
    }

    /** Elementwise sum with broadcasting; equal shapes use the native kernel. */
    public Tensor add(Tensor other) {
        if (Arrays.equals(shape, other.shape)) {
            double[] out = Generated.sillstone_tensor_add_f64(data, other.data);
            if (out == null) {
                throw new SillstoneFfiException("sillstone_tensor_add_f64", -2);
            }
            return new Tensor(shape, out, true);
        }
        return zip(other, Double::sum);
    }

    /** Adds a scalar. */
    public Tensor add(double v) {
        return zip(scalar(v), Double::sum);
    }

    /** Elementwise difference with broadcasting. */
    public Tensor subtract(Tensor other) {
        return zip(other, (x, y) -> x - y);
    }

    /** Elementwise product with broadcasting. */
    public Tensor multiply(Tensor other) {
        return zip(other, (x, y) -> x * y);
    }

    /** Multiplies by a scalar. */
    public Tensor multiply(double v) {
        return zip(scalar(v), (x, y) -> x * y);
    }

    /** Elementwise quotient with broadcasting. */
    public Tensor divide(Tensor other) {
        return zip(other, (x, y) -> x / y);
    }

    /** Applies {@code fn} to every element. */
    public Tensor map(DoubleUnaryOperator fn) {
        double[] out = new double[data.length];
        for (int i = 0; i < out.length; i++) {
            out[i] = fn.applyAsDouble(data[i]);
        }
        return new Tensor(shape, out, true);
    }

    /** 2-D matrix product through the native GEMM; 1-D operands are promoted like NumPy. */
    public Tensor matmul(Tensor other) {
        if (shape.length > 2 || other.shape.length > 2) {
            throw new IllegalArgumentException("Matmul requires 1D or 2D tensors");
        }
        Tensor left = shape.length == 1 ? reshape(new int[] {1, data.length}) : this;
        Tensor right = other.shape.length == 1 ? other.reshape(new int[] {other.data.length, 1}) : other;
        int m = left.shape[0];
        int k1 = left.shape[1];
        int k2 = right.shape[0];
        int n = right.shape[1];
        if (k1 != k2) {
            throw new IllegalArgumentException("Inner matrix dimensions must agree");
        }
        double[] out = Generated.sillstone_tensor_matmul_f64(left.data, m, k1, right.data, k2, n);
        if (out == null) {
            throw new SillstoneFfiException("sillstone_tensor_matmul_f64", -2);
        }
        int[] outShape = shape.length == 1 && other.shape.length == 1 ? new int[] {1}
            : shape.length == 1 ? new int[] {n}
            : other.shape.length == 1 ? new int[] {m} : new int[] {m, n};
        return new Tensor(outShape, out, true);
    }

    /** Inner product of two 1-D tensors. */
    public double dot(Tensor other) {
        if (shape.length != 1 || other.shape.length != 1) {
            throw new IllegalArgumentException("dot needs 1D tensors; use matmul for matrices");
        }
        return matmul(other).data[0];
    }

    private interface Step {
        double apply(double acc, double x, int k);
    }

    private Tensor reduce(int axis, double init, Step step, DoubleBinaryOperator finish) {
        int ax = axis < 0 ? axis + shape.length : axis;
        if (ax < 0 || ax >= shape.length) {
            throw new IllegalArgumentException("axis " + axis + " is out of range for " + shape.length + " dimensions");
        }
        int inner = 1;
        for (int i = ax + 1; i < shape.length; i++) {
            inner *= shape[i];
        }
        int len = shape[ax];
        int outer = data.length / (inner * len);
        int[] outShape = new int[shape.length == 1 ? 1 : shape.length - 1];
        if (shape.length == 1) {
            outShape[0] = 1;
        } else {
            for (int i = 0, j = 0; i < shape.length; i++) {
                if (i != ax) {
                    outShape[j++] = shape[i];
                }
            }
        }
        double[] out = new double[outer * inner];
        for (int o = 0; o < outer; o++) {
            for (int i = 0; i < inner; i++) {
                double acc = init;
                for (int k = 0; k < len; k++) {
                    acc = step.apply(acc, data[(o * len + k) * inner + i], k);
                }
                out[o * inner + i] = finish.applyAsDouble(acc, len);
            }
        }
        return new Tensor(outShape, out, true);
    }

    /** Sum along {@code axis}. */
    public Tensor sum(int axis) {
        return reduce(axis, 0.0, (a, x, k) -> a + x, (a, n) -> a);
    }

    /** Mean along {@code axis}. */
    public Tensor mean(int axis) {
        return reduce(axis, 0.0, (a, x, k) -> a + x, (a, n) -> a / n);
    }

    /** Mean of all elements. */
    public double mean() {
        return sum() / data.length;
    }

    /** Minimum along {@code axis}. */
    public Tensor min(int axis) {
        return reduce(axis, Double.POSITIVE_INFINITY, (a, x, k) -> x < a || Double.isNaN(x) ? x : a, (a, n) -> a);
    }

    /** Maximum along {@code axis}. */
    public Tensor max(int axis) {
        return reduce(axis, Double.NEGATIVE_INFINITY, (a, x, k) -> x > a || Double.isNaN(x) ? x : a, (a, n) -> a);
    }

    /** Smallest element. */
    public double min() {
        double acc = Double.POSITIVE_INFINITY;
        for (double v : data) {
            acc = v < acc || Double.isNaN(v) ? v : acc;
        }
        return acc;
    }

    /** Largest element. */
    public double max() {
        double acc = Double.NEGATIVE_INFINITY;
        for (double v : data) {
            acc = v > acc || Double.isNaN(v) ? v : acc;
        }
        return acc;
    }

    /** Whether shapes agree and every element differs by at most {@code tol}. */
    public boolean allClose(Tensor other, double tol) {
        if (!Arrays.equals(shape, other.shape)) {
            return false;
        }
        for (int i = 0; i < data.length; i++) {
            if (!(Math.abs(data[i] - other.data[i]) <= tol)) {
                return false;
            }
        }
        return true;
    }

    @Override
    public boolean equals(Object o) {
        return o instanceof Tensor t && Arrays.equals(shape, t.shape) && Arrays.equals(data, t.data);
    }

    @Override
    public int hashCode() {
        return 31 * Arrays.hashCode(shape) + Arrays.hashCode(data);
    }

    @Override
    public String toString() {
        return "Tensor" + Arrays.toString(shape) + Arrays.toString(data);
    }
}
