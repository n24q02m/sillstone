//! Conformance tests for TLSH (Trend Micro Locality Sensitive Hash) and N-gram utilities.

use sillstone_text::ffi::{PITH_E_INVALID, PITH_E_REJECTED, PITH_OK, sillstone_text_tlsh, sillstone_text_tlsh_distance};
use sillstone_text::{ngram_jaccard, ngrams, tlsh, Tlsh};

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
    let txt = include_bytes!(
        "../../../../pith-hash-audit-2026-10-12/wave2/vectors/tlsh/example_data/021106_yossivassa.txt"
    );
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
    let txt_yossi = include_bytes!(
        "../../../../pith-hash-audit-2026-10-12/wave2/vectors/tlsh/example_data/021106_yossivassa.txt"
    );
    let txt_web = include_bytes!(
        "../../../../pith-hash-audit-2026-10-12/wave2/vectors/tlsh/example_data/website_course_descriptors06-07.txt"
    );

    let t_yossi = tlsh(txt_yossi).expect("yossi hash");
    let t_web = tlsh(txt_web).expect("web hash");

    let dist = t_web.distance(&t_yossi);
    assert_eq!(dist, 190);
    assert_eq!(t_yossi.distance(&t_web), 190);
}

#[test]
fn test_tlsh_ffi() {
    let txt = include_bytes!(
        "../../../../pith-hash-audit-2026-10-12/wave2/vectors/tlsh/example_data/021106_yossivassa.txt"
    );

    let mut buf = [0u8; 72];
    let status = unsafe { sillstone_text_tlsh(txt.as_ptr(), txt.len(), buf.as_mut_ptr()) };
    assert_eq!(status, PITH_OK);

    let str_hash = core::str::from_utf8(&buf).expect("utf-8");
    assert_eq!(
        str_hash,
        "T11FA1B357F78913B236924271569EA6D1FB2C451C33668484552C812D33138B8C73FFCE"
    );

    // Test refusal on null / short
    assert_eq!(
        unsafe { sillstone_text_tlsh(core::ptr::null(), 0, buf.as_mut_ptr()) },
        PITH_E_REJECTED
    );
    assert_eq!(
        unsafe { sillstone_text_tlsh(txt.as_ptr(), 20, buf.as_mut_ptr()) },
        PITH_E_REJECTED
    );
    assert_eq!(
        unsafe { sillstone_text_tlsh(txt.as_ptr(), txt.len(), core::ptr::null_mut()) },
        PITH_E_INVALID
    );

    // Test FFI distance
    let txt_web = include_bytes!(
        "../../../../pith-hash-audit-2026-10-12/wave2/vectors/tlsh/example_data/website_course_descriptors06-07.txt"
    );
    let mut buf_web = [0u8; 72];
    assert_eq!(
        unsafe { sillstone_text_tlsh(txt_web.as_ptr(), txt_web.len(), buf_web.as_mut_ptr()) },
        PITH_OK
    );

    let mut dist = 0u32;
    let dist_status = unsafe {
        sillstone_text_tlsh_distance(buf_web.as_ptr(), buf.as_ptr(), &mut dist)
    };
    assert_eq!(dist_status, PITH_OK);
    assert_eq!(dist, 190);
}
