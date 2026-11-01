//! Brotli decoder against the reference encoder.
//!
//! * `committed_corpus_decodes_to_the_recorded_digests` always runs. It
//!   decodes `fixtures/brotli-corpus.bin`, a corpus the reference `brotli` CLI
//!   produced from generated inputs, and checks length and SHA-256 of every
//!   result. The corpus also feeds a no-panic sweep of mutated streams.
//! * `dictionary_transforms_match_the_recorded_digests` always runs. It builds
//!   one tiny stream per (word length, transform) by hand - an empty window
//!   forces the copy to be a static dictionary reference - and checks the
//!   decoded bytes against `fixtures/brotli-transforms.bin`, whose expected
//!   values were produced by the reference decoder.
//! * `embedded_dictionary_matches_the_rfc_checksums` reads every dictionary
//!   word back through the decoder and pins the whole dictionary to the
//!   CRC-32 RFC 7932 publishes for it.
//! * `oracle_fuzz_against_brotli_cli` is `#[ignore]`d: it needs the reference
//!   `brotli` binary named by `SILLSTONE_BROTLI_BIN`, compresses
//!   `SILLSTONE_BROTLI_CASES` (default 1000) fresh inputs with a spread of
//!   qualities, window sizes and content classes, and requires a byte-exact
//!   round trip. `SILLSTONE_BROTLI_CORPUS_OUT=<path>` rewrites the committed
//!   corpus from the first cases; `SILLSTONE_BROTLI_TRANSFORMS_OUT=<path>`
//!   rewrites the transform fixture, using the same binary as the oracle.
//!
//! Corpus record: `u32 LE compressed length | u64 LE plaintext length |
//! 32-byte SHA-256 of the plaintext | compressed bytes`.

use sillstone_digest::sha256;
use sillstone_inflate::{Limits, inflate_brotli};
use std::io::Write;
use std::process::{Command, Stdio};

/// log2 of the number of dictionary words of each length (RFC 7932 section 8).
const NDBITS: [u32; 25] = [
    0, 0, 0, 0, 10, 10, 11, 11, 10, 10, 10, 10, 10, 9, 9, 8, 7, 7, 8, 7, 7, 6, 6, 5, 5,
];

const CORPUS: &[u8] = include_bytes!("fixtures/brotli-corpus.bin");
const TRANSFORMS: &[u8] = include_bytes!("fixtures/brotli-transforms.bin");

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

/// Common English and web words: the sort of thing the static dictionary
/// holds, so the reference encoder emits dictionary references for them.
const WORDS: &[&str] = &[
    "the",
    "of",
    "and",
    "to",
    "in",
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
    "this",
    "are",
    "or",
    "from",
    "at",
    "which",
    "but",
    "have",
    "an",
    "they",
    "you",
    "were",
    "their",
    "one",
    "all",
    "can",
    "has",
    "there",
    "been",
    "more",
    "when",
    "will",
    "would",
    "who",
    "information",
    "government",
    "between",
    "important",
    "development",
    "international",
    "community",
    "following",
    "including",
    "because",
    "through",
    "however",
    "between",
    "against",
    "without",
    "another",
    "business",
    "company",
    "service",
    "system",
    "program",
    "children",
    "national",
    "university",
    "history",
    "education",
    "science",
    "software",
    "download",
    "search",
    "contact",
    "privacy",
    "policy",
    "copyright",
    "reserved",
    "javascript",
    "function",
    "return",
    "width",
    "height",
    "border",
    "padding",
    "margin",
    "color",
    "background",
    "font",
    "style",
];

const TAGS: &[&str] = &[
    "<div class=\"",
    "</div>",
    "<a href=\"http://www.",
    "</a>",
    "<p>",
    "</p>",
    "<span>",
    "</span>",
    "<li>",
    "</li>",
    "<td>",
    "</td>",
    "<img src=\"",
    "\" alt=\"",
    "\">\n",
    "&nbsp;",
    "&amp;",
    ".com/",
    ".html",
    "=\"",
    "\"",
];

