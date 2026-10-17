//! Regenerates and verifies `reference.json`: the hex-exact cross-SDK
//! test vectors for `sillstone-digest`.
//!
//! Every vector is computed live from the library, so `gen` is a true
//! regeneration and `verify` fails the build the moment committed bytes
//! drift from what the code actually produces. The corpus is the
//! published-specification set the conformance tests pin (FIPS 180-2,
//! RFC 1950, the PNG spec, the FNV reference, the splitmix64 paper)
//! plus the padding-boundary inputs; all values are lowercase hex
//! strings so Python, Node and Go SDKs compare the same bytes.
//!
//! Usage:
//!
//! ```text
//! gen-reference gen      # write ./reference.json
//! gen-reference verify   # compare ./reference.json byte-for-byte
//! ```

use std::fs;
use std::process::ExitCode;

use sillstone_digest::{SplitMix64, adler32, crc32, fnv1a64, sha256};

/// The file the suite contract fixes at the repository root.
const PATH: &str = "reference.json";

/// The SHA-256 corpus: the published FIPS 180-2 appendix vectors and
/// the padding-boundary lengths around the 55/56/64-byte tails. The
/// one-million-'a' multi-block vector stays in the Rust conformance
/// suite; this file carries only inputs an SDK test can replay cheaply.
const SHA256_INPUTS: [&[u8]; 8] = [
    b"",
    b"abc",
    b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq",
    &[b'x'; 55],
    &[b'x'; 56],
    &[b'x'; 64],
    &[b'x'; 65],
    b"xxxx",
];

/// The CRC-32 corpus: the canonical check value and a text sample.
const CRC32_INPUTS: [&[u8]; 3] = [
    b"",
    b"123456789",
    b"The quick brown fox jumps over the lazy dog",
];

/// The Adler-32 corpus: the RFC 1950 examples.
const ADLER32_INPUTS: [&[u8]; 4] = [b"", b"a", b"abc", b"Wikipedia"];

/// The FNV-1a 64 corpus: the reference vectors.
const FNV1A64_INPUTS: [&[u8]; 3] = [b"", b"a", b"foobar"];

/// The splitmix64 corpus: seeds with their first eight outputs.
const SPLITMIX64_SEEDS: [u64; 3] = [0, 42, 7];

/// The number of outputs recorded per splitmix64 seed.
const SPLITMIX64_VALUES: usize = 8;

/// The SHA-1 corpus: the FIPS 180-4 appendix vectors (the empty
/// message and "abc" replay cheaply in every SDK) plus one text case.
const SHA1_INPUTS: [&[u8]; 4] = [
    b"",
    b"abc",
    b"foobar",
    b"The quick brown fox jumps over the lazy dog",
];

/// The SHA-512 corpus: the same shape as the SHA-1 one.
const SHA512_INPUTS: [&[u8]; 4] = [
    b"",
    b"abc",
    b"foobar",
    b"The quick brown fox jumps over the lazy dog",
];

/// The HMAC corpora: `(key, data)` pairs — RFC 2104 test cases 1 and
/// 2, then the RFC 4231 test-case-6 over-long key (RFC 2202 test case
/// 6 for the SHA-1 instantiation, with its 80-byte key).
const HMAC_SHA1_CASES: [(&[u8], &[u8]); 3] = [
    (&[0x0b; 20], b"Hi There"),
    (b"Jefe", b"what do ya want for nothing?"),
    (
        &[0xaa; 80],
        b"Test Using Larger Than Block-Size Key - Hash Key First",
    ),
];

const HMAC_SHA256_CASES: [(&[u8], &[u8]); 3] = [
    (&[0x0b; 20], b"Hi There"),
    (b"Jefe", b"what do ya want for nothing?"),
    (
        &[0xaa; 131],
        b"Test Using Larger Than Block-Size Key - Hash Key First",
    ),
];

const HMAC_SHA512_CASES: [(&[u8], &[u8]); 3] = [
    (&[0x0b; 20], b"Hi There"),
    (b"Jefe", b"what do ya want for nothing?"),
    (
        &[0xaa; 131],
        b"Test Using Larger Than Block-Size Key - Hash Key First",
    ),
];

/// The xxHash64 corpus: (seed, input) pairs spanning the byte, word,
/// doubleword and stripe branches.
const XXH64_CASES: [(u64, &[u8]); 6] = [
    (0, b""),
    (1, b""),
    (0, b"abc"),
    (0, b"Hello, world!"),
    (
        7,
        &[
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
            24, 25, 26, 27, 28, 29, 30,
        ],
    ),
    (
        0,
        &[
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
            24, 25, 26, 27, 28, 29, 30, 31, 32,
        ],
    ),
];

