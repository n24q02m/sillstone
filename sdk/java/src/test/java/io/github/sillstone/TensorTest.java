package io.github.sillstone;

import org.junit.jupiter.api.Test;
import static org.junit.jupiter.api.Assertions.*;

public class TensorTest {
    @Test
    public void testTensorBasicsAndMatmul() {
        Tensor t = new Tensor(new int[]{2, 3}, new double[]{1, 2, 3, 4, 5, 6});
        assertEquals(2, t.getNdim());
        assertEquals(6, t.getSize());
        assertEquals(21.0, t.sum(), 1e-9);

        Tensor b = new Tensor(new int[]{3, 2}, new double[]{7, 8, 9, 1, 2, 3});
        Tensor c = t.matmul(b);
        assertArrayEquals(new int[]{2, 2}, c.getShape());
        assertArrayEquals(new double[]{31, 19, 85, 55}, c.getData(), 1e-9);
    }

    private static final com.google.gson.JsonObject GOLDEN = golden();

    private static com.google.gson.JsonObject golden() {
        try {
            return com.google.gson.JsonParser.parseString(java.nio.file.Files.readString(
                SillstoneNative.repoRoot().resolve("tests/fixtures/facade/golden.json"))).getAsJsonObject();
        } catch (Exception e) {
            throw new IllegalStateException(e);
        }
    }

    private static double[] doubles(com.google.gson.JsonArray array) {
        double[] out = new double[array.size()];
        for (int i = 0; i < out.length; i++) {
            out[i] = array.get(i).getAsDouble();
        }
        return out;
    }

    private static Tensor tensor(String key) {
        com.google.gson.JsonObject rec = GOLDEN.getAsJsonObject("tensor").getAsJsonObject(key);
        com.google.gson.JsonArray shape = rec.getAsJsonArray("shape");
        return new Tensor(new int[] {shape.get(0).getAsInt(), shape.get(1).getAsInt()}, doubles(rec.getAsJsonArray("data")));
    }

    @Test
    public void matmulAddAndSumMatchThePythonSdk() {
        Tensor a = tensor("a");
        Tensor product = a.matmul(tensor("b"));
        assertArrayEquals(new int[] {3, 2}, product.getShape());
        assertArrayEquals(doubles(GOLDEN.getAsJsonObject("tensor").getAsJsonArray("matmul")), product.getData(), 1e-9);
        assertArrayEquals(doubles(GOLDEN.getAsJsonObject("tensor").getAsJsonArray("add")), a.add(tensor("a2")).getData(), 1e-9);
        assertEquals(GOLDEN.getAsJsonObject("tensor").get("sum").getAsDouble(), a.sum(), 1e-9);
    }

    @Test
    public void broadcastingTransposeAndReductions() {
        Tensor m = new Tensor(new int[] {2, 3}, new double[] {1, 2, 3, 4, 5, 6});
        assertArrayEquals(new double[] {11, 22, 33, 14, 25, 36},
            m.add(new Tensor(new int[] {3}, new double[] {10, 20, 30})).getData(), 0);
        assertArrayEquals(new double[] {1, 4, 2, 5, 3, 6}, m.transpose().getData(), 0);
        assertArrayEquals(new double[] {5, 7, 9}, m.sum(0).getData(), 0);
        assertArrayEquals(new double[] {2, 5}, m.mean(1).getData(), 1e-12);
        assertEquals(6.0, m.max(), 0);
        assertEquals(12.0, Tensor.arange(0, 24, 1).reshape(new int[] {2, 3, 4}).transpose(1, 0, 2).getData()[4], 0);
        Tensor v = new Tensor(new int[] {3}, new double[] {1, 2, 3});
        assertEquals(14.0, v.dot(v), 0);
        assertThrows(IllegalArgumentException.class, () -> m.add(new Tensor(new int[] {2}, new double[] {1, 2})));
        assertThrows(IllegalArgumentException.class, () -> m.matmul(m));
    }

    @Test
    public void emptyAndMismatchedShapesAreRejectedAndImagesRoundTrip() {
        assertThrows(IllegalArgumentException.class, () -> new Tensor(new int[] {0, 3}, new double[0]));
        assertThrows(IllegalArgumentException.class, () -> new Tensor(new int[] {2}, new double[] {1, 2, 3}));
        Image img = Image.create(Image.Mode.RGB, 3, 2, 1, 2, 3);
        Tensor t = Tensor.fromImage(img);
        assertArrayEquals(new int[] {2, 3, 3}, t.getShape());
        assertEquals(img, t.toImage());
    }
}
