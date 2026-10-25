//! Raw raster buffers, BMP decoding, tier-3 local features and the
//! 64-bit pHash — the image lane of the pith hashing suite.
//!
//! sillstone-image merges three crates of the upstream `modhash` kit into one
//! dependency-free package, ported byte-compatibly (algorithms, public
//! API shape, tests and fixtures unchanged):
//!
//! * [`raster`] — the raw image buffer every decoder writes and every
//!   hash reads (tight row-major layout, integer box-average
//!   resampling, BT.601 luma, EXIF orientation), from `modhash-raster`.
//! * [`bmp`] — BMP decoding for 24/32-bit images and RLE8 compression,
//!   from `modhash-bmp`.
//! * [`tier3`] — FAST-9 corners, rBRIEF descriptors, RANSAC geometric
//!   verification, the D₄ symmetries and banded DTW, from
//!   `modhash-tier3`.
//! * [`phash`] — the 64-bit image perceptual hash
//!   (`luma → 32×32 box average → DCT-II → low 8×8 minus DC → median
//!   threshold`), bit-for-bit identical to the `modhash` facade and
//!   pinned by the committed oracle vectors in [`reference`].
//!
//! The only dependencies are the suite's own [`sillstone-digest`] and
//! [`sillstone-math`] crates; CI enforces the zero-third-party gate.
//!
//! [`sillstone-digest`]: https://github.com/sillstone-hash/sillstone-digest
//! [`sillstone-math`]: https://github.com/sillstone-hash/sillstone-math

// `unsafe` is denied everywhere except `ffi`, the C ABI surface the
// language SDKs bind through: raw pointers exist only at that boundary,
// and every exported function is a documented `unsafe extern "C"` fn.
#![deny(unsafe_code)]
#![deny(missing_docs)]

// The `raster` and `bmp` modules were `no_std` crates upstream and keep
// their `alloc`-only buffer code unchanged.
extern crate alloc;

pub mod bmp;
pub mod ffi;
pub mod filter;
pub mod imagehash;
pub mod qoi_codec;
// The Java SDK's native-method surface: `Java_hash_sillstone_image_*`
// exports that forward to the C ABI above. Compiled out of the unit
// test build (the `#[no_mangle]` exports would collide with the test
// binary's copies); `tests/java_ffi.rs` covers the glue against a
// synthetic JNI environment instead. Private module: the JVM links the
// exports by symbol name, so nothing here needs to be publicly
// nameable in Rust.
#[cfg(not(test))]
mod ffi_jni;
pub mod phash;
pub mod raster;
pub mod reference;
pub mod tier3;
pub mod transform;

pub use bmp::{decode as decode_bmp, encode_bmp_rgb, encode_bmp_rgba};
pub use filter::{
    BuiltinFilter, adjust_brightness, apply_filter, convolve_3x3, gaussian_blur, invert,
};
pub use imagehash::{
    image_ahash, image_colorhash, image_dhash, image_phash_imagehash, image_whash,
};
pub use phash::image_phash;
pub use qoi_codec::{qoi_decode_rgb, qoi_decode_rgba, qoi_encode_rgb, qoi_encode_rgba};
pub use raster::{ResamplingFilter, resize};
pub use transform::{TransposeMethod, crop, rotate, transpose};
