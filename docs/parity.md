# Sillstone Parity & Conformance Matrix

<!-- Generated and verified against reference.json and multi-language test suites. -->

This document establishes the feature, algorithm, and test parity across Rust core, the CLI tool, and all four foreign SDKs (Python, Node.js, Go, Java).

## 1. Multi-Language SDK Parity Matrix

Every feature is implemented with bit-identical outputs and zero third-party runtime dependencies across all supported runtimes.

| Category / Feature | Rust Core | CLI (`sillstone`) | Python SDK | Node.js SDK | Go SDK | Java SDK |
|---|---|---|---|---|---|---|
| **Version & Info** | `version()` | `sillstone version` | `version()` | `version()` | `SillstoneVersion*()` | `Generated.sillstone_version_*()` |
| **FNV-1a 64-bit** | `buffer_checksum` | `sillstone checksum` | `buffer_checksum` | `bufferChecksum` | `SillstoneBufferChecksum` | `sillstone_buffer_checksum` |
| **SHA-256** | `digest::sha256` | `digest --alg sha256` | `sha256` | `sha256` | `SillstoneDigestSha256` | `sillstone_digest_sha256` |
| **SHA3-256 / SHA3-512**| `digest::sha3_*` | `digest --alg sha3-*` | `sha3_256`, `sha3_512` | `sha3_256`, `sha3_512`| `SillstoneDigestSha3*` | `sillstone_digest_sha3_*` |
| **BLAKE2b (512)** | `digest::blake2b` | `digest --alg blake2b` | `blake2b_512` | `blake2b512` | `SillstoneDigestBlake2b512`| `sillstone_digest_blake2b_512`|
| **BLAKE3** | `digest::blake3` | `digest --alg blake3` | `blake3` | `blake3` | `SillstoneDigestBlake3`| `sillstone_digest_blake3` |
| **Discrete Sine (DST)** | `math::dst1..3` | - | `dst1`, `dst2`, `dst3`| `dst1`, `dst2`, `dst3` | `SillstoneMathDst*` | `sillstone_math_dst*` |
| **Unicode Graphemes** | `unicode::graphemes`| - | `grapheme_count` | `graphemeCount` | `SillstoneUnicodeGraphemeCount`| `sillstone_unicode_grapheme_count` |
| **Unicode Category** | `unicode::category` | - | `char_category` | `charCategory` | `SillstoneUnicodeCharCategory` | `sillstone_unicode_char_category` |
| **Format Detection** | `curator::detect` | `sillstone describe` | `detect` | `detect` | `Detect` | `SillstoneHash.detect` |
| **Content Hash (Tier 1)**| `curator::content_hash`| `sillstone hash` | `content_hash` | `contentHash` | `ContentHash` | `SillstoneHash.contentHash` |
| **Tier-2 Signature** | `curator::signature` | `sillstone hash --tier 2`| `signature` | `signature` | `Signature` | `SillstoneHash.signature` |
| **Canonical Describe** | `curator::describe` | `sillstone describe` | `describe` | `describe` | `Describe` | `SillstoneHash.describe` |
| **Signature Match** | `curator::match_` | `sillstone match` | `match_signatures` | `matchSignatures` | `MatchSignatures` | `SillstoneHash.match` |
| **Image pHash / aHash** | `image::*hash` | `image --alg *` | `image_phash`, `ahash` | `imagePhash`, `ahash` | `SillstoneImage*hash` | `SillstoneHash.*` |
| **Image dHash / wHash** | `image::*hash` | `image --alg *` | `image_dhash`, `whash` | `imageDhash`, `whash` | `SillstoneImage*hash` | `SillstoneHash.*` |
| **Image ColorHash** | `image::colorhash`| `image --alg colorhash`| `image_colorhash` | `imageColorhash` | `SillstoneImageColorhash` | `SillstoneHash.*` |
| **Image ops** (resize, crop, transpose, rotate, filter, brightness, invert, convert) | `image::*` | - | `Image.*` | `Image.*` | `Image.Resize` ... `Image.Convert` | `Image.*` |
| **Image codecs** (QOI, BMP) | `image::qoi`, `image::bmp` | - | `qoi_encode`, `bmp_encode` | `qoiEncode`, `bmpEncode` | `QoiEncode`, `BmpEncode` | `Image.qoiEncode`, `bmpEncode` |
| **N-d array** (matmul, add, sum) | `math::tensor` | - | `Tensor` | `Tensor` | `Tensor` | `Tensor` |
| **Text TLSH & Distance**| `text::tlsh` | `text --alg tlsh` | `tlsh`, `tlsh_distance` | `textTlsh`, `textTlshDistance` | `SillstoneTextTlsh*` | `SillstoneHash.*` |

---

## 2. Reference Vector Conformance

All 45 reference vectors defined in `reference.json` are executed across all four language bindings with identical expectations:

