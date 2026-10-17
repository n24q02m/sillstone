# Sillstone Capabilities & Operational Limits

<!-- Generated from crate manifests and reference vectors. Do not hand-edit limits without updating conformance tests. -->

This document specifies the algorithmic guarantees, operational limits, input bounds, and behavior on malformed data for all modules in `sillstone`.

## 1. Cryptographic & Fast Hashing (`sillstone-digest`)

| Algorithm | Wire / Name | Output Length | Chunk / Block Size | Memory Usage | Guarantees & Standards |
|---|---|---|---|---|---|
| **FNV-1a 64** | `fnv1a64` | 8 bytes (64-bit) | Streaming 1 byte | $O(1)$ (8 bytes state) | Pinned fold constant `0xcbf29ce484222325`, prime `0x100000001b3` |
| **xxHash64** | `xxh64` | 8 bytes (64-bit) | 32-byte stripe | $O(1)$ (32 bytes state) | Exact xxHash64 specification, seeded or unseeded |
| **SHA-256** | `sha256` | 32 bytes (256-bit) | 64-byte block | $O(1)$ (64 bytes state) | FIPS 180-4 standard, zero-allocation streaming |
| **SHA3-256** | `sha3-256` | 32 bytes (256-bit) | 136-byte rate (r=1088) | $O(1)$ (200 bytes state) | FIPS 202 Keccak-f[1600], domain pad `0x06...0x80` |
| **SHA3-512** | `sha3-512` | 64 bytes (512-bit) | 72-byte rate (r=576) | $O(1)$ (200 bytes state) | FIPS 202 Keccak-f[1600], domain pad `0x06...0x80` |
| **BLAKE2b** | `blake2b` | 1–64 bytes (default 64) | 128-byte block | $O(1)$ (256 bytes state) | RFC 7693, supports key up to 64 bytes |
| **BLAKE3** | `blake3` | 32 bytes (extensible) | 1024-byte chunk | $O(\log N)$ stack (tree) | Complete Merkle tree, keyed mode, XOF output |

**Operational Limits:**
- Streaming hash digest capacity: Unbounded stream length up to $2^{64}-1$ bytes.
- Key limits: BLAKE2b key length $\le 64$ bytes; BLAKE3 key length exactly 32 bytes.

---

## 2. Mathematical Kernels (`sillstone-math`)

| Kernel | Precision | Input Size ($N$) | Time Complexity | Bounds & Invariants |
|---|---|---|---|---|
| **DST-I** | `f64` | $N \ge 1$ | $O(N^2)$ direct | $X_k = \sum_{n=0}^{N-1} x_n \sin\left(\frac{\pi (n+1)(k+1)}{N+1}\right)$ |
| **DST-II** | `f64` | $N \ge 1$ | $O(N^2)$ direct | Standard orthonormalized type-II discrete sine transform |
| **DST-III** | `f64` | $N \ge 1$ | $O(N^2)$ direct | Exact inverse of DST-II up to scaling factor $(N/2)$ |
| **Trapezoid** | `f64` | $n \ge 1$ steps | $O(n)$ evaluations | Second-order numerical quadrature |
| **Simpson** | `f64` | $n \ge 2$ even steps | $O(n)$ evaluations | Fourth-order numerical quadrature |
| **Gauss–Legendre** | `f64` | $n \in \{1..5\}$ nodes | $O(n)$ evaluations | Degree $\le 2n - 1$ exact polynomial integration |
| **Runge–Kutta 4** | `f64` | $n \ge 1$ steps | $O(n)$ evaluations | Fixed-step classical 4th-order ODE solver |
| **RK45 (Dormand–Prince)** | `f64` | Adaptive steps | Dynamic | Error tolerance controlled step-size adaptation |

**Operational Limits:**
- Infinite/NaN outputs: Rejected with `Error::InvalidInput`.
- $a = b$ integration limits: Returns 0.0 without division by zero.

---

## 3. Unicode Segmentation & Categories (`sillstone-unicode`)

- **Standard:** Unicode Standard Version 15.1.0.
- **UAX #29 Grapheme Cluster Boundaries:** Extended grapheme cluster rule evaluation over UTF-8 streams.
- **Character Categories:** Full lookup matching `UnicodeData.txt` general categories (`Lu`, `Ll`, `Nd`, `Zs`, etc.).
- **Memory Invariant:** Zero heap allocations during grapheme boundary stepping; static lookup tables embedded without third-party crates.

---

