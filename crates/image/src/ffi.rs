#![allow(clippy::missing_safety_doc)]
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


/// Resizes a raw image buffer using the specified resampling filter.
/// Filter: 0=Nearest, 1=Box, 2=Bilinear, 3=Bicubic, 4=Lanczos.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sillstone_image_resize(
    data: *const u8,
    len: usize,
    in_w: u32,
    in_h: u32,
    layout: u32,
    out_w: u32,
    out_h: u32,
    filter: u32,
    out_buf: *mut u8,
    out_len: usize,
) -> i32 {
    if data.is_null() || out_buf.is_null() {
        return PITH_E_INVALID;
    }
    let r_filter = match filter {
        0 => crate::raster::ResamplingFilter::Nearest,
        1 => crate::raster::ResamplingFilter::Box,
        2 => crate::raster::ResamplingFilter::Bilinear,
        3 => crate::raster::ResamplingFilter::Bicubic,
        4 => crate::raster::ResamplingFilter::Lanczos,
        _ => return PITH_E_INVALID,
    };

    let channels = match layout {
        PITH_LAYOUT_GRAY8 => 1,
        PITH_LAYOUT_RGB8 => 3,
        PITH_LAYOUT_RGBA8 => 4,
        _ => return PITH_E_INVALID,
    };

    if len != (in_w as usize) * (in_h as usize) * channels {
        return PITH_E_INVALID;
    }
    let expected_out_len = (out_w as usize) * (out_h as usize) * channels;
    if out_len < expected_out_len {
        return PITH_E_INVALID;
    }

    let src_slice = unsafe { core::slice::from_raw_parts(data, len) };
    let out_slice = unsafe { core::slice::from_raw_parts_mut(out_buf, expected_out_len) };

    match layout {
        PITH_LAYOUT_GRAY8 => {
            let img = match Image::<Gray, u8>::from_vec(in_w, in_h, src_slice.to_vec()) {
                Ok(im) => im,
                Err(_) => return PITH_E_REJECTED,
            };
            let res = match crate::raster::resize(&img, out_w, out_h, r_filter) {
                Ok(r) => r,
                Err(_) => return PITH_E_REJECTED,
            };
            out_slice.copy_from_slice(res.as_slice());
        }
        PITH_LAYOUT_RGB8 => {
            let img = match Image::<Rgb, u8>::from_vec(in_w, in_h, src_slice.to_vec()) {
                Ok(im) => im,
                Err(_) => return PITH_E_REJECTED,
            };
            let res = match crate::raster::resize(&img, out_w, out_h, r_filter) {
                Ok(r) => r,
                Err(_) => return PITH_E_REJECTED,
            };
            out_slice.copy_from_slice(res.as_slice());
        }
        PITH_LAYOUT_RGBA8 => {
            let img = match Image::<Rgba, u8>::from_vec(in_w, in_h, src_slice.to_vec()) {
                Ok(im) => im,
                Err(_) => return PITH_E_REJECTED,
            };
            let res = match crate::raster::resize(&img, out_w, out_h, r_filter) {
                Ok(r) => r,
                Err(_) => return PITH_E_REJECTED,
            };
            out_slice.copy_from_slice(res.as_slice());
        }
        _ => return PITH_E_INVALID,
    }

    PITH_OK
}

/// Crops a rectangular box `[x0, y0, x1, y1)` from an image.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sillstone_image_crop(
    data: *const u8,
    len: usize,
    in_w: u32,
    in_h: u32,
    layout: u32,
    x0: u32,
    y0: u32,
    x1: u32,
    y1: u32,
    out_buf: *mut u8,
    out_len: usize,
) -> i32 {
    if data.is_null() || out_buf.is_null() || x0 >= x1 || y0 >= y1 || x1 > in_w || y1 > in_h {
        return PITH_E_INVALID;
    }
    let channels = match layout {
        PITH_LAYOUT_GRAY8 => 1,
        PITH_LAYOUT_RGB8 => 3,
        PITH_LAYOUT_RGBA8 => 4,
        _ => return PITH_E_INVALID,
    };
    let out_w = x1 - x0;
    let out_h = y1 - y0;
    let needed = (out_w as usize) * (out_h as usize) * channels;
    if out_len < needed {
        return PITH_E_INVALID;
    }

    let src = unsafe { core::slice::from_raw_parts(data, len) };
    let dst = unsafe { core::slice::from_raw_parts_mut(out_buf, needed) };

    let mut offset = 0usize;
    for y in y0..y1 {
        let start = (y as usize * in_w as usize + x0 as usize) * channels;
        let end = start + (out_w as usize) * channels;
        dst[offset..offset + (end - start)].copy_from_slice(&src[start..end]);
        offset += end - start;
    }
    PITH_OK
}

