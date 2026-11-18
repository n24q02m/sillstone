package io.github.sillstone;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.io.File;
import java.io.IOException;
import java.lang.management.ManagementFactory;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.concurrent.TimeUnit;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

/**
 * The cdylib discovery chain of {@link SillstoneNative}, each case in a child JVM with its own environment, working
 * directory and classpath. A JaCoCo agent on this JVM is passed on to the children so their coverage is recorded too.
 */
class NativeLoaderTest {
    private static final String LIB = System.mapLibraryName("sillstone_ffi");
    private static final String PLATFORM =
        SillstoneNative.platformId(System.getProperty("os.name"), System.getProperty("os.arch"));
    private static final String NOT_FOUND = "ERR no %s found: set SILLSTONE_CDYLIB or SILLSTONE_CDYLIB_DIR, "
        + "or run `cargo build --release` in the repository";

    @TempDir
    Path tmp;

    /** Runs the probe; the working directory is {@code tmp/a/b/c}, so all four search roots lie inside {@code tmp}. */
    private String probe(Map<String, String> env, List<String> jvmArgs, Path extraClasspath, String... args)
        throws IOException, InterruptedException {
        Path cwd = Files.createDirectories(tmp.resolve("a/b/c"));
        List<String> cmd = new ArrayList<>();
        cmd.add(Paths.get(System.getProperty("java.home"), "bin", "java").toString());
        for (String arg : ManagementFactory.getRuntimeMXBean().getInputArguments()) {
            if (arg.startsWith("-javaagent:") && arg.contains("jacoco")) {
                cmd.add(arg);
            }
        }
        cmd.addAll(jvmArgs);
        String classpath = System.getProperty("java.class.path");
        if (extraClasspath != null) {
            classpath = extraClasspath + File.pathSeparator + classpath;
        }
        cmd.add("-cp");
        cmd.add(classpath);
        cmd.add(NativeLoaderProbe.class.getName());
        cmd.addAll(List.of(args));
        ProcessBuilder pb = new ProcessBuilder(cmd).directory(cwd.toFile());
        pb.environment().remove("SILLSTONE_CDYLIB");
        pb.environment().remove("SILLSTONE_CDYLIB_DIR");
        pb.environment().putAll(env);
        Path stderr = tmp.resolve("stderr.txt");
        pb.redirectError(stderr.toFile());
        Process p = pb.start();
        String out = new String(p.getInputStream().readAllBytes(), StandardCharsets.UTF_8);
        assertTrue(p.waitFor(60, TimeUnit.SECONDS), "probe timed out");
        assertEquals(0, p.exitValue(), () -> "probe failed: " + out + read(stderr));
        return out;
    }

    private static String read(Path file) {
        try {
            return Files.readString(file);
        } catch (IOException e) {
            return e.toString();
        }
    }

    private static Path touch(Path file, String content) throws IOException {
        Files.createDirectories(file.getParent());
        return Files.writeString(file, content);
    }

    private static void assertFound(Path want, String out) throws IOException {
        assertTrue(out.startsWith("OK "), out);
        assertTrue(Files.isSameFile(want, Paths.get(out.substring(3))), out + " is not " + want);
    }

    /** A classpath directory carrying a bundled native for this platform, as a release JAR does. */
    private Path bundle() throws IOException {
        Path root = tmp.resolve("bundle");
        touch(root.resolve("io/github/sillstone/native/" + PLATFORM + "/" + LIB), "bundled");
        return root;
    }

    @Test
    void explicitPathMustNameAFile() throws Exception {
        Path lib = touch(tmp.resolve("custom/any-name.bin"), "explicit");
        assertFound(lib, probe(Map.of("SILLSTONE_CDYLIB", lib.toString()), List.of(), null, "find"));

        String out = probe(Map.of("SILLSTONE_CDYLIB", tmp.resolve("absent.bin").toString()), List.of(), null, "find");
        assertTrue(out.startsWith("ERR SILLSTONE_CDYLIB does not name a file: "), out);
        assertTrue(out.endsWith("absent.bin"), out);
    }

    @Test
    void blankExplicitPathFallsThroughToADirectoryUnderAnAncestor() throws Exception {
        Path lib = touch(tmp.resolve("a/natives/" + LIB), "dir");
        assertFound(lib, probe(Map.of("SILLSTONE_CDYLIB", " ", "SILLSTONE_CDYLIB_DIR", "natives"), List.of(), null,
            "find"));

        String out = probe(Map.of("SILLSTONE_CDYLIB_DIR", "elsewhere"), List.of(), null, "find");
        assertTrue(out.startsWith("ERR no " + LIB + " under SILLSTONE_CDYLIB_DIR="), out);
        assertTrue(out.endsWith("elsewhere (or relative to any search root)"), out);
    }

