//! Refusal paths of the base64 and PRNG C ABI exports that the surface
//! tests leave unexercised: null guards, undersized output buffers and
//! non-canonical decode inputs. Every refusal asserts the exact status
//! code and that the caller's out-slots keep their sentinel value; each
//! one is paired with a control call that succeeds on the same shape of
//! input, so a refusal cannot pass by the export refusing everything.
//!
//! The external oracle is the RFC 4648 section 10 test-vector table.

use core::ptr::{null, null_mut};
use sillstone_digest::ffi;

/// RFC 4648 section 10: `BASE64("")` through `BASE64("foobar")`.
const RFC_4648_S10: [(&[u8], &[u8]); 7] = [
    (b"", b""),
    (b"f", b"Zg=="),
    (b"fo", b"Zm8="),
    (b"foo", b"Zm9v"),
    (b"foob", b"Zm9vYg=="),
    (b"fooba", b"Zm9vYmE="),
    (b"foobar", b"Zm9vYmFy"),
];

/// A byte the export must never write over on a refusal.
const SENTINEL: u8 = 0xA5;

/// An `out_len` the export must never write over on a refusal.
const LEN_SENTINEL: usize = usize::MAX;

fn encode(data: &[u8], cap: usize) -> (i32, Vec<u8>, usize) {
    let mut out = vec![SENTINEL; 32];
    let mut out_len = LEN_SENTINEL;
    let status = unsafe {
        ffi::sillstone_digest_base64_encode(
            data.as_ptr(),
            data.len(),
            out.as_mut_ptr(),
            cap,
            &mut out_len,
        )
    };
    (status, out, out_len)
}

fn decode(data: &[u8], cap: usize) -> (i32, Vec<u8>, usize) {
    let mut out = vec![SENTINEL; 32];
    let mut out_len = LEN_SENTINEL;
    let status = unsafe {
        ffi::sillstone_digest_base64_decode(
            data.as_ptr(),
            data.len(),
            out.as_mut_ptr(),
            cap,
            &mut out_len,
        )
    };
    (status, out, out_len)
}

#[test]
fn base64_rfc_4648_vectors_pass_through_the_c_abi_both_ways() {
    for (plain, encoded) in RFC_4648_S10 {
        let (status, out, out_len) = encode(plain, 32);
        assert_eq!(status, ffi::PITH_OK, "encode {plain:?}");
        assert_eq!(&out[..out_len], encoded, "encode {plain:?}");
        let (status, out, out_len) = decode(encoded, 32);
        assert_eq!(status, ffi::PITH_OK, "decode {encoded:?}");
        assert_eq!(&out[..out_len], plain, "decode {encoded:?}");
    }
}

#[test]
fn base64_encode_refuses_null_arguments_and_leaves_the_slots_alone() {
    let mut out = [SENTINEL; 8];
    let mut out_len = LEN_SENTINEL;
    unsafe {
        // A null out_len slot.
        assert_eq!(
            ffi::sillstone_digest_base64_encode(
                b"foo".as_ptr(),
                3,
                out.as_mut_ptr(),
                8,
                null_mut()
            ),
            ffi::PITH_E_INVALID
        );
        // A null out buffer.
        assert_eq!(
            ffi::sillstone_digest_base64_encode(b"foo".as_ptr(), 3, null_mut(), 8, &mut out_len),
            ffi::PITH_E_INVALID
        );
        // A null data pointer that claims a length.
        assert_eq!(
            ffi::sillstone_digest_base64_encode(null(), 3, out.as_mut_ptr(), 8, &mut out_len),
            ffi::PITH_E_INVALID
        );
    }
    assert_eq!(out, [SENTINEL; 8], "out buffer untouched");
    assert_eq!(out_len, LEN_SENTINEL, "out_len untouched");

    // Control: the same call with every pointer valid succeeds, and a
    // null data pointer with length zero is the empty input.
    let (status, bytes, written) = encode(b"foo", 8);
    assert_eq!(status, ffi::PITH_OK);
    assert_eq!(&bytes[..written], b"Zm9v");
    let status = unsafe {
        ffi::sillstone_digest_base64_encode(null(), 0, out.as_mut_ptr(), 8, &mut out_len)
    };
    assert_eq!(status, ffi::PITH_OK);
    assert_eq!(out_len, 0, "empty input encodes to zero bytes");
    assert_eq!(out, [SENTINEL; 8], "empty input writes nothing");
}

#[test]
fn base64_decode_refuses_null_arguments_and_leaves_the_slots_alone() {
    let mut out = [SENTINEL; 8];
    let mut out_len = LEN_SENTINEL;
    unsafe {
        assert_eq!(
            ffi::sillstone_digest_base64_decode(
                b"Zm9v".as_ptr(),
                4,
                out.as_mut_ptr(),
                8,
                null_mut()
            ),
            ffi::PITH_E_INVALID
        );
        assert_eq!(
            ffi::sillstone_digest_base64_decode(b"Zm9v".as_ptr(), 4, null_mut(), 8, &mut out_len),
            ffi::PITH_E_INVALID
        );
        assert_eq!(
            ffi::sillstone_digest_base64_decode(null(), 4, out.as_mut_ptr(), 8, &mut out_len),
            ffi::PITH_E_INVALID
        );
    }
    assert_eq!(out, [SENTINEL; 8], "out buffer untouched");
    assert_eq!(out_len, LEN_SENTINEL, "out_len untouched");

    // Control: valid pointers decode; a null data pointer with length
    // zero is the empty input.
    let (status, bytes, written) = decode(b"Zm9v", 8);
    assert_eq!(status, ffi::PITH_OK);
    assert_eq!(&bytes[..written], b"foo");
    let status = unsafe {
        ffi::sillstone_digest_base64_decode(null(), 0, out.as_mut_ptr(), 8, &mut out_len)
    };
    assert_eq!(status, ffi::PITH_OK);
    assert_eq!(out_len, 0, "empty input decodes to zero bytes");
    assert_eq!(out, [SENTINEL; 8], "empty input writes nothing");
}