/// Short words in several scripts, so two- and three-byte UTF-8 characters
/// reach the dictionary's uppercase transforms.
const UTF8_WORDS: &[&str] = &[
    "caf\u{e9}",
    "na\u{ef}ve",
    "\u{fc}ber",
    "se\u{f1}or",
    "\u{43f}\u{440}\u{438}\u{432}\u{435}\u{442}",
    "\u{43c}\u{438}\u{440}",
    "\u{3b5}\u{3bb}\u{3bb}\u{3b7}\u{3bd}\u{3b9}\u{3ba}\u{3ac}",
    "\u{4e2d}\u{6587}",
    "\u{65e5}\u{672c}\u{8a9e}",
    "\u{d55c}\u{ad6d}\u{c5b4}",
    "\u{5e9}\u{5dc}\u{5d5}\u{5dd}",
    "\u{645}\u{631}\u{62d}\u{628}\u{627}",
];

fn push_word(rng: &mut Rng, out: &mut Vec<u8>, word: &str) {
    let bytes = word.as_bytes();
    match rng.below(8) {
        0 => {
            // Capitalised.
            out.push(bytes[0].to_ascii_uppercase());
            out.extend_from_slice(&bytes[1..]);
        }
        1 => out.extend(bytes.iter().map(u8::to_ascii_uppercase)),
        _ => out.extend_from_slice(bytes),
    }
}

