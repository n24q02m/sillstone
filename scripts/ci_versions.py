#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright (c) 2026 sillstone Contributors
"""Derive the five CI language matrices from the versions this repository declares.

A hand-written matrix is checked against nothing, so it rots: the floor moves in
``sdk/python/pyproject.toml`` (or the pom, the ``go.mod``, the package's
``engines``, the workspace ``rust-version``) and the workflow keeps testing a
version the package no longer claims -- or stops testing one it just started
claiming. Every list below is read from the declaring file at run time::

    language  declaring file and key
    python    sdk/python/pyproject.toml   project.requires-python
    node      sdk/node/package.json       engines.node
    java      sdk/java/pom.xml            maven.compiler.release (else .source)
    go        sdk/go/go.mod               go
    rust      Cargo.toml                  workspace.package.rust-version
                                         (else rust-toolchain.toml [toolchain].channel)

The top of each window is not "the newest release upstream ships" but "the newest
release this repository can actually install and run". A row that cannot resolve is
worse than an absent row: it turns every PR red for a reason no PR caused. Each
ceiling is therefore a constant shipped with the evidence it was read from and the
date it was read, the same way the rest of this fleet records a ceiling.

Rows are capped at five. When a language's window is wider than that, the floor row
is kept -- it is the promise that rots -- together with the newest four minors, where
breakage from a new compiler or interpreter actually lands. Every dropped version is
reported in ``excluded`` with its reason; nothing is ever dropped silently.

Usage::

    python scripts/ci_versions.py                   # JSON, every language
    python scripts/ci_versions.py --format github   # $GITHUB_OUTPUT lines
    python scripts/ci_versions.py --format plain    # one line per language

``scripts/test_ci_versions.py`` is the drift guard: it re-reads the declarations
with its own parser, asserts every list stays inside the declared window, and fails
when ``.github/workflows/ci.yml`` carries a literal version pin for one of these
five languages again.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
import tomllib
import xml.etree.ElementTree as ElementTree
from dataclasses import dataclass
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]

#: The Q34 contract caps every language matrix at five rows.
MAX_MATRIX_ROWS = 5

#: Date the ceilings below were read. Re-read them, do not guess, before moving one.
AS_OF = "2026-11-11"

# ---------------------------------------------------------------------------
# Ceilings, with the evidence they were read from.
# ---------------------------------------------------------------------------
# CPython ships one minor every October, so 3.15 is the newest stable line as of
# AS_OF. It is also the interpreter the canonical CI suite and the release build
# already run, so a row above it would be installed by nothing.
NEWEST_CPYTHON_MINOR = 15
CPYTHON_CEILING_REASON = (
    f"3.{NEWEST_CPYTHON_MINOR} is the newest stable CPython line as of {AS_OF}: "
    "CPython ships one minor every October and both the canonical CI suite and the "
    "release build already run this one, so nothing would install a row above it."
)

# The bottom of the window is where the test *oracle* still installs, not where the
# package claims to run. The resize/parity suites compare sillstone against Pillow
# bit for bit and ship pillow==13.0.0 as the oracle, and Pillow 13 declares
# requires-python >=3.11 (PyPI metadata read on AS_OF; pip refuses the pin on 3.10:
# "Could not find a version that satisfies the requirement pillow==13.0.0").
# A row below that cannot verify the parity claim, so it is reported as excluded and
# not silently dropped.
PYTHON_ORACLE_FLOOR_MINOR = 11
PYTHON_ORACLE_REASON = (
    "the parity oracle pillow==13.0.0 declares requires-python >=3.11 "
    f"(PyPI metadata read {AS_OF}) and the resize/parity suites compare against it "
    "bit for bit, so a row below 3.11 cannot verify the SDK's parity claim; open the "
    "row by dropping the oracle pin, not by testing against one that disagrees"
)
PYTHON_GRID_RULE = (
    "Rows run every CPython minor from the newest of the declared floor and the "
    f"oracle floor (3.{PYTHON_ORACLE_FLOOR_MINOR}, under the row cap) to 3."
    f"{NEWEST_CPYTHON_MINOR}, because that is the window whose bit-for-bit parity "
    "claim against Pillow can actually be executed. requires-python still declares the "
    "wider promise; the exclusions below say which rows that promise costs."
)

# nodejs.org/dist/index.json as of AS_OF: 26 is the current Active LTS ("Lithium",
# LTS since 2026-10-28), 24 and 22 are in maintenance (22 until 2027-04).
NODE_LTS_MAJORS = (22, 24, 26)
NODE_GRID_RULE = (
    "Only LTS lines are rows. Even majors enter LTS in October of their release year "
    f"and keep it for 30 months, so as of {AS_OF} the maintainable grid is "
    f"{', '.join(str(major) for major in NODE_LTS_MAJORS)}. Odd majors go EOL six "
    "months after release: a green row on one is a clean bill of health for a runtime "
    "no consumer should be running."
)

# Adoptium available_releases as of AS_OF: available_lts_releases [8, 11, 17, 21, 25],
# most_recent_lts 25, most_recent_feature_release 27.
JAVA_LTS_MAJORS = (17, 21, 25)
JAVA_GRID_RULE = (
    "Only LTS lines are rows, because the pom compiles for source/target 17 and "
    "consumers deploy on LTS. Adoptium's available_lts_releases reads [8, 11, 17, 21, "
    f"25] with most_recent_lts 25 as of {AS_OF}; feature releases are dropped after "
    "six months and were never a support promise."
)

# go.dev/dl/?mode=json as of AS_OF: go1.27.4 is the newest stable release, and the
# canonical CI job already builds with 1.27.
NEWEST_GO_MINOR = 27
GO_CEILING_REASON = (
    f"1.{NEWEST_GO_MINOR} is the newest stable Go release as of {AS_OF} "
    "(go.dev/dl lists go1.27.4); the canonical CI job already builds with it, so a "
    "row above it would be installed by nothing."
)

# static.rust-lang.org/dist/channel-rust-1.98.0.toml resolves as of AS_OF.
NEWEST_STABLE_RUST = "1.98.0"
RUST_GRID_RULE = (
    "The grid is the declared MSRV plus the current stable toolchain. Rust's stable "
    "channel is a backward-compatible superset of every earlier minor, so interim "
    "minors add build minutes without a configuration that can fail differently. "
    f"{NEWEST_STABLE_RUST} is the stable release as of {AS_OF}."
)

PYTHON_SOURCE = "sdk/python/pyproject.toml project.requires-python"
NODE_SOURCE = "sdk/node/package.json engines.node"
JAVA_SOURCE = "sdk/java/pom.xml maven.compiler.release|source"
GO_SOURCE = "sdk/go/go.mod go directive"
RUST_SOURCE = "Cargo.toml workspace.package.rust-version"


@dataclass(frozen=True)
class Matrix:
    """One language's resolved support window."""

    language: str
    floor: str
    source: str
    grid_rule: str
    versions: list[str]
    excluded: list[dict[str, str]]

    @property
    def newest(self) -> str:
        return self.versions[-1]

    def as_dict(self) -> dict[str, object]:
        return {
            "floor": self.floor,
            "newest": self.newest,
            "source": self.source,
            "grid_rule": self.grid_rule,
            "versions": self.versions,
            "excluded": self.excluded,
        }


