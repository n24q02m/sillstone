package io.github.sillstone;

import org.junit.jupiter.api.Test;
import static org.junit.jupiter.api.Assertions.*;

public class ChecksumTest {
    @Test
    public void testVersion() {
        assertEquals(0, Generated.sillstone_version_major());
        assertEquals(2, Generated.sillstone_version_minor());
        assertEquals(0, Generated.sillstone_version_patch());
    }

    @Test
    public void testChecksum() {
        byte[] input = new byte[32];
        for (int i = 0; i < input.length; i++) {
            input[i] = (byte) i;
        }
        long sum = Generated.sillstone_buffer_checksum(input, input.length);
        assertEquals(0xe6cb594c1a148ac5L, sum);
    }

    @Test
    public void testDigests() {
        byte[] empty = new byte[0];
        byte[] b3 = Generated.sillstone_digest_blake3(empty);
        assertNotNull(b3);
        assertEquals(32, b3.length);
        // Check prefix 0xaf, 0x13
        assertEquals((byte) 0xaf, b3[0]);
        assertEquals((byte) 0x13, b3[1]);

        byte[] abc = "abc".getBytes(java.nio.charset.StandardCharsets.UTF_8);
        byte[] s256 = Generated.sillstone_digest_sha256(abc);
        assertNotNull(s256);
        assertEquals(32, s256.length);
        assertEquals((byte) 0xba, s256[0]);
        assertEquals((byte) 0x78, s256[1]);
    }

    @Test
    public void testDst() {
        double[] x = new double[] { 1.0, 2.0, 3.0, 4.0 };
        double[] d1 = Generated.sillstone_math_dst1(x);
        assertNotNull(d1);
        assertEquals(4, d1.length);
        double[] rec1 = Generated.sillstone_math_dst1(d1);
        assertNotNull(rec1);
        for (int i = 0; i < x.length; i++) {
            assertEquals(x[i], rec1[i], 1e-10);
        }

        double[] d2 = Generated.sillstone_math_dst2(x);
        assertNotNull(d2);
        double[] rec2 = Generated.sillstone_math_dst3(d2);
        assertNotNull(rec2);
        for (int i = 0; i < x.length; i++) {
            assertEquals(x[i], rec2[i], 1e-10);
        }
    }

    @Test
    public void testUnicode() throws Exception {
        String fam = "👨‍👩‍👧‍👦";
        long cnt = Generated.sillstone_unicode_grapheme_count(fam.getBytes(java.nio.charset.StandardCharsets.UTF_8));
        assertEquals(1, cnt);
        long hello = Generated.sillstone_unicode_grapheme_count("Hello".getBytes(java.nio.charset.StandardCharsets.UTF_8));
        assertEquals(5, hello);
        int cat = Generated.sillstone_unicode_char_category((int)'A');
        assertTrue(cat >= 0);
    }
}
