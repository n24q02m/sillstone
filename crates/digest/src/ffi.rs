//! The C ABI surface of `sillstone-digest`: the entry points the Python
//! (ctypes), Node (koffi) and Go (cgo) SDKs bind through.
//!
//! The suite's FFI convention, defined by this module and mirrored by
//! every `pith-*` cdylib:
//!
//! * one flat set of `#[unsafe(no_mangle)] pub unsafe extern "C"`
//!   functions — raw pointers plus lengths, no structs across the
//!   boundary;
//! * every function returns a status code (see the constants below),
//!   never a `Result`, never a panic: a `panic = "abort"` cdylib must
//!   not be reachable from a foreign caller;
//! * every operation here writes into caller-provided out-parameters
//!   and allocates nothing, so the crate ships no `_free` — there is
//!   no buffer this side ever hands out;
//! * the `unsafe` allowance is confined to this module; every core
//!   module stays unsafe-free behind the crate-root `#![deny]`.

#![allow(unsafe_code)]

use crate::error::Result;
use crate::{SplitMix64, adler32, crc32, fnv1a64, sha256};

/// Borrows `data` as a byte slice; a null pointer (only legal with the
/// zero length the callers validate first) borrows the empty slice —
/// `from_raw_parts` itself would panic on the null even at length 0.
fn borrow<'a>(data: *const u8, len: usize) -> &'a [u8] {
    if data.is_null() {
        &[]
    } else {
        unsafe { core::slice::from_raw_parts(data, len) }
    }
}

/// The mutable counterpart of [`borrow`] for out-slots.
fn borrow_mut<'a>(out: *mut u64, count: usize) -> &'a mut [u64] {
    if out.is_null() {
        &mut []
    } else {
        unsafe { core::slice::from_raw_parts_mut(out, count) }
    }
}

/// Status: success.
pub const PITH_OK: i32 = 0;
/// Status: a caller argument is invalid — a null out-slot, or a null
/// data pointer that claims a nonzero length.
pub const PITH_E_INVALID: i32 = -1;
/// Status: the core primitive refused the input. With `sillstone-digest`
/// this is reachable for a SHA-256/SHA-1 input beyond the padding
/// format's length ceiling — unreachable for any buffer a real caller
/// can pass — and for the base64 pair (an output buffer smaller than
/// the operation needs, or a non-canonical decode input), but the
/// code exists so a foreign caller never has to reason about a panic.
pub const PITH_E_REJECTED: i32 = -2;

/// The SHA-256 digest of `data`, written as 32 raw bytes through `out`.
///
/// # Safety
///
/// `data` must point to `len` readable bytes and `out` to 32 writable
/// bytes; both must stay valid for the duration of the call. The
/// function retains nothing.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sillstone_digest_sha256(data: *const u8, len: usize, out: *mut u8) -> i32 {
    // A null `data` with `len == 0` is a valid empty preimage; only a
    // null out-slot, or a null data pointer that claims length, is a
    // caller bug.
    if out.is_null() || (data.is_null() && len > 0) {
        return PITH_E_INVALID;
    }
    let bytes = borrow(data, len);
    match sha256(bytes) {
        Ok(digest) => {
            unsafe { core::ptr::copy_nonoverlapping(digest.as_bytes().as_ptr(), out, 32) };
            PITH_OK
        }
        Err(_) => PITH_E_REJECTED,
    }
}

/// The CRC-32 (IEEE 802.3, reflected) of `data`, written through `out`.
///
/// # Safety
///
/// `data` must point to `len` readable bytes and `out` to one writable
/// `u32`; both must stay valid for the duration of the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sillstone_digest_crc32(data: *const u8, len: usize, out: *mut u32) -> i32 {
    // A null `data` with `len == 0` is a valid empty preimage; only a
    // null out-slot, or a null data pointer that claims length, is a
    // caller bug.
    if out.is_null() || (data.is_null() && len > 0) {
        return PITH_E_INVALID;
    }
    let bytes = borrow(data, len);
    unsafe { *out = crc32(bytes) };
    PITH_OK
}

