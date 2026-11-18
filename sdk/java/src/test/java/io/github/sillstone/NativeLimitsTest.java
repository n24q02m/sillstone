package io.github.sillstone;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;

import io.github.sillstone.Image.Mode;
import io.github.sillstone.Image.Resampling;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.function.Executable;

/**
 * Sizes the native layer cannot serve are refused by the facades before the JNI call, so nothing is
 * copied or allocated for them: an image over the core's 1 GiB raster ceiling, and a matmul result
 * no Java array can hold.
 */
class NativeLimitsTest {
    /** 32768 x 32768 one-byte samples is exactly the core's 1 GiB ceiling. */
    private static final int SIDE = 1 << 15;

    private static String refusal(Executable call) {
        return assertThrows(IllegalArgumentException.class, call).getMessage();
    }

    @Test
    void imagesOverTheNativeRasterCeilingAreRefused() {
        // One row over 1 GiB: refused before the data length is looked at.
        assertEquals("image is too large", refusal(() -> new Image(Mode.L, SIDE, SIDE + 1, new byte[1])));
        // 16384 x 16385 RGBA is 1 GiB + 64 KiB: the ceiling counts bytes, so channels count too.
        assertEquals("image is too large", refusal(() -> new Image(Mode.RGBA, SIDE / 2, SIDE / 2 + 1, new byte[1])));
        // Exactly 1 GiB is within the ceiling, so the length check is the one that fails.
        assertEquals("data length 1 does not match 32768x32768 L (1073741824 bytes)",
            refusal(() -> new Image(Mode.L, SIDE, SIDE, new byte[1])));
        // create refuses before it allocates the fill buffer.
        assertEquals("image is too large", refusal(() -> Image.create(Mode.L, SIDE, SIDE + 1)));
    }

    @Test
    void anOutputOverTheCeilingIsRefusedBeforeTheNativeCall() {
        Image pixel = Image.create(Mode.L, 1, 1, 9);
        assertEquals("resize: output size 32768x32769 is out of range",
            refusal(() -> pixel.resize(SIDE, SIDE + 1, Resampling.NEAREST)));
        // 18919 x 18919 RGB is 1073785683 bytes: over the ceiling only once the channels are counted.
        assertEquals("resize: output size 18919x18919 is out of range",
            refusal(() -> Image.create(Mode.RGB, 1, 1).resize(18919, 18919, Resampling.NEAREST)));
    }

    @Test
    void matmulRefusesAResultNoJavaArrayCanHold() {
        // [2^20, 1] x [1, 2^20] asks for 2^40 doubles; each operand is only 8 MiB.
        Tensor column = Tensor.ones(1 << 20, 1);
        Tensor row = Tensor.ones(1, 1 << 20);
        assertEquals("matmul result 1048576x1048576 is too large for a Java array",
            refusal(() -> column.matmul(row)));
        // 46341 x 46341 is 2147488281 elements, just over the largest Java array.
        assertEquals("matmul result 46341x46341 is too large for a Java array",
            refusal(() -> Tensor.ones(46341, 1).matmul(Tensor.ones(1, 46341))));
    }
}
