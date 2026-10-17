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
}