def _read(relative: str) -> str:
    return (REPO_ROOT / relative).read_text(encoding="utf-8")


def _sort_key(version: str) -> tuple[int, ...]:
    return tuple(int(part) for part in version.split("."))


def _cap(versions: list[str]) -> tuple[list[str], list[dict[str, str]]]:
    """Trim a window to MAX_MATRIX_ROWS rows, keeping the floor and the newest rows."""
    if len(versions) <= MAX_MATRIX_ROWS:
        return versions, []
    kept = [versions[0], *versions[-(MAX_MATRIX_ROWS - 1) :]]
    dropped = [version for version in versions if version not in kept]
    return kept, [
        {
            "version": version,
            "reason": (
                f"dropped by the {MAX_MATRIX_ROWS}-row cap: the declared floor row and "
                "the newest four minors are the rows that can fail"
            ),
        }
        for version in dropped
    ]


# ---------------------------------------------------------------------------
# Declared floors. Each reader fails loudly rather than guessing a default.
# ---------------------------------------------------------------------------
def python_floor() -> str:
    document = tomllib.loads(_read("sdk/python/pyproject.toml"))
    try:
        declared = document["project"]["requires-python"]
    except KeyError as exc:  # pragma: no cover - the file declares it today
        raise ValueError("sdk/python/pyproject.toml declares no project.requires-python") from exc
    for clause in str(declared).split(","):
        match = re.fullmatch(r">=\s*3\.(\d+)", clause.strip())
        if match:
            return f"3.{int(match.group(1))}"
    raise ValueError(
        f"sdk/python/pyproject.toml requires-python {declared!r} declares no >=3.X floor"
    )


