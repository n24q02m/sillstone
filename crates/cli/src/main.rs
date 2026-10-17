//! The `pith` command line interface (design spec §4.5, phase P18).
//!
//! Thin argument parsing over `sillstone_cli`'s library surface:
//!
//! ```text
//! pith describe <file> [--json]
//! pith hash <file> [--json]
//! pith match <a> <b> [--json]
//! pith dedup <dir> [--all] [--threshold N] [--json]
//! pith bench [--iters N] [--json]
//! pith calibrate <pairs.csv> --profile dedup|search [--json]
//! ```
//!
//! Exit codes (documented in `--help`): `0` ok · `1` pipeline refusal
//! or non-match · `2` usage or I/O error.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use sillstone_curator::{Facts, Signature};
use sillstone_cli::{DedupReport, Input, dedup_inputs, fixture};

/// `0` — command produced its answer.
const OK: u8 = 0;
/// `1` — the input was refused (`Unsupported`/`Decode`/…) or `match`
/// answered "no".
const FAILED: u8 = 1;
/// `2` — the invocation itself was wrong or a path could not be read.
const USAGE: u8 = 2;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let Some(cmd) = args.next() else {
        print!("{USAGE_TEXT}");
        return ExitCode::from(OK);
    };
    let rest: Vec<String> = args.collect();
    let code = match cmd.as_str() {
        "-h" | "--help" | "help" => {
            print!("{USAGE_TEXT}");
            OK
        }
        "version" | "--version" => {
            println!("pith {}", env!("CARGO_PKG_VERSION"));
            OK
        }
        "describe" => cmd_describe(&rest),
        "hash" => cmd_hash(&rest),
        "match" => cmd_match(&rest),
        "dedup" => cmd_dedup(&rest),
        "bench" => cmd_bench(&rest),
        "calibrate" => cmd_calibrate(&rest),
        "digest" => cmd_digest(&rest),
        "image" => cmd_image(&rest),
        other => {
            eprintln!("pith: unknown command `{other}`\n");
            eprint!("{USAGE_TEXT}");
            USAGE
        }
    };
    ExitCode::from(code)
}

// ---------------------------------------------------------------------
// Argument plumbing
// ---------------------------------------------------------------------

/// Parsed trailing flags shared by every subcommand.
struct Opts {
    /// Bare (non-flag) arguments.
    pos: Vec<String>,
    /// `--json` output requested.
    json: bool,
    /// `--all` (dedup: list singletons too).
    all: bool,
    /// `--threshold N` (dedup: scalar distance bound).
    threshold: Option<u32>,
    /// `--profile NAME` (calibrate).
    profile: Option<String>,
    /// `--iters N` (bench).
    iters: Option<u32>,
    /// `--alg NAME` (digest, image).
    alg: Option<String>,
}

impl Opts {
    fn parse(args: &[String]) -> Result<Opts, String> {
        let mut o = Opts {
            pos: Vec::new(),
            json: false,
            all: false,
            threshold: None,
            profile: None,
            iters: None,
            alg: None,
        };
        let mut it = args.iter().peekable();
        while let Some(a) = it.next() {
            match a.as_str() {
                "--json" => o.json = true,
                "--all" => o.all = true,
                "--threshold" => {
                    let v = it.next().ok_or("--threshold needs a value")?;
                    o.threshold = Some(v.parse().map_err(|_| format!("bad --threshold `{v}`"))?);
                }
                "--profile" => {
                    let v = it.next().ok_or("--profile needs a value")?;
                    o.profile = Some(v.clone());
                }
                "--iters" => {
                    let v = it.next().ok_or("--iters needs a value")?;
                    o.iters = Some(v.parse().map_err(|_| format!("bad --iters `{v}`"))?);
                }
                "--alg" => {
                    let v = it.next().ok_or("--alg needs a value")?;
                    o.alg = Some(v.clone());
                }
                "-h" | "--help" => o.pos.push("help".to_string()),
                other if other.starts_with("--") => return Err(format!("unknown flag `{other}`")),
                other => o.pos.push(other.to_string()),
            }
        }
        Ok(o)
    }
}