fn fill_words(rng: &mut Rng, out: &mut Vec<u8>, len: usize, html: bool, utf8: bool) {
    while out.len() < len {
        let r = rng.below(1000);
        if html && rng.below(4) == 0 {
            out.extend_from_slice(TAGS[rng.below(TAGS.len() as u64) as usize].as_bytes());
        } else if utf8 && rng.below(3) == 0 {
            let w = UTF8_WORDS[rng.below(UTF8_WORDS.len() as u64) as usize];
            push_word(rng, out, w);
        } else {
            let idx = ((r * r) / 1000 * WORDS.len() as u64 / 1000) as usize;
            push_word(rng, out, WORDS[idx.min(WORDS.len() - 1)]);
        }
        match rng.below(14) {
            0 => out.extend_from_slice(b".\n"),
            1 => out.extend_from_slice(b", "),
            2 => out.extend_from_slice(b"ing "),
            3 => out.extend_from_slice(b"s "),
            4 => out.extend_from_slice(b"\" "),
            _ => out.push(b' '),
        }
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
        1 => fill_words(rng, out, target, false, false),
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
        4 => fill_words(rng, out, target, true, false),
        5 => fill_words(rng, out, target, false, true),
        6 => {
            // Geometric byte distribution: ideal for Huffman literals.
            while out.len() < target {
                let mut b = 0u8;
                while b < 250 && rng.below(3) != 0 {
                    b += 1;
                }
                out.push(b);
            }
        }
        7 => {
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
        _ => {
            // Copies of earlier slices at varied distances separated by
            // fresh literals.
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
        _ => 300_000 + rng.below(1_500_000) as usize,
    };
    let mut out = Vec::with_capacity(len);
    // Mix classes in segments so meta-blocks see more than one statistic.
    while out.len() < len {
        let seg = if rng.below(3) == 0 {
            len - out.len()
        } else {
            (1 + rng.below(60_000) as usize).min(len - out.len())
        };
        let class = rng.below(9);
        fill_class(rng, class, &mut out, seg);
    }
    out
}

/// One compressor configuration for the reference CLI.
fn make_args(rng: &mut Rng) -> Vec<String> {
    let mut a = vec!["-c".to_string(), format!("--quality={}", rng.below(12))];
    match rng.below(4) {
        0 => {}
        _ => a.push(format!("--lgwin={}", 10 + rng.below(15))),
    }
    if rng.below(12) == 0 {
        // A stream comment: the encoder emits it as a metadata meta-block.
        a.push("--comment=c2lsbHN0b25lIGNvbW1lbnQ=".into());
    }
    a
}

fn run(bin: &str, args: &[String], data: &[u8]) -> Vec<u8> {
    let mut child = Command::new(bin)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn brotli");
    let mut stdin = child.stdin.take().unwrap();
    let owned = data.to_vec();
    let writer = std::thread::spawn(move || {
        let _ = stdin.write_all(&owned);
    });
    let out = child.wait_with_output().expect("brotli output");
    writer.join().unwrap();
    assert!(out.status.success(), "brotli failed for args {args:?}");
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

/// Packs bits least-significant first, the way a Brotli stream is laid out.
#[derive(Default)]
struct BitWriter {
    bytes: Vec<u8>,
    nbits: usize,
}

impl BitWriter {
    fn put(&mut self, value: u32, n: u32) {
        for i in 0..n {
            if self.nbits % 8 == 0 {
                self.bytes.push(0);
            }
            if (value >> i) & 1 == 1 {
                *self.bytes.last_mut().unwrap() |= 1 << (self.nbits % 8);
            }
            self.nbits += 1;
        }
    }
    /// A simple prefix code with one symbol (`abits` wide): costs no bits
    /// when the symbol is later coded.
    fn single_symbol_code(&mut self, symbol: u32, abits: u32) {
        self.put(1, 2); // simple
        self.put(0, 2); // NSYM - 1
        self.put(symbol, abits);
    }
}

/// A stream whose only command copies `len` bytes from the static dictionary
/// word `word_id` (distance = word_id + 1, past an empty window).
fn dictionary_stream(len: usize, word_id: usize) -> Vec<u8> {
    // Distance code layout with NPOSTFIX = 0 and NDIRECT = 0.
    let distance = word_id + 1;
    let mut ndistbits = 1u32;
    let (dcode, extra) = loop {
        let low = (2u32 << ndistbits) - 4 + 1; // smallest distance with this width
        let high = low + (1 << ndistbits) - 1;
        let low2 = (3u32 << ndistbits) - 4 + 1;
        let high2 = low2 + (1 << ndistbits) - 1;
        if (low..=high).contains(&(distance as u32)) {
            break (16 + 2 * (ndistbits - 1), distance as u32 - low);
        }
        if (low2..=high2).contains(&(distance as u32)) {
            break (16 + 2 * (ndistbits - 1) + 1, distance as u32 - low2);
        }
        ndistbits += 1;
    };
    // Insert-and-copy symbol: insert length 0, the copy length code for `len`.
    let (copy_code, copy_extra_bits, copy_extra) = match len {
        2..=9 => (len - 2, 0, 0),
        10..=11 => (8, 1, len - 10),
        12..=13 => (9, 1, len - 12),
        14..=17 => (10, 2, len - 14),
        18..=21 => (11, 2, len - 18),
        _ => (12, 3, len - 22),
    };
    // Cell 2 (explicit distance), insert code 0, copy codes 0..=7; cell 3 for 8..=15.
    let cmd = if copy_code < 8 {
        128 + copy_code
    } else {
        192 + (copy_code - 8)
    };

    let mut w = BitWriter::default();
    w.put(0, 1); // WBITS = 16
    w.put(1, 1); // ISLAST
    w.put(0, 1); // not empty
    w.put(0, 2); // MNIBBLES = 4
    // MLEN - 1 is a placeholder here; `with_mlen` patches it in place.
    w.put(0, 16);
    w.put(0, 1); // NBLTYPESL = 1
    w.put(0, 1); // NBLTYPESI = 1
    w.put(0, 1); // NBLTYPESD = 1
    w.put(0, 2); // NPOSTFIX
    w.put(0, 4); // NDIRECT
    w.put(0, 2); // context mode of the one literal type
    w.put(0, 1); // NTREESL = 1
    w.put(0, 1); // NTREESD = 1
    w.single_symbol_code(0, 8); // literal code (unused)
    w.single_symbol_code(cmd as u32, 10);
    w.single_symbol_code(dcode, 6);
    w.put(copy_extra as u32, copy_extra_bits);
    w.put(extra, ndistbits);
    w.bytes
}

#[test]
fn committed_corpus_decodes_to_the_recorded_digests() {
    let recs = records(CORPUS);
    assert!(recs.len() >= 100, "corpus has {} records", recs.len());
    for (i, (stream, plen, digest)) in recs.iter().enumerate() {
        let out = inflate_brotli(stream, limits()).unwrap_or_else(|e| panic!("record {i}: {e}"));
        assert_eq!(out.len() as u64, *plen, "record {i} length");
        assert_eq!(
            sha256(&out).unwrap().as_bytes(),
            digest,
            "record {i} digest"
        );
    }
}

#[test]
fn mutated_streams_never_panic() {
    let recs = records(CORPUS);
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    for (stream, _, _) in &recs {
        for _ in 0..24 {
            let mut m = stream.to_vec();
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
            // Brotli has no checksum, so damage may decode to other bytes;
            // the contract is no panic and no output past the limit.
            let cap = Limits {
                max_output: 4 << 20,
                max_input: 64 << 20,
            };
            if let Ok(out) = inflate_brotli(&m, cap) {
                assert!(out.len() <= cap.max_output);
            }
        }
    }
}

#[test]
fn every_truncation_of_a_compressed_stream_is_an_error() {
    let recs = records(CORPUS);
    let mut checked = 0;
    for (stream, _, _) in recs.iter().filter(|(s, _, _)| s.len() < 400) {
        for cut in 0..stream.len() {
            // A cut stream can only be valid if what is left is itself a
            // complete stream, which a proper prefix of an encoder's output
            // never is.
            assert!(
                inflate_brotli(&stream[..cut], limits()).is_err(),
                "prefix of {cut}/{} bytes must not decode",
                stream.len()
            );
            checked += 1;
        }
    }
    assert!(checked > 500, "only {checked} prefixes checked");
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xEDB8_8320 & (!(crc & 1)).wrapping_add(1));
        }
    }
    !crc
}

