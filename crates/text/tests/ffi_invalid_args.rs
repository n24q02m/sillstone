//! Refusals of the text C ABI, called from a consumer crate the way the
//! language SDKs call it.
//!
//! Every refusal asserts the exact status code and that the output
//! slots stay untouched. The Jaccard known answer (two of four equal
//! words, so 0.5) is worked out by hand, not recomputed from the
//! crate; the TLSH digests are the pinned values of the official
//! reference vectors used by `tlsh_ngram.rs`.

use sillstone_text::ffi::{
    SILLSTONE_E_INVALID, SILLSTONE_E_REJECTED, SILLSTONE_OK, sillstone_text_fingerprint,
    sillstone_text_free, sillstone_text_jaccard, sillstone_text_tlsh, sillstone_text_tlsh_distance,
    sillstone_text_tlsh_distance_checked,
};

/// Official TLSH digest of `fixtures/021106_yossivassa.txt`.
const DIGEST: &[u8; 72] =
    b"T11FA1B357F78913B236924271569EA6D1FB2C451C33668484552C812D33138B8C73FFCE";

/// Sentinel the output slots start with, so a stray write is visible.
const SENTINEL: u32 = 0xDEAD_BEEF;

#[test]
fn fingerprint_refuses_null_out_slots_and_null_data_with_a_length() {
    let text = b"alpha beta gamma";
    let mut out: *mut u8 = core::ptr::null_mut();
    let mut out_len: usize = 0;

    let status = unsafe {
        sillstone_text_fingerprint(
            text.as_ptr(),
            text.len(),
            core::ptr::null_mut(),
            &mut out_len,
        )
    };
    assert_eq!(status, SILLSTONE_E_INVALID, "null out");
    assert_eq!(out_len, 0);

    let status = unsafe {
        sillstone_text_fingerprint(text.as_ptr(), text.len(), &mut out, core::ptr::null_mut())
    };
    assert_eq!(status, SILLSTONE_E_INVALID, "null out_len");
    assert!(out.is_null());

    // Null data with a non-zero length is a caller bug, not an empty input.
    let status =
        unsafe { sillstone_text_fingerprint(core::ptr::null(), 5, &mut out, &mut out_len) };
    assert_eq!(status, SILLSTONE_E_INVALID, "null data with length");
    assert!(out.is_null(), "out written on refusal");
    assert_eq!(out_len, 0, "out_len written on refusal");

    // Bytes that are not UTF-8 are a rejection, not an argument error.
    let status =
        unsafe { sillstone_text_fingerprint(b"\xff\xfe".as_ptr(), 2, &mut out, &mut out_len) };
    assert_eq!(status, SILLSTONE_E_REJECTED, "non-UTF-8");
    assert!(out.is_null());
    assert_eq!(out_len, 0);

    // The valid call next to the refusals still succeeds and frees cleanly.
    let status =
        unsafe { sillstone_text_fingerprint(text.as_ptr(), text.len(), &mut out, &mut out_len) };
    assert_eq!(status, SILLSTONE_OK);
    assert!(!out.is_null());
    unsafe { sillstone_text_free(out, out_len) };
}

