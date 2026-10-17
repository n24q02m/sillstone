//! The C ABI surface of `sillstone-image`: the entry points the Python
//! (ctypes), Node (koffi) and Go (cgo) SDKs bind through.
//!
//! The suite's FFI convention, defined by this module and mirrored by
//! every `pith-*` cdylib:
//!
//! * one flat set of `#[no_mangle] pub unsafe extern "C"` functions —
//!   raw pointers plus lengths, no structs across the boundary;
//! * every function returns a status code (see the constants below),
//!   never a `Result`, never a panic: a `panic = "abort"` cdylib must
//!   not be reachable from a foreign caller;
//! * an operation either hands ownership to the caller (and ships a
//!   matching `_free`) or writes into caller-provided out-parameters —
//!   [`sillstone_image_phash`] allocates nothing, so there is no free.
//! * the `unsafe` allowance is confined to this module; every core
//!   crate stays unsafe-free behind the crate-root `#![deny]`.
//!
//! Layout codes follow the declaration order of
//! [`RawLayout`](crate::reference::RawLayout) — the same "declaration
//! order is the wire code" rule the `Pixels` variant tag uses.

#![allow(unsafe_code)]

use crate::ahash::image_ahash;
use crate::colorhash::image_colorhash;
use crate::dhash::image_dhash;
use crate::phash::image_phash;
use crate::raster::{Gray, Image, Layout, Rgb, Rgba};
use crate::whash::image_whash;
use sillstone_digest::Error;

/// Status: success.
pub const PITH_OK: i32 = 0;
/// Status: a caller argument is invalid — a null pointer, an unknown
/// layout code, or a `len` that does not match the declared geometry.
pub const PITH_E_INVALID: i32 = -1;
/// Status: the core pipeline refused the input. With a well-formed
/// [`Image`] this is unreachable; the code exists so a foreign caller
/// never has to reason about a panic.
pub const PITH_E_REJECTED: i32 = -2;

/// Layout code: one `u8` luma sample per pixel.
pub const PITH_LAYOUT_GRAY8: u32 = 0;
/// Layout code: one `u16` luma sample per pixel, little-endian.
pub const PITH_LAYOUT_GRAY16: u32 = 1;
/// Layout code: three `u8` samples per pixel, `R G B`.
pub const PITH_LAYOUT_RGB8: u32 = 2;
/// Layout code: three `u16` samples per pixel, `R G B`, little-endian.
pub const PITH_LAYOUT_RGB16: u32 = 3;
/// Layout code: four `u8` samples per pixel, `R G B A` (alpha ignored
/// by the pHash luma step).
pub const PITH_LAYOUT_RGBA8: u32 = 4;

/// Computes the 64-bit perceptual hash of a raw pixel dump.
///
/// `data` points at `len` bytes of a flat, row-major,
/// channel-interleaved dump of a `width`×`height` image — exactly the
/// format the fixtures under `tests/fixtures/phash/` commit (little-
/// endian for the 16-bit layouts). `layout` is one of the
/// `PITH_LAYOUT_*` codes above. On success writes the hash through
/// `out` and returns [`PITH_OK`]; the hash is also the return value of
/// no error path, so `out` is only meaningful with `PITH_OK`.
///
/// # Safety
///
/// `data` must point to `len` readable bytes, and `out` to one
/// writable `u64`; both must stay valid for the duration of the call.
/// The function does not retain either pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sillstone_image_phash(
    data: *const u8,
    len: usize,
    width: u32,
    height: u32,
    layout: u32,
    out: *mut u64,
) -> i32 {
    if data.is_null() || out.is_null() {
        return PITH_E_INVALID;
    }
    let dump = unsafe { core::slice::from_raw_parts(data, len) };
    match phash_of_dump(dump, width, height, layout) {
        Ok(hash) => {
            unsafe { *out = hash };
            PITH_OK
        }
        Err(status) => status,
    }
}

/// Computes the 64-bit Average Hash (aHash) of a raw pixel dump.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sillstone_image_ahash(
    data: *const u8,
    len: usize,
    width: u32,
    height: u32,
    layout: u32,
    out: *mut u64,
) -> i32 {
    if data.is_null() || out.is_null() {
        return PITH_E_INVALID;
    }
    let dump = unsafe { core::slice::from_raw_parts(data, len) };
    match ahash_of_dump(dump, width, height, layout) {
        Ok(hash) => {
            unsafe { *out = hash };
            PITH_OK
        }
        Err(status) => status,
    }
}

