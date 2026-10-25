package io.github.sillstone;

import java.util.Arrays;

/**
 * Pillow-shaped 8-bit image facade over the native image engine: modes {@code L}, {@code RGB}
 * and {@code RGBA}, row-major and channel-interleaved, the same layouts the Python, Node and Go
 * SDKs use. An {@code Image} is immutable except through {@link #putPixel(int, int, int...)};
 * every operation runs in the Rust core and returns a new image.
 *
 * <p>Native refusals surface as {@link SillstoneFfiException}. The JNI layer does not carry the
 * core's status code, so the exception reports {@code -2} (core refusal) for failed operations
 * and {@code -1} for argument errors caught here.
 *
 * <p>Image hashes return the 64-bit hash as a {@code long} bit pattern; use
 * {@link Long#toUnsignedString(long)} to compare with the unsigned values Python and Node print.
 * The JNI hash entry points signal failure with {@code -1}, which is also a valid all-ones hash,
 * so a hash of exactly {@code -1L} is indistinguishable from a refusal.
 */
public final class Image {
    /** Pixel layouts the facade accepts. */
    public enum Mode {
        /** One 8-bit gray channel. */
        L(1, 0),
        /** Three 8-bit channels. */
        RGB(3, 2),
        /** Four 8-bit channels including alpha. */
        RGBA(4, 4);

        private final int channels;
        private final int layout;

        Mode(int channels, int layout) {
            this.channels = channels;
            this.layout = layout;
        }

        /** Samples per pixel. */
        public int channels() {
            return channels;
        }

        static Mode ofLayout(int layout) {
            for (Mode m : values()) {
                if (m.layout == layout) {
                    return m;
                }
            }
            throw new SillstoneFfiException("sillstone_image_qoi_decode", -2);
        }
    }

    /** Resampling filters; the codes are the ABI's. */
    public enum Resampling {
        /** Nearest neighbour. */
        NEAREST(0),
        /** Box filter. */
        BOX(1),
        /** Bilinear. */
        BILINEAR(2),
        /** Bicubic (Catmull-Rom). */
        BICUBIC(3),
        /** Lanczos-3. */
        LANCZOS(4);

        private final int code;

        Resampling(int code) {
            this.code = code;
        }
    }

    /** Flips and quarter turns. */
    public enum Transpose {
        /** Mirror horizontally. */
        FLIP_LEFT_RIGHT(0),
        /** Mirror vertically. */
        FLIP_TOP_BOTTOM(1),
        /** Rotate 90 degrees counter-clockwise. */
        ROTATE_90(2),
        /** Rotate 180 degrees. */
        ROTATE_180(3),
        /** Rotate 270 degrees counter-clockwise. */
        ROTATE_270(4),
        /** Mirror across the main diagonal. */
        TRANSPOSE(5),
        /** Mirror across the anti-diagonal. */
        TRANSVERSE(6);

        private final int code;

        Transpose(int code) {
            this.code = code;
        }

        private boolean swapsAxes() {
            return this == ROTATE_90 || this == ROTATE_270 || this == TRANSPOSE || this == TRANSVERSE;
        }
    }

    /** Convolution filters. */
    public enum ImageFilter {
        /** Blur. */
        BLUR(0),
        /** Sharpen. */
        SHARPEN(1),
        /** Contour. */
        CONTOUR(2),
        /** Detail. */
        DETAIL(3),
        /** Edge enhance. */
        EDGE_ENHANCE(4),
        /** Find edges. */
        FIND_EDGES(5),
        /** Emboss. */
        EMBOSS(6);

        private final int code;

        ImageFilter(int code) {
            this.code = code;
        }
    }

    private final Mode mode;
    private final int width;
    private final int height;
    private final byte[] data;

    /**
     * @param mode pixel layout
     * @param width width in pixels, at least 1
     * @param height height in pixels, at least 1
     * @param data interleaved samples, {@code width * height * channels} bytes (copied)
     */
    public Image(Mode mode, int width, int height, byte[] data) {
        if (mode == null || data == null) {
            throw new NullPointerException("mode and data are required");
        }
        if (width < 1 || height < 1) {
            throw new IllegalArgumentException("width and height must be at least 1");
        }
        long want = (long) width * height * mode.channels;
        if (want > Integer.MAX_VALUE - 8) {
            throw new IllegalArgumentException("image is too large");
        }
        if (data.length != want) {
            throw new IllegalArgumentException(
                "data length " + data.length + " does not match " + width + "x" + height + " " + mode + " (" + want + " bytes)");
        }
        this.mode = mode;
        this.width = width;
        this.height = height;
        this.data = Arrays.copyOf(data, data.length);
    }