/// Transposes or rotates by 90-degree steps.
/// Method: 0=FlipH, 1=FlipV, 2=Rot90, 3=Rot180, 4=Rot270, 5=Transpose, 6=Transverse.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sillstone_image_transpose(
    data: *const u8,
    len: usize,
    in_w: u32,
    in_h: u32,
    layout: u32,
    method: u32,
    out_buf: *mut u8,
    out_len: usize,
) -> i32 {
    if data.is_null() || out_buf.is_null() {
        return PITH_E_INVALID;
    }
    let t_method = match method {
        0 => crate::transform::TransposeMethod::FlipHorizontal,
        1 => crate::transform::TransposeMethod::FlipVertical,
        2 => crate::transform::TransposeMethod::Rotate90,
        3 => crate::transform::TransposeMethod::Rotate180,
        4 => crate::transform::TransposeMethod::Rotate270,
        5 => crate::transform::TransposeMethod::Transpose,
        6 => crate::transform::TransposeMethod::Transverse,
        _ => return PITH_E_INVALID,
    };

    let channels = match layout {
        PITH_LAYOUT_GRAY8 => 1,
        PITH_LAYOUT_RGB8 => 3,
        PITH_LAYOUT_RGBA8 => 4,
        _ => return PITH_E_INVALID,
    };
    let total_samples = (in_w as usize) * (in_h as usize) * channels;
    if len != total_samples || out_len < total_samples {
        return PITH_E_INVALID;
    }

    let src = unsafe { core::slice::from_raw_parts(data, len) };
    let dst = unsafe { core::slice::from_raw_parts_mut(out_buf, total_samples) };

    macro_rules! do_transpose {
        ($layout_ty:ty) => {{
            let img = match Image::<$layout_ty, u8>::from_vec(in_w, in_h, src.to_vec()) {
                Ok(im) => im,
                Err(_) => return PITH_E_REJECTED,
            };
            let res = match crate::transform::transpose(&img, t_method) {
                Ok(r) => r,
                Err(_) => return PITH_E_REJECTED,
            };
            dst.copy_from_slice(res.as_slice());
        }};
    }

    match layout {
        PITH_LAYOUT_GRAY8 => do_transpose!(Gray),
        PITH_LAYOUT_RGB8 => do_transpose!(Rgb),
        PITH_LAYOUT_RGBA8 => do_transpose!(Rgba),
        _ => return PITH_E_INVALID,
    }

    PITH_OK
}

