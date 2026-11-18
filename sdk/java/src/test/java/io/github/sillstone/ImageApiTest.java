package io.github.sillstone;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotEquals;
import static org.junit.jupiter.api.Assertions.assertNotSame;
import static org.junit.jupiter.api.Assertions.assertThrows;

import io.github.sillstone.Image.Mode;
import io.github.sillstone.Image.Resampling;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.function.Executable;

/** The image facade's own checks, fill rules and value semantics, and the native refusals it reports. */
class ImageApiTest {
    private static void rejects(String message, Executable call) {
        assertEquals(message, assertThrows(IllegalArgumentException.class, call).getMessage());
    }

    @Test
    void constructorChecksEveryArgument() {
        assertEquals("mode and data are required",
            assertThrows(NullPointerException.class, () -> new Image(null, 1, 1, new byte[1])).getMessage());
        assertEquals("mode and data are required",
            assertThrows(NullPointerException.class, () -> new Image(Mode.L, 1, 1, null)).getMessage());
        rejects("width and height must be at least 1", () -> new Image(Mode.L, 0, 1, new byte[0]));
        rejects("width and height must be at least 1", () -> new Image(Mode.L, 1, -1, new byte[0]));
        // 65536 x 32768 RGBA is 8 GiB: refused before the data length is even looked at.
        rejects("image is too large", () -> new Image(Mode.RGBA, 65536, 32768, new byte[0]));
        rejects("data length 11 does not match 2x2 RGB (12 bytes)", () -> new Image(Mode.RGB, 2, 2, new byte[11]));
    }

    @Test
    void createAndPutPixelFillMissingChannelsWithZero() {
        rejects("width and height must be at least 1", () -> Image.create(Mode.L, 0, 1));
        rejects("image is too large", () -> Image.create(Mode.RGBA, 65536, 32768));
        assertArrayEquals(new int[] {0, 0, 0}, Image.create(Mode.RGB, 1, 1).getPixel(0, 0));
        assertArrayEquals(new int[] {9, 9, 9, 9}, Image.create(Mode.RGBA, 1, 1, 9).getPixel(0, 0));
        assertArrayEquals(new int[] {1, 2, 0, 0}, Image.create(Mode.RGBA, 1, 1, 1, 2).getPixel(0, 0));

        Image img = Image.create(Mode.RGB, 2, 1, 7, 7, 7);
        img.putPixel(0, 0);
        img.putPixel(1, 0, 5, 6);
        assertArrayEquals(new int[] {0, 0, 0}, img.getPixel(0, 0));
        assertArrayEquals(new int[] {5, 6, 0}, img.getPixel(1, 0));
        assertEquals("pixel (0, -1) lies outside the 2x1 image",
            assertThrows(IndexOutOfBoundsException.class, () -> img.putPixel(0, -1, 1)).getMessage());
    }

    @Test
    void nonFiniteRotationAndBrightnessAreRejected() {
        Image img = Image.create(Mode.RGB, 2, 2, 1, 2, 3);
        for (double bad : new double[] {Double.NaN, Double.POSITIVE_INFINITY, Double.NEGATIVE_INFINITY}) {
            rejects("angle must be finite", () -> img.rotate(bad, Resampling.NEAREST, false));
            rejects("factor must be finite", () -> img.brightness(bad));
        }
    }

    @Test
    void sameModeConvertIsAnIndependentCopy() {
        Image img = Image.create(Mode.RGB, 2, 1, 10, 20, 30);
        Image copy = img.convert(Mode.RGB);
        assertEquals(img, copy);
        assertNotSame(img, copy);
        copy.putPixel(0, 0, 0);
        assertArrayEquals(new int[] {10, 20, 30}, img.getPixel(0, 0));
    }

    @Test
    void grayImagesHaveNoQoiOrBmpEncoding() {
        // The core encodes QOI and BMP from RGB and RGBA only; the JNI layer reports every refusal as -2.
        Image gray = Image.create(Mode.L, 2, 2, 7);
        SillstoneFfiException qoi = assertThrows(SillstoneFfiException.class, gray::qoiEncode);
        assertEquals("sillstone_image_qoi_encode", qoi.getOp());
        assertEquals(SillstoneHash.SILLSTONE_E_REJECTED, qoi.getStatus());
        assertEquals("sillstone_image_qoi_encode failed with status -2", qoi.getMessage());
        SillstoneFfiException bmp = assertThrows(SillstoneFfiException.class, gray::bmpEncode);
        assertEquals("sillstone_image_bmp_encode", bmp.getOp());
        assertEquals(SillstoneHash.SILLSTONE_E_REJECTED, bmp.getStatus());
    }

    @Test
    void equalImagesHashAlikeAndEveryFieldCounts() {
        Image a = Image.create(Mode.RGB, 2, 1, 1, 2, 3);
        Image b = Image.create(Mode.RGB, 2, 1, 1, 2, 3);
        assertEquals(a, b);
        assertEquals(a.hashCode(), b.hashCode());
        assertNotEquals(a, Image.create(Mode.RGB, 1, 2, 1, 2, 3));
        assertNotEquals(a, Image.create(Mode.RGB, 2, 1, 1, 2, 4));
        assertNotEquals(Image.create(Mode.L, 3, 1), Image.create(Mode.RGB, 1, 1));
        assertNotEquals(a, (Object) "image");
    }
}