#[test]
fn jaccard_refuses_null_operands_and_a_null_out_slot() {
    let a: [u64; 4] = [1, 2, 3, 4];
    let b: [u64; 4] = [1, 2, 9, 9];

    // Known answer: two of the four words are equal.
    let mut bits: u64 = 0;
    let status = unsafe { sillstone_text_jaccard(a.as_ptr(), 4, b.as_ptr(), 4, &mut bits) };
    assert_eq!(status, SILLSTONE_OK);
    assert_eq!(f64::from_bits(bits), 0.5);

    let mut bits: u64 = u64::from(SENTINEL);
    let status =
        unsafe { sillstone_text_jaccard(a.as_ptr(), 4, b.as_ptr(), 4, core::ptr::null_mut()) };
    assert_eq!(status, SILLSTONE_E_INVALID, "null out_bits");

    let status = unsafe { sillstone_text_jaccard(core::ptr::null(), 4, b.as_ptr(), 4, &mut bits) };
    assert_eq!(
        status, SILLSTONE_E_INVALID,
        "null first operand with length"
    );
    assert_eq!(bits, u64::from(SENTINEL), "out_bits written on refusal");

    let status = unsafe { sillstone_text_jaccard(a.as_ptr(), 4, core::ptr::null(), 4, &mut bits) };
    assert_eq!(
        status, SILLSTONE_E_INVALID,
        "null second operand with length"
    );
    assert_eq!(bits, u64::from(SENTINEL), "out_bits written on refusal");

    // Null with a zero length is the legal empty operand, which scores 1.0.
    let status = unsafe { sillstone_text_jaccard(core::ptr::null(), 0, b.as_ptr(), 4, &mut bits) };
    assert_eq!(status, SILLSTONE_OK, "null first operand, zero length");
    assert_eq!(f64::from_bits(bits), 1.0);
}

#[test]
fn tlsh_refuses_null_data_with_a_length_and_leaves_the_buffer_alone() {
    let mut buf = [0xA5u8; 72];
    let status = unsafe { sillstone_text_tlsh(core::ptr::null(), 100, buf.as_mut_ptr()) };
    assert_eq!(status, SILLSTONE_E_INVALID);
    assert_eq!(buf, [0xA5u8; 72], "buffer written on refusal");
}

#[test]
fn tlsh_distance_refuses_null_pointers_and_leaves_out_alone() {
    let mut out = SENTINEL;
    for (a, b, o) in [
        (core::ptr::null(), DIGEST.as_ptr(), &mut out as *mut u32),
        (DIGEST.as_ptr(), core::ptr::null(), &mut out as *mut u32),
        (DIGEST.as_ptr(), DIGEST.as_ptr(), core::ptr::null_mut()),
    ] {
        let status = unsafe { sillstone_text_tlsh_distance(a, b, o) };
        assert_eq!(status, SILLSTONE_E_INVALID);
    }
    assert_eq!(out, SENTINEL, "out written on refusal");
}

/// Both distance exports share one parser: each operand can be refused on
/// its own, whether it is not UTF-8 or not a well-formed digest.
#[test]
fn tlsh_distance_refuses_each_malformed_operand_on_both_exports() {
    let not_utf8 = [0xFFu8; 72];
    let not_hex = {
        let mut d = *DIGEST;
        d[10] = b'z';
        d
    };
    let no_prefix = {
        let mut d = *DIGEST;
        d[0] = b'X';
        d
    };

    for (what, bad) in [
        ("not UTF-8", not_utf8),
        ("non-hex byte", not_hex),
        ("missing T1 prefix", no_prefix),
    ] {
        for (position, a, b) in [("first", &bad, DIGEST), ("second", DIGEST, &bad)] {
            let mut out = SENTINEL;
            let status = unsafe { sillstone_text_tlsh_distance(a.as_ptr(), b.as_ptr(), &mut out) };
            assert_eq!(
                status, SILLSTONE_E_INVALID,
                "unchecked, {position} operand {what}"
            );
            assert_eq!(
                out, SENTINEL,
                "unchecked, {position} operand {what}: out written"
            );

            let status = unsafe {
                sillstone_text_tlsh_distance_checked(a.as_ptr(), 72, b.as_ptr(), 72, &mut out)
            };
            assert_eq!(
                status, SILLSTONE_E_INVALID,
                "checked, {position} operand {what}"
            );
            assert_eq!(
                out, SENTINEL,
                "checked, {position} operand {what}: out written"
            );
        }
    }

    // The pair that passes is the same digest twice: distance 0.
    let mut out = SENTINEL;
    let status =
        unsafe { sillstone_text_tlsh_distance(DIGEST.as_ptr(), DIGEST.as_ptr(), &mut out) };
    assert_eq!(status, SILLSTONE_OK);
    assert_eq!(out, 0);
}