/// Rotates an image counter-clockwise by arbitrary angle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sillstone_image_rotate(
    data: *const u8,
    len: usize,
    in_w: u32,
    in_h: u32,
    layout: u32,
    angle: f64,
    expand: u32,
    filter: u32,
    out_buf: *mut u8,
    out_len: usize,
) -> i32 {
    if data.is_null() || out_buf.is_null() {
        return PITH_E_INVALID;
    }
    let r_filter = match filter {
        0 => crate::raster::ResamplingFilter::Nearest,
        _ => crate::raster::ResamplingFilter::Bilinear,
    };
    let _channels = match layout {
        PITH_LAYOUT_GRAY8 => 1,
        PITH_LAYOUT_RGB8 => 3,
        PITH_LAYOUT_RGBA8 => 4,
        _ => return PITH_E_INVALID,
    };
    let src = unsafe { core::slice::from_raw_parts(data, len) };

    macro_rules! do_rotate {
        ($layout_ty:ty) => {{
            let img = match Image::<$layout_ty, u8>::from_vec(in_w, in_h, src.to_vec()) {
                Ok(im) => im,
                Err(_) => return PITH_E_REJECTED,
            };
            let res = match crate::transform::rotate(&img, angle, expand != 0, r_filter) {
                Ok(r) => r,
                Err(_) => return PITH_E_REJECTED,
            };
            let res_bytes = res.as_slice().len();
            if out_len < res_bytes {
                return PITH_E_INVALID;
            }
            let dst = unsafe { core::slice::from_raw_parts_mut(out_buf, res_bytes) };
            dst.copy_from_slice(res.as_slice());
        }};
    }

    match layout {
        PITH_LAYOUT_GRAY8 => do_rotate!(Gray),
        PITH_LAYOUT_RGB8 => do_rotate!(Rgb),
        PITH_LAYOUT_RGBA8 => do_rotate!(Rgba),
        _ => return PITH_E_INVALID,
    }

    PITH_OK
}

/// Applies built-in 2D convolution filters.
/// Filter ID: 0=Blur, 1=Sharpen, 2=Contour, 3=Detail, 4=EdgeEnhance, 5=FindEdges, 6=Emboss.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sillstone_image_filter(
    data: *const u8,
    len: usize,
    in_w: u32,
    in_h: u32,
    layout: u32,
    filter_id: u32,
    out_buf: *mut u8,
    out_len: usize,
) -> i32 {
    if data.is_null() || out_buf.is_null() || out_len < len {
        return PITH_E_INVALID;
    }
    let builtin = match filter_id {
        0 => crate::filter::BuiltinFilter::Blur,
        1 => crate::filter::BuiltinFilter::Sharpen,
        2 => crate::filter::BuiltinFilter::Contour,
        3 => crate::filter::BuiltinFilter::Detail,
        4 => crate::filter::BuiltinFilter::EdgeEnhance,
        5 => crate::filter::BuiltinFilter::FindEdges,
        6 => crate::filter::BuiltinFilter::Emboss,
        _ => return PITH_E_INVALID,
    };

    let src = unsafe { core::slice::from_raw_parts(data, len) };
    let dst = unsafe { core::slice::from_raw_parts_mut(out_buf, len) };

    macro_rules! do_filter {
        ($layout_ty:ty) => {{
            let img = match Image::<$layout_ty, u8>::from_vec(in_w, in_h, src.to_vec()) {
                Ok(im) => im,
                Err(_) => return PITH_E_REJECTED,
            };
            let res = match crate::filter::apply_filter(&img, builtin) {
                Ok(r) => r,
                Err(_) => return PITH_E_REJECTED,
            };
            dst.copy_from_slice(res.as_slice());
        }};
    }

    match layout {
        PITH_LAYOUT_GRAY8 => do_filter!(Gray),
        PITH_LAYOUT_RGB8 => do_filter!(Rgb),
        PITH_LAYOUT_RGBA8 => do_filter!(Rgba),
        _ => return PITH_E_INVALID,
    }

    PITH_OK
}

/// Adjusts image properties: 0=brightness, 1=invert.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sillstone_image_adjust(
    data: *const u8,
    len: usize,
    in_w: u32,
    in_h: u32,
    layout: u32,
    op_id: u32,
    factor: f64,
    out_buf: *mut u8,
    out_len: usize,
) -> i32 {
    if data.is_null() || out_buf.is_null() || out_len < len {
        return PITH_E_INVALID;
    }
    let src = unsafe { core::slice::from_raw_parts(data, len) };
    let dst = unsafe { core::slice::from_raw_parts_mut(out_buf, len) };

    macro_rules! do_adjust {
        ($layout_ty:ty) => {{
            let img = match Image::<$layout_ty, u8>::from_vec(in_w, in_h, src.to_vec()) {
                Ok(im) => im,
                Err(_) => return PITH_E_REJECTED,
            };
            let res = match op_id {
                0 => crate::filter::adjust_brightness(&img, factor),
                1 => crate::filter::invert(&img),
                _ => return PITH_E_INVALID,
            };
            match res {
                Ok(r) => dst.copy_from_slice(r.as_slice()),
                Err(_) => return PITH_E_REJECTED,
            }
        }};
    }

    match layout {
        PITH_LAYOUT_GRAY8 => do_adjust!(Gray),
        PITH_LAYOUT_RGB8 => do_adjust!(Rgb),
        PITH_LAYOUT_RGBA8 => do_adjust!(Rgba),
        _ => return PITH_E_INVALID,
    }

    PITH_OK
}

