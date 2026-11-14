#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright (c) 2026 sillstone Contributors
"""Drift guard for the derived language matrices.

``scripts/ci_versions.py`` derives five support windows: the upstream-supported lines
inside the window this repo declares. A derived list is only worth the guard behind
it: the failure this file exists to stop is a workflow that goes back to a
hand-written pin, a row that keeps testing a line upstream stopped supporting, or a
resolver that quietly disagrees with the file it claims to read.

Four things are asserted:

1. the floors are the declarations (each re-read here with its own deliberately
   dumb parser, so a resolver that guesses cannot agree by accident);
2. every row is an upstream line that is released and not end-of-life today (Rust's
   declared MSRV is the one row allowed outside that), and every supported line in
   the declared window is a row unless the five-row cap says why not;
3. the windows are bounded, ascending, and fully accounted for: every version
   between the floor and the ceiling is either a row or carries a written reason;
4. ``.github/workflows/ci.yml`` gets every list from ``needs.versions.outputs.*``,
   carries no literal pin for these five languages, and any pin that survives
   stays inside the resolved window (a new interpreter in a non-matrix job fails
   here before it silently tests a version the package does not claim).

The resolver's own branches -- the cap, end-of-life and not-yet-released lines, the
error path of every floor reader, the CLI -- are exercised against synthetic tables
and manifests in the unit tests at the bottom.

Run::

    python -m unittest scripts/ci/test_ci_versions.py -v
"""

from __future__ import annotations

import contextlib
import io
import json
import re
import sys
import tempfile
import unittest
from datetime import date, timedelta
from pathlib import Path
from unittest import mock

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
#: plus the current stable toolchain on purpose (see RUST_UPSTREAM.policy): its
#: interim minors are backward-compatible supersets, so they are not enumerated.
FULL_NUMERIC_WINDOW = ("python", "node", "java", "go")


def _key(version: str) -> tuple[int, ...]:
    return tuple(int(part) for part in version.split("."))


def _supported_on(language: str, floor: str, today: date) -> list[str]:
    """Upstream lines at or above ``floor`` released and not end-of-life on ``today``."""
    return sorted(
        (
            line.version
            for line in ci_versions.UPSTREAM[language].lines
            if _key(line.version) >= _key(floor)
            and line.released <= today
            and (line.eol is None or today < line.eol)
        ),
        key=_key,
    )


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
        cls.today = ci_versions._utc_today()
        cls.matrices = ci_versions.resolve(cls.today)

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
                # The first row is the declared floor unless upstream no longer
                # supports it, and then the exclusion says why.
                self.assertGreaterEqual(
                    ci_versions._sort_key(matrix.versions[0]),
                    ci_versions._sort_key(matrix.floor),
                )

    def test_newest_row_is_the_newest_supported_line(self) -> None:
        for language, matrix in self.matrices.items():
            with self.subTest(language=language):
                supported = _supported_on(language, matrix.floor, self.today)
                self.assertEqual(matrix.newest, supported[-1])

    def test_every_row_is_supported_upstream_today(self) -> None:
        for language, matrix in self.matrices.items():
            with self.subTest(language=language):
                supported = _supported_on(language, matrix.floor, self.today)
                rows = list(matrix.versions)
                if language == "rust" and rows[0] == matrix.floor:
                    rows = rows[1:]  # the declared MSRV, kept as a row on purpose
                for version in rows:
                    self.assertIn(
                        version,
                        supported,
                        f"{language} {version} is a row but upstream does not support it "
                        f"on {self.today}",
                    )

    def test_every_supported_line_is_a_row_or_capped(self) -> None:
        for language, matrix in self.matrices.items():
            with self.subTest(language=language):
                capped = {
                    entry["version"]
                    for entry in matrix.excluded
                    if entry["reason"].startswith("dropped by the")
                }
                for version in _supported_on(language, matrix.floor, self.today):
                    self.assertTrue(
                        version in matrix.versions or version in capped,
                        f"{language} {version} is supported upstream but is not a row",
                    )

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

    def test_grid_rule_and_upstream_source_are_written_down(self) -> None:
        for language, matrix in self.matrices.items():
            with self.subTest(language=language):
                self.assertGreater(len(matrix.grid_rule.strip()), 40)
                self.assertGreater(len(matrix.upstream.strip()), 20)