/// Reads `path` or explains the failure as a `USAGE` exit.
fn must_read(path: &str) -> Result<Vec<u8>, u8> {
    std::fs::read(path).map_err(|e| {
        eprintln!("pith: {path}: {e}");
        USAGE
    })
}

/// Prints a pipeline refusal to stderr in the modality-first spelling.
fn refuse(cmd: &str, path: &str, e: &sillstone_curator::Error) -> u8 {
    eprintln!("pith {cmd}: {path}: {e}");
    FAILED
}

// ---------------------------------------------------------------------
// describe
// ---------------------------------------------------------------------

const DESCRIBE_HELP: &str = "\
usage: pith describe <file> [--json]

Prints the three tiers for one input:
  tier1  SHA-256 over normalized content
  tier2  the modality signature (phash / audio peaks / minhash / fastcdc)
  tier3  local features (image: orb keypoints)

A pending lane reports 'pending: true' and exits 0 — the report is
honest, not a crash. (All DAG-table lanes have landed.)

exit: 0 ok | 1 refused | 2 usage/io
";

fn cmd_describe(args: &[String]) -> u8 {
    let Ok(o) = Opts::parse(args).map_err(|e| {
        eprintln!("pith describe: {e}");
    }) else {
        return USAGE;
    };
    if o.pos.first().is_some_and(|s| s == "help") {
        print!("{DESCRIBE_HELP}");
        return OK;
    }
    let Some(path) = o.pos.first() else {
        eprint!("{DESCRIBE_HELP}");
        return USAGE;
    };
    let Ok(bytes) = must_read(path) else {
        return USAGE;
    };
    let det = sillstone_curator::detect(&bytes);
    match sillstone_curator::describe(&bytes) {
        Ok(d) => {
            if o.json {
                println!("{}", describe_json(path, &d));
            } else {
                println!("{}", sillstone_cli::render_description(&d));
            }
            OK
        }
        Err(e) => {
            // Pending lanes (mp4 video) are a report, not a failure:
            // detection still answered.
            if det.pending {
                if o.json {
                    println!(
                        "{{\"file\": {}, \"format\": \"{}\", \"modality\": \"{}\", \"pending\": true, \"error\": {}}}",
                        sillstone_cli::json_string(path),
                        det.format.as_str(),
                        det.modality.as_str(),
                        sillstone_cli::json_string(&e.to_string()),
                    );
                } else {
                    println!("modality: {}", det.modality);
                    println!("format: {}", det.format);
                    println!("pending: true");
                    println!("unsupported: {e}");
                }
                return OK;
            }
            refuse("describe", path, &e)
        }
    }
}

fn describe_json(path: &str, d: &sillstone_curator::Description) -> String {
    let mut s = String::with_capacity(512);
    s.push('{');
    s.push_str(&format!("\"file\": {}, ", sillstone_cli::json_string(path)));
    s.push_str(&format!("\"format\": \"{}\", ", d.format.as_str()));
    s.push_str(&format!("\"modality\": \"{}\", ", d.modality.as_str()));
    s.push_str(&format!("\"tier1\": \"{}\", ", d.tier1));
    match (&d.signature, &d.facts) {
        (
            Signature::Image(p),
            Facts::Image {
                width,
                height,
                keypoints,
            },
        ) => {
            s.push_str(&format!(
                "\"tier2\": \"phash:{p:#018x}\", \"tier3\": \"orb:{keypoints}\", \"facts\": {{\"width\": {width}, \"height\": {height}}}"
            ));
        }
        (
            Signature::Audio(_),
            Facts::Audio {
                sample_rate,
                channels,
                frames,
                peaks,
            },
        ) => {
            s.push_str(&format!(
                "\"tier2\": \"audio-peaks:{peaks}\", \"tier3\": \"dtw\", \"facts\": {{\"sample_rate\": {sample_rate}, \"channels\": {channels}, \"frames\": {frames}, \"peaks\": {peaks}}}"
            ));
        }
        (
            Signature::Text(sig),
            Facts::Text {
                words,
                canonical_len,
            },
        ) => {
            s.push_str(&format!(
                "\"tier2\": \"minhash:{}\", \"tier3\": null, \"facts\": {{\"words\": {words}, \"canonical_len\": {canonical_len}}}",
                sig.len()
            ));
        }
        (Signature::Binary(_), Facts::Binary { len, chunks }) => {
            s.push_str(&format!(
                "\"tier2\": \"fastcdc:{chunks}\", \"tier3\": null, \"facts\": {{\"len\": {len}, \"chunks\": {chunks}}}"
            ));
        }
        (
            Signature::Video(v),
            Facts::Video {
                width,
                height,
                duration_s,
                frames_sampled,
            },
        ) => {
            s.push_str(&format!(
                "\"tier2\": \"video:{}f\", \"tier3\": null, \"facts\": {{\"width\": {width}, \"height\": {height}, \"duration_s\": {duration_s}, \"frames_sampled\": {frames_sampled}, \"minhash_words\": {}}}",
                v.frame_hashes.len(),
                v.minhash.len()
            ));
        }
        _ => s.push_str("\"tier2\": null, \"tier3\": null, \"facts\": null"),
    }
    s.push('}');
    s
}

