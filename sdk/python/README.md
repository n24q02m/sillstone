# Sillstone

Pure-Rust, zero-dependency multimedia, hashing, and array computing suite.

## Overview

Sillstone provides verified, zero-dependency implementations of hashing, codecs, audio/image processing, and tensor abstractions with bindings for Python, Node.js, Go, and Java.

- **Zero third-party dependencies:** The entire Rust core depends strictly on `std`.
- **Single ABI definition:** Cross-language interfaces are deterministically generated from machine-readable IDL.
- **Formally verified & cross-checked:** Implementations are validated against CAVP, RFC, and standard test vectors.

## Crates

| Layer | Crate | Description |
|---|---|---|
| T0 Foundation | `sillstone-buffer` | Contiguous memory, striding, dtype, and slice views |
| T1 Core | `sillstone-math`, `sillstone-digest`, `sillstone-inflate`, `sillstone-unicode` | Arithmetic kernels, cryptographic/non-cryptographic digests, decompression, text algorithms |
| T2 Codecs | `sillstone-png`, `sillstone-jpeg`, `sillstone-mp3`, `sillstone-flac`, etc. | Pure-Rust media decoders and encoders |
| T3 Domains | `sillstone-image`, `sillstone-audio`, `sillstone-video`, `sillstone-text`, `sillstone-file` | Domain abstractions and perception pipelines |
| T4 Fingerprints | `sillstone-fingerprint`, `sillstone-cdc` | Perceptual hashing (pHash, minhash, chromaprint) and chunking |

## Language SDKs

- **Python:** `pip install sillstone` (ctypes)
- **Node.js:** `npm install sillstone` (native N-API)
- **Go:** `go get github.com/n24q02m/sillstone/sdk/go/sillstone` (cgo)
- **Java:** `io.github.sillstone:sillstone` (JNI)

## License

Apache-2.0. See [LICENSE](LICENSE) and [NOTICE](NOTICE) for details.