    /** A new image filled with {@code color} (one value, or one per channel). */
    public static Image create(Mode mode, int width, int height, int... color) {
        if (width < 1 || height < 1) {
            throw new IllegalArgumentException("width and height must be at least 1");
        }
        int ch = mode.channels;
        byte[] px = new byte[ch];
        for (int c = 0; c < ch; c++) {
            px[c] = (byte) (color.length == 0 ? 0 : color.length == 1 ? color[0] : c < color.length ? color[c] : 0);
        }
        long total = (long) width * height * ch;
        if (total > Integer.MAX_VALUE - 8) {
            throw new IllegalArgumentException("image is too large");
        }
        byte[] out = new byte[(int) total];
        for (int i = 0; i < out.length; i += ch) {
            System.arraycopy(px, 0, out, i, ch);
        }
        return new Image(mode, width, height, out);
    }

    /** Decodes a QOI stream. */
    public static Image qoiDecode(byte[] bytes) {
        byte[] raw = Generated.sillstone_image_qoi_decode(bytes);
        if (raw == null || raw.length < 12) {
            throw new SillstoneFfiException("sillstone_image_qoi_decode", -2);
        }
        int w = leInt(raw, 0);
        int h = leInt(raw, 4);
        Mode mode = Mode.ofLayout(leInt(raw, 8));
        return new Image(mode, w, h, Arrays.copyOfRange(raw, 12, raw.length));
    }

    private static int leInt(byte[] b, int off) {
        return (b[off] & 0xff) | (b[off + 1] & 0xff) << 8 | (b[off + 2] & 0xff) << 16 | (b[off + 3] & 0xff) << 24;
    }

    /** The pixel layout. */
    public Mode getMode() {
        return mode;
    }

    /** Width in pixels. */
    public int getWidth() {
        return width;
    }

    /** Height in pixels. */
    public int getHeight() {
        return height;
    }

    /** A copy of the interleaved pixel bytes. */
    public byte[] toBytes() {
        return Arrays.copyOf(data, data.length);
    }

    private static int checked(String op, long outW, long outH, int channels) {
        long len = outW * outH * channels;
        if (outW < 1 || outH < 1 || len > Integer.MAX_VALUE - 8) {
            throw new IllegalArgumentException(op + ": output size " + outW + "x" + outH + " is out of range");
        }
        return (int) len;
    }

    private Image result(String op, byte[] out, Mode outMode, int outW, int outH) {
        if (out == null) {
            throw new SillstoneFfiException(op, -2);
        }
        return new Image(outMode, outW, outH, out);
    }

    /** Resamples to {@code outWidth x outHeight}, Pillow-exact for every filter. */
    public Image resize(int outWidth, int outHeight, Resampling resample) {
        int outLen = checked("resize", outWidth, outHeight, mode.channels);
        byte[] out = Generated.sillstone_image_resize(data, width, height, mode.layout, outWidth, outHeight, resample.code, outLen);
        return result("sillstone_image_resize", out, mode, outWidth, outHeight);
    }

    /** Crops {@code [left, upper, right, lower)}, which must lie inside the image. */
    public Image crop(int left, int upper, int right, int lower) {
        if (left < 0 || upper < 0 || right <= left || lower <= upper || right > width || lower > height) {
            throw new IllegalArgumentException("crop box lies outside the " + width + "x" + height + " image");
        }
        int ow = right - left;
        int oh = lower - upper;
        byte[] out = Generated.sillstone_image_crop(data, width, height, mode.layout, left, upper, right, lower, checked("crop", ow, oh, mode.channels));
        return result("sillstone_image_crop", out, mode, ow, oh);
    }

    /** Flips or quarter-turns the image. */
    public Image transpose(Transpose method) {
        int ow = method.swapsAxes() ? height : width;
        int oh = method.swapsAxes() ? width : height;
        byte[] out = Generated.sillstone_image_transpose(data, width, height, mode.layout, method.code, checked("transpose", ow, oh, mode.channels));
        return result("sillstone_image_transpose", out, mode, ow, oh);
    }

    /** Rotates counter-clockwise by {@code angle} degrees; {@code expand} grows the canvas to fit. */
    public Image rotate(double angle, Resampling resample, boolean expand) {
        if (Double.isNaN(angle) || Double.isInfinite(angle)) {
            throw new IllegalArgumentException("angle must be finite");
        }
        int[] dims = Generated.sillstone_image_rotate_size(width, height, angle, expand ? 1 : 0);
        if (dims == null) {
            throw new SillstoneFfiException("sillstone_image_rotate_size", -1);
        }
        byte[] out = Generated.sillstone_image_rotate(data, width, height, mode.layout, angle, expand ? 1 : 0, resample.code, checked("rotate", dims[0], dims[1], mode.channels));
        return result("sillstone_image_rotate", out, mode, dims[0], dims[1]);
    }

