#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright (c) 2026 sillstone Contributors
"""Drift guard for the derived language matrices.

``scripts/ci_versions.py`` derives five support windows from the versions this repo
declares. A derived list is only worth the guard behind it: the failure this file
exists to stop is a workflow that goes back to a hand-written pin -- the exact shape
that let the old matrix fall behind a floor -- or a resolver that quietly disagrees
with the file it claims to read.

Three things are asserted:

1. the floors are the declarations (each re-read here with its own deliberately
   dumb parser, so a resolver that guesses cannot agree by accident);
2. the windows are bounded, ascending, and fully accounted for: every version
   between the floor and the ceiling is either a row or carries a written reason;
3. ``.github/workflows/ci.yml`` gets every list from ``needs.versions.outputs.*``,
   carries no literal pin for these five languages, and any pin that survives
   stays inside the resolved window (a new interpreter in a non-matrix job fails
   here before it silently tests a version the package does not claim).

Run::

    python -m unittest scripts/ci/test_ci_versions.py -v
"""

from __future__ import annotations

import json
import re
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import ci_versions  # noqa: E402  (path bootstrap above, resolver lives in scripts/)

REPO_ROOT = ci_versions.REPO_ROOT
WORKFLOW_PATH = REPO_ROOT / ".github/workflows/ci.yml"

LANGUAGES = tuple(ci_versions.RESOLVERS)

#: Job id that carries each language's matrix in ci.yml.
MATRIX_JOB = {language: f"{language}-matrix" for language in LANGUAGES}

#: A hand-written pin, in the shape the setup-* actions accept.
PIN_PATTERN = {
    "python": re.compile(r"python-version:\s*['\"]?(3\.\d+)"),
    "node": re.compile(r"node-version:\s*['\"]?(\d+)"),
    "java": re.compile(r"java-version:\s*['\"]?(\d+)"),
    "go": re.compile(r"go-version:\s*['\"]?(\d+\.\d+)"),
    # Channel aliases (stable, beta, nightly) are not pins; a numeric toolchain is.
    "rust": re.compile(r"(?:rust-toolchain@|toolchain:\s*['\"]?)(\d+\.\d+(?:\.\d+)?)"),
}

#: Languages whose support window is "every minor in the range", so the accounting
#: check below can demand a row or a reason for each one. Rust's grid is the MSRV
#: plus the current stable toolchain on purpose (see RUST_GRID_RULE): its interim
#: minors are backward-compatible supersets, so they are not enumerated.
FULL_NUMERIC_WINDOW = ("python", "node", "java", "go")


def _workflow_jobs(text: str) -> dict[str, str]:
    """Split the ``jobs:`` block of a workflow into ``{job id: block text}``."""
    jobs: dict[str, list[str]] = {}
    in_jobs = False
    current: str | None = None
    for line in text.splitlines():
        if not in_jobs:
            if re.fullmatch(r"jobs:\s*", line):
                in_jobs = True
            continue
        if line and not line[0].isspace():
            break  # a new top-level key closes the jobs block
        header = re.fullmatch(r"  ([A-Za-z0-9_-]+):\s*", line)
        if header:
            current = header.group(1)
            jobs[current] = []
            continue
        if current is not None:
            jobs[current].append(line)
    return {job: "\n".join(lines) for job, lines in jobs.items()}


def _inside_window(versions: list[str], pin: str) -> bool:
    """True when ``pin`` names one of ``versions`` (``1.85`` matching ``1.85.0``)."""
    return any(version == pin or version.startswith(f"{pin}.") for version in versions)


