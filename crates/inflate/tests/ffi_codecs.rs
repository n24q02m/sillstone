//! The gzip, Brotli and Zstandard C exports, called the way the language SDKs
//! call them: raw pointers in, an owned buffer out, `sillstone_inflate_free` back.
//!
//! Expected outputs are not taken from this crate. The committed fixtures were
//! produced by the reference tools (`gzip` 1.14, `brotli`, `zstd`); their
//! plaintexts were re-derived independently for this test: Python's `gzip` for
//! the `.gz` files, the reference `brotli 1.2.1` and `zstd 1.5.7` command-line
//! decoders for the `.br` / `.zst` files, each compared byte for byte with the
//! committed plaintext. The same tools reject every strict prefix of
//! `text-repeat.br` and `text-repeat.zst`, which is what the truncation checks
//! below require of the exports.

use sillstone_inflate::ffi::{
    SILLSTONE_E_INVALID, SILLSTONE_E_REJECTED, SILLSTONE_OK, sillstone_inflate_free,
    sillstone_inflate_inflate_brotli, sillstone_inflate_inflate_gzip,
    sillstone_inflate_inflate_zstd,
};

type Export = unsafe extern "C" fn(*const u8, usize, *mut *mut u8, *mut usize) -> i32;

const HELLO_GZ: &[u8] = include_bytes!("fixtures/gzip/hello.gz");
const TWO_MEMBERS_GZ: &[u8] = include_bytes!("fixtures/gzip/two-members.gz");
const EMPTY_GZ: &[u8] = include_bytes!("fixtures/gzip/empty.gz");
const TEXT: &[u8] = include_bytes!("fixtures/text-repeat.bin");
const TEXT_BR: &[u8] = include_bytes!("fixtures/text-repeat.br");
const TEXT_ZST: &[u8] = include_bytes!("fixtures/text-repeat.zst");
const RANDOM: &[u8] = include_bytes!("fixtures/random400.bin");
const RANDOM_BR: &[u8] = include_bytes!("fixtures/random400.br");
const RANDOM_ZST_CK: &[u8] = include_bytes!("fixtures/random400-checksum.zst");
const RANDOM_ZST_NO_CK: &[u8] = include_bytes!("fixtures/random400-nochecksum.zst");
const EMPTY_ZST: &[u8] = include_bytes!("fixtures/empty-block.zst");

/// Calls `op`; on success copies the handed-out buffer, frees it and returns the copy.
/// On failure checks the output slots were left untouched.
fn call(op: Export, input: &[u8]) -> Result<Vec<u8>, i32> {
    let mut out: *mut u8 = core::ptr::null_mut();
    let mut out_len: usize = 0;
    let status = unsafe { op(input.as_ptr(), input.len(), &mut out, &mut out_len) };
    if status != SILLSTONE_OK {
        assert!(out.is_null(), "status {status}: out pointer was written");
        assert_eq!(out_len, 0, "status {status}: out_len was written");
        return Err(status);
    }
    let copy = unsafe { core::slice::from_raw_parts(out, out_len) }.to_vec();
    unsafe { sillstone_inflate_free(out, out_len) };
    Ok(copy)
}

#[test]
fn gzip_decodes_single_multi_member_and_empty_streams() {
    let hello = call(sillstone_inflate_inflate_gzip, HELLO_GZ).expect("hello.gz");
    assert_eq!(
        hello,
        b"Hello, gzip! The quick brown fox jumps over the lazy dog.\n"
    );

    // Two members, outputs concatenated: 8 x the first phrase, then the second member.
    let mut want = b"the first member's payload, repeated. ".repeat(8);
    want.extend_from_slice(b"second member\n");
    let both = call(sillstone_inflate_inflate_gzip, TWO_MEMBERS_GZ).expect("two-members.gz");
    assert_eq!(both, want);

    // A full member around an empty payload: success with a zero-length buffer.
    let empty = call(sillstone_inflate_inflate_gzip, EMPTY_GZ).expect("empty.gz");
    assert!(empty.is_empty());
}