    /** Applies a 3x3 or 5x5 convolution filter. */
    public Image filter(ImageFilter f) {
        byte[] out = Generated.sillstone_image_filter(data, width, height, mode.layout, f.code, data.length);
        return result("sillstone_image_filter", out, mode, width, height);
    }

    /** Scales every sample by {@code factor} (1 = identity, 0 = black, above 1 brighter), clamped. */
    public Image brightness(double factor) {
        if (Double.isNaN(factor) || Double.isInfinite(factor)) {
            throw new IllegalArgumentException("factor must be finite");
        }
        byte[] out = Generated.sillstone_image_adjust(data, width, height, mode.layout, 0, factor, data.length);
        return result("sillstone_image_adjust", out, mode, width, height);
    }

    /** Inverts every sample. */
    public Image invert() {
        byte[] out = Generated.sillstone_image_adjust(data, width, height, mode.layout, 1, 0.0, data.length);
        return result("sillstone_image_adjust", out, mode, width, height);
    }

    /** Converts to {@code target}. */
    public Image convert(Mode target) {
        if (target == mode) {
            return new Image(mode, width, height, data);
        }
        byte[] out = Generated.sillstone_image_color_convert(data, width, height, mode.layout, target.layout, checked("convert", width, height, target.channels));
        return result("sillstone_image_color_convert", out, target, width, height);
    }

    /** Encodes as QOI (lossless). */
    public byte[] qoiEncode() {
        byte[] out = Generated.sillstone_image_qoi_encode(data, width, height, mode.layout);
        if (out == null) {
            throw new SillstoneFfiException("sillstone_image_qoi_encode", -2);
        }
        return out;
    }

    /** Encodes as BMP. */
    public byte[] bmpEncode() {
        byte[] out = Generated.sillstone_image_bmp_encode(data, width, height, mode.layout);
        if (out == null) {
            throw new SillstoneFfiException("sillstone_image_bmp_encode", -2);
        }
        return out;
    }

    private int offset(int x, int y) {
        if (x < 0 || y < 0 || x >= width || y >= height) {
            throw new IndexOutOfBoundsException("pixel (" + x + ", " + y + ") lies outside the " + width + "x" + height + " image");
        }
        return (y * width + x) * mode.channels;
    }

    /** The samples of the pixel at {@code (x, y)}, 0..255 each. */
    public int[] getPixel(int x, int y) {
        int off = offset(x, y);
        int[] px = new int[mode.channels];
        for (int c = 0; c < px.length; c++) {
            px[c] = data[off + c] & 0xff;
        }
        return px;
    }

    /** Sets the pixel at {@code (x, y)}: one value for every channel, or one per channel. */
    public void putPixel(int x, int y, int... value) {
        int off = offset(x, y);
        for (int c = 0; c < mode.channels; c++) {
            data[off + c] = (byte) (value.length == 0 ? 0 : value.length == 1 ? value[0] : c < value.length ? value[c] : 0);
        }
    }

    /** 64-bit perceptual hash, bit-exact with {@code imagehash.phash}. */
    public long phash() {
        return Generated.sillstone_image_phash(data, width, height, mode.layout);
    }

    /** Bit-exact with {@code imagehash.average_hash}. */
    public long ahash() {
        return Generated.sillstone_image_ahash(data, width, height, mode.layout);
    }

    /** Bit-exact with {@code imagehash.dhash}. */
    public long dhash() {
        return Generated.sillstone_image_dhash(data, width, height, mode.layout);
    }

    /** Bit-exact with {@code imagehash.whash} (Haar, default scale). */
    public long whash() {
        return Generated.sillstone_image_whash(data, width, height, mode.layout);
    }

    /** Bit-exact with {@code imagehash.colorhash} (3 bits per bin). */
    public long colorhash() {
        return Generated.sillstone_image_colorhash(data, width, height, mode.layout);
    }

    @Override
    public boolean equals(Object o) {
        return o instanceof Image other && other.mode == mode && other.width == width && other.height == height
            && Arrays.equals(other.data, data);
    }

    @Override
    public int hashCode() {
        return Arrays.hashCode(new int[] {mode.ordinal(), width, height, Arrays.hashCode(data)});
    }
}