// ---------------------------------------------------------------------
// hash
// ---------------------------------------------------------------------

const HASH_HELP: &str = "\
usage: pith hash <file> [--json]

Prints the tier-1 canonical hash: SHA-256 over the normalized content
(decoded pixels / downmixed PCM / canonical text / raw bytes).

exit: 0 ok | 1 refused | 2 usage/io
";

fn cmd_hash(args: &[String]) -> u8 {
    let Ok(o) = Opts::parse(args).map_err(|e| {
        eprintln!("pith hash: {e}");
    }) else {
        return USAGE;
    };
    if o.pos.first().is_some_and(|s| s == "help") {
        print!("{HASH_HELP}");
        return OK;
    }
    let Some(path) = o.pos.first() else {
        eprint!("{HASH_HELP}");
        return USAGE;
    };
    let Ok(bytes) = must_read(path) else {
        return USAGE;
    };
    match sillstone_curator::content_hash(&bytes) {
        Ok(d) => {
            if o.json {
                println!(
                    "{{\"file\": {}, \"tier1\": \"{}\"}}",
                    sillstone_cli::json_string(path),
                    d
                );
            } else {
                println!("{d}");
            }
            OK
        }
        Err(e) => refuse("hash", path, &e),
    }
}

// ---------------------------------------------------------------------
const DIGEST_HELP: &str = "\
usage: pith digest <file> [--alg sha256|sha3-256|sha3-512|blake2b|blake3] [--json]

Computes cryptographic digest of raw file bytes using the specified algorithm (default sha256).

exit: 0 ok | 1 refused | 2 usage/io
";

fn cmd_digest(args: &[String]) -> u8 {
    let Ok(o) = Opts::parse(args).map_err(|e| {
        eprintln!("pith digest: {e}");
    }) else {
        return USAGE;
    };
    if o.pos.first().is_some_and(|s| s == "help") {
        print!("{DIGEST_HELP}");
        return OK;
    }
    let Some(path) = o.pos.first() else {
        eprint!("{DIGEST_HELP}");
        return USAGE;
    };
    let Ok(bytes) = must_read(path) else {
        return USAGE;
    };
    let alg = o.alg.as_deref().unwrap_or("sha256");
    let hex_digest = match alg {
        "sha256" => sillstone_digest::sha256(&bytes).map(|d| d.to_string()),
        "sha3-256" | "sha3_256" => sillstone_digest::sha3_256(&bytes).map(|d| d.to_string()),
        "sha3-512" | "sha3_512" => sillstone_digest::sha3_512(&bytes).map(|d| d.to_string()),
        "blake2b" | "blake2b-512" | "blake2b_512" => {
            sillstone_digest::blake2b_512(&bytes).map(|d| d.to_string())
        }
        "blake3" => sillstone_digest::blake3(&bytes).map(|d| d.to_string()),
        other => {
            eprintln!("pith digest: unknown algorithm `{other}`");
            return USAGE;
        }
    };

    match hex_digest {
        Ok(h) => {
            if o.json {
                println!(
                    "{{\"file\": {}, \"alg\": \"{}\", \"digest\": \"{}\"}}",
                    sillstone_cli::json_string(path),
                    alg,
                    h
                );
            } else {
                println!("{h}");
            }
            OK
        }
        Err(e) => {
            eprintln!("pith digest: {path}: {e}");
            FAILED
        }
    }
}