#[test]
fn gzip_refuses_corruption_and_truncation() {
    // Flip a bit of the CRC-32 in the trailer (the 8 bytes before the end are CRC-32 + ISIZE).
    let mut bad_crc = HELLO_GZ.to_vec();
    let crc_at = bad_crc.len() - 8;
    bad_crc[crc_at] ^= 0x01;
    assert_eq!(
        call(sillstone_inflate_inflate_gzip, &bad_crc),
        Err(SILLSTONE_E_REJECTED)
    );

    // Flip a bit of ISIZE.
    let mut bad_size = HELLO_GZ.to_vec();
    let size_at = bad_size.len() - 4;
    bad_size[size_at] ^= 0x01;
    assert_eq!(
        call(sillstone_inflate_inflate_gzip, &bad_size),
        Err(SILLSTONE_E_REJECTED)
    );

    // Not gzip at all, and every strict prefix of a valid member.
    assert_eq!(
        call(sillstone_inflate_inflate_gzip, b"definitely not gzip"),
        Err(SILLSTONE_E_REJECTED)
    );
    for cut in 0..HELLO_GZ.len() {
        assert_eq!(
            call(sillstone_inflate_inflate_gzip, &HELLO_GZ[..cut]),
            Err(SILLSTONE_E_REJECTED),
            "prefix of {cut} bytes"
        );
    }
}

#[test]
fn brotli_decodes_the_reference_streams() {
    assert_eq!(
        call(sillstone_inflate_inflate_brotli, TEXT_BR).expect("text"),
        TEXT
    );
    assert_eq!(
        call(sillstone_inflate_inflate_brotli, RANDOM_BR).expect("random"),
        RANDOM
    );
}

#[test]
fn brotli_refuses_every_truncation_and_foreign_framing() {
    for cut in 0..TEXT_BR.len() {
        assert_eq!(
            call(sillstone_inflate_inflate_brotli, &TEXT_BR[..cut]),
            Err(SILLSTONE_E_REJECTED),
            "prefix of {cut} bytes"
        );
    }
    // A gzip member is not a Brotli stream.
    assert_eq!(
        call(sillstone_inflate_inflate_brotli, HELLO_GZ),
        Err(SILLSTONE_E_REJECTED)
    );
}

#[test]
fn zstd_decodes_the_reference_frames() {
    assert_eq!(
        call(sillstone_inflate_inflate_zstd, TEXT_ZST).expect("text"),
        TEXT
    );
    assert_eq!(
        call(sillstone_inflate_inflate_zstd, RANDOM_ZST_CK).expect("random, content checksum"),
        RANDOM
    );
    assert_eq!(
        call(sillstone_inflate_inflate_zstd, RANDOM_ZST_NO_CK).expect("random, no checksum"),
        RANDOM
    );
    // A frame whose only block is empty: success with a zero-length buffer.
    let empty = call(sillstone_inflate_inflate_zstd, EMPTY_ZST).expect("empty-block.zst");
    assert!(empty.is_empty());
}

#[test]
fn zstd_refuses_corruption_truncation_and_foreign_framing() {
    // A flipped payload byte is caught by the frame's content checksum.
    let mut bad = RANDOM_ZST_CK.to_vec();
    bad[40] ^= 0x01;
    assert_eq!(
        call(sillstone_inflate_inflate_zstd, &bad),
        Err(SILLSTONE_E_REJECTED)
    );

    for cut in 0..TEXT_ZST.len() {
        assert_eq!(
            call(sillstone_inflate_inflate_zstd, &TEXT_ZST[..cut]),
            Err(SILLSTONE_E_REJECTED),
            "prefix of {cut} bytes"
        );
    }
    assert_eq!(
        call(sillstone_inflate_inflate_zstd, TEXT_BR),
        Err(SILLSTONE_E_REJECTED)
    );
}

#[test]
fn every_codec_refuses_bad_arguments_without_touching_the_slots() {
    let ops: [(&str, Export); 3] = [
        ("gzip", sillstone_inflate_inflate_gzip),
        ("brotli", sillstone_inflate_inflate_brotli),
        ("zstd", sillstone_inflate_inflate_zstd),
    ];
    for (name, op) in ops {
        let mut out: *mut u8 = core::ptr::null_mut();
        let mut out_len: usize = 0;

        // Null data with a non-zero length is a caller bug.
        let status = unsafe { op(core::ptr::null(), 5, &mut out, &mut out_len) };
        assert_eq!(status, SILLSTONE_E_INVALID, "{name}: null data");

        // Null data with zero length is an empty input: a decoder refusal, not a caller bug.
        let status = unsafe { op(core::ptr::null(), 0, &mut out, &mut out_len) };
        assert_eq!(status, SILLSTONE_E_REJECTED, "{name}: empty input");

        let byte = [0u8];
        let status = unsafe { op(byte.as_ptr(), 1, core::ptr::null_mut(), &mut out_len) };
        assert_eq!(status, SILLSTONE_E_INVALID, "{name}: null out");
        let status = unsafe { op(byte.as_ptr(), 1, &mut out, core::ptr::null_mut()) };
        assert_eq!(status, SILLSTONE_E_INVALID, "{name}: null out_len");

        assert!(out.is_null(), "{name}: out was written");
        assert_eq!(out_len, 0, "{name}: out_len was written");
    }
}
