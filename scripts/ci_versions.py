#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright (c) 2026 sillstone Contributors
"""Derive the five CI language matrices: the upstream-supported lines this repo declares.

Every library in this fleet runs CI on the language versions upstream still supports,
at most five rows per language. A row is therefore the intersection of two windows:

* the **declared** window -- the floor this repository promises, read at run time
  from the declaring file, so the matrix cannot drift away from the manifest::

      language  declaring file and key
      python    sdk/python/pyproject.toml   project.requires-python
      node      sdk/node/package.json       engines.node
      java      sdk/java/pom.xml            maven.compiler.release (else .source)
      go        sdk/go/go.mod               go
      rust      Cargo.toml                  workspace.package.rust-version
                                           (else rust-toolchain.toml [toolchain].channel)

* the **upstream** window -- every line that has been released and has not reached
  end of life on the day the resolver runs. Each language carries a dated table read
  from the upstream's own schedule (``source`` and ``LAST_VERIFIED`` below). Filtering
  by today's date means a line leaves the matrix on its end-of-life date without anyone
  editing this file; a new line enters when the table is re-read, never by guessing.

A declared floor below the upstream window is not tested silently either way: every
line between the floor and the newest row that is not a row is listed in ``excluded``
with its end-of-life date. Rust is the one deliberate exception: upstream supports only
the current stable, while ``rust-version`` is a compiler promise Cargo enforces, so the
declared MSRV stays a row next to the supported stable line.

Rows are capped at five. When the window is wider, the oldest supported line is kept --
it is the promise that rots -- together with the newest four, where breakage from a new
compiler or interpreter lands. Every dropped version is reported; nothing is dropped
silently.

Usage::

    python scripts/ci_versions.py                     # JSON, every language
    python scripts/ci_versions.py --format github     # $GITHUB_OUTPUT lines
    python scripts/ci_versions.py --format plain      # one line per language
    python scripts/ci_versions.py --today 2027-11-01  # resolve as of another day

``scripts/ci/test_ci_versions.py`` is the drift guard: it re-reads the declarations
with its own parser, asserts every list stays inside both windows, and fails when
``.github/workflows/ci.yml`` carries a literal version pin for one of these five
languages again.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
import tomllib
import xml.etree.ElementTree as ElementTree
from dataclasses import dataclass
from datetime import date, datetime, timedelta, timezone
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]

#: Every language matrix is capped at five rows.
MAX_MATRIX_ROWS = 5

#: Day the upstream tables below were read. Re-read them, do not guess, before adding
#: a line: a row that cannot install turns every PR red for a reason no PR caused.
LAST_VERIFIED = date(2026, 11, 14)

#: After this long the resolver warns that the tables need a re-read. A line that
#: reaches end of life still drops on its own; a new line only appears on a re-read.
REVIEW_AFTER = timedelta(days=120)


@dataclass(frozen=True)
class Line:
    """One upstream release line and its support dates."""

    version: str
    released: date
    #: ``None`` while upstream has not scheduled the end of support yet.
    eol: date | None = None

    def supported_on(self, today: date) -> bool:
        return self.released <= today and (self.eol is None or today < self.eol)


@dataclass(frozen=True)
class Upstream:
    """An upstream's support policy and the dated lines read from its schedule."""

    source: str
    policy: str
    lines: tuple[Line, ...]


def _d(text: str) -> date:
    return date.fromisoformat(text)


# ---------------------------------------------------------------------------
# Upstream support tables, read on LAST_VERIFIED from the sources named in each.
# ---------------------------------------------------------------------------
PYTHON_UPSTREAM = Upstream(
    source=(
        "peps.python.org/api/release-cycle.json (the data behind the devguide's "
        "'Status of Python versions') and endoflife.date/api/python.json for the day"
    ),
    policy=(
        "CPython supports a minor for five years from its October release: bugfix "
        "releases for two years, security-only releases after that."
    ),
    lines=(
        Line("3.9", _d("2020-10-05"), _d("2025-10-31")),
        Line("3.10", _d("2021-10-04"), _d("2026-10-01")),
        Line("3.11", _d("2022-10-24"), _d("2027-10-31")),
        Line("3.12", _d("2023-10-02"), _d("2028-10-31")),
        Line("3.13", _d("2024-10-07"), _d("2029-10-31")),
        Line("3.14", _d("2025-10-07"), _d("2030-10-31")),
        Line("3.15", _d("2026-10-09"), _d("2031-10-31")),
    ),
)

NODE_UPSTREAM = Upstream(
    source="github.com/nodejs/Release schedule.json (start and end of every line)",
    policy=(
        "Node.js supports an even major for 30 months (LTS from the October after its "
        "release) and an odd major for about seven months."
    ),
    lines=(
        Line("22", _d("2024-04-24"), _d("2027-04-30")),
        Line("23", _d("2024-10-16"), _d("2025-06-01")),
        Line("24", _d("2025-05-06"), _d("2028-04-30")),
        Line("25", _d("2025-10-15"), _d("2026-06-01")),
        Line("26", _d("2026-05-05"), _d("2029-04-30")),
    ),
)