/// The Adler-32 (RFC 1950) of `data`, written through `out`.
///
/// # Safety
///
/// `data` must point to `len` readable bytes and `out` to one writable
/// `u32`; both must stay valid for the duration of the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sillstone_digest_adler32(data: *const u8, len: usize, out: *mut u32) -> i32 {
    // A null `data` with `len == 0` is a valid empty preimage; only a
    // null out-slot, or a null data pointer that claims length, is a
    // caller bug.
    if out.is_null() || (data.is_null() && len > 0) {
        return PITH_E_INVALID;
    }
    let bytes = borrow(data, len);
    unsafe { *out = adler32(bytes) };
    PITH_OK
}

/// The FNV-1a 64 of `data`, written through `out`.
///
/// # Safety
///
/// `data` must point to `len` readable bytes and `out` to one writable
/// `u64`; both must stay valid for the duration of the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sillstone_digest_fnv1a64(data: *const u8, len: usize, out: *mut u64) -> i32 {
    // A null `data` with `len == 0` is a valid empty preimage; only a
    // null out-slot, or a null data pointer that claims length, is a
    // caller bug.
    if out.is_null() || (data.is_null() && len > 0) {
        return PITH_E_INVALID;
    }
    let bytes = borrow(data, len);
    unsafe { *out = fnv1a64(bytes) };
    PITH_OK
}

/// Fills `out` with the first `count` sequential splitmix64 outputs of
/// a generator seeded with `seed` — exactly the byte stream
/// [`SplitMix64::fill_bytes`] derives its words from, one `u64` per
/// step.
///
/// # Safety
///
/// `out` must point to `count` writable `u64`s and stay valid for the
/// duration of the call. `count` may be zero (a no-op).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sillstone_digest_splitmix64_fill(
    seed: u64,
    out: *mut u64,
    count: usize,
) -> i32 {
    // A null `out` with `count == 0` is a no-op, not a refusal.
    if out.is_null() && count > 0 {
        return PITH_E_INVALID;
    }
    let slots = borrow_mut(out, count);
    let mut rng = SplitMix64::new(seed);
    for slot in slots {
        *slot = rng.next_u64();
    }
    PITH_OK
}

/// The mutable byte-buffer counterpart of [`borrow`] for sized
/// out-buffers (the base64 pair).
fn borrow_mut_bytes<'a>(out: *mut u8, cap: usize) -> &'a mut [u8] {
    if out.is_null() {
        &mut []
    } else {
        unsafe { core::slice::from_raw_parts_mut(out, cap) }
    }
}

/// The SHA-1 digest of `data`, written as 20 raw bytes through `out`.
///
/// # Safety
///
/// `data` must point to `len` readable bytes and `out` to 20 writable
/// bytes; both must stay valid for the duration of the call. The
/// function retains nothing.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sillstone_digest_sha1(data: *const u8, len: usize, out: *mut u8) -> i32 {
    if out.is_null() || (data.is_null() && len > 0) {
        return PITH_E_INVALID;
    }
    let bytes = borrow(data, len);
    match crate::sha1(bytes) {
        Ok(digest) => {
            unsafe { core::ptr::copy_nonoverlapping(digest.as_bytes().as_ptr(), out, 20) };
            PITH_OK
        }
        Err(_) => PITH_E_REJECTED,
    }
}

/// The SHA-512 digest of `data`, written as 64 raw bytes through
/// `out`.
///
/// # Safety
///
/// `data` must point to `len` readable bytes and `out` to 64 writable
/// bytes; both must stay valid for the duration of the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sillstone_digest_sha512(data: *const u8, len: usize, out: *mut u8) -> i32 {
    if out.is_null() || (data.is_null() && len > 0) {
        return PITH_E_INVALID;
    }
    let bytes = borrow(data, len);
    let digest = crate::sha512(bytes);
    unsafe { core::ptr::copy_nonoverlapping(digest.as_bytes().as_ptr(), out, 64) };
    PITH_OK
}

