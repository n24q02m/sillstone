package io.github.sillstone;

import org.junit.jupiter.api.Test;
import static org.junit.jupiter.api.Assertions.*;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;

/** The pre-rename names stay usable: each deprecated alias resolves to its canonical twin. */
public class DeprecatedAliasTest {
    @Test
    @SuppressWarnings("deprecation")
    public void statusAliasesEqualTheCanonicalConstants() {
        assertEquals(0, SillstoneHash.SILLSTONE_OK);
        assertEquals(-1, SillstoneHash.SILLSTONE_E_INVALID);
        assertEquals(-2, SillstoneHash.SILLSTONE_E_REJECTED);
        assertEquals(SillstoneHash.SILLSTONE_OK, SillstoneHash.PITH_OK);
        assertEquals(SillstoneHash.SILLSTONE_E_INVALID, SillstoneHash.PITH_E_INVALID);
        assertEquals(SillstoneHash.SILLSTONE_E_REJECTED, SillstoneHash.PITH_E_REJECTED);
    }

    @Test
    public void bundledNativeResolvesUnderTheCanonicalRoot() throws java.io.IOException {
        String path = SillstoneNative.extractBundled("test-canonical", "probe.bin");
        assertNotNull(path);
        assertEquals("sillstone canonical native probe\n",
                Files.readString(Path.of(path), StandardCharsets.UTF_8));
    }

    @Test
    public void bundledNativeFallsBackToTheLegacyRoot() throws java.io.IOException {
        String path = SillstoneNative.extractBundled("test-legacy", "probe.bin");
        assertNotNull(path);
        assertEquals("sillstone legacy native probe\n",
                Files.readString(Path.of(path), StandardCharsets.UTF_8));
    }

    @Test
    public void missingBundledNativeIsNull() {
        assertNull(SillstoneNative.extractBundled("test-missing", "probe.bin"));
    }
}