class ResolvedWindowsTest(unittest.TestCase):
    """Properties every derived window must hold, whatever the declarations say."""

    @classmethod
    def setUpClass(cls) -> None:
        cls.matrices = ci_versions.resolve()

    def test_every_language_resolves(self) -> None:
        self.assertEqual(set(self.matrices), {"python", "node", "java", "go", "rust"})

    def test_rows_stay_within_the_cap(self) -> None:
        for language, matrix in self.matrices.items():
            with self.subTest(language=language):
                self.assertGreater(len(matrix.versions), 0)
                self.assertLessEqual(len(matrix.versions), ci_versions.MAX_MATRIX_ROWS)

    def test_rows_ascend_from_the_declared_floor(self) -> None:
        for language, matrix in self.matrices.items():
            with self.subTest(language=language):
                keys = [ci_versions._sort_key(version) for version in matrix.versions]
                self.assertEqual(keys, sorted(keys), f"{language} rows are not ascending")
                self.assertEqual(len(set(matrix.versions)), len(matrix.versions))
                # The first row is the declared floor unless the floor row is
                # impossible to run, and then the exclusion says why.
                self.assertGreaterEqual(
                    ci_versions._sort_key(matrix.versions[0]),
                    ci_versions._sort_key(matrix.floor),
                )

    def test_newest_row_is_the_ceiling(self) -> None:
        expected = {
            "python": f"3.{ci_versions.NEWEST_CPYTHON_MINOR}",
            "node": str(max(ci_versions.NODE_LTS_MAJORS)),
            "java": str(max(ci_versions.JAVA_LTS_MAJORS)),
            "go": f"1.{ci_versions.NEWEST_GO_MINOR}",
            "rust": ci_versions.NEWEST_STABLE_RUST,
        }
        for language, matrix in self.matrices.items():
            with self.subTest(language=language):
                self.assertEqual(matrix.newest, expected[language])

    def test_every_skipped_version_has_a_reason(self) -> None:
        for language, matrix in self.matrices.items():
            with self.subTest(language=language):
                resolved = set(matrix.versions)
                excluded = {entry["version"] for entry in matrix.excluded}
                for entry in matrix.excluded:
                    self.assertTrue(entry["reason"].strip(), f"{language} exclusion has no reason")
                    self.assertNotIn(entry["version"], resolved)
                # The floor must never leave the matrix silently: it is the promise.
                if matrix.floor not in resolved:
                    self.assertIn(matrix.floor, excluded)
                if language not in FULL_NUMERIC_WINDOW:
                    continue
                # Full accounting: every step of the window is a row or explained.
                # Python and Go step by minor, Node and Java by major -- in both
                # shapes the moving component is the last one.
                first = ci_versions._sort_key(matrix.floor)[-1]
                last = ci_versions._sort_key(matrix.newest)[-1]
                covered = {ci_versions._sort_key(version)[-1] for version in resolved}
                explained = {ci_versions._sort_key(version)[-1] for version in excluded}
                for step in range(first, last + 1):
                    self.assertTrue(
                        step in covered or step in explained,
                        f"{language} {step} is neither a row nor an explained exclusion",
                    )

    def test_grid_rule_is_written_down(self) -> None:
        for language, matrix in self.matrices.items():
            with self.subTest(language=language):
                self.assertGreater(len(matrix.grid_rule.strip()), 40)


class DeclaredFloorTest(unittest.TestCase):
    """Each floor is re-read from its declaring file with an independent parser."""

    def test_python_floor_is_requires_python(self) -> None:
        text = (REPO_ROOT / "sdk/python/pyproject.toml").read_text(encoding="utf-8")
        match = re.search(r'requires-python\s*=\s*">=3\.(\d+)"', text)
        self.assertIsNotNone(match, "pyproject.toml no longer declares a simple >=3.X floor")
        self.assertEqual(f"3.{match.group(1)}", ci_versions.python_floor())

    def test_node_floor_is_engines_node(self) -> None:
        document = json.loads((REPO_ROOT / "sdk/node/package.json").read_text(encoding="utf-8"))
        self.assertIn("engines", document, "sdk/node/package.json must declare engines.node")
        match = re.fullmatch(r">=\s*(\d+)", document["engines"]["node"].strip())
        self.assertIsNotNone(match, "engines.node is no longer a simple >=N floor")
        self.assertEqual(match.group(1), ci_versions.node_floor())

    def test_java_floor_is_the_compiler_target(self) -> None:
        text = (REPO_ROOT / "sdk/java/pom.xml").read_text(encoding="utf-8")
        match = re.search(r"<maven\.compiler\.(?:release|source)>\s*(\d+)\s*</", text)
        self.assertIsNotNone(match, "pom.xml no longer declares maven.compiler.release|source")
        self.assertEqual(match.group(1), ci_versions.java_floor())

    def test_go_floor_is_the_go_directive(self) -> None:
        text = (REPO_ROOT / "sdk/go/go.mod").read_text(encoding="utf-8")
        match = re.search(r"^go\s+(\d+\.\d+)$", text, re.MULTILINE)
        self.assertIsNotNone(match, "sdk/go/go.mod no longer declares a `go` directive")
        self.assertEqual(match.group(1), ci_versions.go_floor())

    def test_rust_floor_is_the_workspace_rust_version(self) -> None:
        text = (REPO_ROOT / "Cargo.toml").read_text(encoding="utf-8")
        match = re.search(r'^rust-version\s*=\s*"([\d.]+)"', text, re.MULTILINE)
        self.assertIsNotNone(match, "Cargo.toml no longer declares [workspace.package] rust-version")
        self.assertEqual(match.group(1), ci_versions.rust_floor())


