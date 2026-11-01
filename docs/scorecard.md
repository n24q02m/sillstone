# Sillstone Crate Scorecard

<!-- Generated from workspace manifests, test suites, and reference.json -->

| Crate | Layer | Zero Deps | Test Vectors | External Oracle / Standard KAT | SDK Parity | Line Coverage (measured) |
|---|---|---|---|---|---|---|
| `sillstone-buffer` | T0 Foundation | Yes (std only) | 12 unit tests | Memory layout invariants | Full | 84.62% (below target) |
| `sillstone-digest` | T1 Core | Yes (std only) | 14 KAT suites | NIST CAVP (SHA256, SHA3), RFC 7693 (BLAKE2b), BLAKE3 KAT | Full (Py, Node, Go, Java) | 88.33% (below target) |
| `sillstone-math` | T1 Core | Yes (std only) | 18 mathematical tests | DST-I/II/III inversion, Gauss-Legendre quadrature, RK45 adaptive ODE | Full (Py, Node, Go, Java) | 93.37% (below target) |
| `sillstone-unicode` | T1 Core | Yes (std only) | 68 GraphemeBreak tests | Unicode 15.1.0 UAX #29 GraphemeBreakTest.txt, UnicodeData.txt | Full (Py, Node, Go, Java) | 95.39% (met target) |
| `sillstone-inflate` | T1 Core | Yes (std only) | 3 RFC test suites | RFC 1951 Deflate (full); RFC 8878 Zstandard (full except dictionaries); RFC 7932 Brotli framing only (compressed meta-blocks refused) | Core | 85.64% (below target) |
| `sillstone-image` | T2 Codecs | Yes (std only) | 24 image tests | PngSuite, JPEG Annex K, Neal Krawetz aHash/dHash/pHash/wHash | Full (Py, Node, Go, Java) | 79.00% (below target) |
| `sillstone-audio` | T2 Codecs | Yes (std only) | 16 audio tests | RIFF/WAV, FLAC Vorbis comment, MP3 ID3v2/Xing, Chromaprint | Full (Py, Node, Go, Java) | 97.17% (met target) |
| `sillstone-video` | T2 Codecs | Yes (std only) | 14 video tests | ISO/IEC 14496-10 AVC bitstreams, MP4 fragmented moof/traf, scene-cut | Full (Py, Node, Go, Java) | 96.88% (met target) |
| `sillstone-text` | T3 Domains | Yes (std only) | 15 text tests | TrendMicro TLSH official KAT, 4KiB prose shingle Jaccard | Full (Py, Node, Go, Java) | 96.98% (met target) |
| `sillstone-file` | T3 Domains | Yes (std only) | 22 container tests | USTAR/PAX path traversal, Zip64 EOCD64, Word/Excel XML extraction | Core | 94.35% (below target) |
| `sillstone-curator` | T4 Facade | Yes (std only) | 45 reference vectors | Multi-language hex-exact match against `reference.json` | Full (Py, Node, Go, Java) | 92.67% (below target) |
| `sillstone-cli` | Presentation | Yes (std only) | 36 CLI tests | Process stdout/JSON verification against reference outputs | Native binary | 90.85% (below target) |
| `sillstone-ffi` | C ABI | Yes (std only) | 15 FFI tests | Flat integer status codes (-1, -2), zero unwinding panics | C ABI | 6.15% (below target) |

## Coverage

The coverage column is the line coverage measured by the `Coverage (measured)` job in `.github/workflows/ci.yml`
(`cargo llvm-cov --workspace --lib --tests`, reduced per crate by `scripts/ci/coverage_report.py`). The target
for every crate remains **95%**; crates under it are marked `below target` and are not rounded up.
Workspace total at measurement time: Rust 89.58% lines (42123/47022); Python binding 79.42% (903/1137).
The per-file shortfall list is printed in each CI run summary (`coverage-reports` artifact).
Largest gaps: `sillstone-ffi` (`jni.rs` has 0 covered lines in the Rust suite), `sillstone-image` (`ffi.rs`,
`raster/resample.rs`, `qoi_codec.rs`), `sillstone-inflate` (`brotli.rs`), `sillstone-digest`
(`blake3.rs`, `blake2b.rs`).

## Quality & Hygiene Gates

- **Forbidden Dependencies:** `scripts/check-zero-deps.py` passes with zero third-party dependencies in `Cargo.lock`.
- **Forbidden Traces:** `scripts/check-forbidden-traces.py` passes with zero forbidden tokens or personal paths.
- **Strict Linting:** `cargo clippy --workspace --all-targets -- -D warnings` clean across all crates.
- **Reference Conformance:** 45 reference vectors verified hex-exact across Python, Node.js, Go, and Java.
