//! The shared vocabulary of the `pith` suite: the one error type, the
//! fixed-width digest value, the Hamming distance between digests, and
//! the closed sets of names that appear in on-disk records. Also the
//! computational primitives every modality crate shares: SHA-256, the
//! checksums (CRC-32, Adler-32, FNV-1a 64), splitmix64 and the bit
//! reader.
//!
//! The foundation crate of the zero-dependency `pith` suite. Every other
//! crate builds on these types and nothing builds beneath them: this
//! crate has an empty `[dependencies]` table, enforced by
//! `scripts/check-zero-deps.py`.
//!
//! The crate is `core`-only in its library shape: no filesystem,
//! network, clock or environment, and no code path allocates. The
//! `std` feature (on by default) links `std` so the `cdylib` the
//! language SDKs bind through carries a panic handler; the library
//! surface itself stays `core`-only either way.

#![cfg_attr(not(feature = "std"), no_std)]
#![deny(unsafe_code)]
#![deny(missing_docs)]

#[cfg(not(feature = "std"))]
extern crate alloc;

mod bitreader;
mod checksums;
mod digest;
mod distance;
mod error;
mod names;
mod sha1;
mod sha256;
mod sha512;
mod splitmix64;
pub mod blake2b;
pub mod blake3;
pub mod sha3;

pub mod base64;
pub mod ffi;
pub mod xxh64;

mod crc32c;
mod hmac;
mod murmur3;
mod xoshiro;

// The Java SDK's native-method surface: `Java_hash_sillstone_digest_*`
// exports that forward to the C ABI above. Compiled out of the
// unit-test build (the `#[no_mangle]` exports would collide with the
// test binary's copies) and out of `--no-default-features` builds (the
// glue needs `std` allocations); `tests/java_ffi.rs` covers the glue
// against a synthetic JNI environment instead. Private module: the JVM
// links the exports by symbol name, so nothing here needs to be
// publicly nameable in Rust.
#[cfg(all(not(test), feature = "std"))]
mod ffi_jni;

pub use crate::base64::{base64_decode, base64_encode};
pub use crate::bitreader::BitReader;
pub use crate::checksums::{adler32, crc32, fnv1a64};
pub use crate::crc32c::crc32c;
pub use crate::digest::{Digest, Read};
pub use crate::distance::hamming;
pub use crate::error::{Error, Result};
pub use crate::hmac::{hmac_sha1, hmac_sha256, hmac_sha512};
pub use crate::murmur3::murmur3_x64_128;
pub use crate::names::{Algorithm, Format};
pub use crate::sha1::sha1;
pub use crate::sha256::sha256;
pub use crate::sha512::sha512;
pub use crate::blake2b::{blake2b, blake2b_512, blake2b_into};
pub use crate::blake3::{blake3, blake3_keyed, blake3_xof, Hasher as Blake3Hasher};
pub use crate::sha3::{sha3_256, sha3_512};
pub use crate::splitmix64::SplitMix64;
pub use crate::xoshiro::Xoshiro256StarStar;
pub use crate::xxh64::xxh64;
