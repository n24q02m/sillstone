#!/usr/bin/env python3
"""Enforces zero third-party dependencies in Cargo.lock."""

import sys
from pathlib import Path

def main() -> int:
    root = Path(__file__).resolve().parent.parent.parent
    lock_file = root / "Cargo.lock"
    if not lock_file.exists():
        print("Cargo.lock not found. Run cargo check first.", file=sys.stderr)
        return 1

    content = lock_file.read_text(encoding="utf-8")
    violations = []
    
    # Simple TOML parser for Cargo.lock packages
    current_pkg = None
    for line in content.splitlines():
        line = line.strip()
        if line == "[[package]]":
            current_pkg = {}
        elif current_pkg is not None:
            if line.startswith("name = "):
                current_pkg["name"] = line.split("=", 1)[1].strip().strip('"')
            elif line.startswith("source = "):
                current_pkg["source"] = line.split("=", 1)[1].strip().strip('"')
                if "registry+" in current_pkg["source"] or "git+" in current_pkg["source"]:
                    violations.append(current_pkg["name"])

    if violations:
        print(f"FAILED: Found non-workspace external dependencies in Cargo.lock: {violations}", file=sys.stderr)
        return 1

    print("PASS: Zero third-party dependencies verified in Cargo.lock.")
    return 0

if __name__ == "__main__":
    sys.exit(main())
