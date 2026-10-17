#!/usr/bin/env python3
"""Enforces zero internal/personal traces and enforces clean separation in new monorepo."""

import os
import sys
from pathlib import Path

FORBIDDEN = [
    b"n24q02m",
    b"pith-image.n24q02m.com",
    b"n24q02m-ci",
]
ALLOWED_EXCEPTIONS = {
    # NOTICE explicitly mandates original MIT source provenance attribution
    "NOTICE",
    "scripts/ci/check-forbidden-traces.py",
}

def main() -> int:
    root = Path(__file__).resolve().parent.parent.parent
    violations = []

    for dirpath, dirnames, filenames in os.walk(root):
        # Skip git and target directories
        if ".git" in dirnames:
            dirnames.remove(".git")
        if "target" in dirnames:
            dirnames.remove("target")
        if "node_modules" in dirnames:
            dirnames.remove("node_modules")
        if "__pycache__" in dirnames:
            dirnames.remove("__pycache__")

        for f in filenames:
            rel = Path(dirpath, f).relative_to(root).as_posix()
            if rel in ALLOWED_EXCEPTIONS:
                continue

            # Check binary or text file
            try:
                data = Path(dirpath, f).read_bytes()
            except Exception:
                continue

            for term in FORBIDDEN:
                if term in data:
                    violations.append((rel, term.decode("utf-8", errors="replace")))

    if violations:
        print("FAILED: Forbidden identity traces detected in repository:", file=sys.stderr)
        for path, term in violations:
            print(f"  - {path}: contains forbidden token '{term}'", file=sys.stderr)
        return 1

    print("PASS: Clean separation confirmed. Zero forbidden traces found.")
    return 0

if __name__ == "__main__":
    sys.exit(main())
