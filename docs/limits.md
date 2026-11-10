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
| **Brotli** | RFC 7932 | Full format except the large-window extension: stream header, empty / metadata / uncompressed / compressed meta-blocks (simple and complex prefix codes, block switching, context maps, all four literal context modes, NPOSTFIX / NDIRECT distance codes, last-distance ring), the 122,784-byte static dictionary with all 121 word transforms. A large-window stream returns `Unsupported` | `max_out` buffer cap and the stream's own window rules; corrupted prefix code, context map, distance, length, padding or truncated input returns an error, never guessed bytes |
| **Zstandard** | RFC 8878 | Full format except dictionaries: frames, skippable frames, raw / RLE / compressed blocks, raw / RLE / Huffman literals (1 and 4 streams, treeless reuse), FSE-coded sequences (predefined / RLE / compressed / repeat modes, repeat offsets), XXH64 content checksum. A frame naming a nonzero dictionary id returns `Unsupported` | `max_out` buffer cap and the frame's own window / block-size rules; corrupted header, entropy table, bitstream or checksum returns an error, never guessed bytes |

---

## 5. Media Codecs & Formats

| Format | Module | Decoder Capabilities | Encoder Capabilities | Limits |
|---|---|---|---|---|
| **PNG** | `sillstone-image` | Grayscale, RGB, RGBA, Palette, 16-bit, Adam7 interlacing | Baseline RGBA, compression level 0..9 | Max dimension $65535 \times 65535$ |
| **JPEG** | `sillstone-image` | Baseline SOF0, Progressive SOF2, 4:4:4, 4:2:2, 4:2:0 | Baseline 4:4:4 and 4:2:0 Annex K quant | Strict dimension and component checks |
| **BMP** | `sillstone-image` | 24-bit RGB, 32-bit RGBA, bottom-up & top-down | 24-bit RGB uncompressed; 32-bit RGBA as V4 header with BITFIELDS masks (alpha preserved; checked against Pillow as an independent decoder: 0 of 144 alpha mismatches on a 16x9 image, all 256 alpha values on a 256x1 image, and top-down files) | Header bounds check; FFI geometry checked against buffer length; the Python decoder rejects (`ValueError`) a zero width/height, a pixel offset outside the file or inside the headers, an info header under 40 bytes, a raster above 1 GiB (`MAX_BUFFER_BYTES`) and pixel data shorter than the header claims, all before any pixel buffer is allocated |
| **WAV** | `sillstone-audio` | PCM 16-bit, 24-bit, 32-bit float, mono/stereo | 16-bit PCM mono/stereo | RIFF size sanity verification |
| **FLAC** | `sillstone-audio` | Native lossless FLAC, Vorbis comment, seektable | Vorbis comment write | Frame sync header check |
| **MP3** | `sillstone-audio` | MPEG-1/2 Audio Layer III, ID3v1, ID3v2.3/v2.4, Xing header | Header stripping & frame sync | Layer III only (rejects Layer I/II); ID3v2 frames that are compressed or encrypted are skipped; ID3v1 text is read as UTF-8 when valid, otherwise Latin-1 |
| **MP4 / MOV** | `sillstone-video` | ISO base media (AVC/H.264), fragmented MP4 (`moof`, `traf`, `trun`; offsets relative to the `moof` box or the implicit §8.8.7 base, non-sync flag is bit 16) | - | Rejects non-AVC bitstreams with status -2; fragmented sample ranges must lie inside the file, `trun` sample counts are bounded by the file size, a `moov` that already holds samples plus fragments is refused |
| **PDF** | `sillstone-file` | FlateDecode text extraction, font CID/ToUnicode, AcroForm fields | - | Linear stream parser, bounds-checked |
| **ZIP** | `sillstone-file` | Standard ZIP, ZIP64 (EOCD64, extra field 0x0001) | Stored / Deflate writer | Traversal sanitization (`..` stripped) |
| **TAR** | `sillstone-file` | USTAR, PAX extended headers (`path`, `size`; records parsed by length; a `g` `size` is a default for every later entry and an `x` `size` overrides it for the next entry, as in POSIX.1-2001 and GNU tar; other parsers disagree on an archive whose ustar size fields are 0 and whose sizes come from `g`/`x` records: GNU tar 1.35 lists all five entries, bsdtar 3.8.9 reports `first` with size 0 and then a damaged archive, and Python `tarfile` 3.11 to 3.14 lists only `first` (size 7) and stops, so one archive with a `g` `size` can have different entry boundaries in different parsers and sillstone must not be paired with bsdtar or `tarfile` as two independent checks of the same untrusted archive; a `g` `size` also reaches directories and symlinks, which in a well-formed archive have no data: for a `g` `size` of 7 followed by `dir/`, `lnk` (symlink) and `file`, all with ustar size 0, sillstone gives `dir/` size 7, reads the next block (the `lnk` header) as its data and lists only `dir/` and `file`; GNU tar 1.35 gives `dir/` and `lnk` size 7 and then fails with "Skipping to next header"; bsdtar 3.8.9 ignores the `g` `size` and reports a damaged archive after `file`; Python `tarfile` 3.13 gives all three size 7 but skips no data for `dir/` and `lnk`, so it lists all three; a `g` `path` applies to the next entry only), GNU long-name (`L`) and long-link (`K`, skipped) | - | Traversal sanitization (`..`, absolute paths, drive letters), ustar prefix honored only for POSIX headers, base-256 sizes checked (negative or overflowing values refused) |
| **DOCX/XLSX**| `sillstone-file` | Fast XML extraction from Office Open XML containers | - | XML memory limits prevent entity expansion |