/// Computes the 64-bit Difference Hash (dHash) of a raw pixel dump.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sillstone_image_dhash(
    data: *const u8,
    len: usize,
    width: u32,
    height: u32,
    layout: u32,
    out: *mut u64,
) -> i32 {
    if data.is_null() || out.is_null() {
        return PITH_E_INVALID;
    }
    let dump = unsafe { core::slice::from_raw_parts(data, len) };
    match dhash_of_dump(dump, width, height, layout) {
        Ok(hash) => {
            unsafe { *out = hash };
            PITH_OK
        }
        Err(status) => status,
    }
}

/// Computes the 64-bit Wavelet Hash (wHash) of a raw pixel dump.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sillstone_image_whash(
    data: *const u8,
    len: usize,
    width: u32,
    height: u32,
    layout: u32,
    level: u32,
    out: *mut u64,
) -> i32 {
    if data.is_null() || out.is_null() {
        return PITH_E_INVALID;
    }
    let dump = unsafe { core::slice::from_raw_parts(data, len) };
    match whash_of_dump(dump, width, height, layout, level) {
        Ok(hash) => {
            unsafe { *out = hash };
            PITH_OK
        }
        Err(status) => status,
    }
}

/// Computes the 64-bit Color Hash of a raw pixel dump.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sillstone_image_colorhash(
    data: *const u8,
    len: usize,
    width: u32,
    height: u32,
    layout: u32,
    bins: u32,
    out: *mut u64,
) -> i32 {
    if data.is_null() || out.is_null() {
        return PITH_E_INVALID;
    }
    let dump = unsafe { core::slice::from_raw_parts(data, len) };
    match colorhash_of_dump(dump, width, height, layout, bins) {
        Ok(hash) => {
            unsafe { *out = hash };
            PITH_OK
        }
        Err(status) => status,
    }
}

fn ahash_of_dump(dump: &[u8], width: u32, height: u32, layout: u32) -> Result<u64, i32> {
    match layout {
        PITH_LAYOUT_GRAY8 => hash_8::<Gray, _>(dump, width, height, 1, image_ahash),
        PITH_LAYOUT_GRAY16 => hash_16::<Gray, _>(dump, width, height, 1, image_ahash),
        PITH_LAYOUT_RGB8 => hash_8::<Rgb, _>(dump, width, height, 3, image_ahash),
        PITH_LAYOUT_RGB16 => hash_16::<Rgb, _>(dump, width, height, 3, image_ahash),
        PITH_LAYOUT_RGBA8 => hash_8::<Rgba, _>(dump, width, height, 4, image_ahash),
        _ => Err(PITH_E_INVALID),
    }
}

fn dhash_of_dump(dump: &[u8], width: u32, height: u32, layout: u32) -> Result<u64, i32> {
    match layout {
        PITH_LAYOUT_GRAY8 => hash_8::<Gray, _>(dump, width, height, 1, image_dhash),
        PITH_LAYOUT_GRAY16 => hash_16::<Gray, _>(dump, width, height, 1, image_dhash),
        PITH_LAYOUT_RGB8 => hash_8::<Rgb, _>(dump, width, height, 3, image_dhash),
        PITH_LAYOUT_RGB16 => hash_16::<Rgb, _>(dump, width, height, 3, image_dhash),
        PITH_LAYOUT_RGBA8 => hash_8::<Rgba, _>(dump, width, height, 4, image_dhash),
        _ => Err(PITH_E_INVALID),
    }
}

fn whash_of_dump(dump: &[u8], width: u32, height: u32, layout: u32, level: u32) -> Result<u64, i32> {
    match layout {
        PITH_LAYOUT_GRAY8 => hash_8::<Gray, _>(dump, width, height, 1, |img| image_whash(img, level)),
        PITH_LAYOUT_GRAY16 => hash_16::<Gray, _>(dump, width, height, 1, |img| image_whash(img, level)),
        PITH_LAYOUT_RGB8 => hash_8::<Rgb, _>(dump, width, height, 3, |img| image_whash(img, level)),
        PITH_LAYOUT_RGB16 => hash_16::<Rgb, _>(dump, width, height, 3, |img| image_whash(img, level)),
        PITH_LAYOUT_RGBA8 => hash_8::<Rgba, _>(dump, width, height, 4, |img| image_whash(img, level)),
        _ => Err(PITH_E_INVALID),
    }
}