const IMAGE_HELP: &str = "\
usage: pith image <file> [--alg phash|ahash|dhash|whash|colorhash] [--json]

Computes perceptual image hash using the specified algorithm (default phash).

exit: 0 ok | 1 refused | 2 usage/io
";

fn cmd_image(args: &[String]) -> u8 {
    let Ok(o) = Opts::parse(args).map_err(|e| {
        eprintln!("pith image: {e}");
    }) else {
        return USAGE;
    };
    if o.pos.first().is_some_and(|s| s == "help") {
        print!("{IMAGE_HELP}");
        return OK;
    }
    let Some(path) = o.pos.first() else {
        eprint!("{IMAGE_HELP}");
        return USAGE;
    };
    let Ok(bytes) = must_read(path) else {
        return USAGE;
    };
    let det = sillstone_curator::detect(&bytes);
    let img = match sillstone_curator::decode_image_rgb8(&bytes, det.format) {
        Ok(i) => i,
        Err(e) => return refuse("image", path, &e),
    };

    let alg = o.alg.as_deref().unwrap_or("phash");
    let hash_val: Result<u64, sillstone_digest::Error> = match alg {
        "phash" => sillstone_image::image_phash(&img),
        "ahash" => sillstone_image::image_ahash(&img),
        "dhash" => sillstone_image::image_dhash(&img),
        "whash" => sillstone_image::image_whash(&img, 1),
        "colorhash" | "color-hash" => sillstone_image::image_colorhash(&img, 4),
        other => {
            eprintln!("pith image: unknown algorithm `{other}`");
            return USAGE;
        }
    };

    match hash_val {
        Ok(h) => {
            let hex_str = format!("{h:#018x}");
            if o.json {
                println!(
                    "{{\"file\": {}, \"alg\": \"{}\", \"hash\": \"{}\"}}",
                    sillstone_cli::json_string(path),
                    alg,
                    hex_str
                );
            } else {
                println!("{hex_str}");
            }
            OK
        }
        Err(e) => {
            eprintln!("pith image: {path}: {e}");
            FAILED
        }
    }
}

// match
// ---------------------------------------------------------------------

const MATCH_HELP: &str = "\
usage: pith match <a> <b> [--json]

Scores two inputs' tier-2 signatures and prints the verdict decided by
the modality's advisory bound.

exit: 0 matched | 1 not matched or refused | 2 usage/io
";

fn cmd_match(args: &[String]) -> u8 {
    let Ok(o) = Opts::parse(args).map_err(|e| {
        eprintln!("pith match: {e}");
    }) else {
        return USAGE;
    };
    if o.pos.first().is_some_and(|s| s == "help") {
        print!("{MATCH_HELP}");
        return OK;
    }
    let (Some(pa), Some(pb)) = (o.pos.first(), o.pos.get(1)) else {
        eprint!("{MATCH_HELP}");
        return USAGE;
    };
    let (Ok(a), Ok(b)) = (must_read(pa), must_read(pb)) else {
        return USAGE;
    };
    let (Ok(sa), Ok(sb)) = (
        sillstone_curator::signature(&a).map_err(|e| refuse("match", pa, &e)),
        sillstone_curator::signature(&b).map_err(|e| refuse("match", pb, &e)),
    ) else {
        return FAILED;
    };
    match sillstone_curator::match_(&sa, &sb) {
        Ok(outcome) => {
            let matched = sillstone_cli::is_match(&outcome);
            let dist = sillstone_cli::distance(&sa, &sb).unwrap_or(u32::MAX);
            if o.json {
                println!(
                    "{{\"a\": {}, \"b\": {}, \"modality\": \"{}\", \"score\": {}, \"distance\": {}, \"bound\": {}, \"matched\": {}}}",
                    sillstone_cli::json_string(pa),
                    sillstone_cli::json_string(pb),
                    sa.modality().as_str(),
                    sillstone_cli::json_string(
                        sillstone_cli::score_line(&outcome).trim_start_matches("score: ")
                    ),
                    dist,
                    sillstone_cli::json_string(
                        sillstone_cli::bound_line(&outcome).trim_start_matches("advisory: ")
                    ),
                    matched,
                );
            } else {
                println!("modality: {}", sa.modality());
                println!("{}", sillstone_cli::score_line(&outcome));
                println!("distance: {dist}");
                println!("verdict: {}", if matched { "match" } else { "no-match" });
                println!("{}", sillstone_cli::bound_line(&outcome));
            }
            if matched { OK } else { FAILED }
        }
        Err(e) => refuse("match", pa, &e),
    }
}

