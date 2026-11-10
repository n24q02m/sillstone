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
| T4 Fingerprints | `sillstone-cdc` (chunking); perceptual hashing, MinHash and audio/video fingerprints live in `sillstone-image`, `sillstone-text`, `sillstone-audio`, `sillstone-video` | Perceptual hashing (pHash, minhash, chromaprint) and chunking |

## Language SDKs

- **Python:** `pip install sillstone` (ctypes)
- **Node.js:** `npm install sillstone` (native N-API)
- **Go:** `go get github.com/n24q02m/sillstone/sdk/go/sillstone` (cgo)
- **Java:** `io.github.n24q02m:sillstone` (JNI)

### Python packaging

`pip install sillstone` resolves to a prebuilt wheel for linux-x64, linux-arm64, darwin-arm64, darwin-x64 and win32-x64. Those carry the Rust `cdylib` and need no toolchain.

Every release also publishes a source distribution. It carries the Rust crates the ctypes extension links and no binary — one library cannot serve five platforms — so installing it compiles the `cdylib` on the machine doing the install. That route therefore needs a Rust toolchain (`cargo`, <https://rustup.rs>); the workspace has no third-party dependencies, so the build itself needs no network:

```console
pip install --no-binary sillstone sillstone
```

Without a toolchain, force a wheel instead:

```console
pip install --only-binary :all: sillstone
```

The archive ships only what `sillstone-ffi` builds: the `sillstone` CLI crate and the IDL generator are published on their own and are not part of it. CI builds the archive and installs it from source on Linux on every change, so a broken source route fails the pipeline rather than a user's install.

## License

Apache-2.0. See [LICENSE](LICENSE) and [NOTICE](NOTICE) for details.
