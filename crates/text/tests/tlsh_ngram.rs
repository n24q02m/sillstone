//! Conformance tests for TLSH (Trend Micro Locality Sensitive Hash) and N-gram utilities.

use sillstone_text::ffi::{
    SILLSTONE_E_INVALID, SILLSTONE_E_REJECTED, SILLSTONE_OK, sillstone_text_tlsh,
    sillstone_text_tlsh_distance, sillstone_text_tlsh_distance_checked,
};
use sillstone_text::{Tlsh, ngram_jaccard, ngrams, tlsh};

#[test]
fn test_ngrams_basic() {
    assert_eq!(ngrams("hello", 0), Vec::<String>::new());
    assert_eq!(ngrams("hi", 3), Vec::<String>::new());
    assert_eq!(
        ngrams("hello", 2),
        vec!["he".to_string(), "el".to_string(), "ll".to_string(), "lo".to_string()]
    );
    assert_eq!(
        ngrams("tiếng", 2),
        vec!["ti".to_string(), "iế".to_string(), "ến".to_string(), "ng".to_string()]
    );
}

#[test]
fn test_ngram_jaccard() {
    let empty: Vec<String> = Vec::new();
    assert_eq!(ngram_jaccard(&empty, &empty), 1.0);

    let a = ngrams("night", 2);
    let b = ngrams("nacht", 2);
    let score = ngram_jaccard(&a, &b);
    assert!(score > 0.0 && score < 1.0);

    assert_eq!(ngram_jaccard(&a, &a), 1.0);
}

#[test]
fn test_tlsh_short_input_refusal() {
    let short = [0u8; 49];
    assert!(tlsh(&short).is_err());
}

#[test]
fn test_tlsh_official_reference_vector() {
    let txt = include_bytes!("fixtures/021106_yossivassa.txt");
    let t = tlsh(txt).expect("tlsh calculation");
    let expected = "T11FA1B357F78913B236924271569EA6D1FB2C451C33668484552C812D33138B8C73FFCE";
    assert_eq!(t.as_str(), expected);

    // Distance against itself must be 0
    assert_eq!(t.distance(&t), 0);

    // Parse roundtrip
    let parsed = Tlsh::parse(expected).expect("parse valid TLSH");
    assert_eq!(parsed, t);
    assert_eq!(parsed.as_str(), expected);
}

#[test]
fn test_tlsh_distance_official_score() {
    let txt_yossi = include_bytes!("fixtures/021106_yossivassa.txt");
    let txt_web = include_bytes!("fixtures/website_course_descriptors06-07.txt");

    let t_yossi = tlsh(txt_yossi).expect("yossi hash");
    let t_web = tlsh(txt_web).expect("web hash");

    let dist = t_web.distance(&t_yossi);
    assert_eq!(dist, 190);
    assert_eq!(t_yossi.distance(&t_web), 190);
}

#[test]
fn test_tlsh_ffi() {
    let txt = include_bytes!("fixtures/021106_yossivassa.txt");

    let mut buf = [0u8; 72];
    let status = unsafe { sillstone_text_tlsh(txt.as_ptr(), txt.len(), buf.as_mut_ptr()) };
    assert_eq!(status, SILLSTONE_OK);

    let str_hash = core::str::from_utf8(&buf).expect("utf-8");
    assert_eq!(
        str_hash,
        "T11FA1B357F78913B236924271569EA6D1FB2C451C33668484552C812D33138B8C73FFCE"
    );

    // Test refusal on null / short
    assert_eq!(
        unsafe { sillstone_text_tlsh(core::ptr::null(), 0, buf.as_mut_ptr()) },
        SILLSTONE_E_REJECTED
    );
    assert_eq!(
        unsafe { sillstone_text_tlsh(txt.as_ptr(), 20, buf.as_mut_ptr()) },
        SILLSTONE_E_REJECTED
    );
    assert_eq!(
        unsafe { sillstone_text_tlsh(txt.as_ptr(), txt.len(), core::ptr::null_mut()) },
        SILLSTONE_E_INVALID
    );

    // Test FFI distance
    let txt_web = include_bytes!("fixtures/website_course_descriptors06-07.txt");
    let mut buf_web = [0u8; 72];
    assert_eq!(
        unsafe { sillstone_text_tlsh(txt_web.as_ptr(), txt_web.len(), buf_web.as_mut_ptr()) },
        SILLSTONE_OK
    );

    let mut dist = 0u32;
    let dist_status =
        unsafe { sillstone_text_tlsh_distance(buf_web.as_ptr(), buf.as_ptr(), &mut dist) };
    assert_eq!(dist_status, SILLSTONE_OK);
    assert_eq!(dist, 190);
}