// ---------------------------------------------------------------------
// dedup
// ---------------------------------------------------------------------

const DEDUP_HELP: &str = "\
usage: pith dedup <dir> [--all] [--threshold N] [--json]

Clusters the regular files in <dir> (top level, sorted by name) by
tier-2 signature. Groups of >= 2 print as `group N (exact)` — `exact`
means every member shares one tier-1 digest. `--threshold N` replaces
the advisory verdict with `distance <= N` on the suite's scalar axis.
`--all` also prints the singletons. Files the pipeline refuses are
listed as skipped, never silently dropped.

exit: 0 ok (groups found or not) | 2 usage/io
";

fn cmd_dedup(args: &[String]) -> u8 {
    let Ok(o) = Opts::parse(args).map_err(|e| {
        eprintln!("pith dedup: {e}");
    }) else {
        return USAGE;
    };
    if o.pos.first().is_some_and(|s| s == "help") {
        print!("{DEDUP_HELP}");
        return OK;
    }
    let Some(dir) = o.pos.first() else {
        eprint!("{DEDUP_HELP}");
        return USAGE;
    };
    let rd = match std::fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(e) => {
            eprintln!("pith dedup: {dir}: {e}");
            return USAGE;
        }
    };
    let mut paths: Vec<PathBuf> = Vec::new();
    for entry in rd {
        match entry {
            Ok(en) if en.file_type().is_ok_and(|t| t.is_file()) => paths.push(en.path()),
            _ => {}
        }
    }
    paths.sort();
    let mut inputs: Vec<Input> = Vec::with_capacity(paths.len());
    for p in &paths {
        match std::fs::read(p) {
            Ok(bytes) => inputs.push(Input {
                path: p.clone(),
                bytes,
            }),
            Err(e) => {
                eprintln!("pith dedup: {}: {e}", p.display());
            }
        }
    }
    let report = dedup_inputs(&inputs, o.threshold);
    print_dedup(&report, &inputs, o.all, o.json);
    OK
}

