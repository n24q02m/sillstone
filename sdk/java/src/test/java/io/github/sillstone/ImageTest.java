package io.github.sillstone;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import io.github.sillstone.Image.ImageFilter;
import io.github.sillstone.Image.Mode;
import io.github.sillstone.Image.Resampling;
import io.github.sillstone.Image.Transpose;
import java.nio.file.Files;
import java.util.HexFormat;
import org.junit.jupiter.api.Test;

/** The image facade against the Python SDK's golden vectors ({@code tests/fixtures/facade}). */
class ImageTest {
    private static final JsonObject GOLDEN = golden();
    private static final HexFormat HEX = HexFormat.of();

    private static JsonObject golden() {
        try {
            return JsonParser.parseString(Files.readString(
                SillstoneNative.repoRoot().resolve("tests/fixtures/facade/golden.json"))).getAsJsonObject();
        } catch (Exception e) {
            throw new IllegalStateException(e);
        }
    }

    private static Image from(JsonObject rec) {
        return new Image(Mode.valueOf(rec.get("mode").getAsString()), rec.get("width").getAsInt(),
            rec.get("height").getAsInt(), HEX.parseHex(rec.get("hex").getAsString()));
    }

    private static void expect(Image actual, String name) {
        Image want = from(GOLDEN.getAsJsonObject("ops").getAsJsonObject(name));
        assertEquals(want.getMode(), actual.getMode(), name + " mode");
        assertEquals(want.getWidth(), actual.getWidth(), name + " width");
        assertEquals(want.getHeight(), actual.getHeight(), name + " height");
        assertArrayEquals(want.toBytes(), actual.toBytes(), name);
    }

    private final Image base = from(GOLDEN.getAsJsonObject("image"));
    private final Image rgba = from(GOLDEN.getAsJsonObject("image_rgba"));

    @Test
    void resizeMatchesPythonForEveryFilter() {
        for (Resampling r : Resampling.values()) {
            expect(base.resize(5, 4, r), "resize_" + r.name().toLowerCase() + "_5x4");
        }
        expect(base.resize(14, 11, Resampling.BICUBIC), "resize_bicubic_14x11");
        expect(rgba.resize(3, 8, Resampling.BILINEAR), "resize_rgba_bilinear_3x8");
    }

    @Test
    void geometryFilterAndColorOpsMatchPython() {
        expect(base.crop(2, 1, 8, 6), "crop_2_1_8_6");
        for (Transpose t : Transpose.values()) {
            expect(base.transpose(t), "transpose_" + t.name().toLowerCase());
        }
        expect(base.rotate(30, Resampling.BILINEAR, false), "rotate_30_bilinear");
        expect(base.rotate(30, Resampling.BILINEAR, true), "rotate_30_bilinear_expand");
        expect(base.rotate(45, Resampling.NEAREST, true), "rotate_45_nearest_expand");
        expect(base.rotate(90, Resampling.BILINEAR, false), "rotate_90");
        expect(base.rotate(-20, Resampling.NEAREST, false), "rotate_neg20_nearest");
        for (ImageFilter f : ImageFilter.values()) {
            expect(base.filter(f), "filter_" + f.name().toLowerCase());
        }
        expect(base.convert(Mode.L), "convert_rgb_to_l");
        expect(base.convert(Mode.RGBA), "convert_rgb_to_rgba");
        expect(rgba.convert(Mode.RGB), "convert_rgba_to_rgb");
        expect(base.brightness(1.5), "brightness_1_5");
        expect(base.invert(), "invert");
    }

    @Test
    void encodersAreByteIdenticalAndQoiRoundTrips() {
        assertEquals(GOLDEN.get("qoi_rgb").getAsString(), HEX.formatHex(base.qoiEncode()));
        assertEquals(GOLDEN.get("bmp_rgb").getAsString(), HEX.formatHex(base.bmpEncode()));
        assertEquals(GOLDEN.get("qoi_rgba").getAsString(), HEX.formatHex(rgba.qoiEncode()));
        Image back = Image.qoiDecode(HEX.parseHex(GOLDEN.get("qoi_rgba").getAsString()));
        assertEquals(rgba, back);
        assertThrows(SillstoneFfiException.class, () -> Image.qoiDecode(new byte[] {1, 2, 3}));
    }

    @Test
    void hashesEqualTheImagehashExactPythonValues() {
        JsonObject h = GOLDEN.getAsJsonObject("hashes");
        assertEquals(h.get("phash").getAsString(), Long.toUnsignedString(base.phash()));
        assertEquals(h.get("ahash").getAsString(), Long.toUnsignedString(base.ahash()));
        assertEquals(h.get("dhash").getAsString(), Long.toUnsignedString(base.dhash()));
        assertEquals(h.get("whash").getAsString(), Long.toUnsignedString(base.whash()));
        assertEquals(h.get("colorhash").getAsString(), Long.toUnsignedString(base.colorhash()));
    }

    @Test
    void nonSquareQuarterTurnSwapsDimensions() {
        Image wide = Image.create(Mode.RGB, 6, 2, 10, 20, 30);
        Image turned = wide.rotate(90, Resampling.BILINEAR, false);
        assertEquals(2, turned.getWidth());
        assertEquals(6, turned.getHeight());
    }

    @Test
    void inputsAreValidated() {
        assertThrows(IllegalArgumentException.class, () -> new Image(Mode.RGB, 2, 2, new byte[11]));
        assertThrows(IllegalArgumentException.class, () -> base.crop(0, 0, 10, 7));
        assertThrows(IllegalArgumentException.class, () -> base.resize(0, 4, Resampling.BOX));
        assertThrows(IndexOutOfBoundsException.class, () -> base.getPixel(9, 0));
        Image img = Image.create(Mode.L, 2, 2, 7);
        img.putPixel(1, 1, 200);
        assertArrayEquals(new int[] {200}, img.getPixel(1, 1));
        assertArrayEquals(new int[] {7}, img.getPixel(0, 0));
        assertTrue(Image.create(Mode.RGBA, 1, 1, 1, 2, 3, 4).getPixel(0, 0)[3] == 4);
    }
}