def node_floor() -> str:
    document = json.loads(_read("sdk/node/package.json"))
    declared = document.get("engines", {}).get("node")
    if declared is None:
        raise ValueError(
            "sdk/node/package.json declares no engines.node; the Node matrix has no floor to read"
        )
    match = re.search(r">=\s*(\d+)", str(declared))
    if match is None:
        raise ValueError(f"sdk/node/package.json engines.node {declared!r} declares no >=N floor")
    return match.group(1)


def java_floor() -> str:
    pom = ElementTree.fromstring(_read("sdk/java/pom.xml"))
    namespace = "{http://maven.apache.org/POM/4.0.0}"
    properties = pom.find(f"{namespace}properties")
    if properties is not None:
        for key in ("maven.compiler.release", "maven.compiler.source"):
            declared = properties.find(f"{namespace}{key}")
            if declared is not None and declared.text and declared.text.strip():
                return declared.text.strip()
    raise ValueError(
        "sdk/java/pom.xml declares neither maven.compiler.release nor maven.compiler.source"
    )


def go_floor() -> str:
    match = re.search(r"^go\s+(\d+\.\d+)\s*$", _read("sdk/go/go.mod"), re.MULTILINE)
    if match is None:
        raise ValueError("sdk/go/go.mod declares no `go` directive")
    return match.group(1)


def rust_floor() -> str:
    document = tomllib.loads(_read("Cargo.toml"))
    declared = document.get("workspace", {}).get("package", {}).get("rust-version")
    if declared:
        return str(declared)
    toolchain = REPO_ROOT / "rust-toolchain.toml"
    if toolchain.exists():
        channel = tomllib.loads(toolchain.read_text(encoding="utf-8")).get("toolchain", {}).get("channel")
        if channel:
            return str(channel)
    raise ValueError("no rust-version in Cargo.toml [workspace.package] and no rust-toolchain.toml")


# ---------------------------------------------------------------------------
# One resolver per language.
# ---------------------------------------------------------------------------
def python_matrix() -> Matrix:
    floor = python_floor()
    floor_minor = _sort_key(floor)[1]
    if floor_minor > NEWEST_CPYTHON_MINOR:
        raise ValueError(
            f"requires-python floor 3.{floor_minor} is above the newest row this repo can "
            f"install (3.{NEWEST_CPYTHON_MINOR}): {CPYTHON_CEILING_REASON}"
        )
    start = max(floor_minor, PYTHON_ORACLE_FLOOR_MINOR)
    excluded = [
        {"version": f"3.{minor}", "reason": PYTHON_ORACLE_REASON}
        for minor in range(floor_minor, start)
    ]
    candidates = [f"3.{minor}" for minor in range(start, NEWEST_CPYTHON_MINOR + 1)]
    versions, dropped = _cap(candidates)
    return Matrix("python", floor, PYTHON_SOURCE, PYTHON_GRID_RULE, versions, excluded + dropped)