fn print_dedup(report: &DedupReport, inputs: &[Input], all: bool, json: bool) {
    if json {
        let mut s = String::with_capacity(256 * (report.groups.len() + 1));
        s.push_str("{\"groups\": [");
        for (gi, g) in report.groups.iter().enumerate() {
            if gi > 0 {
                s.push_str(", ");
            }
            s.push_str("{\"members\": [");
            for (mi, &m) in g.members.iter().enumerate() {
                if mi > 0 {
                    s.push_str(", ");
                }
                s.push_str(&sillstone_cli::json_string(
                    &inputs[m].path.display().to_string(),
                ));
            }
            s.push_str(&format!("], \"exact\": {}}}", g.exact));
        }
        s.push_str("], \"singles\": [");
        for (i, &m) in report.singles.iter().enumerate() {
            if i > 0 {
                s.push_str(", ");
            }
            s.push_str(&sillstone_cli::json_string(
                &inputs[m].path.display().to_string(),
            ));
        }
        s.push_str("], \"skipped\": [");
        for (i, &m) in report.skipped.iter().enumerate() {
            if i > 0 {
                s.push_str(", ");
            }
            s.push_str(&sillstone_cli::json_string(
                &inputs[m].path.display().to_string(),
            ));
        }
        s.push_str("]}");
        println!("{s}");
        return;
    }
    for (gi, g) in report.groups.iter().enumerate() {
        println!("group {}{}:", gi + 1, if g.exact { " (exact)" } else { "" });
        for &m in &g.members {
            println!("  {}", inputs[m].path.display());
        }
    }
    if all {
        for &m in &report.singles {
            println!("single: {}", inputs[m].path.display());
        }
    }
    for &m in &report.skipped {
        eprintln!("skipped: {}", inputs[m].path.display());
    }
    println!(
        "{} group(s), {} file(s) clustered, {} single(s), {} skipped",
        report.groups.len(),
        report.groups.iter().map(|g| g.members.len()).sum::<usize>(),
        report.singles.len(),
        report.skipped.len(),
    );
}

// ---------------------------------------------------------------------
// bench
// ---------------------------------------------------------------------

const BENCH_HELP: &str = "\
usage: pith bench [--iters N] [--json]

Measures tier-2 signature throughput per modality on deterministic
generated fixtures (no corpus download). Numbers are wall-clock —
repeat runs will differ; the inputs never do.

exit: 0 ok | 2 usage
";

/// One bench fixture: modality name, synthesized bytes, iteration cap.
struct BenchCase {
    name: &'static str,
    bytes: Vec<u8>,
}

fn bench_cases() -> Vec<BenchCase> {
    let px = fixture::patterned_pixels(0xBEEF_0001, 512, 384);
    let pcm = fixture::tone(0xBEEF_0002, 1.0);
    vec![
        BenchCase {
            name: "image-bmp",
            bytes: fixture::bmp24(512, 384, &px),
        },
        BenchCase {
            name: "audio-wav",
            bytes: fixture::wav16(44_100, &pcm),
        },
        BenchCase {
            name: "text",
            bytes: fixture::prose(0xBEEF_0003, 256 * 1024).into_bytes(),
        },
        BenchCase {
            name: "binary",
            bytes: fixture::binary_blob(0xBEEF_0004, 4 * 1024 * 1024),
        },
    ]
}

fn cmd_bench(args: &[String]) -> u8 {
    let Ok(o) = Opts::parse(args).map_err(|e| {
        eprintln!("pith bench: {e}");
    }) else {
        return USAGE;
    };
    if o.pos.first().is_some_and(|s| s == "help") {
        print!("{BENCH_HELP}");
        return OK;
    }
    let iters = o.iters.unwrap_or(5).max(1);
    let mut rows: Vec<String> = Vec::new();
    for case in bench_cases() {
        let len = case.bytes.len();
        // Warm-up pass also validates the fixture: a refused input is a
        // bench bug, not a data point.
        if let Err(e) = sillstone_curator::signature(&case.bytes) {
            eprintln!("pith bench: {} fixture refused: {e}", case.name);
            return FAILED;
        }
        let start = Instant::now();
        let mut sink: u64 = 0;
        for _ in 0..iters {
            match sillstone_curator::signature(&case.bytes) {
                Ok(Signature::Image(h)) => sink ^= h,
                Ok(Signature::Audio(s)) => sink ^= s.peaks().len() as u64,
                Ok(Signature::Text(t)) => sink ^= t.first().copied().unwrap_or(0),
                Ok(Signature::Binary(b)) => sink ^= b.len() as u64,
                Ok(Signature::Video(v)) => sink ^= v.frame_hashes.first().copied().unwrap_or(0),
                Err(_) => unreachable!("warm-up proved the fixture signs"),
            }
        }
        let secs = start.elapsed().as_secs_f64();
        std::hint::black_box(sink);
        let total = (len * iters as usize) as f64;
        let mbs = total / secs / 1e6;
        rows.push(format!(
            "{{\"name\": \"{}\", \"input_bytes\": {len}, \"iters\": {iters}, \"seconds\": {secs:.6}, \"mb_per_s\": {mbs:.2}}}",
            case.name,
        ));
        if !o.json {
            println!(
                "{:<10} {:>9} B x{:>3} iters  {:>7.3} s  {:>9.2} MB/s",
                case.name, len, iters, secs, mbs
            );
        }
    }
    if o.json {
        println!(
            "{{\"bench\": [{}], \"release\": {}}}",
            rows.join(", "),
            cfg!(not(debug_assertions)),
        );
    } else {
        println!(
            "# wall-clock tier-2 signature throughput; release={} arch={}",
            cfg!(not(debug_assertions)),
            std::env::consts::ARCH,
        );
    }
    OK
}