| Test Group | Vector Count | Checked Properties | Pass Status Across All SDKs |
|---|---|---|---|
| **Container Detection** | 13 vectors | Format code, Modality code, Pending flag | 100% (13/13 Pass in Py, Node, Go, Java) |
| **Tier-2 Signature Folds**| 14 vectors | Hex-exact sub-digests, peak counts, word/chunk counts | 100% (14/14 Pass in Py, Node, Go, Java) |
| **Canonical Describe** | 7 vectors | 32-byte Tier-1 hex, signature stream, modality facts | 100% (7/7 Pass in Py, Node, Go, Java) |
| **Pairwise Matching** | 9 vectors | Match tag, advisory verdict boolean, 4 raw slot words | 100% (9/9 Pass in Py, Node, Go, Java) |
| **Refusal & Error Vectors**| 4 vectors | Non-AVC MP4 refusal (-2), Corrupt PNG refusal (-2), Malformed PDF refusal (-2), Cross-modality match (-2) | 100% (4/4 Pass in Py, Node, Go, Java) |
| **Literal Pinned Digest**| 1 vector | Byte-exact SHA-256 content digest of PDF text page | 100% (1/1 Pass in Py, Node, Go, Java) |

---

## 3. Specifications & Standards Conformance

| Domain | Standard / Specification | Conformance Level | Verification Method |
|---|---|---|---|
| **Cryptographic Digest** | FIPS 180-4 (SHA-256) | Full bit-exact | NIST CAVP vectors |
| **Cryptographic Digest** | FIPS 202 (SHA3-256, SHA3-512) | Full bit-exact | NIST CAVP short/long message KAT vectors |
| **Cryptographic Digest** | RFC 7693 (BLAKE2b) | Full bit-exact | Official BLAKE2 KAT vector suite (`blake2-kat.json`) |
| **Cryptographic Digest** | BLAKE3 Specification (2020) | Full bit-exact | Official BLAKE3 test vectors (`test_vectors.json`) |
| **Unicode Graphemes** | Unicode Standard 15.1.0 UAX #29 | Full bit-exact | Official `GraphemeBreakTest.txt` test suite |
| **Decompression** | RFC 1951 (Deflate) | Full decompression | Round-trip tests with gzip/deflate bitstreams |
| **Decompression** | RFC 7932 (Brotli) | Full decompression | Official Brotli test vectors (`testdata`) |
| **Decompression** | RFC 8878 (Zstandard) | Full decompression | Official Zstandard test vectors (`golden-decompression`) |
| **Locality Sensitive Hash**| TrendMicro TLSH (2013) | Full bit-exact | Official TrendMicro TLSH test suite vectors |
| **Container Standards** | POSIX.1-1988 (USTAR), POSIX.1-2001 (PAX) | Full compliance | Path traversal protection, long names |
| **Archive Standards** | ZIP Format Specification / ZIP64 | Full compliance | Zip64 extra field 0x0001, EOCD64 |

---

## 4. Image Hash Parity With `imagehash` / Pillow

`sillstone_image_{a,d,p,w,color}hash` (and every SDK facade over them) reproduce `imagehash` 4.3.2 on Pillow 13.0.0 bit for bit:

- `Image.convert("L")` is the fixed-point luma `(19595 R + 38470 G + 7471 B + 0x8000) >> 16`; `convert("HSV")` mirrors `rgb2hsv_row`.
- `Image.resize(.., LANCZOS)` is Pillow's separable convolution: `f64` coefficients per output pixel, `PRECISION_BITS = 22` fixed point, an 8-bit intermediate between the two passes, and the pass order of `ImagingResampleInner` (vertical first when the height shrinks by more than twice the width). Bilinear, bicubic and nearest (affine, accumulated centre) are exact too; box stays an integer box average.
- `whash` replays `pywt`'s Haar `wavedec2`/`waverec2` operation for operation in `f64` (verified bitwise against `pywt`), so ties at the median resolve as in the reference.
- Parameters: `whash` `level` is `log2(image_scale)` (`0` = imagehash default, else `3..=15`); `colorhash` `bins` is `binbits` (`0` = default `3`, else `1..=4`). Bit order is row-major, MSB first.

Measured against a live `imagehash` (`sdk/python/tests/test_imagehash_parity.py`, plus a 500-image / 2500-hash sweep over noise, gradient, block, flat and smooth content in Gray/RGB/RGBA, sizes from 1 px to 700 px): `ahash`, `dhash`, `whash`, `colorhash` Hamming 0 in all cases; resize identical to Pillow in 1600/1600 random cases.

**Limit — `phash` on exactly rank-deficient content.** `imagehash.phash` thresholds the 8x8 low-frequency DCT block at its median. When the true coefficients are exactly zero (flat image, rows or columns that repeat after the 32x32 resize, perfectly symmetric block patterns) the bits come from `scipy.fftpack.dct` rounding noise. Our DCT sums in a fixed order and cannot reproduce another FFT's noise. In the sweep, 167 of 500 `phash` values differed, all from such content (images narrower or shorter than 32 px, which are upscaled from a single repeated row/column, and synthetic 16-px block images); every `phash` of noise, smooth or gradient content of at least 32x32 px matched. RGBA is filtered per channel without premultiplying alpha, as `convert("L")` drops it first.

