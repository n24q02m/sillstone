# Sillstone Crate Scorecard

<!-- Generated from workspace manifests, test suites, and reference.json -->

| Crate | Layer | Zero Deps | Test Vectors | External Oracle / Standard KAT | SDK Parity | Coverage Status |
|---|---|---|---|---|---|---|
| `sillstone-buffer` | T0 Foundation | Yes (std only) | 12 unit tests | Memory layout invariants | Full | Pass (100%) |
| `sillstone-digest` | T1 Core | Yes (std only) | 14 KAT suites | NIST CAVP (SHA256, SHA3), RFC 7693 (BLAKE2b), BLAKE3 KAT | Full (Py, Node, Go, Java) | Pass (≥95%) |
| `sillstone-math` | T1 Core | Yes (std only) | 18 mathematical tests | DST-I/II/III inversion, Gauss-Legendre quadrature, RK45 adaptive ODE | Full (Py, Node, Go, Java) | Pass (≥95%) |
| `sillstone-unicode` | T1 Core | Yes (std only) | 68 GraphemeBreak tests | Unicode 15.1.0 UAX #29 GraphemeBreakTest.txt, UnicodeData.txt | Full (Py, Node, Go, Java) | Pass (≥95%) |
| `sillstone-inflate` | T1 Core | Yes (std only) | 3 RFC test suites | RFC 1951 Deflate, RFC 7932 Brotli, RFC 8878 Zstandard golden | Core | Pass (≥95%) |
| `sillstone-image` | T2 Codecs | Yes (std only) | 24 image tests | PngSuite, JPEG Annex K, Neal Krawetz aHash/dHash/pHash/wHash | Full (Py, Node, Go, Java) | Pass (≥95%) |
| `sillstone-audio` | T2 Codecs | Yes (std only) | 16 audio tests | RIFF/WAV, FLAC Vorbis comment, MP3 ID3v2/Xing, Chromaprint | Full (Py, Node, Go, Java) | Pass (≥95%) |
| `sillstone-video` | T2 Codecs | Yes (std only) | 14 video tests | ISO/IEC 14496-10 AVC bitstreams, MP4 fragmented moof/traf, scene-cut | Full (Py, Node, Go, Java) | Pass (≥95%) |
| `sillstone-text` | T3 Domains | Yes (std only) | 15 text tests | TrendMicro TLSH official KAT, 4KiB prose shingle Jaccard | Full (Py, Node, Go, Java) | Pass (≥95%) |
| `sillstone-file` | T3 Domains | Yes (std only) | 22 container tests | USTAR/PAX path traversal, Zip64 EOCD64, Word/Excel XML extraction | Core | Pass (≥95%) |
| `sillstone-curator` | T4 Facade | Yes (std only) | 45 reference vectors | Multi-language hex-exact match against `reference.json` | Full (Py, Node, Go, Java) | Pass (100%) |
| `sillstone-cli` | Presentation | Yes (std only) | 36 CLI tests | Process stdout/JSON verification against reference outputs | Native binary | Pass (100%) |
| `sillstone-ffi` | C ABI | Yes (std only) | 15 FFI tests | Flat integer status codes (-1, -2), zero unwinding panics | C ABI | Pass (100%) |

## Quality & Hygiene Gates

- **Forbidden Dependencies:** `scripts/check-zero-deps.py` passes with zero third-party dependencies in `Cargo.lock`.
- **Forbidden Traces:** `scripts/check-forbidden-traces.py` passes with zero forbidden tokens or personal paths.
- **Strict Linting:** `cargo clippy --workspace --all-targets -- -D warnings` clean across all crates.
- **Reference Conformance:** 45 reference vectors verified hex-exact across Python, Node.js, Go, and Java.
