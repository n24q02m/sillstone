# Sillstone Crate Scorecard

<!-- Generated from workspace manifests, test suites, and reference.json -->

| Crate | Layer | Zero Deps | Test Vectors | External Oracle / Standard KAT | SDK Parity | Line Coverage (measured) |
|---|---|---|---|---|---|---|
| `sillstone-buffer` | T0 Foundation | Yes (std only) | 12 unit tests | Memory layout invariants | Full | 84.62% (below target) |
| `sillstone-digest` | T1 Core | Yes (std only) | 14 KAT suites | NIST CAVP (SHA256, SHA3), RFC 7693 (BLAKE2b), BLAKE3 KAT | Full (Py, Node, Go, Java) | 98.28% (met target) |
| `sillstone-math` | T1 Core | Yes (std only) | 18 mathematical tests | DST-I/II/III inversion, Gauss-Legendre quadrature, RK45 adaptive ODE | Full (Py, Node, Go, Java) | 98.46% (met target) |
| `sillstone-unicode` | T1 Core | Yes (std only) | 68 GraphemeBreak tests | Unicode 15.1.0 UAX #29 GraphemeBreakTest.txt, UnicodeData.txt | Full (Py, Node, Go, Java) | 97.52% (met target) |
| `sillstone-inflate` | T1 Core | Yes (std only) | 3 RFC test suites | RFC 1951 Deflate (full); RFC 8878 Zstandard (full except dictionaries); RFC 7932 Brotli (full except the large-window extension) | Core | 96.26% (met target) |
| `sillstone-image` | T2 Codecs | Yes (std only) | 24 image tests | PngSuite, JPEG Annex K, Neal Krawetz aHash/dHash/pHash/wHash | Full (Py, Node, Go, Java) | 96.92% (met target) |
| `sillstone-audio` | T2 Codecs | Yes (std only) | 16 audio tests | RIFF/WAV, FLAC Vorbis comment, MP3 ID3v2/Xing, Chromaprint | Full (Py, Node, Go, Java) | 98.30% (met target) |
| `sillstone-video` | T2 Codecs | Yes (std only) | 14 video tests | ISO/IEC 14496-10 AVC bitstreams, MP4 fragmented moof/traf, scene-cut | Full (Py, Node, Go, Java) | 97.71% (met target) |
| `sillstone-text` | T3 Domains | Yes (std only) | 15 text tests | TrendMicro TLSH official KAT, 4KiB prose shingle Jaccard | Full (Py, Node, Go, Java) | 98.80% (met target) |
| `sillstone-file` | T3 Domains | Yes (std only) | 22 container tests | USTAR/PAX path traversal, Zip64 EOCD64, Word/Excel XML extraction | Core | 97.82% (met target) |
| `sillstone-curator` | T4 Facade | Yes (std only) | 45 reference vectors | Multi-language hex-exact match against `reference.json` | Full (Py, Node, Go, Java) | 96.12% (met target) |
| `sillstone-cli` | Presentation | Yes (std only) | 36 CLI tests | Process stdout/JSON verification against reference outputs | Native binary | 90.85% (below target) |
| `sillstone-ffi` | C ABI | Yes (std only) | 15 FFI tests | Flat integer status codes (-1, -2), zero unwinding panics | C ABI | 99.22% (met target) |

## Coverage

The coverage column is the line coverage measured by the `Coverage (measured)` job in `.github/workflows/ci.yml`:
one instrumented build whose profiles come from the Rust workspace tests, the generators, the Java JNI suite and the
Python ctypes suite, merged by `cargo llvm-cov report` and reduced per crate by `scripts/ci/coverage_report.py`.
Both report steps run with `--fail-under 95`, so a run that drops under the target fails CI instead of only printing
the number. The target for every crate remains **95%**; crates under it are marked `below target` and are not
rounded up.
Workspace total at that measurement: Rust 96.89% lines (47071/48580); Python binding 96.15% (1373/1428) — run
[41752440263](https://github.com/n24q02m/sillstone/actions/runs/41752440263) on `8abb9957`, 2026-11-04.
The per-file shortfall list is printed in each CI run summary and in the `coverage-reports` artifact.
Largest remaining file gaps: `crates/image/tools/gen-reference/main.rs` 55.56%, `crates/cdc/src/buzhash.rs` 68.89%,
`crates/math/src/dst.rs` 80.18%, `crates/buffer/src/lib.rs` 84.62%. Crates under target: `sillstone-buffer`
(84.62%) and `sillstone-cli` (90.85%).

### SDK bindings

The same job measures each binding's own code with its native tool, against the same 95% target:

| Binding | Tool | Line coverage (measured) | CI gate (`--fail-under`) | Largest gaps |
|---|---|---|---|---|
| Python (ctypes) | coverage.py | 96.15% (1373/1428), met target | 95 (target) | - |
| Go (cgo) | `go test -coverprofile` (statements) | 70.45% (403/572), below target | 70 (ratchet) | `image.go` 16.39%, `generated.go` 61.24%, `tensor.go` 82.22% |
| Node.js (N-API) | `node --test --experimental-test-coverage` (lcov) | 91.41% (936/1024), below target | 91 (ratchet) | `array.js` 86.65%, `native.js` 89.86%, `index.js` 92.03% |
| Java (JNI) | JaCoCo (`-Pcoverage`) | 78.15% (490/627), below target | 78 (ratchet) | `SillstoneNative` 56.63%, `Tensor` 69.64%, `Generated` 50.00% |

Measured by run [43534010010](https://github.com/n24q02m/sillstone/actions/runs/43534010010) on `26e1b5c`,
2026-11-14. A binding under target keeps a ratchet instead of the target: its gate is the coverage it already has, so
no change can lower it, and the value is raised to the new measurement in the PR that lifts it until it reaches 95.
The way up is tests for the paths the gap lists name: the error returns of the Go image and tensor wrappers and of
the generated entry points, the Node.js array dtype/shape error branches and library-loading fallbacks, and the Java
`SillstoneNative` loader paths and `Tensor` argument checks.

## Quality & Hygiene Gates

- **Forbidden Dependencies:** `scripts/check-zero-deps.py` passes with zero third-party dependencies in `Cargo.lock`.
- **Forbidden Traces:** `scripts/check-forbidden-traces.py` passes with zero forbidden tokens or personal paths.
- **Strict Linting:** `cargo clippy --workspace --all-targets -- -D warnings` clean across all crates.
- **Reference Conformance:** 45 reference vectors verified hex-exact across Python, Node.js, Go, and Java.
