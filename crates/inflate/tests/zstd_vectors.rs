//! Zstandard decoder vectors generated from the reference encoder.
//!
//! `tests/fixtures/zstd_vectors.jsonl` is produced by
//! `scripts/golden/gen_zstd_vectors.py`, which drives the Python `zstandard`
//! binding over the reference `libzstd` and refuses to write a case the oracle
//! disagrees with. The oracle is a development-time dependency only: this
//! suite replays the committed lines, nothing in the crate links `libzstd`,
//! and the generator's own `--check` mode proves the committed bytes are
//! reproducible from the seed recorded in the provenance line.
//!
//! The corpus is deliberately categorical rather than random: empty frames,
//! stored and RLE blocks, entropy-coded blocks at four levels, a 1 KiB window
//! (which forces treeless literals and repeat FSE tables across dozens of
//! blocks), a binary blob, phrases that make the encoder emit repeat offsets,
//! a 160 KiB input whose matches land past the 128 KiB block maximum, a
//! 62-case sweep over levels 1-22, sizes up to 2500 bytes and window logs
//! 10/17/27, two- and three-frame streams, skippable frames with empty and
//! non-empty payloads, a frame with no content size, and the fail-closed
//! cases: checksum mismatch,
//! every truncation, trailing garbage, a reserved block type, a reserved
//! descriptor bit, a dictionary id, an oversized block and a content size that
//! disagrees with the output.
//!
//! Regenerate with:
//!
//! ```text
//! pip install -r scripts/golden/requirements.txt
//! python scripts/golden/gen_zstd_vectors.py
//! python scripts/golden/gen_zstd_vectors.py --check
//! ```

use sillstone_digest::{Error, sha256};
use sillstone_inflate::{Limits, inflate_zstd};

const VECTORS: &str = include_str!("fixtures/zstd_vectors.jsonl");

/// One case: the frame, what the oracle produced from it, and what this
/// decoder is required to answer.
struct Case {
    id: String,
    expect: String,
    err: String,
    checksum: bool,
    plain_len: usize,
    /// Empty when the generator stored only the digest (plaintext above
    /// 4096 bytes).
    plain: Vec<u8>,
    plain_sha256: [u8; 32],
    frame: Vec<u8>,
}

fn limits() -> Limits {
    Limits {
        max_output: 64 << 20,
        max_input: 64 << 20,
    }
}

/// Minimal parser for one flat JSON object (strings and numbers, no escapes
/// beyond `\"` and `\\`): enough for the generator's output, without a JSON
/// dependency.
fn parse_object(line: &str) -> Vec<(String, String)> {
    let b = line.as_bytes();
    let mut i = 0;
    let mut out = Vec::new();
    let skip_ws = |i: &mut usize| {
        while *i < b.len() && (b[*i] == b' ' || b[*i] == b',') {
            *i += 1;
        }
    };
    let string = |i: &mut usize| -> String {
        assert_eq!(b[*i], b'"');
        *i += 1;
        let mut s = String::new();
        while b[*i] != b'"' {
            if b[*i] == b'\\' {
                *i += 1;
            }
            s.push(b[*i] as char);
            *i += 1;
        }
        *i += 1;
        s
    };
    assert_eq!(b[i], b'{');
    i += 1;
    loop {
        skip_ws(&mut i);
        if b[i] == b'}' {
            break;
        }
        let key = string(&mut i);
        assert_eq!(b[i], b':');
        i += 1;
        let val = if b[i] == b'"' {
            string(&mut i)
        } else {
            let start = i;
            while b[i] != b',' && b[i] != b'}' {
                i += 1;
            }
            line[start..i].to_string()
        };
        out.push((key, val));
    }
    out
}

fn unhex(s: &str) -> Vec<u8> {
    assert_eq!(s.len() % 2, 0, "hex length");
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).expect("hex"))
        .collect()
}

fn provenance() -> Vec<(String, String)> {
    let meta = VECTORS.lines().next().expect("provenance line");
    let kv = parse_object(meta);
    let field = |k: &str| -> &str { &kv.iter().find(|(key, _)| key == k).expect("field").1 };
    for required in [
        "_provenance",
        "oracle",
        "libzstd",
        "seed",
        "cases",
        "regenerate",
        "check",
        "note",
    ] {
        assert!(!field(required).is_empty(), "provenance field {required}");
    }
    assert_eq!(field("_provenance"), "scripts/golden/gen_zstd_vectors.py");
    kv
}

