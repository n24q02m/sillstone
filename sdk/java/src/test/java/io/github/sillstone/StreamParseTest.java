package io.github.sillstone;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.nio.ByteBuffer;
import java.util.Arrays;
import org.junit.jupiter.api.Test;

/** The canonical stream parsers on hand-built streams: the fold accessors and the malformed-stream errors. */
class StreamParseTest {
    private static byte[] range(int from, int count) {
        byte[] out = new byte[count];
        for (int i = 0; i < count; i++) {
            out[i] = (byte) (from + i);
        }
        return out;
    }

    @Test
    void textWordsFoldBackLittleEndian() {
        byte[] wire = ByteBuffer.allocate(25).put((byte) SillstoneHash.MATCH_TEXT).putLong(16)
            .putLong(0x0102030405060708L).putLong(0x1112131415161718L).array();
        SillstoneHash.Signature sig = SillstoneHash.Signature.parse(wire);
        assertEquals(16, sig.wordCount);
        assertArrayEquals(new long[] {0x0102030405060708L, 0x1112131415161718L}, sig.words);
        assertArrayEquals(new byte[] {8, 7, 6, 5, 4, 3, 2, 1, 0x18, 0x17, 0x16, 0x15, 0x14, 0x13, 0x12, 0x11},
            sig.wordsLeFold());
    }

    @Test
    void videoFoldsReproduceTheirWireBytes() {
        byte[] frames = range(0x11, 8);
        byte[] minhash = range(0x21, 16);
        byte[] wire = ByteBuffer.allocate(21 + frames.length + minhash.length).put((byte) SillstoneHash.MATCH_VIDEO)
            .putInt(frames.length).putInt(64).putInt(48).putLong(Double.doubleToLongBits(1.5))
            .put(frames).put(minhash).array();
        SillstoneHash.Signature sig = SillstoneHash.Signature.parse(wire);
        assertEquals(8, sig.frameCount);
        assertEquals(64, sig.width);
        assertEquals(48, sig.height);
        assertEquals(1.5, Double.longBitsToDouble(sig.durationBits), 0);
        assertEquals(1, sig.frameHashes.length);
        assertEquals(2, sig.minhash.length);
        assertArrayEquals(frames, sig.framesLeFold());
        assertArrayEquals(minhash, sig.minhashLeFold());
    }

    @Test
    void unknownSignatureTagIsRejected() {
        IllegalArgumentException e = assertThrows(IllegalArgumentException.class,
            () -> SillstoneHash.Signature.parse(new byte[] {9}));
        assertEquals("unknown signature tag 9", e.getMessage());
    }

    @Test
    void unknownModalityIsRejectedAfterTheSignature() {
        byte[] imageSignature = ByteBuffer.allocate(9).put((byte) SillstoneHash.MATCH_IMAGE).putLong(42).array();
        byte[] wire = ByteBuffer.allocate(2 + 32 + imageSignature.length + 16)
            .put((byte) SillstoneHash.FORMAT_PNG).put((byte) 7).put(new byte[32]).put(imageSignature)
            .put(new byte[16]).array();
        IllegalArgumentException e = assertThrows(IllegalArgumentException.class,
            () -> SillstoneHash.Description.parse(wire));
        assertEquals("unknown modality 7", e.getMessage());
    }

    @Test
    void matchWireMustCarrySixLongs() {
        IllegalArgumentException e = assertThrows(IllegalArgumentException.class,
            () -> SillstoneHash.MatchResult.parse(new long[5]));
        assertEquals("match wire must carry 6 longs", e.getMessage());
        SillstoneHash.MatchResult r = SillstoneHash.MatchResult.parse(new long[] {3, -1, 2, 3, 4, 1});
        assertEquals(SillstoneHash.MATCH_BINARY, r.tag);
        assertArrayEquals(new long[] {-1, 2, 3, 4}, r.slots);
        assertTrue(r.matched);
        assertEquals(4, Arrays.stream(r.slots).count());
    }
}