/// Converts between color spaces (e.g. RGB to Gray, RGBA to RGB, etc.).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sillstone_image_color_convert(
    data: *const u8,
    len: usize,
    in_w: u32,
    in_h: u32,
    from_layout: u32,
    to_layout: u32,
    out_buf: *mut u8,
    out_len: usize,
) -> i32 {
    if data.is_null() || out_buf.is_null() {
        return PITH_E_INVALID;
    }
    let src = unsafe { core::slice::from_raw_parts(data, len) };
    let total_pixels = (in_w as usize) * (in_h as usize);

    if from_layout == PITH_LAYOUT_RGB8 && to_layout == PITH_LAYOUT_GRAY8 {
        if out_len < total_pixels {
            return PITH_E_INVALID;
        }
        let img = match Image::<Rgb, u8>::from_vec(in_w, in_h, src.to_vec()) {
            Ok(im) => im,
            Err(_) => return PITH_E_REJECTED,
        };
        let gray = img.to_gray();
        let dst = unsafe { core::slice::from_raw_parts_mut(out_buf, total_pixels) };
        dst.copy_from_slice(gray.as_slice());
        PITH_OK
    } else if from_layout == PITH_LAYOUT_RGBA8 && to_layout == PITH_LAYOUT_GRAY8 {
        if out_len < total_pixels {
            return PITH_E_INVALID;
        }
        let img = match Image::<Rgba, u8>::from_vec(in_w, in_h, src.to_vec()) {
            Ok(im) => im,
            Err(_) => return PITH_E_REJECTED,
        };
        let gray = img.to_gray();
        let dst = unsafe { core::slice::from_raw_parts_mut(out_buf, total_pixels) };
        dst.copy_from_slice(gray.as_slice());
        PITH_OK
    } else if from_layout == PITH_LAYOUT_RGBA8 && to_layout == PITH_LAYOUT_RGB8 {
        if out_len < total_pixels * 3 {
            return PITH_E_INVALID;
        }
        let img = match Image::<Rgba, u8>::from_vec(in_w, in_h, src.to_vec()) {
            Ok(im) => im,
            Err(_) => return PITH_E_REJECTED,
        };
        let rgb = img.to_rgb();
        let dst = unsafe { core::slice::from_raw_parts_mut(out_buf, total_pixels * 3) };
        dst.copy_from_slice(rgb.as_slice());
        PITH_OK
    } else if from_layout == PITH_LAYOUT_RGB8 && to_layout == PITH_LAYOUT_RGBA8 {
        if out_len < total_pixels * 4 {
            return PITH_E_INVALID;
        }
        let img = match Image::<Rgb, u8>::from_vec(in_w, in_h, src.to_vec()) {
            Ok(im) => im,
            Err(_) => return PITH_E_REJECTED,
        };
        let rgba = img.to_rgba();
        let dst = unsafe { core::slice::from_raw_parts_mut(out_buf, total_pixels * 4) };
        dst.copy_from_slice(rgba.as_slice());
        PITH_OK
    } else {
        PITH_E_INVALID
    }
}