/// The MurmurHash3 x64 128 corpus: (seed, input) pairs across the
/// tail and body branches.
const MURMUR3_CASES: [(u32, &[u8]); 5] = [
    (0, b""),
    (1, b""),
    (0, b"hello"),
    (
        0,
        &[
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
            24, 25, 26, 27, 28, 29, 30, 31,
        ],
    ),
    (
        42,
        &[
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
            24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45,
            46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67,
            68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 89,
            90, 91, 92, 93, 94, 95, 96, 97, 98, 99,
        ],
    ),
];

/// The CRC-32C corpus: the RFC 3720 B.4 zero and one patterns, the
/// RFC 4960 check value and the empty input.
const CRC32C_INPUTS: [&[u8]; 4] = [&[0x00; 32], &[0xff; 32], b"123456789", b""];

/// The RFC 4648 section 10 corpus.
const BASE64_CASES: [&[u8]; 7] = [b"", b"f", b"fo", b"foo", b"foob", b"fooba", b"foobar"];

/// The xoshiro256** corpus: seeds with their first four outputs
/// (splitmix64 seeding, the reference implementation's convention).
const XOSHIRO256_SEEDS: [u64; 3] = [0, 1, 42];

/// The number of outputs recorded per xoshiro256** seed.
const XOSHIRO256_VALUES: usize = 4;

fn main() -> ExitCode {
    let mode = match std::env::args().nth(1) {
        Some(mode) => mode,
        None => {
            eprintln!(
                "usage: gen-reference <gen|verify>  (operates on {PATH} in the working directory)"
            );
            return ExitCode::from(2);
        }
    };
    match mode.as_str() {
        "gen" => {
            let contents = render();
            match fs::write(PATH, &contents) {
                Ok(()) => {
                    println!("wrote {PATH} ({} bytes)", contents.len());
                    ExitCode::SUCCESS
                }
                Err(err) => {
                    eprintln!("FAIL: could not write {PATH}: {err}");
                    ExitCode::from(1)
                }
            }
        }
        "verify" => {
            let expected = render();
            let actual = match fs::read(PATH) {
                Ok(bytes) => bytes,
                Err(err) => {
                    eprintln!("FAIL: could not read {PATH}: {err}");
                    return ExitCode::from(1);
                }
            };
            if actual == expected.as_bytes() {
                println!(
                    "reference vectors are current: {PATH} matches the live library ({} bytes)",
                    expected.len()
                );
                ExitCode::SUCCESS
            } else {
                let first = actual
                    .iter()
                    .zip(expected.as_bytes())
                    .position(|(a, e)| a != e)
                    .unwrap_or_else(|| actual.len().min(expected.len()));
                eprintln!(
                    "FAIL: {PATH} is stale: {} actual bytes vs {} expected, first difference at byte {first}",
                    actual.len(),
                    expected.len(),
                );
                eprintln!("regenerate with: cargo run --bin gen-reference -- gen");
                ExitCode::from(1)
            }
        }
        other => {
            eprintln!("usage: gen-reference <gen|verify>  (got {other:?})");
            ExitCode::from(2)
        }
    }
}