// ---------------------------------------------------------------------
// calibrate
// ---------------------------------------------------------------------

const CALIBRATE_HELP: &str = "\
usage: pith calibrate <pairs.csv> --profile dedup|search [--json]

pairs.csv: one labelled pair per line, `#` comments allowed:
    <path-a>,<path-b>,<label>       label in 1|0|true|false

Both paths must carry the same modality. The scalar distance is the
suite's per-modality axis (image hamming, audio u32::MAX-votes,
text/binary (1-jaccard)*1e6) and the receipt prints that axis so the
threshold is self-describing.

profiles: dedup = prefer false negatives (F0.5), search = prefer false
positives (F2). Measured on THIS dataset only — ship it with the pair
count.

exit: 0 ok | 1 pipeline refusal | 2 usage/io
";

/// Parses a `label` cell: 1/true = positive, 0/false = negative.
fn parse_label(s: &str) -> Option<bool> {
    match s.trim() {
        "1" | "true" | "yes" => Some(true),
        "0" | "false" | "no" => Some(false),
        _ => None,
    }
}

fn cmd_calibrate(args: &[String]) -> u8 {
    let Ok(o) = Opts::parse(args).map_err(|e| {
        eprintln!("pith calibrate: {e}");
    }) else {
        return USAGE;
    };
    if o.pos.first().is_some_and(|s| s == "help") {
        print!("{CALIBRATE_HELP}");
        return OK;
    }
    let (Some(file), Some(profile)) = (o.pos.first(), o.profile.as_deref()) else {
        eprint!("{CALIBRATE_HELP}");
        return USAGE;
    };
    let profile = match profile {
        "dedup" => sillstone_curator::Profile::Dedup,
        "search" => sillstone_curator::Profile::Search,
        other => {
            eprintln!("pith calibrate: unknown --profile `{other}` (dedup|search)");
            return USAGE;
        }
    };
    let Ok(text) = std::fs::read_to_string(file) else {
        eprintln!("pith calibrate: {file}: unreadable");
        return USAGE;
    };
    let mut seen_modality: Option<sillstone_curator::Modality> = None;
    let mut pairs: Vec<(u32, bool)> = Vec::new();
    let mut counted = (0usize, 0usize); // positives, negatives
    for (ln, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut cells = line.split(',');
        let (Some(pa), Some(pb), Some(lab)) = (cells.next(), cells.next(), cells.next()) else {
            eprintln!("pith calibrate: {file}:{}: expected 3 cells", ln + 1);
            return USAGE;
        };
        let Some(label) = parse_label(lab) else {
            eprintln!("pith calibrate: {file}:{}: bad label `{lab}`", ln + 1);
            return USAGE;
        };
        let (Ok(a), Ok(b)) = (must_read(pa.trim()), must_read(pb.trim())) else {
            return USAGE;
        };
        let sa = match sillstone_curator::signature(&a) {
            Ok(s) => s,
            Err(e) => return refuse("calibrate", pa.trim(), &e),
        };
        let sb = match sillstone_curator::signature(&b) {
            Ok(s) => s,
            Err(e) => return refuse("calibrate", pb.trim(), &e),
        };
        if sa.modality() != sb.modality() {
            eprintln!(
                "pith calibrate: {file}:{}: modality mismatch ({} vs {})",
                ln + 1,
                sa.modality(),
                sb.modality()
            );
            return USAGE;
        }
        // One threshold per axis: every pair in the set must share one
        // modality, or "threshold" would compare hamming to votes.
        match seen_modality {
            None => seen_modality = Some(sa.modality()),
            Some(m) if m != sa.modality() => {
                eprintln!(
                    "pith calibrate: {file}:{}: pair is {} but the set is {m} — one modality per dataset",
                    ln + 1,
                    sa.modality()
                );
                return USAGE;
            }
            _ => {}
        }
        let d = match sillstone_cli::distance(&sa, &sb) {
            Ok(d) => d,
            Err(e) => return refuse("calibrate", pa.trim(), &e),
        };
        pairs.push((d, label));
        if label {
            counted.0 += 1;
        } else {
            counted.1 += 1;
        }
    }
    match sillstone_curator::calibrate(&pairs, profile) {
        Ok(cal) => {
            let thr = cal
                .threshold
                .map_or_else(|| "none".to_string(), |t| t.to_string());
            if o.json {
                println!(
                    "{{\"dataset\": {}, \"pairs\": {}, \"positive\": {}, \"negative\": {}, \"profile\": \"{:?}\", \"threshold\": {}, \"score\": {:.6}, \"precision\": {:.6}, \"recall\": {:.6}, \"tp\": {}, \"fp\": {}, \"fn\": {}, \"tn\": {}}}",
                    sillstone_cli::json_string(file),
                    pairs.len(),
                    counted.0,
                    counted.1,
                    profile,
                    cal.threshold
                        .map_or_else(|| "null".to_string(), |t| t.to_string()),
                    cal.score,
                    cal.precision(),
                    cal.recall(),
                    cal.true_positives,
                    cal.false_positives,
                    cal.false_negatives,
                    cal.true_negatives,
                );
            } else {
                println!(
                    "dataset: {file} ({} pairs: {} positive, {} negative)",
                    pairs.len(),
                    counted.0,
                    counted.1
                );
                println!("profile: {profile:?}");
                println!("threshold: {thr}   # match iff distance <= threshold");
                println!("score (f-beta): {:.6}", cal.score);
                println!("precision: {:.6}", cal.precision());
                println!("recall: {:.6}", cal.recall());
                println!(
                    "counts: tp={} fp={} fn={} tn={}",
                    cal.true_positives,
                    cal.false_positives,
                    cal.false_negatives,
                    cal.true_negatives
                );
                println!(
                    "# receipt: measured on {file} with pith {}",
                    env!("CARGO_PKG_VERSION")
                );
            }
            OK
        }
        Err(e) => {
            eprintln!("pith calibrate: {file}: {e}");
            FAILED
        }
    }
}