class UpstreamTableTest(unittest.TestCase):
    """The dated tables are internally consistent."""

    def test_lines_are_unique_and_dated_in_order(self) -> None:
        for language, upstream in ci_versions.UPSTREAM.items():
            with self.subTest(language=language):
                versions = [line.version for line in upstream.lines]
                self.assertEqual(len(set(versions)), len(versions))
                self.assertEqual(versions, sorted(versions, key=_key))
                for line in upstream.lines:
                    if line.eol is not None:
                        self.assertLess(line.released, line.eol, f"{language} {line.version}")

    def test_tables_are_not_dated_in_the_future(self) -> None:
        # A line released after the day the tables were read was guessed, not read.
        for language, upstream in ci_versions.UPSTREAM.items():
            for line in upstream.lines:
                with self.subTest(language=language, version=line.version):
                    self.assertLessEqual(line.released, ci_versions.LAST_VERIFIED)


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

    def test_declared_floors_sit_inside_the_upstream_window(self) -> None:
        # A manifest floor below every supported line is a promise CI cannot test:
        # raise the manifest instead. Rust's MSRV is the deliberate exception.
        today = ci_versions._utc_today()
        for language in FULL_NUMERIC_WINDOW:
            with self.subTest(language=language):
                floor = ci_versions.RESOLVERS[language](today).floor
                supported = _supported_on(language, "0", today)
                self.assertGreaterEqual(
                    _key(floor),
                    _key(supported[0]),
                    f"{language} declares {floor} but upstream supports only {supported}",
                )


class WorkflowDriftTest(unittest.TestCase):
    """ci.yml consumes the resolved lists and carries no literal pins."""

    @classmethod
    def setUpClass(cls) -> None:
        cls.text = WORKFLOW_PATH.read_text(encoding="utf-8")
        cls.jobs = _workflow_jobs(cls.text)
        cls.matrices = ci_versions.resolve(ci_versions._utc_today())

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


# ---------------------------------------------------------------------------
# Unit tests of the resolver against synthetic tables and manifests.
# ---------------------------------------------------------------------------
D = date.fromisoformat
SYNTHETIC = ci_versions.Upstream(
    source="synthetic schedule",
    policy="A synthetic upstream used to exercise every branch of the resolver.",
    lines=(
        ci_versions.Line("1.1", D("2020-01-01"), D("2021-01-01")),
        ci_versions.Line("1.2", D("2020-06-01"), D("2030-01-01")),
        ci_versions.Line("1.3", D("2021-01-01"), D("2030-01-01")),
        ci_versions.Line("1.4", D("2021-06-01"), D("2030-01-01")),
        ci_versions.Line("1.5", D("2022-01-01"), D("2030-01-01")),
        ci_versions.Line("1.6", D("2022-06-01"), D("2030-01-01")),
        ci_versions.Line("1.7", D("2023-01-01")),
        ci_versions.Line("1.8", D("2099-01-01")),
    ),
)


def _window(floor: str, today: str, **kwargs: bool) -> ci_versions.Matrix:
    return ci_versions.support_window("demo", floor, "demo manifest", SYNTHETIC, D(today), **kwargs)