/// Encodes raw image pixels to QOI format. Allocates buffer owned by caller.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sillstone_image_qoi_encode(
    data: *const u8,
    len: usize,
    in_w: u32,
    in_h: u32,
    layout: u32,
    out_ptr: *mut *mut u8,
    out_len: *mut usize,
) -> i32 {
    if data.is_null() || out_ptr.is_null() || out_len.is_null() {
        return PITH_E_INVALID;
    }
    let src = unsafe { core::slice::from_raw_parts(data, len) };
    let channels = match layout {
        PITH_LAYOUT_RGB8 => 3,
        PITH_LAYOUT_RGBA8 => 4,
        _ => return PITH_E_INVALID,
    };

    let encoded = match sillstone_qoi::encode(in_w, in_h, channels, 0, src) {
        Ok(enc) => enc,
        Err(_) => return PITH_E_REJECTED,
    };

    let mut boxed = encoded.into_boxed_slice();
    unsafe {
        *out_ptr = boxed.as_mut_ptr();
        *out_len = boxed.len();
    }
    core::mem::forget(boxed);
    PITH_OK
}

/// Decodes QOI bytes to raw image pixels.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sillstone_image_qoi_decode(
    data: *const u8,
    len: usize,
    out_w: *mut u32,
    out_h: *mut u32,
    out_layout: *mut u32,
    out_ptr: *mut *mut u8,
    out_len: *mut usize,
) -> i32 {
    if data.is_null() || out_w.is_null() || out_h.is_null() || out_layout.is_null() || out_ptr.is_null() || out_len.is_null() {
        return PITH_E_INVALID;
    }
    let src = unsafe { core::slice::from_raw_parts(data, len) };
    let (header, pixels) = match sillstone_qoi::decode(src) {
        Ok(res) => res,
        Err(_) => return PITH_E_REJECTED,
    };

    let layout = match header.channels {
        3 => PITH_LAYOUT_RGB8,
        4 => PITH_LAYOUT_RGBA8,
        _ => return PITH_E_REJECTED,
    };

    let mut boxed = pixels.into_boxed_slice();
    unsafe {
        *out_w = header.width;
        *out_h = header.height;
        *out_layout = layout;
        *out_ptr = boxed.as_mut_ptr();
        *out_len = boxed.len();
    }
    core::mem::forget(boxed);
    PITH_OK
}

/// Encodes raw image pixels to standard BMP format.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sillstone_image_bmp_encode(
    data: *const u8,
    len: usize,
    in_w: u32,
    in_h: u32,
    layout: u32,
    out_ptr: *mut *mut u8,
    out_len: *mut usize,
) -> i32 {
    if data.is_null() || out_ptr.is_null() || out_len.is_null() {
        return PITH_E_INVALID;
    }
    let src = unsafe { core::slice::from_raw_parts(data, len) };
    let encoded = match layout {
        PITH_LAYOUT_RGB8 => {
            let img = match Image::<Rgb, u8>::from_vec(in_w, in_h, src.to_vec()) {
                Ok(im) => im,
                Err(_) => return PITH_E_REJECTED,
            };
            match crate::bmp::encode_bmp_rgb(&img) {
                Ok(b) => b,
                Err(_) => return PITH_E_REJECTED,
            }
        }
        PITH_LAYOUT_RGBA8 => {
            let img = match Image::<Rgba, u8>::from_vec(in_w, in_h, src.to_vec()) {
                Ok(im) => im,
                Err(_) => return PITH_E_REJECTED,
            };
            match crate::bmp::encode_bmp_rgba(&img) {
                Ok(b) => b,
                Err(_) => return PITH_E_REJECTED,
            }
        }
        _ => return PITH_E_INVALID,
    };

    let mut boxed = encoded.into_boxed_slice();
    unsafe {
        *out_ptr = boxed.as_mut_ptr();
        *out_len = boxed.len();
    }
    core::mem::forget(boxed);
    PITH_OK
}

/// Frees a heap buffer allocated across FFI boundaries by sillstone image operations.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sillstone_image_free_bytes(ptr: *mut u8, len: usize) {
    if !ptr.is_null() && len > 0 {
        let _ = unsafe { alloc::boxed::Box::from_raw(core::slice::from_raw_parts_mut(ptr, len)) };
    }
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
