//! Zstandard decoder against the reference encoder.
//!
//! Two tests share one deterministic case generator:
//!
//! * `committed_corpus_decodes_to_the_recorded_digests` always runs. It
//!   decodes `fixtures/zstd-corpus.bin`, a corpus the reference `zstd` CLI
//!   produced from generated inputs, and checks length and SHA-256 of every
//!   result. The corpus also feeds a no-panic sweep of mutated frames.
//! * `oracle_fuzz_against_zstd_cli` is `#[ignore]`d: it needs the reference
//!   `zstd` binary named by `SILLSTONE_ZSTD_BIN`, compresses
//!   `SILLSTONE_ZSTD_CASES` (default 1000) fresh inputs with a spread of
//!   levels, window sizes and checksum settings, and requires a byte-exact
//!   round trip. Setting `SILLSTONE_ZSTD_CORPUS_OUT=<path>` additionally
//!   rewrites the committed corpus from the first cases.
//!
//! Corpus record: `u32 LE compressed length | u64 LE plaintext length |
//! 32-byte SHA-256 of the plaintext | compressed bytes`.

use sillstone_digest::sha256;
use sillstone_inflate::{Limits, inflate_zstd};
use std::io::Write;
use std::process::{Command, Stdio};

const CORPUS: &[u8] = include_bytes!("fixtures/zstd-corpus.bin");

fn limits() -> Limits {
    Limits {
        max_output: 64 << 20,
        max_input: 64 << 20,
    }
}

/// xorshift64*: small, deterministic, identical on every platform.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }
}

const WORDS: &[&str] = &[
    "the",
    "of",
    "and",
    "to",
    "in",
    "a",
    "is",
    "that",
    "for",
    "it",
    "as",
    "was",
    "with",
    "be",
    "by",
    "on",
    "not",
    "he",
    "this",
    "are",
    "or",
    "his",
    "from",
    "at",
    "which",
    "but",
    "have",
    "an",
    "had",
    "they",
    "you",
    "were",
    "their",
    "one",
    "all",
    "we",
    "can",
    "her",
    "has",
    "there",
    "been",
    "if",
    "more",
    "when",
    "will",
    "would",
    "who",
    "so",
    "no",
    "sillstone",
    "zstandard",
    "finite",
    "state",
    "entropy",
    "huffman",
    "literal",
    "sequence",
    "offset",
    "match",
    "window",
];

fn fill_text(rng: &mut Rng, out: &mut Vec<u8>, len: usize) {
    while out.len() < len {
        // Zipf-ish: squaring the draw biases towards the front of the list.
        let r = rng.below(1000);
        let idx = ((r * r) / 1000 * WORDS.len() as u64 / 1000) as usize;
        out.extend_from_slice(WORDS[idx.min(WORDS.len() - 1)].as_bytes());
        out.push(if rng.below(12) == 0 { b'\n' } else { b' ' });
    }
    out.truncate(len);
}

fn fill_class(rng: &mut Rng, class: u64, out: &mut Vec<u8>, len: usize) {
    let base = out.len();
    let target = base + len;
    match class {
        0 => {
            while out.len() < target {
                out.push(rng.next() as u8);
            }
        }
        1 => fill_text(rng, out, target),
        2 => {
            while out.len() < target {
                let b = rng.next() as u8;
                let run = 1 + rng.below(300) as usize;
                out.extend(std::iter::repeat_n(b, run.min(target - out.len())));
            }
        }
        3 => {
            let alphabet = 2 + rng.below(6) as u8;
            while out.len() < target {
                out.push(b'a' + (rng.next() % alphabet as u64) as u8);
            }
        }
        4 => {
            let mut counter = rng.next() as u32;
            while out.len() < target {
                counter = counter.wrapping_add(1 + rng.below(3) as u32);
                out.extend_from_slice(&counter.to_le_bytes());
                out.extend_from_slice(&(rng.below(50) as u16).to_le_bytes());
                out.push(rng.below(4) as u8);
                out.push(0);
            }
            out.truncate(target);
        }
        5 => {
            // Geometric byte distribution: ideal for Huffman literals.
            while out.len() < target {
                let mut b = 0u8;
                while b < 250 && rng.below(3) != 0 {
                    b += 1;
                }
                out.push(b);
            }
        }
        _ => {
            // Copies of earlier slices at varied distances (repeat offsets,
            // long matches) separated by fresh literals.
            while out.len() < target {
                if out.len() > 8 && rng.below(3) != 0 {
                    let dist = 1 + rng.below(out.len() as u64) as usize;
                    let n = (3 + rng.below(200) as usize).min(target - out.len());
                    for _ in 0..n {
                        let b = out[out.len() - dist];
                        out.push(b);
                    }
                } else {
                    let n = (1 + rng.below(40) as usize).min(target - out.len());
                    for _ in 0..n {
                        out.push(rng.next() as u8);
                    }
                }
            }
        }
    }
    out.truncate(target);
}

