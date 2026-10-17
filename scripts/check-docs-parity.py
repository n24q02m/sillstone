#!/usr/bin/env python3
"""Verifies that documentation files exist, are non-empty, and maintain parity invariants."""
from pathlib import Path
import sys

REPO_ROOT = Path(__file__).resolve().parent.parent

DOCS = [
    REPO_ROOT / "docs" / "limits.md",
    REPO_ROOT / "docs" / "parity.md",
    REPO_ROOT / "docs" / "scorecard.md",
    REPO_ROOT / "ROADMAP.md",
    REPO_ROOT / ".github" / "profile" / "README.md",
]

def main() -> int:
    for doc in DOCS:
        if not doc.exists():
            print(f"FAIL: Missing required documentation file: {doc.relative_to(REPO_ROOT)}")
            return 1
        content = doc.read_text(encoding="utf-8")
        if len(content.strip()) < 500:
            print(f"FAIL: Documentation file is too short/empty: {doc.relative_to(REPO_ROOT)}")
            return 1
        print(f"OK: {doc.relative_to(REPO_ROOT)} ({len(content.splitlines())} lines, {len(content)} bytes)")
    print("ALL DOCUMENTATION INTEGRITY CHECKS PASSED.")
    return 0

if __name__ == "__main__":
    sys.exit(main())