## 4. Decompression Engines (`sillstone-inflate`)

| Codec | RFC Specification | Features Supported | Limits & Protection |
|---|---|---|---|
| **Deflate** | RFC 1951 | Fixed & dynamic Huffman, uncompressed blocks | `max_out` bounds check prevents zip bombs |
| **Brotli** | RFC 7932 | Static 122KB dictionary, meta-blocks, context modeling | Enforced `max_out` buffer cap; corrupted stream fails safely |
| **Zstandard** | RFC 8878 | FSE (Finite State Entropy), Huffman, raw/RLE/compressed blocks, XXH64 checksum | Window size capped; corrupted header returns error |

---

## 5. Media Codecs & Formats

| Format | Module | Decoder Capabilities | Encoder Capabilities | Limits |
|---|---|---|---|---|
| **PNG** | `sillstone-image` | Grayscale, RGB, RGBA, Palette, 16-bit, Adam7 interlacing | Baseline RGBA, compression level 0..9 | Max dimension $65535 \times 65535$ |
| **JPEG** | `sillstone-image` | Baseline SOF0, Progressive SOF2, 4:4:4, 4:2:2, 4:2:0 | Baseline 4:4:4 and 4:2:0 Annex K quant | Strict dimension and component checks |
| **BMP** | `sillstone-image` | 24-bit RGB, 32-bit RGBA, bottom-up & top-down | 24-bit RGB uncompressed | Header bounds check |
| **WAV** | `sillstone-audio` | PCM 16-bit, 24-bit, 32-bit float, mono/stereo | 16-bit PCM mono/stereo | RIFF size sanity verification |
| **FLAC** | `sillstone-audio` | Native lossless FLAC, Vorbis comment, seektable | Vorbis comment write | Frame sync header check |
| **MP3** | `sillstone-audio` | MPEG-1/2 Audio Layer III, ID3v1, ID3v2.3/v2.4, Xing header | Header stripping & frame sync | Layer III only (rejects Layer I/II) |
| **MP4 / MOV** | `sillstone-video` | ISO base media (AVC/H.264), fragmented MP4 (`moof`, `traf`, `trun`) | - | Rejects non-AVC bitstreams with status -2 |
| **PDF** | `sillstone-file` | FlateDecode text extraction, font CID/ToUnicode, AcroForm fields | - | Linear stream parser, bounds-checked |
| **ZIP** | `sillstone-file` | Standard ZIP, ZIP64 (EOCD64, extra field 0x0001) | Stored / Deflate writer | Traversal sanitization (`..` stripped) |
| **TAR** | `sillstone-file` | USTAR, PAX extended headers, GNU long-name/link | - | Traversal sanitization, safe extraction |
| **DOCX/XLSX**| `sillstone-file` | Fast XML extraction from Office Open XML containers | - | XML memory limits prevent entity expansion |

---

## 6. Multi-Modal Perceptual Fingerprints (`sillstone-curator`)

- **Image Hashes:**
  - `pHash`: 64-bit DCT-based perceptual hash (resistant to scaling and gamma shifts).
  - `aHash`: 64-bit Average hash (8x8 box average).
  - `dHash`: 64-bit Difference hash (horizontal gradient comparison).
  - `wHash`: 64-bit Discrete Wavelet Transform hash (Haar LL sub-band).
  - `colorhash`: 64-bit Color histogram distribution hash.
- **Text Hashes:**
  - `TLSH`: TrendMicro Locality Sensitive Hash (72-hex character representation, distance 0..1000+).
  - `n-gram Jaccard`: Character-level k-shingle MinHash.
- **Audio Fingerprints:**
  - Spectral landmark peak extraction (time-frequency constellation).
  - Chromaprint-style 32-bit sub-fingerprint vector.
- **Video Fingerprints:**
  - Frame-level pHash chain with shingled MinHash.
  - Scene cut detection via chi-square color histogram divergence.

---

## 7. FFI & Foreign Function Interface Guarantees

- **No Panics Across FFI Boundaries:** Every C ABI export catches unwinding panics and translates errors into flat negative integer status codes (`0` = OK, `-1` = Invalid Argument, `-2` = Core Refusal).
- **Zero Third-Party Dependencies:**
  - Python: `ctypes` standard library binding.
  - Node.js: Native N-API addon compiled directly against `node.exe` / `libnode.a`, zero npm dependencies.
  - Go: Zero external Go modules, `cgo` integration.
  - Java: Direct JNI integration with automatic library resolution (`System.load` with fallback chain).