JAVA_UPSTREAM = Upstream(
    source=(
        "endoflife.date/api/eclipse-temurin.json, cross-checked against "
        "api.adoptium.net/v3/info/available_releases (most_recent_lts 25, "
        "most_recent_feature_release 27); every CI JDK is a Temurin build"
    ),
    policy=(
        "Temurin supports an LTS line (17, 21, 25) for years and a feature release "
        "only until the next one ships six months later."
    ),
    lines=(
        Line("17", _d("2021-09-22"), _d("2027-10-31")),
        Line("18", _d("2022-03-24"), _d("2022-09-30")),
        Line("19", _d("2022-09-26"), _d("2023-03-31")),
        Line("20", _d("2023-03-23"), _d("2023-09-19")),
        Line("21", _d("2023-10-10"), _d("2029-12-31")),
        Line("22", _d("2024-03-20"), _d("2024-09-17")),
        Line("23", _d("2024-09-17"), _d("2025-03-18")),
        Line("24", _d("2025-03-20"), _d("2025-09-22")),
        Line("25", _d("2025-09-22"), _d("2031-09-30")),
        Line("26", _d("2026-03-23"), _d("2026-09-22")),
        Line("27", _d("2026-09-22"), _d("2027-03-23")),
    ),
)

GO_UPSTREAM = Upstream(
    source=(
        "go.dev/doc/devel/release (policy), go.dev/dl/?mode=json (go1.27.4 and "
        "go1.26.11 are the supported stable releases) and endoflife.date/api/go.json"
    ),
    policy=(
        "Go supports a release until there are two newer major releases, so exactly "
        "the two newest minors are supported; the end of one is the next release's date."
    ),
    lines=(
        Line("1.22", _d("2024-02-06"), _d("2025-02-11")),
        Line("1.23", _d("2024-08-13"), _d("2025-08-12")),
        Line("1.24", _d("2025-02-11"), _d("2026-02-10")),
        Line("1.25", _d("2025-08-12"), _d("2026-08-19")),
        Line("1.26", _d("2026-02-10")),
        Line("1.27", _d("2026-08-19")),
    ),
)

RUST_UPSTREAM = Upstream(
    source=(
        "static.rust-lang.org/dist/channel-rust-stable.toml (rustc 1.100.0, channel "
        "dated 2026-11-12) and endoflife.date/api/rust.json"
    ),
    policy=(
        "Rust supports only the current stable release, superseded six weeks later. The "
        "declared MSRV is kept as a row on top: rust-version is a promise Cargo enforces, "
        "and stable is a backward-compatible superset of every minor in between."
    ),
    lines=(
        Line("1.99.0", _d("2026-10-01"), _d("2026-11-12")),
        Line("1.100.0", _d("2026-11-12")),
    ),
)

UPSTREAM = {
    "python": PYTHON_UPSTREAM,
    "node": NODE_UPSTREAM,
    "java": JAVA_UPSTREAM,
    "go": GO_UPSTREAM,
    "rust": RUST_UPSTREAM,
}

# requires-python cannot drop below 3.11 while the parity suites pin pillow==13.0.0 as
# their bit-for-bit oracle: Pillow 13 declares requires-python >=3.11.
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
    upstream: str = ""

    @property
    def newest(self) -> str:
        return self.versions[-1]

    def as_dict(self) -> dict[str, object]:
        return {
            "floor": self.floor,
            "newest": self.newest,
            "source": self.source,
            "grid_rule": self.grid_rule,
            "upstream": self.upstream,
            "versions": self.versions,
            "excluded": self.excluded,
        }


def _read(relative: str) -> str:
    return (REPO_ROOT / relative).read_text(encoding="utf-8")


def _sort_key(version: str) -> tuple[int, ...]:
    return tuple(int(part) for part in version.split("."))


def _utc_today() -> date:
    return datetime.now(timezone.utc).date()