fn colorhash_of_dump(dump: &[u8], width: u32, height: u32, layout: u32, bins: u32) -> Result<u64, i32> {
    match layout {
        PITH_LAYOUT_GRAY8 => hash_8::<Gray, _>(dump, width, height, 1, |img| image_colorhash(img, bins)),
        PITH_LAYOUT_GRAY16 => hash_16::<Gray, _>(dump, width, height, 1, |img| image_colorhash(img, bins)),
        PITH_LAYOUT_RGB8 => hash_8::<Rgb, _>(dump, width, height, 3, |img| image_colorhash(img, bins)),
        PITH_LAYOUT_RGB16 => hash_16::<Rgb, _>(dump, width, height, 3, |img| image_colorhash(img, bins)),
        PITH_LAYOUT_RGBA8 => hash_8::<Rgba, _>(dump, width, height, 4, |img| image_colorhash(img, bins)),
        _ => Err(PITH_E_INVALID),
    }
}

fn hash_8<L: Layout, F>(dump: &[u8], width: u32, height: u32, channels: usize, f: F) -> Result<u64, i32>
where
    F: FnOnce(&Image<L, u8>) -> Result<u64, Error>,
{
    let Some(samples) = expected_samples(width, height, channels, 1) else {
        return Err(PITH_E_INVALID);
    };
    if dump.len() != samples {
        return Err(PITH_E_INVALID);
    }
    let img = Image::<L, u8>::from_vec(width, height, dump.to_vec()).map_err(|_| PITH_E_INVALID)?;
    f(&img).map_err(|_| PITH_E_REJECTED)
}

fn hash_16<L: Layout, F>(dump: &[u8], width: u32, height: u32, channels: usize, f: F) -> Result<u64, i32>
where
    F: FnOnce(&Image<L, u16>) -> Result<u64, Error>,
{
    let Some(samples) = expected_samples(width, height, channels, 2) else {
        return Err(PITH_E_INVALID);
    };
    if dump.len() != samples {
        return Err(PITH_E_INVALID);
    }
    let mut pixels = Vec::with_capacity(samples / 2);
    for chunk in dump.chunks_exact(2) {
        pixels.push(u16::from_le_bytes([chunk[0], chunk[1]]));
    }
    let img = Image::<L, u16>::from_vec(width, height, pixels).map_err(|_| PITH_E_INVALID)?;
    f(&img).map_err(|_| PITH_E_REJECTED)
}

/// The safe core of [`sillstone_image_phash`]: validates the dump against
/// the declared geometry and runs the pHash pipeline.
fn phash_of_dump(dump: &[u8], width: u32, height: u32, layout: u32) -> Result<u64, i32> {
    match layout {
        PITH_LAYOUT_GRAY8 => hash_8::<Gray, _>(dump, width, height, 1, image_phash),
        PITH_LAYOUT_GRAY16 => hash_16::<Gray, _>(dump, width, height, 1, image_phash),
        PITH_LAYOUT_RGB8 => hash_8::<Rgb, _>(dump, width, height, 3, image_phash),
        PITH_LAYOUT_RGB16 => hash_16::<Rgb, _>(dump, width, height, 3, image_phash),
        PITH_LAYOUT_RGBA8 => hash_8::<Rgba, _>(dump, width, height, 4, image_phash),
        _ => Err(PITH_E_INVALID),
    }
}

