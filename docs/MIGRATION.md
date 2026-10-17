# Migration Guide: Pith Suite to Sillstone Monorepo

This document provides instructions for migrating from the 19 standalone `pith-*` repositories and packages to the unified **Sillstone** monorepo.

---

## 1. Overview of Architecture Transition

Prior to release 0.2.0, the codebase was distributed across 19 individual repositories under the `pith-hash` organization. Each repository maintained its own Cargo workspace, foreign function interfaces (FFI), and language SDK bindings (often relying on dynamic loaders such as `koffi` in Node.js).

Beginning with version 0.2.0, the suite is consolidated into a single zero-dependency monorepo: **[github.com/sillstone/sillstone](https://github.com/sillstone/sillstone)**.

Key benefits of the migration:
1. **Single Source of Truth**: All multimedia, hashing, array operations, and codecs compile and test together with zero cross-repo drift.
2. **Zero Runtime Dependencies**: The core Rust workspace has zero third-party crate dependencies. The multi-language SDKs (Node.js, Python, Go, Java) run with zero third-party package dependencies.
3. **Consistent C ABI and Multi-Language Surface**: A single dynamic library (`sillstone_ffi`) exposes the complete capability set with uniform error codes and semantics across all four language bindings.

---

## 2. Rust Crates Mapping

Every standalone `pith-*` crate maps directly to a `sillstone-*` workspace crate:

| Standalone Crate (`pith-*`) | Monorepo Crate (`sillstone-*`) | Layer / Domain |
|---|---|---|
| `pith-hash` (facade) | `sillstone` / `sillstone-core` | Monorepo root facade & unified API |
| `pith-math` | `sillstone-math` | 1D kernels, DST, ODE, integration, DCT, FFT |
| `pith-digest` | `sillstone-digest` | Cryptographic & checksum digests (SHA3, Blake2b, Blake3, FNV, CRC) |
| `pith-unicode` | `sillstone-unicode` | UAX29 grapheme segmentation, category classification |
| `pith-inflate` | `sillstone-inflate` | Zero-dependency DEFLATE, Brotli, and Zstandard decompressors |
| `pith-png` | `sillstone-png` | Pure-Rust PNG decoding and encoding |
| `pith-jpeg` | `sillstone-jpeg` | Pure-Rust baseline JPEG decoding and encoding |
| `pith-image` | `sillstone-image` | Raster primitives, BMP, perceptual hashing (pHash, dHash, wHash) |
| `pith-audio` | `sillstone-audio` | Audio waveforms, tone analysis, acoustic fingerprinting |
| `pith-video` | `sillstone-video` | Video frame processing, video perceptual hashing |
| `pith-text` | `sillstone-text` | MinHash-128 text fingerprinting, text metrics |
| `pith-cdc` | `sillstone-cdc` | Content-Defined Chunking (FastCDC algorithms) |
| `pith-file` | `sillstone-file` | File format sniffing and container validation |
| `pith-zip` | `sillstone-zip` | Pure-Rust ZIP archive reading and writing |
| `pith-flac` | `sillstone-flac` | Pure-Rust FLAC audio container decoding |
| `pith-mp3` | `sillstone-mp3` | Pure-Rust MP3 audio frame decoding |
| `pith-mp4` | `sillstone-mp4` | Pure-Rust MP4 / ISO Base Media File Format parser |
| `pith-h264` | `sillstone-h264` | Pure-Rust H.264 bitstream parsing and deblocking |
| `pith-pdf` | `sillstone-pdf` | Pure-Rust PDF structure parsing |

### Rust Dependency Update

```toml
# Old (deprecated)
[dependencies]
pith-hash = "0.1.1"

# New (recommended)
[dependencies]
sillstone = "0.2.0-beta.1"
```

---

## 3. Multi-Language SDK Migration

### Python

#### Package Installation
```bash
# Old
pip install pith-hash

# New
pip install sillstone
```

#### Code Updates
```python
# Old
import pith_hash

res = pith_hash.detect(data)
hash_val = pith_hash.content_hash(data)

# New
import sillstone

res = sillstone.detect(data)
hash_val = sillstone.content_hash(data)
sig = sillstone.signature(data)
match_res = sillstone.match_signatures(sig_a, sig_b)
```

---

### Node.js / JavaScript / TypeScript

#### Package Installation
```bash
# Old (relied on koffi dynamic loader)
npm install pith-hash

# New (pure zero-dependency native N-API addon)
npm install sillstone
```

#### Code Updates
```javascript
// Old
const pith = require("pith-hash");
const desc = pith.describe(buffer);

// New
const sillstone = require("sillstone");

const desc = sillstone.describe(buffer);
const detection = sillstone.detect(buffer);
const contentHash = sillstone.contentHash(buffer);
const sig = sillstone.signature(buffer);
const matches = sillstone.matchSignatures(sigA, sigB);
```

---

### Go

#### Module Import
```bash
# Old
go get github.com/pith-hash/pith-hash

# New
go get github.com/sillstone/sillstone/sdk/go/sillstone
```

#### Code Updates
```go
package main

import (
    "fmt"
    "github.com/sillstone/sillstone/sdk/go/sillstone"
)

func main() {
    data := []byte("example payload")
    res, err := sillstone.Detect(data)
    if err != nil {
        panic(err)
    }
    fmt.Println("Detected format:", res.Format)

    hash, err := sillstone.ContentHash(data)
    if err != nil {
        panic(err)
    }
    fmt.Printf("Content hash: %x\n", hash)
}
```

---

### Java

#### Maven Dependency
```xml
<!-- Old -->
<dependency>
    <groupId>io.github.pith-hash</groupId>
    <artifactId>pith-hash</artifactId>
    <version>0.1.1</version>
</dependency>

<!-- New -->
<dependency>
    <groupId>io.github.sillstone</groupId>
    <artifactId>sillstone</artifactId>
    <version>0.2.0-beta.1</version>
</dependency>
```

#### Code Updates
```java
// Old
import io.github.pithhash.PithNative;
byte[] hash = PithNative.contentHash(data);

// New
import io.github.sillstone.SillstoneNative;
byte[] hash = SillstoneNative.contentHash(data);
int format = SillstoneNative.detect(data);
```

---

## 4. Deprecation Timeline & Backward Compatibility

1. **Standalone Repositories**: The 19 repositories under `github.com/pith-hash/*` will have final redirect notices published to their README files and will subsequently be marked read-only / archived on GitHub.
2. **Registry Notice**: Versions published under the legacy names on crates.io, PyPI, npm, and Maven Central remain accessible for existing builds, with deprecation warnings pointing to the unified `sillstone` packages.
3. **Continuous Support**: All future feature development, bug fixes, performance optimizations, and security patches will be published exclusively through the `sillstone` monorepo.
