// SPDX-License-Identifier: MIT
// Copyright (c) 2026 sillstone-curator
//! Conformance: every published reference vector, replayed through the
//! library AND its C ABI surface.

use sillstone_digest::{SplitMix64, adler32, crc32, ffi, fnv1a64, sha256};

/// The FFI surface reproduces the library's own results byte-for-byte
/// (one representative vector per operation; the full vector corpus is
/// the lib unit tests' job).
#[test]
fn ffi_surface_matches_the_library() {
    // sha256: FIPS 180-2 "abc".
    let mut out = [0u8; 32];
    let status = unsafe { ffi::sillstone_digest_sha256(b"abc".as_ptr(), 3, out.as_mut_ptr()) };
    assert_eq!(status, ffi::PITH_OK);
    assert_eq!(out, *sha256(b"abc").expect("within size limit").as_bytes());

    // crc32 of "123456789".
    let mut c = 0u32;
    let status = unsafe { ffi::sillstone_digest_crc32(b"123456789".as_ptr(), 9, &mut c) };
    assert_eq!(status, ffi::PITH_OK);
    assert_eq!(c, crc32(b"123456789"));
    assert_eq!(c, 0xcbf4_3926);

    // adler32 of "Wikipedia".
    let mut a = 0u32;
    let status = unsafe { ffi::sillstone_digest_adler32(b"Wikipedia".as_ptr(), 9, &mut a) };
    assert_eq!(status, ffi::PITH_OK);
    assert_eq!(a, adler32(b"Wikipedia"));
    assert_eq!(a, 0x11e6_0398);

    // fnv1a64 of "foobar".
    let mut f = 0u64;
    let status = unsafe { ffi::sillstone_digest_fnv1a64(b"foobar".as_ptr(), 6, &mut f) };
    assert_eq!(status, ffi::PITH_OK);
    assert_eq!(f, fnv1a64(b"foobar"));
    assert_eq!(f, 0x8594_4171_f739_67e8);

    // splitmix64: the paper's seed-0 sequence.
    let mut seq = [0u64; 2];
    let status = unsafe { ffi::sillstone_digest_splitmix64_fill(0, seq.as_mut_ptr(), 2) };
    assert_eq!(status, ffi::PITH_OK);
    let mut rng = SplitMix64::new(0);
    assert_eq!(seq, [rng.next_u64(), rng.next_u64()]);

    // Refusals: null out-slots are invalid arguments, never a write
    // through null; null data with length is refused.
    unsafe {
        assert_eq!(
            ffi::sillstone_digest_sha256(core::ptr::null(), 1, out.as_mut_ptr()),
            ffi::PITH_E_INVALID
        );
        assert_eq!(
            ffi::sillstone_digest_crc32(b"x".as_ptr(), 1, core::ptr::null_mut()),
            ffi::PITH_E_INVALID
        );
        assert_eq!(
            ffi::sillstone_digest_adler32(b"x".as_ptr(), 1, core::ptr::null_mut()),
            ffi::PITH_E_INVALID
        );
        assert_eq!(
            ffi::sillstone_digest_fnv1a64(b"x".as_ptr(), 1, core::ptr::null_mut()),
            ffi::PITH_E_INVALID
        );
        assert_eq!(
            ffi::sillstone_digest_splitmix64_fill(0, core::ptr::null_mut(), 1),
            ffi::PITH_E_INVALID
        );
        // null data with zero length is the valid empty preimage.
        let status = ffi::sillstone_digest_sha256(core::ptr::null(), 0, out.as_mut_ptr());
        assert_eq!(status, ffi::PITH_OK);
        assert_eq!(out, *sha256(b"").expect("within size limit").as_bytes());
    }
}