/// The HMAC-SHA-1 of `data` under `key`, written as 20 raw bytes
/// through `out`.
///
/// # Safety
///
/// `key` must point to `key_len` readable bytes, `data` to `len`
/// readable bytes and `out` to 20 writable bytes; all must stay valid
/// for the duration of the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sillstone_digest_hmac_sha1(
    key: *const u8,
    key_len: usize,
    data: *const u8,
    len: usize,
    out: *mut u8,
) -> i32 {
    unsafe {
        hmac_export(key, key_len, data, len, out, |k, d| {
            crate::hmac_sha1(k, d).map(|digest| *digest.as_bytes())
        })
    }
}

/// The HMAC-SHA-256 of `data` under `key`, written as 32 raw bytes
/// through `out`.
///
/// # Safety
///
/// `key` must point to `key_len` readable bytes, `data` to `len`
/// readable bytes and `out` to 32 writable bytes; all must stay valid
/// for the duration of the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sillstone_digest_hmac_sha256(
    key: *const u8,
    key_len: usize,
    data: *const u8,
    len: usize,
    out: *mut u8,
) -> i32 {
    unsafe {
        hmac_export(key, key_len, data, len, out, |k, d| {
            crate::hmac_sha256(k, d).map(|digest| *digest.as_bytes())
        })
    }
}

/// The HMAC-SHA-512 of `data` under `key`, written as 64 raw bytes
/// through `out`.
///
/// # Safety
///
/// `key` must point to `key_len` readable bytes, `data` to `len`
/// readable bytes and `out` to 64 writable bytes; all must stay valid
/// for the duration of the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sillstone_digest_hmac_sha512(
    key: *const u8,
    key_len: usize,
    data: *const u8,
    len: usize,
    out: *mut u8,
) -> i32 {
    unsafe {
        hmac_export(key, key_len, data, len, out, |k, d| {
            Ok(*crate::hmac_sha512(k, d).as_bytes())
        })
    }
}

/// The shared body of the three HMAC exports: validates the pointers,
/// runs `hmac`, copies the `N`-byte digest.
///
/// # Safety
///
/// The caller has already validated every pointer's writability
/// contract; this helper only centralizes the null checks and the
/// copy.
unsafe fn hmac_export<const N: usize>(
    key: *const u8,
    key_len: usize,
    data: *const u8,
    len: usize,
    out: *mut u8,
    hmac: impl FnOnce(&[u8], &[u8]) -> Result<[u8; N]>,
) -> i32 {
    if out.is_null() || (key.is_null() && key_len > 0) || (data.is_null() && len > 0) {
        return PITH_E_INVALID;
    }
    let key_bytes = borrow(key, key_len);
    let data_bytes = borrow(data, len);
    match hmac(key_bytes, data_bytes) {
        Ok(digest) => {
            unsafe { core::ptr::copy_nonoverlapping(digest.as_ptr(), out, N) };
            PITH_OK
        }
        Err(_) => PITH_E_REJECTED,
    }
}

/// The XXH64 hash of `data` under `seed`, written through `out`.
///
/// # Safety
///
/// `data` must point to `len` readable bytes and `out` to one writable
/// `u64`; both must stay valid for the duration of the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sillstone_digest_xxh64(
    data: *const u8,
    len: usize,
    seed: u64,
    out: *mut u64,
) -> i32 {
    if out.is_null() || (data.is_null() && len > 0) {
        return PITH_E_INVALID;
    }
    let bytes = borrow(data, len);
    unsafe { *out = crate::xxh64(bytes, seed) };
    PITH_OK
}

/// The MurmurHash3 x64 128-bit digest of `data` under `seed`, written
/// as 16 raw bytes (h1 little-endian, then h2 little-endian) through
/// `out`.
///
/// # Safety
///
/// `data` must point to `len` readable bytes and `out` to 16 writable
/// bytes; both must stay valid for the duration of the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sillstone_digest_murmur3_x64_128(
    data: *const u8,
    len: usize,
    seed: u32,
    out: *mut u8,
) -> i32 {
    if out.is_null() || (data.is_null() && len > 0) {
        return PITH_E_INVALID;
    }
    let bytes = borrow(data, len);
    let digest = crate::murmur3_x64_128(bytes, seed);
    unsafe { core::ptr::copy_nonoverlapping(digest.as_bytes().as_ptr(), out, 16) };
    PITH_OK
}