fn cases() -> Vec<Case> {
    let meta = provenance();
    let declared: usize = meta
        .iter()
        .find(|(key, _)| key == "cases")
        .expect("cases")
        .1
        .parse()
        .expect("case count");
    let cases: Vec<Case> = VECTORS
        .lines()
        .skip(1)
        .filter(|l| !l.is_empty())
        .map(|l| {
            let kv = parse_object(l);
            let get = |k: &str| -> &str { &kv.iter().find(|(key, _)| key == k).expect("key").1 };
            let digest = unhex(get("plain_sha256"));
            let mut plain_sha256 = [0u8; 32];
            if !digest.is_empty() {
                plain_sha256.copy_from_slice(&digest);
            }
            Case {
                id: get("id").to_string(),
                expect: get("expect").to_string(),
                err: get("err").to_string(),
                checksum: get("checksum") == "1",
                plain_len: get("plain_len").parse().expect("plain_len"),
                plain: unhex(get("plain")),
                plain_sha256,
                frame: unhex(get("frame")),
            }
        })
        .collect();
    assert_eq!(cases.len(), declared, "the provenance case count is stale");
    assert!(cases.len() >= 90, "only {} vectors", cases.len());
    cases
}

/// The error kind a case is expected to produce: the variant names in the
/// fixture, mapped onto the crate's [`Error`].
fn category(err: Error) -> &'static str {
    match err {
        Error::Truncated { .. } => "truncated",
        Error::InvalidMagic { .. } => "invalid_magic",
        Error::BadValue(_) => "bad_value",
        Error::Unsupported(_) => "unsupported",
        Error::TooLarge { .. } => "too_large",
    }
}

/// Asserts `out` is exactly what the oracle produced for this case.
fn assert_matches_oracle(case: &Case, out: &[u8]) {
    assert_eq!(out.len(), case.plain_len, "{}: length", case.id);
    if case.plain.is_empty() {
        assert_eq!(
            sha256(out).unwrap().as_bytes(),
            &case.plain_sha256,
            "{}: digest",
            case.id
        );
    } else {
        assert_eq!(out, case.plain.as_slice(), "{}: bytes", case.id);
    }
}

#[test]
fn every_committed_vector_matches_the_oracle() {
    for case in cases() {
        match inflate_zstd(&case.frame, limits()) {
            Ok(out) => {
                assert_eq!(
                    case.expect, "ok",
                    "{}: decoded but the oracle refuses it",
                    case.id
                );
                assert_matches_oracle(&case, &out);
            }
            Err(e) => {
                assert_eq!(
                    case.expect, "err",
                    "{}: refused a frame the oracle accepts: {e}",
                    case.id
                );
                assert_eq!(category(e), case.err, "{}: wrong error kind", case.id);
            }
        }
    }
}

#[test]
fn the_corpus_covers_the_categories_it_claims() {
    let cases = cases();
    let ids: Vec<&str> = cases.iter().map(|c| c.id.as_str()).collect();
    for prefix in [
        "sweep-",
        "empty-",
        "raw-",
        "rle-",
        "text-",
        "blob-",
        "long-",
        "repeats-",
        "multi-frame-",
        "skippable-",
        "no-content-size",
        "err-checksum-mismatch",
        "err-truncated-",
        "err-trailing-garbage",
        "err-reserved-",
        "err-dictionary-id",
        "err-block-larger-than-window",
        "err-content-size-mismatch",
    ] {
        assert!(
            ids.iter().any(|id| id.starts_with(prefix)),
            "the corpus lost its {prefix} case"
        );
    }
    for kind in ["truncated", "invalid_magic", "bad_value", "unsupported"] {
        assert!(
            cases.iter().any(|c| c.err == kind),
            "no case pins the {kind} refusal"
        );
    }
    assert!(
        cases.iter().filter(|c| c.checksum).count() >= 10,
        "too few checksummed frames"
    );
    assert!(
        cases.iter().filter(|c| !c.checksum).count() >= 5,
        "too few frames without a checksum"
    );
    assert!(
        cases.iter().any(|c| !c.plain.is_empty()),
        "every vector hides its bytes behind a digest"
    );
    // The sweep is what makes the corpus a differential run rather than a
    // handful of hand-picked frames: every level, several window sizes.
    let sweep: Vec<&Case> = cases
        .iter()
        .filter(|c| c.id.starts_with("sweep-"))
        .collect();
    assert!(sweep.len() >= 60, "only {} sweep cases", sweep.len());
    for level in 1..=22u8 {
        let tag = format!("-l{level}-");
        assert!(
            sweep.iter().any(|c| c.id.contains(&tag)),
            "the sweep never compresses at level {level}"
        );
    }
    for window in ["-w0", "-w10", "-w17", "-w27"] {
        assert!(
            sweep.iter().any(|c| c.id.ends_with(window)),
            "the sweep never uses window log {window}"
        );
    }
}