/// Builds the complete byte-exact `reference.json` contents from the
/// live library. Field order, indentation and the trailing newline are
/// fixed so two runs on any machine produce identical bytes.
fn render() -> String {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str("  \"schema\": \"sillstone-digest/reference-vectors/1\",\n");

    out.push_str("  \"sha256\": [\n");
    for (i, input) in SHA256_INPUTS.iter().enumerate() {
        let digest = hex(sha256(input).expect("within size limit").as_bytes());
        out.push_str(&format!(
            "    {{\"input_hex\": \"{}\", \"digest\": \"{}\"}}{}\n",
            hex(input),
            digest,
            comma(i, SHA256_INPUTS.len()),
        ));
    }
    out.push_str("  ],\n");

    section_u32(&mut out, "crc32", &CRC32_INPUTS, crc32);
    section_u32(&mut out, "adler32", &ADLER32_INPUTS, adler32);

    out.push_str("  \"fnv1a64\": [\n");
    for (i, input) in FNV1A64_INPUTS.iter().enumerate() {
        out.push_str(&format!(
            "    {{\"input_hex\": \"{}\", \"fnv1a64\": \"{}\"}}{}\n",
            hex(input),
            hex_u64(fnv1a64(input)),
            comma(i, FNV1A64_INPUTS.len()),
        ));
    }
    out.push_str("  ],\n");

    out.push_str("  \"splitmix64\": [\n");
    for (i, seed) in SPLITMIX64_SEEDS.iter().enumerate() {
        let mut rng = SplitMix64::new(*seed);
        out.push_str(&format!(
            "    {{\"seed_hex\": \"{}\", \"outputs\": [",
            hex_u64(*seed),
        ));
        for v in 0..SPLITMIX64_VALUES {
            out.push_str(&format!("\"{}\"", hex_u64(rng.next_u64())));
            if v + 1 < SPLITMIX64_VALUES {
                out.push_str(", ");
            }
        }
        out.push_str(&format!("]}}{}\n", comma(i, SPLITMIX64_SEEDS.len())));
    }
    out.push_str("  ],\n");

    out.push_str("  \"sha1\": [\n");
    for (i, input) in SHA1_INPUTS.iter().enumerate() {
        let digest = hex(sillstone_digest::sha1(input)
            .expect("within size limit")
            .as_bytes());
        out.push_str(&format!(
            "    {{\"input_hex\": \"{}\", \"digest\": \"{}\"}}{}\n",
            hex(input),
            digest,
            comma(i, SHA1_INPUTS.len()),
        ));
    }
    out.push_str("  ],\n");

    out.push_str("  \"sha512\": [\n");
    for (i, input) in SHA512_INPUTS.iter().enumerate() {
        let digest = hex(sillstone_digest::sha512(input).as_bytes());
        out.push_str(&format!(
            "    {{\"input_hex\": \"{}\", \"digest\": \"{}\"}}{}\n",
            hex(input),
            digest,
            comma(i, SHA512_INPUTS.len()),
        ));
    }
    out.push_str("  ],\n");

    section_pairs(&mut out, "hmac_sha1", &HMAC_SHA1_CASES, |key, data| {
        hex(sillstone_digest::hmac_sha1(key, data)
            .expect("legal inputs")
            .as_bytes())
    });
    section_pairs(&mut out, "hmac_sha256", &HMAC_SHA256_CASES, |key, data| {
        hex(sillstone_digest::hmac_sha256(key, data)
            .expect("legal inputs")
            .as_bytes())
    });
    section_pairs(&mut out, "hmac_sha512", &HMAC_SHA512_CASES, |key, data| {
        hex(sillstone_digest::hmac_sha512(key, data).as_bytes())
    });

    out.push_str("  \"xxh64\": [\n");
    for (i, (seed, input)) in XXH64_CASES.iter().enumerate() {
        out.push_str(&format!(
            "    {{\"input_hex\": \"{}\", \"seed_hex\": \"{}\", \"hash\": \"{}\"}}{}\n",
            hex(input),
            hex_u64(*seed),
            hex_u64(sillstone_digest::xxh64(input, *seed)),
            comma(i, XXH64_CASES.len()),
        ));
    }
    out.push_str("  ],\n");

    out.push_str("  \"murmur3_x64_128\": [\n");
    for (i, (seed, input)) in MURMUR3_CASES.iter().enumerate() {
        out.push_str(&format!(
            "    {{\"input_hex\": \"{}\", \"seed_hex\": \"{}\", \"digest\": \"{}\"}}{}\n",
            hex(input),
            hex_u32(*seed),
            hex(sillstone_digest::murmur3_x64_128(input, *seed).as_bytes()),
            comma(i, MURMUR3_CASES.len()),
        ));
    }
    out.push_str("  ],\n");

    section_u32(&mut out, "crc32c", &CRC32C_INPUTS, sillstone_digest::crc32c);

    out.push_str("  \"base64\": [\n");
    for (i, raw) in BASE64_CASES.iter().enumerate() {
        let enc_len = raw.len().div_ceil(3) * 4;
        let mut buf = [0u8; 12];
        sillstone_digest::base64_encode(raw, &mut buf[..enc_len]).expect("buffer fits");
        out.push_str(&format!(
            "    {{\"input_hex\": \"{}\", \"encoded_hex\": \"{}\"}}{}\n",
            hex(raw),
            hex(&buf[..enc_len]),
            comma(i, BASE64_CASES.len()),
        ));
    }
    out.push_str("  ],\n");

    out.push_str("  \"xoshiro256ss\": [\n");
    for (i, seed) in XOSHIRO256_SEEDS.iter().enumerate() {
        let mut rng = sillstone_digest::Xoshiro256StarStar::from_seed(*seed);
        out.push_str(&format!(
            "    {{\"seed_hex\": \"{}\", \"outputs\": [",
            hex_u64(*seed),
        ));
        for v in 0..XOSHIRO256_VALUES {
            out.push_str(&format!("\"{}\"", hex_u64(rng.next_u64())));
            if v + 1 < XOSHIRO256_VALUES {
                out.push_str(", ");
            }
        }
        out.push_str(&format!("]}}{}\n", comma(i, XOSHIRO256_SEEDS.len())));
    }
    out.push_str("  ]\n");
    out.push_str("}\n");
    out
}

