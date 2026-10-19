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
}