---

## 6. Multi-Modal Perceptual Fingerprints (`sillstone-curator`)

- **Image Hashes:**
  - `pHash`, `aHash`, `dHash`, `wHash`, `colorhash`: bit-exact with the Python `imagehash` library (default parameters, `hash_size = 8`, `binbits = 3`) on 8-bit Gray / RGB / RGBA input; see `docs/parity.md` §4 for the measured parity and its one documented limit. 16-bit samples are reduced with `>> 8` first.
  - Tier-2 curator signatures keep their own pinned pHash lineage (`image_phash`, box-average, DC excluded); it is not the `imagehash` one.
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
- **Image / array facades (Node.js, Java, Go):** 8-bit `L`, `RGB`, `RGBA` only; the same operation set as Python (resize, crop, transpose, rotate, filter, brightness, invert, convert, QOI/BMP encode, QOI decode, `Tensor` matmul/add/sum), checked against shared golden vectors (`tests/fixtures/facade/golden.json`, produced by the Python SDK through the same core).
  - `whash` and `colorhash` use the imagehash defaults (`level` 0, `binbits` 3) in the facades; the ABI accepts other values.
  - The Java hash JNI entry points return `-1` on error, which is also a valid all-ones hash; the facade cannot tell them apart. JNI does not carry status codes, so the Java facade reports `-1`/`-2` approximately from argument checks.
  - `sillstone_tensor_matmul_f64` multiplies straight into the caller's `out` buffer: it allocates nothing, so no payload bytes cross the boundary twice.
  - QOI decode refuses a header whose pixel count exceeds what the stream can encode (62 pixels per opcode byte) before it reserves the output buffer, so a 22-byte file cannot request gigabytes. QOI encode returns an error, not a panic, when `width * height * channels` overflows.
  - Maven Central publication is prepared but not enabled. `sdk/java/pom.xml` has a `release` profile (sources, javadoc, GPG signature, Central Portal publishing, coordinates `io.github.n24q02m:sillstone`) and `cd.yml` deploys through it, but the namespace must be verified in the Central Portal by the account owner, and the `CENTRAL_PORTAL_USERNAME`, `CENTRAL_PORTAL_PASSWORD`, `MAVEN_GPG_PRIVATE_KEY` and `MAVEN_GPG_PASSPHRASE` secrets are not configured. Until then a beta records an explicit SKIPPED notice and a stable release fails; the Java artifact is built and tested but not published.

