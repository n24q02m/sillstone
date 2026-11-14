//! The C ABI of the SHA-3, BLAKE2b, BLAKE3 and HMAC exports: each one must
//! reproduce its library counterpart byte for byte and refuse invalid
//! pointer arguments without touching memory.

use core::ptr::{null, null_mut};
use sillstone_digest::{
    blake2b, blake3, blake3_keyed, blake3_xof, ffi, hmac_sha1, hmac_sha256, hmac_sha512, sha3_256,
    sha3_512,
};

const DATA: &[u8] = b"The quick brown fox jumps over the lazy dog";
const KEY: &[u8; 32] = b"whats the Elvish word for friend";

#[test]
fn sha3_exports_match_the_library() {
    let mut out32 = [0u8; 32];
    let mut out64 = [0u8; 64];
    unsafe {
        assert_eq!(
            ffi::sillstone_digest_sha3_256(DATA.as_ptr(), DATA.len(), out32.as_mut_ptr()),
            ffi::SILLSTONE_OK
        );
        assert_eq!(
            ffi::sillstone_digest_sha3_512(DATA.as_ptr(), DATA.len(), out64.as_mut_ptr()),
            ffi::SILLSTONE_OK
        );
    }
    assert_eq!(out32, *sha3_256(DATA).unwrap().as_bytes());
    assert_eq!(out64, *sha3_512(DATA).unwrap().as_bytes());

    // A null data pointer with length zero is the empty preimage.
    let mut empty = [0u8; 32];
    unsafe {
        assert_eq!(
            ffi::sillstone_digest_sha3_256(null(), 0, empty.as_mut_ptr()),
            ffi::SILLSTONE_OK
        );
    }
    assert_eq!(empty, *sha3_256(b"").unwrap().as_bytes());

    unsafe {
        assert_eq!(
            ffi::sillstone_digest_sha3_256(DATA.as_ptr(), 1, null_mut()),
            ffi::SILLSTONE_E_INVALID
        );
        assert_eq!(
            ffi::sillstone_digest_sha3_256(null(), 1, out32.as_mut_ptr()),
            ffi::SILLSTONE_E_INVALID
        );
        assert_eq!(
            ffi::sillstone_digest_sha3_512(DATA.as_ptr(), 1, null_mut()),
            ffi::SILLSTONE_E_INVALID
        );
        assert_eq!(
            ffi::sillstone_digest_sha3_512(null(), 1, out64.as_mut_ptr()),
            ffi::SILLSTONE_E_INVALID
        );
    }
}

#[test]
fn blake2b_exports_match_the_library() {
    let mut keyed = [0u8; 40];
    unsafe {
        assert_eq!(
            ffi::sillstone_digest_blake2b(
                DATA.as_ptr(),
                DATA.len(),
                KEY.as_ptr(),
                KEY.len(),
                keyed.as_mut_ptr(),
                keyed.len()
            ),
            ffi::SILLSTONE_OK
        );
    }
    assert_eq!(keyed[..], blake2b(DATA, KEY, 40).unwrap()[..]);

    let mut full = [0u8; 64];
    unsafe {
        assert_eq!(
            ffi::sillstone_digest_blake2b_512(DATA.as_ptr(), DATA.len(), full.as_mut_ptr()),
            ffi::SILLSTONE_OK
        );
    }
    assert_eq!(full[..], blake2b(DATA, b"", 64).unwrap()[..]);

    unsafe {
        // Null out, null data with length, null key with key length.
        assert_eq!(
            ffi::sillstone_digest_blake2b(DATA.as_ptr(), 1, null(), 0, null_mut(), 32),
            ffi::SILLSTONE_E_INVALID
        );
        assert_eq!(
            ffi::sillstone_digest_blake2b(null(), 1, null(), 0, full.as_mut_ptr(), 32),
            ffi::SILLSTONE_E_INVALID
        );
        assert_eq!(
            ffi::sillstone_digest_blake2b(DATA.as_ptr(), 1, null(), 4, full.as_mut_ptr(), 32),
            ffi::SILLSTONE_E_INVALID
        );
        // The core refuses a zero or oversized output and an oversized key.
        assert_eq!(
            ffi::sillstone_digest_blake2b(DATA.as_ptr(), 1, null(), 0, full.as_mut_ptr(), 0),
            ffi::SILLSTONE_E_REJECTED
        );
        let mut big = [0u8; 65];
        assert_eq!(
            ffi::sillstone_digest_blake2b(DATA.as_ptr(), 1, null(), 0, big.as_mut_ptr(), 65),
            ffi::SILLSTONE_E_REJECTED
        );
        assert_eq!(
            ffi::sillstone_digest_blake2b(
                DATA.as_ptr(),
                1,
                big.as_ptr(),
                65,
                full.as_mut_ptr(),
                32
            ),
            ffi::SILLSTONE_E_REJECTED
        );
        assert_eq!(
            ffi::sillstone_digest_blake2b_512(DATA.as_ptr(), 1, null_mut()),
            ffi::SILLSTONE_E_INVALID
        );
    }
}

