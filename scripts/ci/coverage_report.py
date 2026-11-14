#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Print measured coverage and the files that fall short of the target.

Reads one coverage report per binding and prints the overall coverage, a
per-crate (or per-package) table and every file below the target:

    kind    report                                    unit
    llvm    ``cargo llvm-cov --json`` export (Rust)   lines
    py      ``coverage json`` report (Python)         statements
    go      ``go test -coverprofile`` profile (Go)    statements
    lcov    LCOV tracefile, e.g. ``node --test
            --test-reporter=lcov`` (Node.js)          lines
    jacoco  JaCoCo ``jacoco.csv`` (Java)              lines, per class

The measured numbers are reported as they are; the script never fails on a
shortfall (``--fail-under`` opts in) and never edits a claim.

Usage:
    coverage_report.py {llvm,py,go,lcov,jacoco} REPORT [--target 95] [--fail-under N]
"""

from __future__ import annotations

import argparse
import csv
import json
import os
import sys
from collections import defaultdict
from pathlib import Path


def load_llvm(path: Path) -> dict[str, tuple[int, int]]:
    """Returns ``{file: (covered_lines, total_lines)}`` from llvm-cov JSON."""
    doc = json.loads(path.read_text(encoding="utf-8"))
    files: dict[str, tuple[int, int]] = {}
    for export in doc["data"]:
        for entry in export["files"]:
            lines = entry["summary"]["lines"]
            files[entry["filename"]] = (lines["covered"], lines["count"])
    return files


def load_py(path: Path) -> dict[str, tuple[int, int]]:
    """Returns ``{file: (covered_lines, total_lines)}`` from coverage.py JSON."""
    doc = json.loads(path.read_text(encoding="utf-8"))
    files: dict[str, tuple[int, int]] = {}
    for name, entry in doc["files"].items():
        summary = entry["summary"]
        files[name] = (summary["covered_lines"], summary["num_statements"])
    return files


def load_go(path: Path) -> dict[str, tuple[int, int]]:
    """Returns ``{file: (covered_statements, statements)}`` from a Go cover profile.

    A block can appear more than once (one entry per test binary that compiled
    it), so blocks are de-duplicated and count as covered if any entry ran them.
    """
    blocks: dict[str, tuple[int, bool]] = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line.strip() or line.startswith("mode:"):
            continue
        block, statements, count = line.rsplit(" ", 2)
        seen = blocks.get(block, (int(statements), False))
        blocks[block] = (seen[0], seen[1] or int(count) > 0)
    files: dict[str, list[int]] = defaultdict(lambda: [0, 0])
    for block, (statements, hit) in blocks.items():
        name = block.rsplit(":", 1)[0]
        files[name][0] += statements if hit else 0
        files[name][1] += statements
    return {name: (cov, tot) for name, (cov, tot) in files.items()}


def load_lcov(path: Path) -> dict[str, tuple[int, int]]:
    """Returns ``{file: (lines_hit, lines_found)}`` from an LCOV tracefile."""
    files: dict[str, tuple[int, int]] = {}
    name, hit, found = None, 0, 0
    for line in path.read_text(encoding="utf-8").splitlines():
        if line.startswith("SF:"):
            name, hit, found = line[3:], 0, 0
        elif line.startswith("LH:"):
            hit = int(line[3:])
        elif line.startswith("LF:"):
            found = int(line[3:])
        elif line == "end_of_record" and name is not None:
            files[name] = (hit, found)
            name = None
    return files


def load_jacoco(path: Path) -> dict[str, tuple[int, int]]:
    """Returns ``{package/Class: (lines_covered, lines)}`` from a JaCoCo CSV report."""
    files: dict[str, tuple[int, int]] = {}
    with path.open(encoding="utf-8", newline="") as handle:
        for row in csv.DictReader(handle):
            covered = int(row["LINE_COVERED"])
            files[f"{row['PACKAGE']}/{row['CLASS']}"] = (
                covered,
                covered + int(row["LINE_MISSED"]),
            )
    return files


LOADERS = {
    "llvm": load_llvm,
    "py": load_py,
    "go": load_go,
    "lcov": load_lcov,
    "jacoco": load_jacoco,
}


def group_key(kind: str, filename: str) -> str:
    parts = Path(filename.replace("\\", "/")).parts
    if kind == "llvm" and "crates" in parts:
        return "crates/" + parts[parts.index("crates") + 1]
    if kind == "llvm" and "tools" in parts:
        return "tools/" + parts[parts.index("tools") + 1]
    return parts[-2] if len(parts) > 1 else parts[0]


def pct(covered: int, total: int) -> float:
    return 100.0 if total == 0 else 100.0 * covered / total


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("kind", choices=sorted(LOADERS))
    ap.add_argument("report", type=Path)
    ap.add_argument("--target", type=float, default=95.0)
    ap.add_argument("--fail-under", type=float, default=None)
    args = ap.parse_args()

    files = LOADERS[args.kind](args.report)
    if args.kind == "llvm":
        # Tests and integration fixtures are not subjects of the measurement.
        files = {
            f: v for f, v in files.items()
            if "/tests/" not in f.replace("\\", "/") and "/.cargo/" not in f.replace("\\", "/")
            and "/rustc/" not in f.replace("\\", "/")
        }

    groups: dict[str, list[int]] = defaultdict(lambda: [0, 0])
    for name, (cov, tot) in files.items():
        key = group_key(args.kind, name)
        groups[key][0] += cov
        groups[key][1] += tot

    total_cov = sum(v[0] for v in groups.values())
    total_all = sum(v[1] for v in groups.values())
    overall = pct(total_cov, total_all)

    out = [f"## Line coverage ({args.kind}): {overall:.2f}% ({total_cov}/{total_all})", ""]
    out += ["| Group | Lines covered | Total | Coverage | Target |", "|---|---:|---:|---:|---|"]
    for key in sorted(groups):
        cov, tot = groups[key]
        mark = "met" if pct(cov, tot) >= args.target else "BELOW"
        out.append(f"| {key} | {cov} | {tot} | {pct(cov, tot):.2f}% | {mark} (>= {args.target:g}%) |")

    short = sorted(
        ((pct(c, t), n, c, t) for n, (c, t) in files.items() if t and pct(c, t) < args.target),
    )
    out += ["", f"### Files below {args.target:g}% ({len(short)} of {len(files)})", ""]
    for p, name, c, t in short:
        out.append(f"- {p:6.2f}% ({c}/{t}) {os.path.relpath(name) if os.path.isabs(name) else name}")

    text = "\n".join(out) + "\n"
    print(text)
    summary = os.environ.get("GITHUB_STEP_SUMMARY")
    if summary:
        with open(summary, "a", encoding="utf-8") as fh:
            fh.write(text)

    if args.fail_under is not None and overall < args.fail_under:
        print(f"coverage {overall:.2f}% is under --fail-under {args.fail_under:g}%", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
