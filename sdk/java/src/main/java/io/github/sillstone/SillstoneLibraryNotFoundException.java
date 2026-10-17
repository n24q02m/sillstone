package io.github.sillstone;

/** No suite cdylib could be located for {@link System#load}.
 *
 * <p>Thrown by the discovery chain ({@code SILLSTONE_CDYLIB} →
 * {@code SILLSTONE_CDYLIB_DIR} → {@code target/release} of the working
 * directory or an ancestor) when none of its stops holds the platform
 * cdylib. Building the repository with {@code cargo build --release}
 * fixes the common case.
 */
public class SillstoneLibraryNotFoundException extends IllegalStateException {
    private static final long serialVersionUID = 1L;

    /** @param message what was searched and where */
    public SillstoneLibraryNotFoundException(String message) {
        super(message);
    }
}