// ---------------------------------------------------------------------
// Usage
// ---------------------------------------------------------------------

const USAGE_TEXT: &str = "\
pith - zero-dependency perceptual and content hashing suite

usage: pith <command> [args] [--json]

commands:
  describe <file>     print all three tiers for one input
  hash <file>         print the tier-1 canonical hash (SHA-256)
  match <a> <b>       score two tier-2 signatures, print the verdict
  dedup <dir>         cluster a directory by tier-2 signature
  bench               tier-2 throughput per modality (wall-clock)
  calibrate <csv> --profile dedup|search
                      fit a match threshold on labelled pairs

options:
  --json        machine-readable output where defined
  --help        per-command usage (pith <command> --help)
  --all         dedup: also list singleton files
  --threshold   dedup: scalar distance bound instead of the advisory verdict
  --profile     calibrate: dedup (F0.5) or search (F2)
  --iters       bench: repetitions per fixture (default 5)

exit codes:
  0  answer produced (describe, hash, dedup, bench, calibrate;
     match: the pair matched)
  1  the pipeline refused (unsupported slot, corrupt container,
     unreadable semantics) or match answered no-match
  2  the invocation was wrong or a path could not be read

video note: mp4/h264 lanes are named but have not landed — describe
reports `pending: true` and the hashing commands exit 1 with the lane
named.
";