/// Appends one named section of `{"key_hex", "data_hex", "<name>"}`
/// keyed cases, the HMAC shape.
fn section_pairs(
    out: &mut String,
    name: &str,
    cases: &[(&[u8], &[u8])],
    f: impl Fn(&[u8], &[u8]) -> String,
) {
    out.push_str(&format!("  \"{name}\": [\n"));
    for (i, (key, data)) in cases.iter().enumerate() {
        out.push_str(&format!(
            "    {{\"key_hex\": \"{}\", \"data_hex\": \"{}\", \"mac\": \"{}\"}}{}\n",
            hex(key),
            hex(data),
            f(key, data),
            comma(i, cases.len()),
        ));
    }
    out.push_str("  ],\n");
}

/// Appends one named section of `{"input_hex", "<name>"}` u32 cases.
fn section_u32(out: &mut String, name: &str, inputs: &[&[u8]], f: fn(&[u8]) -> u32) {
    out.push_str(&format!("  \"{name}\": [\n"));
    for (i, input) in inputs.iter().enumerate() {
        out.push_str(&format!(
            "    {{\"input_hex\": \"{}\", \"{name}\": \"{}\"}}{}\n",
            hex(input),
            hex_u32(f(input)),
            comma(i, inputs.len()),
        ));
    }
    out.push_str("  ],\n");
}

/// The comma that separates a JSON array item from the next one.
fn comma(index: usize, len: usize) -> &'static str {
    if index + 1 == len { "" } else { "," }
}

/// Lowercase hex of a byte slice.
fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        out.push(DIGITS[usize::from(byte >> 4)] as char);
        out.push(DIGITS[usize::from(byte & 0x0f)] as char);
    }
    out
}

/// Lowercase 8-digit hex of a `u32`.
fn hex_u32(value: u32) -> String {
    hex(&value.to_be_bytes())
}