#[test]
fn embedded_dictionary_matches_the_rfc_checksums() {
    // The dictionary is not exported; recover it through dictionary streams:
    // word (len, index) with transform 0 decodes to the raw dictionary word.
    let mut dict = Vec::new();
    for (len, &bits) in NDBITS.iter().enumerate().skip(4) {
        for index in 0..(1usize << bits) {
            let out = inflate_brotli(&with_mlen(len, index, len), limits())
                .unwrap_or_else(|e| panic!("word {len}/{index}: {e}"));
            assert_eq!(out.len(), len);
            dict.extend_from_slice(&out);
        }
    }
    assert_eq!(dict.len(), 122_784);
    // RFC 7932 Appendix A: CRC-32 of the dictionary.
    assert_eq!(crc32(&dict), 0x5136_cb04);
    assert_eq!(
        sha256(&dict).unwrap().as_bytes()[..],
        hex32("20e42eb1b511c21806d4d227d07e5dd06877d8ce7b3a817f378f313653f35c70")
    );
}

fn hex32(s: &str) -> [u8; 32] {
    let mut out = [0u8; 32];
    for (i, b) in out.iter_mut().enumerate() {
        *b = u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap();
    }
    out
}

/// A dictionary stream with the MLEN field filled in: `mlen` must equal the
/// length of the transformed word.
fn with_mlen(len: usize, word_id: usize, mlen: usize) -> Vec<u8> {
    let mut s = dictionary_stream(len, word_id);
    // MLEN - 1 occupies bits 5..21 of the stream (after WBITS, ISLAST,
    // ISLASTEMPTY and MNIBBLES); patch it in place.
    let v = (mlen - 1) as u32;
    for i in 0..16 {
        let pos = 5 + i;
        let bit = (v >> i) & 1;
        let byte = &mut s[pos / 8];
        *byte = (*byte & !(1 << (pos % 8))) | ((bit as u8) << (pos % 8));
    }
    s
}

#[test]
fn dictionary_transforms_match_the_recorded_digests() {
    let recs = records(TRANSFORMS);
    assert!(recs.len() >= 121 * 4, "fixture has {} records", recs.len());
    for (i, (stream, plen, digest)) in recs.iter().enumerate() {
        let out = inflate_brotli(stream, limits()).unwrap_or_else(|e| panic!("record {i}: {e}"));
        assert_eq!(out.len() as u64, *plen, "record {i} length");
        assert_eq!(
            sha256(&out).unwrap().as_bytes(),
            digest,
            "record {i} digest"
        );
    }
}

/// The (length, word index, transform) triples the transform fixture covers:
/// every transform on a spread of word lengths, indexes chosen to include
/// ASCII, two-byte and three-byte UTF-8 openings.
fn transform_cases() -> Vec<(usize, usize, usize)> {
    let mut cases = Vec::new();
    let mut rng = Rng(0xD1C7_10A4);
    for t in 0..121usize {
        for len in [4usize, 5, 6, 9, 13, 24] {
            for _ in 0..2 {
                let idx = rng.below(1 << NDBITS[len]) as usize;
                cases.push((len, idx, t));
            }
        }
    }
    cases
}