/// The CRC-32C (Castagnoli, RFC 3720 B.4) of `data`, written through
/// `out`.
///
/// # Safety
///
/// `data` must point to `len` readable bytes and `out` to one writable
/// `u32`; both must stay valid for the duration of the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sillstone_digest_crc32c(data: *const u8, len: usize, out: *mut u32) -> i32 {
    if out.is_null() || (data.is_null() && len > 0) {
        return PITH_E_INVALID;
    }
    let bytes = borrow(data, len);
    unsafe { *out = crate::crc32c(bytes) };
    PITH_OK
}

/// Encodes `data` as canonical padded base64 into `out`, writing the
/// encoded length through `out_len` (RFC 4648 §4).
///
/// # Safety
///
/// `data` must point to `len` readable bytes, `out` to `cap` writable
/// bytes and `out_len` to one writable `usize`; all must stay valid
/// for the duration of the call. [`PITH_E_REJECTED`] means `cap` is
/// below [`crate::base64::encoded_len`]`(len)` and nothing was
/// written.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sillstone_digest_base64_encode(
    data: *const u8,
    len: usize,
    out: *mut u8,
    cap: usize,
    out_len: *mut usize,
) -> i32 {
    if out_len.is_null() || out.is_null() || (data.is_null() && len > 0) {
        return PITH_E_INVALID;
    }
    let bytes = borrow(data, len);
    let buffer = borrow_mut_bytes(out, cap);
    match crate::base64::base64_encode(bytes, buffer) {
        Ok(()) => {
            unsafe { *out_len = crate::base64::encoded_len(len) };
            PITH_OK
        }
        Err(_) => PITH_E_REJECTED,
    }
}

/// Decodes canonical padded base64 from `data` into `out`, writing
/// the decoded length through `out_len` (RFC 4648 §4).
///
/// # Safety
///
/// `data` must point to `len` readable bytes, `out` to `cap` writable
/// bytes and `out_len` to one writable `usize`; all must stay valid
/// for the duration of the call. [`PITH_E_REJECTED`] means `cap` is
/// below `len / 4 * 3` or the input is not canonical base64; nothing
/// was written.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sillstone_digest_base64_decode(
    data: *const u8,
    len: usize,
    out: *mut u8,
    cap: usize,
    out_len: *mut usize,
) -> i32 {
    if out_len.is_null() || out.is_null() || (data.is_null() && len > 0) {
        return PITH_E_INVALID;
    }
    let bytes = borrow(data, len);
    let buffer = borrow_mut_bytes(out, cap);
    match crate::base64::base64_decode(bytes, buffer) {
        Ok(written) => {
            unsafe { *out_len = written };
            PITH_OK
        }
        Err(_) => PITH_E_REJECTED,
    }
}

/// Fills `out` with the first `count` sequential xoshiro256** outputs
/// of a generator seeded with `seed` (the splitmix64 expansion the
/// reference implementation recommends).
///
/// # Safety
///
/// `out` must point to `count` writable `u64`s and stay valid for the
/// duration of the call. `count` may be zero (a no-op).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sillstone_digest_xoshiro256_fill(
    seed: u64,
    out: *mut u64,
    count: usize,
) -> i32 {
    if out.is_null() && count > 0 {
        return PITH_E_INVALID;
    }
    let slots = borrow_mut(out, count);
    let mut rng = crate::Xoshiro256StarStar::from_seed(seed);
    for slot in slots {
        *slot = rng.next_u64();
    }
    PITH_OK
}

#[cfg(test)]
mod tests {
    use super::{
        PITH_E_INVALID, PITH_OK, sillstone_digest_adler32, sillstone_digest_crc32, sillstone_digest_fnv1a64,
        sillstone_digest_sha256, sillstone_digest_splitmix64_fill,
    };
    use crate::sha256;

