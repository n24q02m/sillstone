# sillstone-qoi

Pure-Rust, zero-dependency implementation of the **Quite OK Image (QOI)** format encoder and decoder.

QOI is an ultra-fast lossless image compression format that matches PNG compression ratios while achieving 10-50x faster encoding and 3-4x faster decoding.

## Features

- Zero third-party dependencies (`alloc` + `core` only in core paths).
- Complete specification compliance with `qoif` headers and 8-byte end markers.
- Fast lossless RGB and RGBA encoding and decoding.
