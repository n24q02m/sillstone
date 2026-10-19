# Deprecated Repository Notice

> **IMPORTANT**: This repository is deprecated and superseded.

Active development, security fixes, and new releases have transitioned to the unified monorepo:

### 👉 **[github.com/n24q02m/sillstone](https://github.com/n24q02m/sillstone)**

---

### Migration Summary

- **Unified Monorepo**: All 19 individual libraries, multimedia codecs, hashing primitives, and multi-language SDKs are now developed together under [sillstone](https://github.com/n24q02m/sillstone).
- **Zero Dependencies**: Core Rust implementation and multi-language SDKs (Python, Node.js, Go, Java) maintain zero third-party dependencies.
- **Package Relocation**:
  - **Rust**: Use crate `sillstone` or `sillstone-{crate}`.
  - **Python**: Use `pip install sillstone` (replacing standalone wheels).
  - **Node.js**: Use `npm install sillstone` (pure N-API, no dynamic loader dependency).
  - **Go**: Use `go get github.com/n24q02m/sillstone/sdk/go/sillstone`.
  - **Java**: Maven coordinates `io.github.sillstone:sillstone`.

Please refer to the comprehensive [Migration Guide](https://github.com/n24q02m/sillstone/blob/main/docs/MIGRATION.md) for full details.
