package io.github.sillstone;

import java.util.Arrays;

/**
 * N-dimensional Tensor for multi-dimensional data array computing.
 */
public class Tensor {
    private final int[] shape;
    private final double[] data;

    public Tensor(int[] shape, double[] data) {
        int size = 1;
        for (int dim : shape) {
            size *= dim;
        }
        if (data.length != size) {
            throw new IllegalArgumentException("Data length does not match shape dimensions");
        }
        this.shape = Arrays.copyOf(shape, shape.length);
        this.data = Arrays.copyOf(data, data.length);
    }

    public int[] getShape() {
        return Arrays.copyOf(shape, shape.length);
    }

    public double[] getData() {
        return Arrays.copyOf(data, data.length);
    }

    public int getNdim() {
        return shape.length;
    }

    public int getSize() {
        return data.length;
    }

    public double sum() {
        double acc = 0.0;
        for (double v : data) {
            acc += v;
        }
        return acc;
    }

    public Tensor reshape(int[] newShape) {
        int newSize = 1;
        for (int dim : newShape) {
            newSize *= dim;
        }
        if (newSize != data.length) {
            throw new IllegalArgumentException("Cannot reshape with differing element count");
        }
        return new Tensor(newShape, data);
    }

    public Tensor matmul(Tensor other) {
        if (getNdim() != 2 || other.getNdim() != 2) {
            throw new IllegalArgumentException("Matmul requires 2D tensors");
        }
        int m = shape[0];
        int k1 = shape[1];
        int k2 = other.shape[0];
        int n = other.shape[1];
        if (k1 != k2) {
            throw new IllegalArgumentException("Inner matrix dimensions must agree");
        }

        double[] out = new double[m * n];
        for (int i = 0; i < m; i++) {
            for (int j = 0; j < n; j++) {
                double acc = 0.0;
                for (int p = 0; p < k1; p++) {
                    acc += data[i * k1 + p] * other.data[p * n + j];
                }
                out[i * n + j] = acc;
            }
        }
        return new Tensor(new int[]{m, n}, out);
    }
}