    /// The empty SHA-256 through raw pointers: the FIPS 180-2 vector.
    #[test]
    fn ffi_sha256_reproduces_the_empty_digest() {
        let mut out = [0u8; 32];
        let status = unsafe { sillstone_digest_sha256(b"abc".as_ptr(), 3, out.as_mut_ptr()) };
        assert_eq!(status, PITH_OK);
        let expect = sha256(b"abc").expect("within size limit");
        assert_eq!(out, *expect.as_bytes());
    }

    /// The checksums through raw pointers against their published
    /// vectors: CRC-32 of "123456789", Adler-32 of "Wikipedia",
    /// FNV-1a 64 of "foobar".
    #[test]
    fn ffi_checksums_reproduce_published_vectors() {
        let mut c = 0u32;
        assert_eq!(
            unsafe { sillstone_digest_crc32(b"123456789".as_ptr(), 9, &mut c) },
            PITH_OK
        );
        assert_eq!(c, 0xcbf4_3926);

        let mut a = 0u32;
        assert_eq!(
            unsafe { sillstone_digest_adler32(b"Wikipedia".as_ptr(), 9, &mut a) },
            PITH_OK
        );
        assert_eq!(a, 0x11e6_0398);

        let mut f = 0u64;
        assert_eq!(
            unsafe { sillstone_digest_fnv1a64(b"foobar".as_ptr(), 6, &mut f) },
            PITH_OK
        );
        assert_eq!(f, 0x8594_4171_f739_67e8);
    }

    /// splitmix64 through raw pointers: seed 0's first two outputs,
    /// a zero count no-op and the null refusal.
    #[test]
    fn ffi_splitmix64_fill_reproduces_the_paper_sequence() {
        let mut out = [0u64; 2];
        assert_eq!(
            unsafe { sillstone_digest_splitmix64_fill(0, out.as_mut_ptr(), 2) },
            PITH_OK
        );
        assert_eq!(out, [0xe220_a839_7b1d_cdaf, 0x6e78_9e6a_a1b9_65f4]);

        let mut zero: [u64; 0] = [];
        assert_eq!(
            unsafe { sillstone_digest_splitmix64_fill(0, zero.as_mut_ptr(), 0) },
            PITH_OK
        );
        assert_eq!(
            unsafe { sillstone_digest_splitmix64_fill(0, core::ptr::null_mut(), 1) },
            PITH_E_INVALID
        );
    }

    /// A null `data` with `len == 0` is the valid empty preimage: the
    /// FIPS 180-2 empty digest through the null-pointer path.
    #[test]
    fn ffi_null_data_with_zero_len_is_the_empty_preimage() {
        let mut out = [0u8; 32];
        let status = unsafe { sillstone_digest_sha256(core::ptr::null(), 0, out.as_mut_ptr()) };
        assert_eq!(status, PITH_OK);
        let expect = sha256(b"").expect("within size limit");
        assert_eq!(out, *expect.as_bytes());

        let mut c = 0u32;
        assert_eq!(
            unsafe { sillstone_digest_crc32(core::ptr::null(), 0, &mut c) },
            PITH_OK
        );
        assert_eq!(c, 0);
    }

    /// A null `data` that claims a nonzero length is a refusal.
    #[test]
    fn ffi_null_data_with_length_is_refused() {
        let mut out = [0u8; 32];
        let status = unsafe { sillstone_digest_sha256(core::ptr::null(), 1, out.as_mut_ptr()) };
        assert_eq!(status, PITH_E_INVALID);
    }

    /// Null output slots are refusals, never a write through null.
    #[test]
    fn ffi_null_out_slots_are_refused() {
        let data = b"x";
        unsafe {
            assert_eq!(
                sillstone_digest_sha256(data.as_ptr(), 1, core::ptr::null_mut()),
                PITH_E_INVALID
            );
            assert_eq!(
                sillstone_digest_crc32(data.as_ptr(), 1, core::ptr::null_mut()),
                PITH_E_INVALID
            );
            assert_eq!(
                sillstone_digest_adler32(data.as_ptr(), 1, core::ptr::null_mut()),
                PITH_E_INVALID
            );
            assert_eq!(
                sillstone_digest_fnv1a64(data.as_ptr(), 1, core::ptr::null_mut()),
                PITH_E_INVALID
            );
        }
    }
}