/// Builds the stream for one transform case. The MLEN has to equal the
/// transformed word's length, which the stream itself does not state: find it
/// by trial with our decoder (it rejects a wrong MLEN), then let the reference
/// decoder confirm the choice by decoding the same stream to the same bytes.
/// A transform that leaves nothing of the word (an "omit first N" on a word
/// no longer than N, with no prefix or suffix) yields no MLEN: `None`.
fn build_transform_stream(len: usize, idx: usize, t: usize) -> Option<Vec<u8>> {
    let word_id = (t << NDBITS[len]) | idx;
    (1..=80)
        .map(|mlen| (mlen, with_mlen(len, word_id, mlen)))
        .find_map(|(mlen, stream)| {
            let out = inflate_brotli(&stream, limits()).ok()?;
            (out.len() == mlen).then_some(stream)
        })
}

fn run_decode(bin: &str, stream: &[u8]) -> Option<Vec<u8>> {
    let mut child = Command::new(bin)
        .args(["-d", "-c"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut stdin = child.stdin.take()?;
    let owned = stream.to_vec();
    let writer = std::thread::spawn(move || {
        let _ = stdin.write_all(&owned);
    });
    let out = child.wait_with_output().ok()?;
    writer.join().ok()?;
    out.status.success().then_some(out.stdout)
}

#[test]
#[ignore = "needs the reference brotli binary: set SILLSTONE_BROTLI_BIN"]
fn oracle_fuzz_against_brotli_cli() {
    let bin = std::env::var("SILLSTONE_BROTLI_BIN").expect("SILLSTONE_BROTLI_BIN");
    let cases: usize = std::env::var("SILLSTONE_BROTLI_CASES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1000);
    let seed: u64 = std::env::var("SILLSTONE_BROTLI_SEED")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0xB407_11C0_5EED);
    let corpus_out = std::env::var("SILLSTONE_BROTLI_CORPUS_OUT").ok();
    let transforms_out = std::env::var("SILLSTONE_BROTLI_TRANSFORMS_OUT").ok();
    let mut rng = Rng(seed);
    let mut corpus = Vec::new();
    let mut corpus_cases = 0usize;
    let mut total_in = 0usize;
    let mut total_out = 0usize;
    for i in 0..cases {
        let plain = make_input(&mut rng);
        let args = make_args(&mut rng);
        let stream = run(&bin, &args, &plain);
        let out = inflate_brotli(&stream, limits())
            .unwrap_or_else(|e| panic!("case {i} args {args:?} len {}: {e}", plain.len()));
        assert!(
            out == plain,
            "case {i} args {args:?} len {}: output differs",
            plain.len()
        );
        total_in += stream.len();
        total_out += out.len();
        // Keep the committed corpus small: modest inputs only.
        if corpus_out.is_some()
            && corpus_cases < 160
            && plain.len() <= 200_000
            && stream.len() <= 6000
        {
            corpus.extend_from_slice(&record(&stream, &plain));
            corpus_cases += 1;
        }
    }
    eprintln!("{cases} cases ok: {total_in} compressed bytes -> {total_out} plain bytes");
    if let Some(path) = corpus_out {
        std::fs::write(&path, &corpus).unwrap();
        eprintln!("wrote {corpus_cases} corpus records to {path}");
    }

    // Transforms: the hand-built streams decode identically in the reference
    // decoder and ours, for every transform.
    let mut fixture = Vec::new();
    let mut n = 0usize;
    for (len, idx, t) in transform_cases() {
        let Some(stream) = build_transform_stream(len, idx, t) else {
            continue;
        };
        let expected = run_decode(&bin, &stream).expect("reference decode");
        let ours = inflate_brotli(&stream, limits())
            .unwrap_or_else(|e| panic!("transform ({len},{idx},{t}): {e}"));
        assert_eq!(ours, expected, "transform ({len},{idx},{t})");
        fixture.extend_from_slice(&record(&stream, &expected));
        n += 1;
    }
    eprintln!("{n} dictionary transform cases ok");
    if let Some(path) = transforms_out {
        std::fs::write(&path, &fixture).unwrap();
        eprintln!("wrote {n} transform records to {path}");
    }
}