#[test]
fn test_tlsh_distance_checked_ffi() {
    let a_txt = include_bytes!("fixtures/021106_yossivassa.txt");
    let b_txt = include_bytes!("fixtures/website_course_descriptors06-07.txt");
    let mut a = [0u8; 72];
    let mut b = [0u8; 72];
    assert_eq!(
        unsafe { sillstone_text_tlsh(a_txt.as_ptr(), a_txt.len(), a.as_mut_ptr()) },
        SILLSTONE_OK
    );
    assert_eq!(
        unsafe { sillstone_text_tlsh(b_txt.as_ptr(), b_txt.len(), b.as_mut_ptr()) },
        SILLSTONE_OK
    );

    // Same result as the unchecked function for 72-byte digests.
    let mut dist = 0u32;
    let status =
        unsafe { sillstone_text_tlsh_distance_checked(b.as_ptr(), 72, a.as_ptr(), 72, &mut dist) };
    assert_eq!(status, SILLSTONE_OK);
    assert_eq!(dist, 190);
    let mut unchecked = 0u32;
    assert_eq!(
        unsafe { sillstone_text_tlsh_distance(b.as_ptr(), a.as_ptr(), &mut unchecked) },
        SILLSTONE_OK
    );
    assert_eq!(dist, unchecked);

    // Every length other than 72 is refused and `out` is left untouched. The memory behind
    // the pointer holds a valid digest twice (144 bytes), so only the length argument can
    // make these calls fail: an implementation that ignored it would return SILLSTONE_OK.
    let twice = [a, a].concat();
    for len in [0usize, 1, 71, 73, 144] {
        let mut out = 7u32;
        let ptr = twice.as_ptr();
        assert_eq!(
            unsafe { sillstone_text_tlsh_distance_checked(ptr, len, a.as_ptr(), 72, &mut out) },
            SILLSTONE_E_INVALID,
            "len_a = {len}"
        );
        assert_eq!(
            unsafe { sillstone_text_tlsh_distance_checked(a.as_ptr(), 72, ptr, len, &mut out) },
            SILLSTONE_E_INVALID,
            "len_b = {len}"
        );
        assert_eq!(out, 7, "out must not be written on error, len = {len}");
    }

    // Null pointers and a null out are refused.
    let mut out = 7u32;
    for (pa, pb, po) in [
        (core::ptr::null(), a.as_ptr(), &mut out as *mut u32),
        (a.as_ptr(), core::ptr::null(), &mut out as *mut u32),
        (a.as_ptr(), b.as_ptr(), core::ptr::null_mut()),
    ] {
        assert_eq!(
            unsafe { sillstone_text_tlsh_distance_checked(pa, 72, pb, 72, po) },
            SILLSTONE_E_INVALID
        );
    }

    // Right length but not a TLSH digest.
    let junk = [b'0'; 72];
    assert_eq!(
        unsafe {
            sillstone_text_tlsh_distance_checked(junk.as_ptr(), 72, a.as_ptr(), 72, &mut out)
        },
        SILLSTONE_E_INVALID
    );
    assert_eq!(out, 7);
}