**Oracle caveat — `whash` on PyWavelets arm64 wheels.** `imagehash.whash` runs `pywt.wavedec2`, whose Haar taps are `c * x1 + c * x0`. Our core rounds the product and the sum separately (the result on every x86-64 PyWavelets build and on PyWavelets 1.10). The PyWavelets 1.9.0 macOS arm64 wheel is compiled with floating-point contraction, so the compiler fuses the two taps into one `fma` and a handful of ties at the median fall on the other side (found on the CI macOS runner: "L 8x8" case; the same image hashes identically on x86-64, Linux and Windows runners, and with PyWavelets 1.10 on macOS). The 1.10 release needs Python 3.12, so CI's Python 3.11 job cannot pin it. `sdk/python/tests/test_imagehash_parity.py::_pywt_haar_is_unfused` probes the installed build and the `whash` oracle assertions run only where the reference rounds the taps separately; the Rust hard-coded `whash` vectors and the x86-64 oracle runs stay exact. `scripts/bench/probe_pywt_fma.py` prints the evidence.

---

## 5. Speed Versus `hashlib` / `imagehash`

Measured with `.github/workflows/bench.yml` (workflow dispatch, 5 repeats per size on `ubuntu-latest`, `windows-latest`, `macos-latest`; the harness `scripts/bench/bench_hash_parity.py` checks byte parity first, then times the Python SDK against `hashlib` / `imagehash`). Ratio = reference time / sillstone time, so **1.0 is parity and larger is faster**. Figures are medians of the five repeats from run `40216476660` on commit `47bf41a`.

**Image hashes (`imagehash` / sillstone) — faster on every OS, every size, every repeat** (minimum ratio over all repeats and cells: 1.07).

| OS | 256x256 ahash / dhash / phash / whash / colorhash | 1024x1024 ahash / dhash / phash / whash / colorhash |
|---|---|---|
| ubuntu | 6.1 / 5.1 / 4.3 / 14.2 / 2.2 | 5.3 / 5.2 / 5.1 / 20.6 / 2.0 |
| windows | 4.1 / 3.5 / 2.9 / 16.1 / 2.3 | 1.6 / 1.5 / 1.6 / 10.7 / 2.3 |
| macos | 1.7 / 1.8 / 1.8 / 11.3 / 5.3 | 1.2 / 1.1 / 1.3 / 14.2 / 5.0 |

**Digests (`hashlib` / sillstone) — parity, not a win.** Median ratio for 64 B / 4 KiB / 1 MiB:

| Algorithm | ubuntu | windows | macos |
|---|---|---|---|
| sha256 | 0.49 / 0.88 / 1.00 | 0.78 / 0.99 / 1.00 | 0.74 / 0.89 / 0.95 |
| sha3_256 | 0.67 / 0.94 / 0.98 | 0.87 / 1.03 / 1.02 | 1.18 / 0.88 / 0.82 |
| sha3_512 | 0.67 / 0.92 / 0.93 | 0.86 / 1.03 / 1.02 | 1.06 / 0.91 / 0.90 |
| blake2b_512 | 0.36 / 0.89 / 0.98 | 0.49 / 0.65 / 0.66 | 0.47 / 0.90 / 0.93 |

The `speed_ratio >= 1.0` bar for sha256 / sha3 / blake2b at 4 KiB and above is **met only in part** (sha3 on Windows, sha256 on Windows/ubuntu at 1 MiB). Causes, measured:

1. **Call overhead.** A `ctypes` call costs about 1.1-1.5 us against about 0.5 us for `hashlib`'s C method, which is the whole gap at 64 B and 25-35 % of a 4 KiB hash. The Python entry points already bypass argument conversion (`_bare`, bound handles), taking a 64 B sha256 from 1.52 us to about 1.19 us; the remainder is the `ctypes` foreign-call path itself.
2. **Kernel throughput at large sizes.** `hashlib` links OpenSSL assembly. sha256 uses SHA-NI / ARM SHA2, sha3 uses AVX2 Keccak (x86-64, BMI and `rorx` build) and ARMv8.2 SHA-3 (`eor3`/`rax1`/`xar`/`bcax`, runtime detected), blake2b uses an AVX2 kernel. These reach 0.93-1.02 of OpenSSL on sha256/sha3. The AVX2 BLAKE2b kernel gathers message words with scalar loads and lands at 650-880 MB/s; `hashlib` (HACL\* / libb2) lands at 735-985 MB/s depending on the host.
3. **Runner hardware varies between runs.** The `ubuntu-latest` pool alternates AMD EPYC 9V74 and 9V45, `windows-latest` and `macos-latest` vary similarly; blake2b on Windows measured 1.25 in one run and 0.66 in another with identical code, so a single ratio below about 10 % of 1.0 is inside the noise of the platform. The raw per-repeat JSON is uploaded as the `bench-*` artifact of every run.

Not done: a vectorised BLAKE2b message loader (the reference AVX2 permute/blend scheme) is the next step that could lift blake2b past `hashlib` on every runner.