/// `width * height * channels * bytes_per_sample`, or `None` on
/// overflow (the `Image` constructor's own ceiling never sees the call).
fn expected_samples(width: u32, height: u32, channels: usize, bytes: usize) -> Option<usize> {
    (width as usize)
        .checked_mul(height as usize)?
        .checked_mul(channels)?
        .checked_mul(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reference::RawLayout;

    /// The committed fixtures: (name, layout, width, height, expected).
    /// These are the nine oracle vectors of `reference.json`; the SDK
    /// conformance suites replay them through the FFI against the
    /// fixture files, this test pins three of them (one per sample
    /// width and channel count class) byte-exactly in-tree.
    const PINNED: &[(&str, RawLayout, u32, u32, u64)] = &[
        ("base_444", RawLayout::Rgb8, 32, 24, 0x8d0a_3f41_1ee5_0f7a),
        (
            "phash_gray16_36x28",
            RawLayout::Gray16,
            36,
            28,
            0xd386_0405_6df8_c37f,
        ),
        (
            "phash_rgba8_48x40",
            RawLayout::Rgba8,
            48,
            40,
            0x8433_4f9d_2c5a_f4a9,
        ),
    ];

    /// The FFI hash of a committed fixture dump equals the oracle
    /// vector, for every sample class.
    #[test]
    fn ffi_hash_matches_oracle_on_pinned_fixtures() {
        for (name, layout, width, height, expected) in PINNED {
            let file = format!(
                "{}/tests/fixtures/phash/{name}.raw",
                env!("CARGO_MANIFEST_DIR")
            );
            let dump = std::fs::read(&file).unwrap_or_else(|e| panic!("cannot read {file}: {e}"));
            assert_eq!(
                dump.len(),
                (*width) as usize * (*height) as usize * layout.channels() * layout.sample_bytes()
            );
            let mut hash = 0u64;
            let code = unsafe {
                sillstone_image_phash(
                    dump.as_ptr(),
                    dump.len(),
                    *width,
                    *height,
                    wire_code(*layout),
                    &mut hash,
                )
            };
            assert_eq!(code, PITH_OK, "{name}");
            assert_eq!(hash, *expected, "{name}");
        }
    }

    /// `RawLayout` to wire code, mirroring the declaration order.
    fn wire_code(layout: RawLayout) -> u32 {
        match layout {
            RawLayout::Gray8 => PITH_LAYOUT_GRAY8,
            RawLayout::Gray16 => PITH_LAYOUT_GRAY16,
            RawLayout::Rgb8 => PITH_LAYOUT_RGB8,
            RawLayout::Rgb16 => PITH_LAYOUT_RGB16,
            RawLayout::Rgba8 => PITH_LAYOUT_RGBA8,
        }
    }

    /// Every layout code the module documents is accepted.
    #[test]
    fn all_documented_layout_codes_are_accepted() {
        let dump = [0u8; 3 * 2 * 6]; // large enough for any 3x2 layout
        // (layout code, dump length in bytes) for a 3x2 image.
        let cases: [(u32, usize); 5] = [
            (PITH_LAYOUT_GRAY8, 6),
            (PITH_LAYOUT_GRAY16, 12),
            (PITH_LAYOUT_RGB8, 18),
            (PITH_LAYOUT_RGB16, 36),
            (PITH_LAYOUT_RGBA8, 24),
        ];
        for (code, samples) in cases {
            let mut hash = 0u64;
            let code_r = unsafe { sillstone_image_phash(dump.as_ptr(), samples, 3, 2, code, &mut hash) };
            assert_eq!(code_r, PITH_OK, "layout {code}");
        }
    }

    /// Unknown layout codes, null pointers and length/geometry
    /// mismatches are [`PITH_E_INVALID`], never a panic.
    #[test]
    fn invalid_arguments_are_refused() {
        let dump = [0u8; 4];
        let mut hash = 0u64;
        let unknown = unsafe { sillstone_image_phash(dump.as_ptr(), dump.len(), 2, 2, 99, &mut hash) };
        assert_eq!(unknown, PITH_E_INVALID);

        let short =
            unsafe { sillstone_image_phash(dump.as_ptr(), 3, 2, 2, PITH_LAYOUT_GRAY8, &mut hash) };
        assert_eq!(short, PITH_E_INVALID);

        let null_data =
            unsafe { sillstone_image_phash(core::ptr::null(), 0, 2, 2, PITH_LAYOUT_GRAY8, &mut hash) };
        assert_eq!(null_data, PITH_E_INVALID);

        let null_out = unsafe {
            sillstone_image_phash(
                dump.as_ptr(),
                dump.len(),
                2,
                2,
                PITH_LAYOUT_GRAY8,
                core::ptr::null_mut(),
            )
        };
        assert_eq!(null_out, PITH_E_INVALID);
    }

    /// Dimensions whose sample count overflows `usize` are refused
    /// before any allocation is attempted.
    #[test]
    fn overflowing_geometry_is_refused() {
        let dump: [u8; 0] = [];
        let mut hash = 0u64;
        let code = unsafe {
            sillstone_image_phash(
                dump.as_ptr(),
                0,
                u32::MAX,
                u32::MAX,
                PITH_LAYOUT_RGBA8,
                &mut hash,
            )
        };
        assert_eq!(code, PITH_E_INVALID);
    }

    /// The safe core agrees with the FFI wrapper on the pinned dumps.
    #[test]
    fn safe_core_matches_ffi() {
        let dump = vec![7u8; 4 * 4];
        let via_core = phash_of_dump(&dump, 4, 4, PITH_LAYOUT_GRAY8).expect("hash");
        let img = Image::<Gray, u8>::from_vec(4, 4, dump).expect("image");
        assert_eq!(via_core, image_phash(&img).expect("pipeline"));
    }
}