def _cap(versions: list[str]) -> tuple[list[str], list[dict[str, str]]]:
    """Trim a window to MAX_MATRIX_ROWS rows, keeping the oldest and the newest rows."""
    if len(versions) <= MAX_MATRIX_ROWS:
        return versions, []
    kept = [versions[0], *versions[-(MAX_MATRIX_ROWS - 1) :]]
    dropped = [version for version in versions if version not in kept]
    return kept, [
        {
            "version": version,
            "reason": (
                f"dropped by the {MAX_MATRIX_ROWS}-row cap: the oldest supported row and "
                "the newest four are the rows that can fail"
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
    except KeyError as exc:
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
# The intersection of the declared and the upstream window.
# ---------------------------------------------------------------------------
def support_window(
    language: str,
    floor: str,
    source: str,
    upstream: Upstream,
    today: date,
    *,
    floor_row: bool = False,
) -> Matrix:
    """Rows = upstream lines supported on ``today`` at or above the declared ``floor``.

    ``floor_row`` keeps the declared floor as a row even when upstream no longer
    supports it (Rust's MSRV); otherwise a floor older than every tabled line is an
    error, because the table cannot say whether the lines in between are supported.
    """
    lines = sorted(upstream.lines, key=lambda line: _sort_key(line.version))
    floor_key = _sort_key(floor)
    if not floor_row and floor_key < _sort_key(lines[0].version):
        raise ValueError(
            f"{language}: the declared floor {floor} predates every line in the upstream "
            f"table (oldest {lines[0].version}); extend the table from {upstream.source}"
        )
    in_window = [line for line in lines if _sort_key(line.version) >= floor_key]
    supported = [line.version for line in in_window if line.supported_on(today)]
    if not supported:
        raise ValueError(
            f"{language}: no line at or above the declared floor {floor} is supported "
            f"upstream on {today.isoformat()}. {upstream.policy}"
        )
    excluded = [
        {
            "version": line.version,
            "reason": (
                f"not released upstream until {line.released.isoformat()}"
                if line.released > today
                else f"end of life upstream on {line.eol.isoformat()}"
            ),
        }
        for line in in_window
        if not line.supported_on(today)
    ]
    rows = supported
    if floor_row and floor_key < _sort_key(supported[0]):
        rows = [floor, *supported]
    versions, dropped = _cap(rows)
    return Matrix(
        language, floor, source, upstream.policy, versions, excluded + dropped, upstream.source
    )


def python_matrix(today: date) -> Matrix:
    return support_window("python", python_floor(), PYTHON_SOURCE, PYTHON_UPSTREAM, today)


def node_matrix(today: date) -> Matrix:
    return support_window("node", node_floor(), NODE_SOURCE, NODE_UPSTREAM, today)


def java_matrix(today: date) -> Matrix:
    return support_window("java", java_floor(), JAVA_SOURCE, JAVA_UPSTREAM, today)


def go_matrix(today: date) -> Matrix:
    return support_window("go", go_floor(), GO_SOURCE, GO_UPSTREAM, today)


def rust_matrix(today: date) -> Matrix:
    return support_window(
        "rust", rust_floor(), RUST_SOURCE, RUST_UPSTREAM, today, floor_row=True
    )


RESOLVERS = {
    "python": python_matrix,
    "node": node_matrix,
    "java": java_matrix,
    "go": go_matrix,
    "rust": rust_matrix,
}


def resolve(today: date | None = None) -> dict[str, Matrix]:
    """Return every language matrix as of ``today`` (UTC), keyed in declaration order."""
    day = _utc_today() if today is None else today
    return {language: resolver(day) for language, resolver in RESOLVERS.items()}


def stale_tables(today: date) -> str | None:
    """A warning when the upstream tables are older than REVIEW_AFTER, else None."""
    if today - LAST_VERIFIED <= REVIEW_AFTER:
        return None
    return (
        f"the upstream support tables in scripts/ci_versions.py were last verified on "
        f"{LAST_VERIFIED.isoformat()}; lines released since then are not rows until the "
        "tables are re-read from their sources"
    )


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Derive the CI language matrices from the declared and upstream windows."
    )
    parser.add_argument(
        "--format",
        choices=("json", "github", "plain"),
        default="json",
        help="json (default), github ($GITHUB_OUTPUT lines) or plain (one line per language)",
    )
    parser.add_argument(
        "--today",
        type=date.fromisoformat,
        default=None,
        help="resolve as of this ISO date instead of today (UTC)",
    )
    arguments = parser.parse_args(argv)
    today = _utc_today() if arguments.today is None else arguments.today
    matrices = resolve(today)
    warning = stale_tables(today)
    if warning is not None:
        prefix = "::warning title=Stale support tables::" if arguments.format == "github" else "warning: "
        print(prefix + warning, file=sys.stderr)
    if arguments.format == "json":
        print(json.dumps({name: matrix.as_dict() for name, matrix in matrices.items()}, indent=2))
        return 0
    if arguments.format == "github":
        for name, matrix in matrices.items():
            print(f"{name}={json.dumps(matrix.versions)}")
            print(f"{name}-newest={matrix.newest}")
        return 0
    print(f"resolved for {today.isoformat()} (tables verified {LAST_VERIFIED.isoformat()})")
    for name, matrix in matrices.items():
        print(
            f"{name:<7}{' '.join(matrix.versions):<28} floor {matrix.floor}  newest "
            f"{matrix.newest}  ({matrix.source})"
        )
        print(f"        upstream: {matrix.upstream}")
        for entry in matrix.excluded:
            print(f"        excluded {entry['version']}: {entry['reason']}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