class SupportWindowTest(unittest.TestCase):
    def test_line_support_is_bounded_by_release_and_end_of_life(self) -> None:
        line = ci_versions.Line("9.9", D("2024-01-01"), D("2025-01-01"))
        self.assertFalse(line.supported_on(D("2023-12-31")))
        self.assertTrue(line.supported_on(D("2024-01-01")))
        self.assertTrue(line.supported_on(D("2024-12-31")))
        self.assertFalse(line.supported_on(D("2025-01-01")))
        self.assertTrue(ci_versions.Line("9.9", D("2024-01-01")).supported_on(D("2999-01-01")))

    def test_rows_are_the_supported_lines_above_the_floor(self) -> None:
        matrix = _window("1.4", "2024-01-01")
        self.assertEqual(matrix.versions, ["1.4", "1.5", "1.6", "1.7"])
        self.assertEqual(matrix.newest, "1.7")
        self.assertEqual(matrix.upstream, "synthetic schedule")
        self.assertEqual(
            matrix.excluded,
            [{"version": "1.8", "reason": "not released upstream until 2099-01-01"}],
        )

    def test_end_of_life_lines_are_excluded_with_their_date(self) -> None:
        matrix = _window("1.1", "2025-01-01")
        self.assertIn(
            {"version": "1.1", "reason": "end of life upstream on 2021-01-01"}, matrix.excluded
        )
        self.assertNotIn("1.1", matrix.versions)

    def test_a_line_drops_on_its_end_of_life_day(self) -> None:
        self.assertEqual(_window("1.6", "2029-12-31").versions, ["1.6", "1.7"])
        self.assertEqual(_window("1.6", "2030-01-01").versions, ["1.7"])

    def test_cap_keeps_the_oldest_row_and_the_newest_four(self) -> None:
        matrix = _window("1.2", "2024-01-01")
        self.assertEqual(matrix.versions, ["1.2", "1.4", "1.5", "1.6", "1.7"])
        capped = [entry for entry in matrix.excluded if entry["reason"].startswith("dropped by")]
        self.assertEqual([entry["version"] for entry in capped], ["1.3"])

    def test_floor_older_than_the_table_is_an_error(self) -> None:
        with self.assertRaisesRegex(ValueError, "predates every line"):
            _window("1.0", "2024-01-01")

    def test_floor_above_every_supported_line_is_an_error(self) -> None:
        with self.assertRaisesRegex(ValueError, "no line at or above the declared floor 1.8"):
            _window("1.8", "2024-01-01")

    def test_floor_row_keeps_an_unsupported_floor_as_a_row(self) -> None:
        matrix = _window("1.0.5", "2031-01-01", floor_row=True)
        self.assertEqual(matrix.versions, ["1.0.5", "1.7"])
        matrix = _window("1.7", "2031-01-01", floor_row=True)
        self.assertEqual(matrix.versions, ["1.7"])

    def test_rust_keeps_the_declared_msrv_next_to_stable(self) -> None:
        matrix = ci_versions.resolve(ci_versions.LAST_VERIFIED)["rust"]
        self.assertEqual(matrix.versions[0], matrix.floor)
        self.assertEqual(matrix.newest, ci_versions.RUST_UPSTREAM.lines[-1].version)

    def test_resolve_defaults_to_today(self) -> None:
        with mock.patch.object(ci_versions, "_utc_today", return_value=ci_versions.LAST_VERIFIED):
            self.assertEqual(
                ci_versions.resolve(), ci_versions.resolve(ci_versions.LAST_VERIFIED)
            )

    def test_cap_leaves_short_windows_alone(self) -> None:
        self.assertEqual(ci_versions._cap(["1", "2"]), (["1", "2"], []))


