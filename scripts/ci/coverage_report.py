#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Print measured coverage and the files that fall short of the target.

Reads either a ``cargo llvm-cov --json`` export (``--format llvm``) or a
``coverage.py`` ``coverage json`` report and prints the overall line
coverage, a per-crate (or per-package) table and every file below the
target. The measured numbers are reported as they are; the script never
fails on a shortfall (``--fail-under`` opts in) and never edits a claim.

Usage:
    coverage_report.py llvm  cov.json  [--target 95] [--fail-under N]
    coverage_report.py py    cov.json  [--target 95] [--fail-under N]
"""

from __future__ import annotations

import argparse
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
    ap.add_argument("kind", choices=["llvm", "py"])
    ap.add_argument("report", type=Path)
    ap.add_argument("--target", type=float, default=95.0)
    ap.add_argument("--fail-under", type=float, default=None)
    args = ap.parse_args()

    files = load_llvm(args.report) if args.kind == "llvm" else load_py(args.report)
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