def node_matrix() -> Matrix:
    floor = node_floor()
    newest = max(NODE_LTS_MAJORS)
    if int(floor) > newest:
        raise ValueError(
            f"engines.node declares >={floor} but the newest LTS line is {newest}: {NODE_GRID_RULE}"
        )
    grid = [major for major in NODE_LTS_MAJORS if major >= int(floor)]
    excluded = [
        {
            "version": str(major),
            "reason": (
                "not an LTS line: odd majors go EOL six months after release, so the row "
                "would prove a runtime no consumer should be running"
            ),
        }
        for major in range(int(floor), newest)
        if major not in grid
    ]
    return Matrix(
        "node", floor, NODE_SOURCE, NODE_GRID_RULE, [str(major) for major in grid], excluded
    )


def java_matrix() -> Matrix:
    floor = int(java_floor())
    newest = max(JAVA_LTS_MAJORS)
    if floor > newest:
        raise ValueError(
            f"the pom compiles for {floor} but the newest LTS line is {newest}: {JAVA_GRID_RULE}"
        )
    grid = [major for major in JAVA_LTS_MAJORS if major >= floor]
    if floor not in grid:
        # A floor that is not itself an LTS line is still the row that must run.
        grid.insert(0, floor)
    excluded = [
        {
            "version": str(major),
            "reason": (
                "not an LTS line: feature releases are dropped after six months and were "
                "never a support promise"
            ),
        }
        for major in range(floor, newest)
        if major not in grid
    ]
    return Matrix(
        "java", str(floor), JAVA_SOURCE, JAVA_GRID_RULE, [str(major) for major in grid], excluded
    )


def go_matrix() -> Matrix:
    floor = go_floor()
    floor_minor = _sort_key(floor)[1]
    if floor_minor > NEWEST_GO_MINOR:
        raise ValueError(
            f"the go directive floor 1.{floor_minor} is above the newest installable row "
            f"(1.{NEWEST_GO_MINOR}): {GO_CEILING_REASON}"
        )
    candidates = [f"1.{minor}" for minor in range(floor_minor, NEWEST_GO_MINOR + 1)]
    versions, excluded = _cap(candidates)
    return Matrix("go", floor, GO_SOURCE, GO_CEILING_REASON, versions, excluded)


def rust_matrix() -> Matrix:
    floor = rust_floor()
    newest = NEWEST_STABLE_RUST
    if _sort_key(floor) > _sort_key(newest):
        raise ValueError(
            f"[workspace.package] rust-version {floor} is above the stable release "
            f"{newest}: {RUST_GRID_RULE}"
        )
    versions = [floor] if _sort_key(floor) == _sort_key(newest) else [floor, newest]
    return Matrix("rust", floor, RUST_SOURCE, RUST_GRID_RULE, versions, [])


RESOLVERS = {
    "python": python_matrix,
    "node": node_matrix,
    "java": java_matrix,
    "go": go_matrix,
    "rust": rust_matrix,
}


def resolve() -> dict[str, Matrix]:
    """Return every language matrix, keyed by language in declaration order."""
    return {language: resolver() for language, resolver in RESOLVERS.items()}


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Derive the CI language matrices from the declared floors."
    )
    parser.add_argument(
        "--format",
        choices=("json", "github", "plain"),
        default="json",
        help="json (default), github ($GITHUB_OUTPUT lines) or plain (one line per language)",
    )
    arguments = parser.parse_args(argv)
    matrices = resolve()
    if arguments.format == "json":
        print(json.dumps({name: matrix.as_dict() for name, matrix in matrices.items()}, indent=2))
        return 0
    if arguments.format == "github":
        for name, matrix in matrices.items():
            print(f"{name}={json.dumps(matrix.versions)}")
            print(f"{name}-newest={matrix.newest}")
        return 0
    for name, matrix in matrices.items():
        print(
            f"{name:<7}{' '.join(matrix.versions):<28} floor {matrix.floor}  newest "
            f"{matrix.newest}  ({matrix.source})"
        )
        for entry in matrix.excluded:
            print(f"        excluded {entry['version']}: {entry['reason']}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