fn make_input(rng: &mut Rng) -> Vec<u8> {
    let len = match rng.below(100) {
        0..=4 => rng.below(8) as usize,
        5..=69 => rng.below(4000) as usize,
        70..=94 => rng.below(300_000) as usize,
        _ => 300_000 + rng.below(2_500_000) as usize,
    };
    let mut out = Vec::with_capacity(len);
    // Mix classes in segments so blocks see more than one statistic.
    while out.len() < len {
        let seg = if rng.below(3) == 0 {
            len - out.len()
        } else {
            (1 + rng.below(60_000) as usize).min(len - out.len())
        };
        let class = rng.below(7);
        fill_class(rng, class, &mut out, seg);
    }
    out
}

/// One compressor configuration for the reference CLI.
fn make_args(rng: &mut Rng) -> Vec<String> {
    let mut a = vec![
        "-c".to_string(),
        "-q".to_string(),
        "--single-thread".to_string(),
    ];
    match rng.below(20) {
        0 => a.push(format!("--fast={}", 1 + rng.below(100))),
        1 => {
            a.push("--ultra".into());
            a.push(format!("-{}", 20 + rng.below(3)));
        }
        _ => a.push(format!("-{}", 1 + rng.below(19))),
    }
    if rng.below(4) == 0 {
        a.push("--no-check".into());
    }
    match rng.below(8) {
        0 => a.push(format!("--zstd=wlog={}", 10 + rng.below(8))),
        1 => a.push("--long=27".into()),
        _ => {}
    }
    a
}

fn compress(bin: &str, args: &[String], data: &[u8]) -> Vec<u8> {
    let mut child = Command::new(bin)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn zstd");
    let mut stdin = child.stdin.take().unwrap();
    let owned = data.to_vec();
    let writer = std::thread::spawn(move || {
        let _ = stdin.write_all(&owned);
    });
    let out = child.wait_with_output().expect("zstd output");
    writer.join().unwrap();
    assert!(out.status.success(), "zstd failed for args {args:?}");
    out.stdout
}

fn record(compressed: &[u8], plain: &[u8]) -> Vec<u8> {
    let mut r = Vec::new();
    r.extend_from_slice(&(compressed.len() as u32).to_le_bytes());
    r.extend_from_slice(&(plain.len() as u64).to_le_bytes());
    r.extend_from_slice(sha256(plain).unwrap().as_bytes());
    r.extend_from_slice(compressed);
    r
}

fn records(mut corpus: &[u8]) -> Vec<(&[u8], u64, [u8; 32])> {
    let mut v = Vec::new();
    while !corpus.is_empty() {
        let clen = u32::from_le_bytes(corpus[..4].try_into().unwrap()) as usize;
        let plen = u64::from_le_bytes(corpus[4..12].try_into().unwrap());
        let digest: [u8; 32] = corpus[12..44].try_into().unwrap();
        v.push((&corpus[44..44 + clen], plen, digest));
        corpus = &corpus[44 + clen..];
    }
    v
}

