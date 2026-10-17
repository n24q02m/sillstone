# Sillstone

Pure-Rust, zero-dependency multimedia, hashing, and array computing suite.

```text
       _ _ _     _                   
  ___ (_) | |___| |_ ___  _ __   ___ 
 / __|| | | / __| __/ _ \| '_ \ / _ \
 \__ \| | | \__ \ || (_) | | | |  __/
 |___/|_|_|_|___/\__\___/|_| |_|\___|
```

## Guarantees

- **Pure Rust, Zero Dependencies:** The entire Rust core depends strictly on `std`. No OpenSSL, no libpng, no ffmpeg, no libc.
- **Deterministic Multi-Language SDKs:** Python, Node.js, Go, and Java bindings are generated from a single machine-readable IDL and verified hex-exact against 45 reference vectors.
- **Safe by Default:** `#![forbid(unsafe_code)]` across all domain modules. Unsafe code is strictly limited to foreign function interface boundaries with verified pointer bounds and no panics across FFI.

## Ecosystem Packages

| Ecosystem | Package | Installation | Link |
|---|---|---|---|
| **Rust / Cargo** | `sillstone` | `cargo add sillstone` | [crates.io/crates/sillstone](https://crates.io/crates/sillstone) |
| **Python** | `sillstone` | `pip install sillstone` | [pypi.org/project/sillstone](https://pypi.org/project/sillstone) |
| **JavaScript / TypeScript** | `sillstone` | `npm install sillstone` | [npmjs.com/package/sillstone](https://npmjs.com/package/sillstone) |
| **Go** | `sillstone` | `go get github.com/sillstone/sillstone/sdk/go/sillstone` | [pkg.go.dev/github.com/sillstone/sillstone/sdk/go/sillstone](https://pkg.go.dev/github.com/sillstone/sillstone/sdk/go/sillstone) |
| **Java** | `sillstone` | `io.github.sillstone:sillstone` | [central.sonatype.com/artifact/io.github.sillstone/sillstone](https://central.sonatype.com/artifact/io.github.sillstone/sillstone) |

## Documentation

- [Capabilities & Limits](https://github.com/sillstone/sillstone/blob/main/docs/limits.md)
- [Multi-Language Parity Matrix](https://github.com/sillstone/sillstone/blob/main/docs/parity.md)
- [Crate Scorecard](https://github.com/sillstone/sillstone/blob/main/docs/scorecard.md)
- [Architecture & Roadmap](https://github.com/sillstone/sillstone/blob/main/ROADMAP.md)

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](https://github.com/sillstone/sillstone/blob/main/LICENSE) and [NOTICE](https://github.com/sillstone/sillstone/blob/main/NOTICE).