#[test]
fn base64_encode_refuses_a_capacity_one_byte_short() {
    // "foobar" needs 8 bytes of output.
    let (status, out, out_len) = encode(b"foobar", 7);
    assert_eq!(status, ffi::PITH_E_REJECTED);
    assert_eq!(out, vec![SENTINEL; 32], "nothing written on a short buffer");
    assert_eq!(out_len, LEN_SENTINEL, "out_len untouched");

    // "f" needs 4 (padding included), not 2.
    let (status, out, out_len) = encode(b"f", 3);
    assert_eq!(status, ffi::PITH_E_REJECTED);
    assert_eq!(out, vec![SENTINEL; 32]);
    assert_eq!(out_len, LEN_SENTINEL);

    // Controls: exactly the needed capacity succeeds.
    let (status, out, out_len) = encode(b"foobar", 8);
    assert_eq!(status, ffi::PITH_OK);
    assert_eq!(&out[..out_len], b"Zm9vYmFy");
    let (status, out, out_len) = encode(b"f", 4);
    assert_eq!(status, ffi::PITH_OK);
    assert_eq!(&out[..out_len], b"Zg==");
}

#[test]
fn base64_decode_refuses_a_capacity_one_byte_short() {
    // Eight characters decode to at most six bytes.
    let (status, out, out_len) = decode(b"Zm9vYmFy", 5);
    assert_eq!(status, ffi::PITH_E_REJECTED);
    assert_eq!(out, vec![SENTINEL; 32], "nothing written on a short buffer");
    assert_eq!(out_len, LEN_SENTINEL, "out_len untouched");

    // The worst case is sized by the input length, padding included:
    // "Zg==" decodes to one byte but is refused below three.
    let (status, out, out_len) = decode(b"Zg==", 2);
    assert_eq!(status, ffi::PITH_E_REJECTED);
    assert_eq!(out, vec![SENTINEL; 32]);
    assert_eq!(out_len, LEN_SENTINEL);

    // Controls: exactly the worst-case capacity succeeds.
    let (status, out, out_len) = decode(b"Zm9vYmFy", 6);
    assert_eq!(status, ffi::PITH_OK);
    assert_eq!(&out[..out_len], b"foobar");
    let (status, out, out_len) = decode(b"Zg==", 3);
    assert_eq!(status, ffi::PITH_OK);
    assert_eq!(&out[..out_len], b"f");
}

#[test]
fn base64_decode_refuses_a_length_that_is_not_a_multiple_of_four() {
    for bad in [&b"Z"[..], b"Zg", b"Zm9", b"Zm9vY", b"Zm9vYmF"] {
        let (status, out, out_len) = decode(bad, 32);
        assert_eq!(status, ffi::PITH_E_REJECTED, "{bad:?}");
        assert_eq!(out, vec![SENTINEL; 32], "{bad:?} writes nothing");
        assert_eq!(out_len, LEN_SENTINEL, "{bad:?}");
    }
    // Control: the same prefixes completed to a full group decode.
    let (status, out, out_len) = decode(b"Zg==", 32);
    assert_eq!(status, ffi::PITH_OK);
    assert_eq!(&out[..out_len], b"f");
}

#[test]
fn base64_decode_refuses_non_canonical_text() {
    // Each input is a whole number of groups, so only the content is
    // at fault. Characters outside the alphabet, including a space, a
    // URL-safe alphabet character and a non-ASCII byte.
    let cases: [(&[u8], &str); 8] = [
        (b"Zm9*YmFy", "asterisk"),
        (b"Zm9v YmF", "space (length 8)"),
        (b"Zm-vYmFy", "url-safe minus"),
        (b"Zm_vYmFy", "url-safe underscore"),
        (b"Zm9\xFFYmFy", "non-ASCII byte"),
        // A pad character inside the body, and one followed by data.
        (b"Zm=vYmFy", "pad in the body"),
        (b"Zg=A", "pad followed by a character"),
        // Padding with nothing in front of it.
        (b"====", "padding only"),
    ];
    for (bad, label) in cases {
        let (status, _out, out_len) = decode(bad, 32);
        assert_eq!(status, ffi::PITH_E_REJECTED, "{label}");
        assert_eq!(out_len, LEN_SENTINEL, "{label}: out_len untouched");
    }
    // Control: the closest canonical neighbours of those inputs decode.
    for good in [&b"Zm9vYmFy"[..], b"Zg==", b"Zm8="] {
        let (status, _out, _out_len) = decode(good, 32);
        assert_eq!(status, ffi::PITH_OK, "{good:?}");
    }
}

#[test]
fn prng_fills_accept_a_null_out_slot_only_for_a_zero_count() {
    unsafe {
        // A null slot with a zero count is a legal no-op.
        assert_eq!(
            ffi::sillstone_digest_splitmix64_fill(1, null_mut(), 0),
            ffi::PITH_OK
        );
        assert_eq!(
            ffi::sillstone_digest_xoshiro256_fill(1, null_mut(), 0),
            ffi::PITH_OK
        );
        // A null slot that claims a count is refused.
        assert_eq!(
            ffi::sillstone_digest_splitmix64_fill(1, null_mut(), 1),
            ffi::PITH_E_INVALID
        );
        assert_eq!(
            ffi::sillstone_digest_xoshiro256_fill(1, null_mut(), 1),
            ffi::PITH_E_INVALID
        );
    }
}