#[test]
fn committed_corpus_decodes_to_the_recorded_digests() {
    let recs = records(CORPUS);
    assert!(recs.len() >= 100, "corpus has {} records", recs.len());
    for (i, (frame, plen, digest)) in recs.iter().enumerate() {
        let out = inflate_zstd(frame, limits()).unwrap_or_else(|e| panic!("record {i}: {e}"));
        assert_eq!(out.len() as u64, *plen, "record {i} length");
        assert_eq!(
            sha256(&out).unwrap().as_bytes(),
            digest,
            "record {i} digest"
        );
    }
}

#[test]
fn mutated_frames_never_panic_and_never_decode_to_wrong_bytes() {
    let recs = records(CORPUS);
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    for (frame, plen, digest) in &recs {
        for _ in 0..24 {
            let mut m = frame.to_vec();
            match rng.below(3) {
                0 if !m.is_empty() => {
                    let cut = rng.below(m.len() as u64) as usize;
                    m.truncate(cut);
                }
                1 if !m.is_empty() => {
                    let i = rng.below(m.len() as u64) as usize;
                    m[i] ^= 1 << rng.below(8);
                }
                _ if !m.is_empty() => {
                    let i = rng.below(m.len() as u64) as usize;
                    m[i] = rng.next() as u8;
                }
                _ => {}
            }
            // Either an error, or - when the damage hit a spot the format
            // does not protect - some output. A frame with a content checksum
            // that still decodes must reproduce the original exactly.
            if let Ok(out) = inflate_zstd(&m, limits()) {
                let original_ok =
                    out.len() as u64 == *plen && sha256(&out).unwrap().as_bytes() == digest;
                let has_checksum = frame.len() > 5 && (frame[4] >> 2) & 1 == 1;
                if has_checksum && !original_ok {
                    // XXH64 low 32 bits can collide by chance; with a few
                    // thousand mutations that is a 2^-32 event per case.
                    panic!("checksummed frame decoded to different bytes");
                }
            }
        }
    }
}

#[test]
#[ignore = "needs the reference zstd binary: set SILLSTONE_ZSTD_BIN"]
fn oracle_fuzz_against_zstd_cli() {
    let bin = std::env::var("SILLSTONE_ZSTD_BIN").expect("SILLSTONE_ZSTD_BIN");
    let cases: usize = std::env::var("SILLSTONE_ZSTD_CASES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1000);
    let seed: u64 = std::env::var("SILLSTONE_ZSTD_SEED")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0x5151_1570_4E5E);
    let corpus_out = std::env::var("SILLSTONE_ZSTD_CORPUS_OUT").ok();
    let mut rng = Rng(seed);
    let mut corpus = Vec::new();
    let mut corpus_cases = 0usize;
    let mut total_in = 0usize;
    let mut total_out = 0usize;
    for i in 0..cases {
        let plain = make_input(&mut rng);
        let args = make_args(&mut rng);
        let frame = compress(&bin, &args, &plain);
        let out = inflate_zstd(&frame, limits())
            .unwrap_or_else(|e| panic!("case {i} args {args:?} len {}: {e}", plain.len()));
        assert!(
            out == plain,
            "case {i} args {args:?} len {}: output differs",
            plain.len()
        );
        total_in += frame.len();
        total_out += out.len();
        // Keep the committed corpus small: modest inputs only, a mix of levels.
        if corpus_out.is_some()
            && corpus_cases < 160
            && plain.len() <= 200_000
            && frame.len() <= 6000
        {
            corpus.extend_from_slice(&record(&frame, &plain));
            corpus_cases += 1;
        }
    }
    eprintln!("{cases} cases ok: {total_in} compressed bytes -> {total_out} plain bytes");
    if let Some(path) = corpus_out {
        std::fs::write(&path, &corpus).unwrap();
        eprintln!("wrote {corpus_cases} corpus records to {path}");
    }
}
