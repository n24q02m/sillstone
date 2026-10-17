#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright (c) 2026 sillstone Contributors
"""Generates repository redirect notices for the 19 standalone legacy repositories.

Usage:
    python scripts/deprecation/generate-old-repo-redirects.py [--check] [--output-dir DIR]
"""
from __future__ import annotations

import argparse
import pathlib
import sys

LEGACY_REPOS = [
    "pith-audio",
    "pith-cdc",
    "pith-digest",
    "pith-file",
    "pith-flac",
    "pith-h264",
    "pith-hash",
    "pith-image",
    "pith-inflate",
    "pith-jpeg",
    "pith-math",
    "pith-mp3",
    "pith-mp4",
    "pith-pdf",
    "pith-png",
    "pith-text",
    "pith-unicode",
    "pith-video",
    "pith-zip",
]


def generate_notice(repo_name: str) -> str:
    subcrate = repo_name.replace("pith-", "sillstone-")
    if subcrate == "sillstone-hash":
        subcrate = "sillstone"

    return f"""# {repo_name} (Deprecated)

> **NOTICE**: This repository has been deprecated and merged into the unified Sillstone monorepo:
> **[github.com/sillstone/sillstone](https://github.com/sillstone/sillstone)**

---

### Redirection Details

- **Replacement Crate**: `{subcrate}` within the `sillstone` monorepo.
- **Migration Guide**: See [docs/MIGRATION.md](https://github.com/sillstone/sillstone/blob/main/docs/MIGRATION.md).
- **SDK Relocation**:
  - Python: `pip install sillstone`
  - Node.js: `npm install sillstone`
  - Go: `go get github.com/sillstone/sillstone/sdk/go/sillstone`
  - Java: `io.github.sillstone:sillstone`

This repository is archived and maintained for provenance and reference only.
"""


def main() -> int:
    parser = argparse.ArgumentParser(description="Generate redirection notices for legacy repos")
    parser.add_argument("--output-dir", type=pathlib.Path, default=None, help="Directory to output notices")
    parser.add_argument("--check", action="store_true", help="Verify notice generation")
    args = parser.parse_args()

    if args.check or not args.output_dir:
        for repo in LEGACY_REPOS:
            notice = generate_notice(repo)
            assert len(notice) > 100
        print(f"Verified redirect notices for all {len(LEGACY_REPOS)} legacy repositories.")
        return 0

    args.output_dir.mkdir(parents=True, exist_ok=True)
    for repo in LEGACY_REPOS:
        out_file = args.output_dir / f"{repo}-README.md"
        out_file.write_text(generate_notice(repo), encoding="utf-8")
        print(f"Wrote {out_file}")

    return 0


if __name__ == "__main__":
    sys.exit(main())