class WorkflowDriftTest(unittest.TestCase):
    """ci.yml consumes the resolved lists and carries no literal pins."""

    @classmethod
    def setUpClass(cls) -> None:
        cls.text = WORKFLOW_PATH.read_text(encoding="utf-8")
        cls.jobs = _workflow_jobs(cls.text)
        cls.matrices = ci_versions.resolve()

    def test_versions_job_publishes_every_output(self) -> None:
        self.assertIn("versions", self.jobs, "ci.yml has no `versions` job")
        block = self.jobs["versions"]
        self.assertIn(
            "scripts/ci_versions.py --format github",
            block,
            "the versions job must resolve the matrices with scripts/ci_versions.py",
        )
        # The resolver must run on the runner's stock interpreter: an
        # actions/setup-python *step* here would let a version typed in the
        # workflow decide what the workflow claims about versions. (The comment
        # above the step names the action on purpose; only `uses:` counts.)
        self.assertIsNone(
            re.search(r"(?m)^\s*uses:\s*actions/setup-python", block),
            "the versions job must not install an interpreter; the resolver is stdlib-only",
        )
        for language, matrix in self.matrices.items():
            with self.subTest(language=language):
                self.assertIn(f"{language}: ${{{{ steps.resolve.outputs.{language} }}}}", block)
                self.assertIn(
                    f"{language}-newest: ${{{{ steps.resolve.outputs.{language}-newest }}}}",
                    block,
                )

    def test_guard_is_wired_into_the_versions_job(self) -> None:
        self.assertIn(
            "scripts/ci/test_ci_versions.py",
            self.jobs["versions"],
            "the drift guard must run in CI, not only on a developer's machine",
        )

    def test_matrix_jobs_consume_the_resolved_lists(self) -> None:
        for language in LANGUAGES:
            job = MATRIX_JOB[language]
            with self.subTest(language=language):
                self.assertIn(job, self.jobs, f"ci.yml has no `{job}` job")
                block = self.jobs[job]
                self.assertRegex(block, r"(?m)^\s+needs:.*\bversions\b")
                self.assertIn(
                    f"fromJSON(needs.versions.outputs.{language})",
                    block,
                    f"{job} must read its versions from the resolver",
                )
                self.assertRegex(
                    block,
                    r"(?m)^\s+fail-fast:\s*false\s*$",
                    f"{job} must not cancel the other rows when one row fails",
                )

    def test_matrix_jobs_carry_no_literal_pin(self) -> None:
        for language, job in MATRIX_JOB.items():
            block = self.jobs[job]
            for match in PIN_PATTERN[language].finditer(block):
                self.fail(
                    f"{job} pins {language} {match.group(1)} by hand; the row must come from "
                    f"needs.versions.outputs.{language}"
                )

    def test_surviving_pins_stay_inside_the_declared_window(self) -> None:
        for language, pattern in PIN_PATTERN.items():
            versions = self.matrices[language].versions
            for match in pattern.finditer(self.text):
                pin = match.group(1)
                with self.subTest(language=language, pin=pin):
                    self.assertTrue(
                        _inside_window(versions, pin),
                        f"ci.yml pins {language} {pin}, which is outside the resolved window "
                        f"{versions}; raise the floor/ceiling in scripts/ci_versions.py or point "
                        "the step at needs.versions.outputs.*",
                    )


if __name__ == "__main__":
    unittest.main()