/// Lowercase 16-digit hex of a `u64`.
fn hex_u64(value: u64) -> String {
    hex(&value.to_be_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two renders are byte-identical: the generator is deterministic.
    #[test]
    fn render_is_deterministic() {
        assert_eq!(render(), render());
    }

    /// The rendered document is complete, balanced JSON with a trailing
    /// newline, and every section is present with plausible entries.
    #[test]
    fn render_shape_is_complete() {
        let doc = render();
        assert!(doc.starts_with("{\n"));
        assert!(doc.ends_with("\"\n}\n") || doc.ends_with("}\n"));
        assert_eq!(doc.matches('{').count(), doc.matches('}').count());
        assert_eq!(doc.matches('[').count(), doc.matches(']').count());
        for section in [
            "\"schema\": \"sillstone-digest/reference-vectors/1\"",
            "\"sha256\": [",
            "\"crc32\": [",
            "\"adler32\": [",
            "\"fnv1a64\": [",
            "\"splitmix64\": [",
            "\"sha1\": [",
            "\"sha512\": [",
            "\"hmac_sha1\": [",
            "\"hmac_sha256\": [",
            "\"hmac_sha512\": [",
            "\"xxh64\": [",
            "\"murmur3_x64_128\": [",
            "\"crc32c\": [",
            "\"base64\": [",
            "\"xoshiro256ss\": [",
        ] {
            assert!(doc.contains(section), "missing {section}");
        }
        // No array item is followed by a stray comma before the close.
        assert!(!doc.contains(",\n  ]"));
        assert!(!doc.contains(", ]\n"));
    }

    /// The recorded vectors are the published values, read back out of
    /// the rendered document through the live library.
    #[test]
    fn recorded_vectors_match_the_specifications() {
        let doc = render();
        // FIPS 180-2 "abc".
        let want = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        assert!(doc.contains(&format!(
            "\"input_hex\": \"{}\", \"digest\": \"{want}\"",
            hex(b"abc")
        )));
        // FIPS 180-2 empty message.
        assert!(doc.contains(
            "\"digest\": \"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855\""
        ));
        // PNG spec CRC-32 check value.
        assert!(doc.contains(&format!("\"crc32\": \"{}\"", hex_u32(0xCBF4_3926))));
        // RFC 1950 Adler-32 of "Wikipedia".
        assert!(doc.contains(&format!(
            "\"input_hex\": \"{}\", \"adler32\": \"{}\"",
            hex(b"Wikipedia"),
            hex_u32(0x11E6_0398)
        )));
        // FNV reference values.
        assert!(doc.contains(&format!(
            "\"fnv1a64\": \"{}\"",
            hex_u64(0xcbf2_9ce4_8422_2325)
        )));
        assert!(doc.contains(&format!(
            "\"input_hex\": \"{}\", \"fnv1a64\": \"{}\"",
            hex(b"foobar"),
            hex_u64(0x8594_4171_f739_67e8)
        )));
        // splitmix64 seed 0 paper sequence.
        assert!(doc.contains(
            "\"seed_hex\": \"0000000000000000\", \"outputs\": [\"e220a8397b1dcdaf\", \
             \"6e789e6aa1b965f4\""
        ));
        // FIPS 180-4 SHA-1 "abc".
        assert!(doc.contains(&format!(
            "\"input_hex\": \"{}\", \"digest\": \"a9993e364706816aba3e25717850c26c9cd0d89d\"",
            hex(b"abc")
        )));
        // FIPS 180-4 SHA-512 "abc".
        assert!(doc.contains(
            "\"digest\": \"ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f\""
        ));
        // RFC 2104 HMAC-SHA-256 test case 1.
        assert!(doc.contains(
            "\"mac\": \"b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7\""
        ));
        // RFC 2104 HMAC-SHA-1 test case 1.
        assert!(doc.contains("\"mac\": \"b617318655057264e28bc0b6fb378c8ef146be00\""));
        // RFC 2104 HMAC-SHA-512 test case 1.
        assert!(doc.contains(
            "\"mac\": \"87aa7cdea5ef619d4ff0b4241a1d6cb02379f4e2ce4ec2787ad0b30545e17cdedaa833b7d6b8a702038b274eaea3f4e4be9d914eeb61f1702e696c203a126854\""
        ));
        // The xxHash reference empty-input constant.
        assert!(doc.contains(&format!(
            "\"input_hex\": \"\", \"seed_hex\": \"{}\", \"hash\": \"{}\"",
            hex_u64(0),
            hex_u64(0xEF46_DB37_51D8_E999)
        )));
        // The SMHasher all-zero empty-input digest.
        assert!(doc.contains("\"digest\": \"00000000000000000000000000000000\""));
        // RFC 3720 B.4 CRC-32C of 32 zero bytes.
        assert!(doc.contains(&format!(
            "\"input_hex\": \"{}\", \"crc32c\": \"{}\"",
            hex(&[0x00; 32]),
            hex_u32(0x8A91_36AA)
        )));
        // RFC 4648 section 10.
        assert!(doc.contains(&format!(
            "\"input_hex\": \"{}\", \"encoded_hex\": \"{}\"",
            hex(b"foobar"),
            hex(b"Zm9vYmFy")
        )));
        // The xoshiro256** reference sequence from splitmix64 seed 0.
        assert!(
            doc.contains("\"seed_hex\": \"0000000000000000\", \"outputs\": [\"99ec5f36cb75f2b4\"")
        );
    }

    /// A written render reads back as current: the verify comparison is
    /// byte-for-byte against exactly these bytes.
    #[test]
    fn written_file_verifies_byte_for_byte() {
        let dir = std::env::temp_dir().join("sillstone-digest-gen-reference-test");
        std::fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join("reference.json");
        std::fs::write(&path, render()).expect("write");
        let read_back = std::fs::read(&path).expect("read");
        assert_eq!(read_back, render().as_bytes());
        std::fs::remove_file(&path).ok();
    }

    /// The hex helpers pin lowercase, fixed-width, big-endian output.
    #[test]
    fn hex_helpers_are_lowercase_fixed_width() {
        assert_eq!(hex(b""), "");
        assert_eq!(hex(&[0x00, 0xff, 0x0a]), "00ff0a");
        assert_eq!(hex_u32(0x0000_0001), "00000001");
        assert_eq!(hex_u64(0xcbf2_9ce4_8422_2325), "cbf29ce484222325");
    }

    /// The separator helper emits commas only between items.
    #[test]
    fn comma_separates_only_between_items() {
        assert_eq!(comma(0, 1), "");
        assert_eq!(comma(0, 3), ",");
        assert_eq!(comma(1, 3), ",");
        assert_eq!(comma(2, 3), "");
    }

    /// One named u32 section renders the exact per-item shape.
    #[test]
    fn section_renders_item_shape() {
        let mut out = String::new();
        section_u32(&mut out, "crc32", &[b"123456789"], crc32);
        assert_eq!(
            out,
            "  \"crc32\": [\n    {\"input_hex\": \"\
             313233343536373839\", \"crc32\": \"cbf43926\"}\n  ],\n"
        );
    }
}