/// The tier-1 exports reproduce the library's own results
/// byte-for-byte, one representative vector per operation, with the
/// same refusal discipline as the original surface.
#[test]
fn ffi_surface_covers_the_tier_one_exports() {
    // sha1: FIPS 180-4 "abc".
    let mut d20 = [0u8; 20];
    let status = unsafe { ffi::sillstone_digest_sha1(b"abc".as_ptr(), 3, d20.as_mut_ptr()) };
    assert_eq!(status, ffi::PITH_OK);
    assert_eq!(
        d20,
        *sillstone_digest::sha1(b"abc")
            .expect("within size limit")
            .as_bytes()
    );

    // sha512: FIPS 180-4 "abc".
    let mut d64 = [0u8; 64];
    let status = unsafe { ffi::sillstone_digest_sha512(b"abc".as_ptr(), 3, d64.as_mut_ptr()) };
    assert_eq!(status, ffi::PITH_OK);
    assert_eq!(d64, *sillstone_digest::sha512(b"abc").as_bytes());

    // hmac_sha256: RFC 2104 test case 1.
    let key = [0x0b_u8; 20];
    let mut mac = [0u8; 32];
    let status = unsafe {
        ffi::sillstone_digest_hmac_sha256(key.as_ptr(), 20, b"Hi There".as_ptr(), 8, mac.as_mut_ptr())
    };
    assert_eq!(status, ffi::PITH_OK);
    assert_eq!(
        hex(&mac),
        "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"
    );

    // xxh64: the reference's empty-input constant at seed 0.
    let mut h = 0u64;
    let status = unsafe { ffi::sillstone_digest_xxh64(core::ptr::null(), 0, 0, &mut h) };
    assert_eq!(status, ffi::PITH_OK);
    assert_eq!(h, 0xEF46_DB37_51D8_E999);

    // murmur3 x64 128: the all-zero empty-input digest.
    let mut d16 = [0u8; 16];
    let status =
        unsafe { ffi::sillstone_digest_murmur3_x64_128(core::ptr::null(), 0, 0, d16.as_mut_ptr()) };
    assert_eq!(status, ffi::PITH_OK);
    assert_eq!(d16, [0u8; 16]);

    // crc32c: the RFC 4960 check value of "123456789".
    let mut c = 0u32;
    let status = unsafe { ffi::sillstone_digest_crc32c(b"123456789".as_ptr(), 9, &mut c) };
    assert_eq!(status, ffi::PITH_OK);
    assert_eq!(c, 0xe306_9283);

    // base64: RFC 4648 "foobar" round-trip through the C ABI.
    let mut enc = [0u8; 12];
    let mut enc_len = 0usize;
    let status = unsafe {
        ffi::sillstone_digest_base64_encode(b"foobar".as_ptr(), 6, enc.as_mut_ptr(), 12, &mut enc_len)
    };
    assert_eq!(status, ffi::PITH_OK);
    assert_eq!(&enc[..enc_len], b"Zm9vYmFy");
    let mut dec = [0u8; 6];
    let mut dec_len = 0usize;
    let status = unsafe {
        ffi::sillstone_digest_base64_decode(enc.as_ptr(), enc_len, dec.as_mut_ptr(), 6, &mut dec_len)
    };
    assert_eq!(status, ffi::PITH_OK);
    assert_eq!(&dec[..dec_len], b"foobar");

    // xoshiro256**: the reference seed-0 sequence.
    let mut slots = [0u64; 2];
    let status = unsafe { ffi::sillstone_digest_xoshiro256_fill(0, slots.as_mut_ptr(), 2) };
    assert_eq!(status, ffi::PITH_OK);
    let mut rng = sillstone_digest::Xoshiro256StarStar::from_seed(0);
    assert_eq!(slots, [rng.next_u64(), rng.next_u64()]);

    // Refusals: null out-slots are invalid arguments, never a write
    // through null; a non-canonical base64 decode is rejected.
    unsafe {
        assert_eq!(
            ffi::sillstone_digest_sha1(b"x".as_ptr(), 1, core::ptr::null_mut()),
            ffi::PITH_E_INVALID
        );
        assert_eq!(
            ffi::sillstone_digest_sha512(b"x".as_ptr(), 1, core::ptr::null_mut()),
            ffi::PITH_E_INVALID
        );
        assert_eq!(
            ffi::sillstone_digest_hmac_sha256(b"x".as_ptr(), 1, b"y".as_ptr(), 1, core::ptr::null_mut()),
            ffi::PITH_E_INVALID
        );
        assert_eq!(
            ffi::sillstone_digest_xxh64(b"x".as_ptr(), 1, 0, core::ptr::null_mut()),
            ffi::PITH_E_INVALID
        );
        assert_eq!(
            ffi::sillstone_digest_murmur3_x64_128(b"x".as_ptr(), 1, 0, core::ptr::null_mut()),
            ffi::PITH_E_INVALID
        );
        assert_eq!(
            ffi::sillstone_digest_crc32c(b"x".as_ptr(), 1, core::ptr::null_mut()),
            ffi::PITH_E_INVALID
        );
        assert_eq!(
            ffi::sillstone_digest_xoshiro256_fill(0, core::ptr::null_mut(), 1),
            ffi::PITH_E_INVALID
        );
        // An encode whose output buffer is too small is rejected, not
        // truncated.
        let mut enc_len = 0usize;
        assert_eq!(
            ffi::sillstone_digest_base64_encode(
                b"foobar".as_ptr(),
                6,
                enc.as_mut_ptr(),
                3,
                &mut enc_len
            ),
            ffi::PITH_E_REJECTED
        );
        let mut dec_len = 0usize;
        assert_eq!(
            ffi::sillstone_digest_base64_decode(b"Zy==".as_ptr(), 4, dec.as_mut_ptr(), 6, &mut dec_len),
            ffi::PITH_E_REJECTED
        );
    }
}

/// Lowercase hex of a byte slice, for the pinned HMAC vector.
fn hex(bytes: &[u8]) -> String {
    let mut out = String::new();
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
}