#[test]
fn blake3_exports_match_the_library() {
    let mut plain = [0u8; 32];
    let mut keyed = [0u8; 32];
    let mut xof = [0u8; 100];
    unsafe {
        assert_eq!(
            ffi::sillstone_digest_blake3(DATA.as_ptr(), DATA.len(), plain.as_mut_ptr()),
            ffi::SILLSTONE_OK
        );
        assert_eq!(
            ffi::sillstone_digest_blake3_keyed(
                KEY.as_ptr(),
                DATA.as_ptr(),
                DATA.len(),
                keyed.as_mut_ptr()
            ),
            ffi::SILLSTONE_OK
        );
        assert_eq!(
            ffi::sillstone_digest_blake3_xof(
                DATA.as_ptr(),
                DATA.len(),
                xof.as_mut_ptr(),
                xof.len()
            ),
            ffi::SILLSTONE_OK
        );
    }
    assert_eq!(plain, *blake3(DATA).unwrap().as_bytes());
    assert_eq!(keyed, *blake3_keyed(KEY, DATA).unwrap().as_bytes());
    let mut expected = [0u8; 100];
    blake3_xof(DATA, &mut expected).unwrap();
    assert_eq!(xof, expected);

    unsafe {
        assert_eq!(
            ffi::sillstone_digest_blake3(DATA.as_ptr(), 1, null_mut()),
            ffi::SILLSTONE_E_INVALID
        );
        assert_eq!(
            ffi::sillstone_digest_blake3(null(), 1, plain.as_mut_ptr()),
            ffi::SILLSTONE_E_INVALID
        );
        assert_eq!(
            ffi::sillstone_digest_blake3_keyed(null(), DATA.as_ptr(), 1, keyed.as_mut_ptr()),
            ffi::SILLSTONE_E_INVALID
        );
        assert_eq!(
            ffi::sillstone_digest_blake3_keyed(KEY.as_ptr(), DATA.as_ptr(), 1, null_mut()),
            ffi::SILLSTONE_E_INVALID
        );
        assert_eq!(
            ffi::sillstone_digest_blake3_keyed(KEY.as_ptr(), null(), 1, keyed.as_mut_ptr()),
            ffi::SILLSTONE_E_INVALID
        );
        assert_eq!(
            ffi::sillstone_digest_blake3_xof(DATA.as_ptr(), 1, null_mut(), 8),
            ffi::SILLSTONE_E_INVALID
        );
        assert_eq!(
            ffi::sillstone_digest_blake3_xof(null(), 1, xof.as_mut_ptr(), 8),
            ffi::SILLSTONE_E_INVALID
        );
    }
}

#[test]
fn blake3_xof_with_no_output_is_an_accepted_no_op() {
    // A zero-length request is valid even with a null out pointer; the
    // export must not build a slice from the null.
    unsafe {
        assert_eq!(
            ffi::sillstone_digest_blake3_xof(DATA.as_ptr(), DATA.len(), null_mut(), 0),
            ffi::SILLSTONE_OK
        );
        assert_eq!(
            ffi::sillstone_digest_blake3_xof(null(), 0, null_mut(), 0),
            ffi::SILLSTONE_OK
        );
    }
}

#[test]
fn hmac_exports_match_the_library() {
    let key = b"Jefe";
    let mut o1 = [0u8; 20];
    let mut o256 = [0u8; 32];
    let mut o512 = [0u8; 64];
    unsafe {
        assert_eq!(
            ffi::sillstone_digest_hmac_sha1(
                key.as_ptr(),
                key.len(),
                DATA.as_ptr(),
                DATA.len(),
                o1.as_mut_ptr()
            ),
            ffi::SILLSTONE_OK
        );
        assert_eq!(
            ffi::sillstone_digest_hmac_sha256(
                key.as_ptr(),
                key.len(),
                DATA.as_ptr(),
                DATA.len(),
                o256.as_mut_ptr()
            ),
            ffi::SILLSTONE_OK
        );
        assert_eq!(
            ffi::sillstone_digest_hmac_sha512(
                key.as_ptr(),
                key.len(),
                DATA.as_ptr(),
                DATA.len(),
                o512.as_mut_ptr()
            ),
            ffi::SILLSTONE_OK
        );
    }
    assert_eq!(o1, *hmac_sha1(key, DATA).unwrap().as_bytes());
    assert_eq!(o256, *hmac_sha256(key, DATA).unwrap().as_bytes());
    assert_eq!(o512, *hmac_sha512(key, DATA).as_bytes());

    // An empty key and message may be passed as null pointers.
    unsafe {
        assert_eq!(
            ffi::sillstone_digest_hmac_sha512(null(), 0, null(), 0, o512.as_mut_ptr()),
            ffi::SILLSTONE_OK
        );
    }
    assert_eq!(o512, *hmac_sha512(b"", b"").as_bytes());

    unsafe {
        assert_eq!(
            ffi::sillstone_digest_hmac_sha1(key.as_ptr(), 4, DATA.as_ptr(), 1, null_mut()),
            ffi::SILLSTONE_E_INVALID
        );
        assert_eq!(
            ffi::sillstone_digest_hmac_sha1(null(), 4, DATA.as_ptr(), 1, o1.as_mut_ptr()),
            ffi::SILLSTONE_E_INVALID
        );
        assert_eq!(
            ffi::sillstone_digest_hmac_sha256(key.as_ptr(), 4, null(), 1, o256.as_mut_ptr()),
            ffi::SILLSTONE_E_INVALID
        );
        assert_eq!(
            ffi::sillstone_digest_hmac_sha512(null(), 1, DATA.as_ptr(), 1, o512.as_mut_ptr()),
            ffi::SILLSTONE_E_INVALID
        );
    }
}