    @Test
    void bundledNativeComesBeforeTheCheckoutAndIsRemovedAtExit() throws Exception {
        touch(tmp.resolve("a/b/c/target/release/" + LIB), "checkout");
        String out = probe(Map.of(), List.of(), bundle(), "find");
        assertTrue(out.startsWith("OK "), out);
        Path extracted = Paths.get(out.substring(3));
        assertEquals(LIB, extracted.getFileName().toString());
        assertTrue(extracted.getParent().getFileName().toString().startsWith("sillstone-native-"), out);
        // The probe JVM has exited: both the copy and its private directory are gone.
        assertFalse(Files.exists(extracted), extracted + " was left behind");
        assertFalse(Files.exists(extracted.getParent()), extracted.getParent() + " was left behind");
    }

    @Test
    void extractionFailureIsNamedInTheFinalError() throws Exception {
        String out = probe(Map.of(), List.of("-Djava.io.tmpdir=" + tmp.resolve("no-such-dir")), bundle(), "find");
        assertTrue(out.startsWith(String.format(NOT_FOUND, LIB) + "; could not extract bundled native /io/github/"
            + "sillstone/native/" + PLATFORM + "/" + LIB + ": java.nio.file.NoSuchFileException: "), out);
    }

    @Test
    void checkoutReleaseIsFoundFromAnAncestorAndDebugFromTheNearestRoot() throws Exception {
        Path release = touch(tmp.resolve("a/target/release/" + LIB), "release");
        assertFound(release, probe(Map.of(), List.of(), null, "find"));

        Path debug = touch(tmp.resolve("a/b/target/debug/" + LIB), "debug");
        assertFound(debug, probe(Map.of(), List.of(), null, "find"));
    }

    @Test
    void libraryNameFollowsTheOperatingSystem() throws Exception {
        String[][] cases = {
            {"Linux", "libsillstone_ffi.so", "linux"},
            {"Mac OS X", "libsillstone_ffi.dylib", "mac_os_x"},
            {"Darwin", "libsillstone_ffi.dylib", "darwin"},
            {"Windows 11", "sillstone_ffi.dll", "windows_11"},
        };
        for (String[] c : cases) {
            // No platform ships a sparc native, so nothing bundled is found and the platform is named.
            assertEquals(String.format(NOT_FOUND, c[1]) + "; unsupported platform " + c[2] + "-sparc (bundled natives:"
                + " linux-x64, linux-arm64, darwin-arm64, darwin-x64, win32-x64)",
                probe(Map.of(), List.of(), null, "find", c[0], "sparc"), c[0]);
        }
    }

    @Test
    void platformIdsMatchTheNpmPrebuilds() {
        assertEquals("darwin-arm64", SillstoneNative.platformId("Mac OS X", "aarch64"));
        assertEquals("darwin-x64", SillstoneNative.platformId("Darwin", "x86_64"));
        assertEquals("linux-arm64", SillstoneNative.platformId("Linux", "arm64"));
        assertEquals("win32-x64", SillstoneNative.platformId("Windows Server 2025", "amd64"));
        SillstoneLibraryNotFoundException e = assertThrows(SillstoneLibraryNotFoundException.class,
            () -> SillstoneNative.platformId("FreeBSD", "amd64"));
        assertTrue(e.getMessage().startsWith("unsupported platform freebsd-amd64 "), e.getMessage());
        assertTrue(e instanceof IllegalStateException);
    }

    @Test
    void repoRootIsTheNearestAncestorWithReferenceJson() throws Exception {
        String out = probe(Map.of(), List.of(), null, "root");
        assertTrue(out.startsWith("ERR no reference.json above "), out);
        assertTrue(out.endsWith("c"), out);

        touch(tmp.resolve("a/reference.json"), "{}");
        assertFound(tmp.resolve("a"), probe(Map.of(), List.of(), null, "root"));
    }

    /** The cdylib this JVM loaded: the first {@code java.library.path} entry holding it, else the discovery chain. */
    private static Path loadedLibrary() {
        for (String dir : System.getProperty("java.library.path", "").split(File.pathSeparator)) {
            Path candidate = Paths.get(dir, LIB);
            if (!dir.isEmpty() && Files.isRegularFile(candidate)) {
                return candidate.toAbsolutePath();
            }
        }
        return Paths.get(SillstoneNative.findCdylib("sillstone_ffi"));
    }

    @Test
    void generatedFallsBackToTheDiscoveryChainWhenTheLibraryPathMisses() throws Exception {
        Path lib = loadedLibrary();
        assertTrue(Files.isRegularFile(lib), "no cdylib to hand the probe: " + lib);
        String out = probe(Map.of("SILLSTONE_CDYLIB", lib.toString()),
            List.of("-Djava.library.path=" + Files.createDirectories(tmp.resolve("empty"))), null, "load");
        assertEquals("OK " + Generated.sillstone_version_major(), out);
    }
}
