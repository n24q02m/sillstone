package io.github.sillstone;

/**
 * Child-process entry point for {@link NativeLoaderTest}. The loader reads the environment, the working directory and
 * {@code java.io.tmpdir}, which a running JVM cannot change, so each case runs in its own JVM.
 *
 * <p>Arguments: {@code find|root|load [os.name os.arch]}. Prints {@code OK <result>} or {@code ERR <message>}.
 */
public final class NativeLoaderProbe {
    private NativeLoaderProbe() {
    }

    public static void main(String[] args) {
        if (args.length == 3) {
            System.setProperty("os.name", args[1]);
            System.setProperty("os.arch", args[2]);
        }
        try {
            switch (args[0]) {
                case "find":
                    System.out.print("OK " + SillstoneNative.findCdylib("sillstone_ffi"));
                    break;
                case "root":
                    System.out.print("OK " + SillstoneNative.repoRoot());
                    break;
                case "load":
                    System.out.print("OK " + Generated.sillstone_version_major());
                    break;
                default:
                    throw new IllegalArgumentException("unknown probe command " + args[0]);
            }
        } catch (SillstoneLibraryNotFoundException e) {
            System.out.print("ERR " + e.getMessage());
        }
    }
}