class FloorReaderErrorTest(unittest.TestCase):
    """Every floor reader refuses a manifest it cannot read instead of guessing."""

    def _with(self, text: str):
        return mock.patch.object(ci_versions, "_read", return_value=text)

    def test_python(self) -> None:
        with self._with('[project]\nname = "x"\n'):
            with self.assertRaisesRegex(ValueError, "declares no project.requires-python"):
                ci_versions.python_floor()
        with self._with('[project]\nrequires-python = "~=3.12"\n'):
            with self.assertRaisesRegex(ValueError, "declares no >=3.X floor"):
                ci_versions.python_floor()
        with self._with('[project]\nrequires-python = "<4, >=3.12"\n'):
            self.assertEqual(ci_versions.python_floor(), "3.12")

    def test_node(self) -> None:
        with self._with("{}"):
            with self.assertRaisesRegex(ValueError, "declares no engines.node"):
                ci_versions.node_floor()
        with self._with('{"engines": {"node": "^22"}}'):
            with self.assertRaisesRegex(ValueError, "declares no >=N floor"):
                ci_versions.node_floor()

    def test_java(self) -> None:
        pom = '<project xmlns="http://maven.apache.org/POM/4.0.0">{}</project>'
        with self._with(pom.format("")):
            with self.assertRaisesRegex(ValueError, "declares neither"):
                ci_versions.java_floor()
        release = "<properties><maven.compiler.release>21</maven.compiler.release></properties>"
        with self._with(pom.format(release)):
            self.assertEqual(ci_versions.java_floor(), "21")

    def test_go(self) -> None:
        with self._with("module example.com/x\n"):
            with self.assertRaisesRegex(ValueError, "declares no `go` directive"):
                ci_versions.go_floor()

    def test_rust(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "Cargo.toml").write_text("[workspace]\nmembers = []\n", encoding="utf-8")
            with mock.patch.object(ci_versions, "REPO_ROOT", root):
                with self.assertRaisesRegex(ValueError, "no rust-version"):
                    ci_versions.rust_floor()
                (root / "rust-toolchain.toml").write_text("[toolchain]\n", encoding="utf-8")
                with self.assertRaisesRegex(ValueError, "no rust-version"):
                    ci_versions.rust_floor()
                (root / "rust-toolchain.toml").write_text(
                    '[toolchain]\nchannel = "1.90.0"\n', encoding="utf-8"
                )
                self.assertEqual(ci_versions.rust_floor(), "1.90.0")


class CommandLineTest(unittest.TestCase):
    def _run(self, *argv: str) -> tuple[int, str, str]:
        out, err = io.StringIO(), io.StringIO()
        with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
            code = ci_versions.main(list(argv))
        return code, out.getvalue(), err.getvalue()

    def test_json(self) -> None:
        code, out, err = self._run("--today", ci_versions.LAST_VERIFIED.isoformat())
        self.assertEqual((code, err), (0, ""))
        document = json.loads(out)
        self.assertEqual(set(document), set(LANGUAGES))
        self.assertEqual(document["go"]["versions"], ["1.26", "1.27"])

    def test_github(self) -> None:
        code, out, _ = self._run("--format", "github", "--today", "2026-11-14")
        self.assertEqual(code, 0)
        self.assertIn('python=["3.11", "3.12", "3.13", "3.14", "3.15"]', out.splitlines())
        self.assertIn("go-newest=1.27", out.splitlines())

    def test_plain(self) -> None:
        code, out, _ = self._run("--format", "plain", "--today", "2026-11-14")
        self.assertEqual(code, 0)
        self.assertIn("resolved for 2026-11-14", out)
        self.assertIn("excluded 25: end of life upstream on 2026-06-01", out)

    def test_default_day_is_today(self) -> None:
        with mock.patch.object(ci_versions, "_utc_today", return_value=ci_versions.LAST_VERIFIED):
            code, out, err = self._run("--format", "github")
        self.assertEqual((code, err), (0, ""))
        self.assertIn("rust-newest=1.100.0", out.splitlines())

    def test_stale_tables_warn_without_failing(self) -> None:
        late = ci_versions.LAST_VERIFIED + ci_versions.REVIEW_AFTER + timedelta(days=1)
        code, _, err = self._run("--format", "github", "--today", late.isoformat())
        self.assertEqual(code, 0)
        self.assertTrue(err.startswith("::warning title=Stale support tables::"), err)
        code, _, err = self._run("--format", "plain", "--today", late.isoformat())
        self.assertTrue(err.startswith("warning: "), err)
        on_time = ci_versions.LAST_VERIFIED + ci_versions.REVIEW_AFTER
        self.assertIsNone(ci_versions.stale_tables(on_time))


if __name__ == "__main__":
    unittest.main()
