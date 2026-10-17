# Sillstone Roadmap

This document outlines the milestones, landed capabilities, and forward development trajectory for the `sillstone` monorepo.

## Landed Milestones

### Phase 1: Foundation & Monorepo Consolidation (Done)
- Consolidated 19 standalone crates into a unified workspace under `crates/`.
- Clean Apache-2.0 licensing with separated `NOTICE` file.
- Single unified C ABI (`sillstone-ffi`) and deterministic multi-language IDL generator (`sillstone-idl-gen`).

### Phase 2: Complete Wave 2 Algorithm Suite (Done)
- **Hashing & Digest:** SHA3-256, SHA3-512, BLAKE2b (64-byte keyed), BLAKE3 (Merkle tree & XOF).
- **Mathematical Transforms:** DST-I, DST-II, DST-III, numerical quadratures (Trapezoid, Simpson, Gauss-Legendre), Runge-Kutta ODE solvers (RK4, adaptive RK45).
- **Unicode:** Full UAX #29 extended grapheme clustering, general category and character name lookups matching Unicode 15.1.0.
- **Decompression:** RFC 7932 Brotli (with 122KB embedded static dictionary) and RFC 8878 Zstandard decompression with XXH64 validation.
- **Codecs & Formats:** Complete PNG encoder, JPEG baseline encoder (Annex K), fragmented MP4/MOV parser (`moof`, `traf`, `trun`), FLAC Vorbis comment writer, MP3 ID3v2/Xing parser, Zip64 writer.
- **Perceptual Fingerprints:** Image hashes (aHash, dHash, wHash Haar DWT, colorhash), text locality-sensitive hashing (TrendMicro TLSH bit-exact), audio landmark peaks and fingerprints, video scene-cut detection.
- **File Containers:** TAR (USTAR, PAX, GNU long-name) archive reader with path traversal protection, Office Open XML (DOCX/XLSX) text extraction.

### Phase 3: Curator Facade & CLI Integration (Done)
- Consolidated multi-modal dispatch (`detect`, `signature`, `describe`, `match_`).
- Command-line tool (`sillstone`) with subcommands: `describe`, `hash`, `match`, `digest`, `image`, `text`, `benchmark`.
- Deterministic 45-vector test suite in `reference.json`.

### Phase 4: Multi-Language SDK Conformance (Done)
- **Python:** `ctypes` binding with zero external dependencies, 100% vector parity (53 tests).
- **Node.js:** Native N-API C addon without `node-gyp` or `koffi`, zero npm dependencies (53 tests).
- **Go:** `cgo` integration with zero external Go modules (15 test suites).
- **Java:** `JNI` binding with automatic library resolution and zero third-party dependencies (15 test suites).

---

## Upcoming Milestones

### Phase 5: Production Packaging & Release Ladder
- Distribution publishing to crates.io, PyPI, npm, and Maven Central.
- Automated release workflow with BSR semantic versioning and content integrity verification.

### Phase 6: Pure-Rust Image Processing & Pillow Parity
- High-performance pixel manipulation primitives in `sillstone-image` (resampling filters: Lanczos, Bicubic, Bilinear, Nearest; color space conversions: sRGB, Linear, HSL, HSV, Lab).
- Zero-dependency drop-in replacements for standard Pillow imaging pipelines.

### Phase 7: Tensor & Array Computing (NumPy Parity)
- N-dimensional strided array primitives in `sillstone-buffer` / `sillstone-tensor`.
- Pure-Rust broadcasting, slicing, linear algebra decompositions, and mathematical element-wise kernels.
