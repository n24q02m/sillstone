# Sillstone

[![CI](https://github.com/n24q02m/sillstone/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/n24q02m/sillstone/actions/workflows/ci.yml) [![License](https://img.shields.io/github/license/n24q02m/sillstone)](LICENSE) [![Release](https://img.shields.io/github/v/release/n24q02m/sillstone)](https://github.com/n24q02m/sillstone/releases/latest) [![codecov](https://codecov.io/gh/n24q02m/sillstone/graph/badge.svg)](https://codecov.io/gh/n24q02m/sillstone)

[![PyPI](https://img.shields.io/pypi/v/sillstone)](https://pypi.org/project/sillstone/) [![npm](https://img.shields.io/npm/v/sillstone)](https://www.npmjs.com/package/sillstone) [![Maven Central](https://img.shields.io/maven-central/v/io.github.n24q02m/sillstone)](https://central.sonatype.com/artifact/io.github.n24q02m/sillstone) [![Go Reference](https://pkg.go.dev/badge/github.com/n24q02m/sillstone/sdk/go/sillstone.svg)](https://pkg.go.dev/github.com/n24q02m/sillstone/sdk/go/sillstone)

<!-- BEGIN: AUTO-GENERATED-CROSS-PROMO -->
<details>
  <summary><strong>Sister projects from n24q02m</strong> (click to expand)</summary>

| Project | Tagline | Tag |
|---|---|---|
| [better-semantic-release](https://github.com/n24q02m/better-semantic-release) | Drop-in python-semantic-release fork with built-in release-safety guards (orp... | Tooling |
| [claude-plugins](https://github.com/n24q02m/claude-plugins) | Claude Code plugin marketplace for the n24q02m MCP servers -- install web sea... | Marketplace |
| [crg](https://github.com/n24q02m/crg) | Knowledge graph for token-efficient code reviews -- semantic search and call-... | MCP |
| [email](https://github.com/n24q02m/email) | IMAP/SMTP email for AI agents -- read, send, organize folders, and manage att... | MCP |
| [fastretrieval](https://github.com/n24q02m/fastretrieval) | Fast multi-model retrieval runtime: ONNX and GGUF embeddings, reranking, and ... | Tooling |
| [hull](https://github.com/n24q02m/hull) | hull is the shared monorepo core for the wet, crg and mnemo self-hosted stack | MCP |
| [jules-task-archiver](https://github.com/n24q02m/jules-task-archiver) | Chrome Extension for bulk operations on Jules tasks via batchexecute API -- a... | Tooling |
| [mnemo](https://github.com/n24q02m/mnemo) | Persistent AI memory with hybrid search. Open, free, unlimited. | MCP |
| [notion](https://github.com/n24q02m/notion) | Markdown-first Notion for AI agents -- pages, databases, blocks, and comments... | MCP |
| [sillstone](https://github.com/n24q02m/sillstone) | Pure-Rust multimedia, hashing, and array suite | Tooling |
| [skret](https://github.com/n24q02m/skret) | Secrets without the server. | CLI |
| [tacet](https://github.com/n24q02m/tacet) | A self-distilling neuro-symbolic cascade that amortises LLM cost across knowl... | Tooling |
| [telegram](https://github.com/n24q02m/telegram) | Telegram for AI agents -- messages, chats, media, and contacts across both bo... | MCP |
| [tools-docs](https://github.com/n24q02m/tools-docs) | Docs portal for the n24q02m open-source tool stack (skret, MCP plugins, hull,... | Tooling |
| [wet](https://github.com/n24q02m/wet) | Open-source MCP server for AI agents: web search, content extraction, and lib... | MCP |
| [workspace](https://github.com/n24q02m/workspace) | Google Workspace MCP server (Docs/Drive/Calendar/Gmail/Sheets/Slides/Tasks/Ch... | MCP |

</details>
<!-- END: AUTO-GENERATED-CROSS-PROMO -->

Pure-Rust, zero-dependency multimedia, hashing, and array computing suite.

## Table of contents

- [Overview](#overview)
- [Crates](#crates)
- [Install](#install)
  - [Python packaging](#python-packaging)
- [License](#license)

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

## Install

Every SDK binds the same Rust core through the `sillstone-ffi` C ABI:

```console
pip install sillstone                                   # Python (ctypes)
npm install sillstone                                   # Node.js (native N-API)
go get github.com/n24q02m/sillstone/sdk/go/sillstone    # Go (cgo)
```

- **Java:** `io.github.n24q02m:sillstone` on Maven Central (JNI)

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