#[test]
fn a_truncated_vector_never_invents_bytes() {
    // A cut either fails (the usual case) or lands exactly on a frame
    // boundary, where the bytes before the cut are already a complete stream:
    // the output then has to be a prefix of the full plaintext, never
    // something else.
    let mut checked = 0usize;
    let mut prefixes = 0usize;
    for case in cases().iter().filter(|c| c.expect == "ok") {
        // Every frame of a few dozen bytes is swept byte for byte; longer
        // ones take a spread of cuts so the sweep stays a test, not a job.
        let stride = case.frame.len().div_ceil(120).max(1);
        for cut in (0..case.frame.len()).step_by(stride) {
            if let Ok(out) = inflate_zstd(&case.frame[..cut], limits()) {
                prefixes += 1;
                assert!(
                    out.len() <= case.plain_len,
                    "{}: truncation at {cut} produced {} of {} bytes",
                    case.id,
                    out.len(),
                    case.plain_len
                );
                if !case.plain.is_empty() {
                    assert!(
                        case.plain.starts_with(&out),
                        "{}: truncation at {cut} produced foreign bytes",
                        case.id
                    );
                }
            }
            checked += 1;
        }
    }
    assert!(checked > 1000, "only {checked} truncations checked");
    // Only a cut that lands exactly on a frame boundary survives, which in
    // this corpus means the streams whose tail is a skippable frame.
    assert!(prefixes < 20, "{prefixes} truncations decoded");
}

#[test]
fn mutated_frames_never_panic_and_checksummed_ones_never_lie() {
    // Deterministic xorshift so every platform mutates identically.
    let mut state = 0x9E37_79B9_7F4A_7C15u64;
    let mut next = move || {
        state ^= state >> 12;
        state ^= state << 25;
        state ^= state >> 27;
        state.wrapping_mul(0x2545_F491_4F6C_DD1D)
    };
    let mut accepted = 0usize;
    let mut refused = 0usize;
    for case in cases().iter().filter(|c| c.expect == "ok") {
        for _ in 0..24 {
            let mut m = case.frame.clone();
            let i = (next() % m.len() as u64) as usize;
            m[i] ^= 1 << (next() % 8);
            match inflate_zstd(&m, limits()) {
                Ok(out) => {
                    accepted += 1;
                    // A frame that carries a content checksum cannot decode to
                    // other bytes: XXH64 would have to collide on the low 32
                    // bits, which is a 2^-32 event per case.
                    if case.checksum {
                        assert_matches_oracle(case, &out);
                    }
                }
                Err(_) => refused += 1,
            }
        }
    }
    assert!(refused > accepted, "{refused} refused, {accepted} accepted");
}

#[test]
fn frames_concatenated_by_the_callers_decode_to_the_concatenation() {
    // Multi-frame streams are a format feature, not only a corpus entry:
    // every pair of committed frames must decode to the concatenation of the
    // two plaintexts, in order.
    let cases: Vec<Case> = cases()
        .into_iter()
        .filter(|c| c.expect == "ok" && !c.plain.is_empty())
        .collect();
    assert!(cases.len() >= 5, "not enough inline vectors to concatenate");
    for a in &cases {
        for b in &cases {
            let mut stream = a.frame.clone();
            stream.extend_from_slice(&b.frame);
            let mut want = a.plain.clone();
            want.extend_from_slice(&b.plain);
            let out = inflate_zstd(&stream, limits())
                .unwrap_or_else(|e| panic!("{} + {}: {e}", a.id, b.id));
            assert_eq!(out, want, "{} + {}", a.id, b.id);
        }
    }
}
