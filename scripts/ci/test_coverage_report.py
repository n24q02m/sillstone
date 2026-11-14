#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright (c) 2026 sillstone Contributors
"""Tests for ``scripts/ci/coverage_report.py``: every report kind, the table and the gate.

Run::

    python -m unittest scripts/ci/test_coverage_report.py -v
"""

from __future__ import annotations

import contextlib
import io
import json
import os
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))

import coverage_report  # noqa: E402  (path bootstrap above)

GO_PROFILE = """mode: set
example.com/m/pkg/a.go:1.1,2.2 3 1
example.com/m/pkg/a.go:3.1,4.2 1 0
example.com/m/pkg/a.go:3.1,4.2 1 1
example.com/m/pkg/b.go:1.1,2.2 2 0

"""

LCOV = """TN:
SF:sdk/node/a.js
DA:1,1
LH:9
LF:10
end_of_record
SF:sdk/node/b.js
LH:1
LF:4
end_of_record
"""

JACOCO = (
    "GROUP,PACKAGE,CLASS,INSTRUCTION_MISSED,INSTRUCTION_COVERED,BRANCH_MISSED,"
    "BRANCH_COVERED,LINE_MISSED,LINE_COVERED,COMPLEXITY_MISSED,COMPLEXITY_COVERED,"
    "METHOD_MISSED,METHOD_COVERED\n"
    "g,io.example,Alpha,0,10,0,0,1,19,0,1,0,1\n"
    "g,io.example,Alpha.Inner,0,10,0,0,0,5,0,1,0,1\n"
)

LLVM = {
    "data": [
        {
            "files": [
                {"filename": "/w/crates/core/src/lib.rs", "summary": {"lines": {"covered": 9, "count": 10}}},
                {"filename": "/w/crates/core/tests/t.rs", "summary": {"lines": {"covered": 0, "count": 50}}},
                {"filename": "/w/tools/gen/src/main.rs", "summary": {"lines": {"covered": 1, "count": 1}}},
            ]
        }
    ]
}

PY = {"files": {"pkg/mod.py": {"summary": {"covered_lines": 19, "num_statements": 20}}}}


class ReportFiles(unittest.TestCase):
    def setUp(self) -> None:
        self._tmp = tempfile.TemporaryDirectory()
        self.root = Path(self._tmp.name)

    def tearDown(self) -> None:
        self._tmp.cleanup()

    def write(self, name: str, text: str) -> Path:
        path = self.root / name
        path.write_text(text, encoding="utf-8")
        return path


class LoaderTest(ReportFiles):
    def test_go_dedupes_blocks_and_counts_statements(self) -> None:
        files = coverage_report.load_go(self.write("go.out", GO_PROFILE))
        self.assertEqual(
            files,
            {"example.com/m/pkg/a.go": (4, 4), "example.com/m/pkg/b.go": (0, 2)},
        )

    def test_lcov_reads_hit_and_found_per_file(self) -> None:
        files = coverage_report.load_lcov(self.write("node.lcov", LCOV))
        self.assertEqual(files, {"sdk/node/a.js": (9, 10), "sdk/node/b.js": (1, 4)})

    def test_jacoco_reads_lines_per_class(self) -> None:
        files = coverage_report.load_jacoco(self.write("jacoco.csv", JACOCO))
        self.assertEqual(files, {"io.example/Alpha": (19, 20), "io.example/Alpha.Inner": (5, 5)})

    def test_llvm_and_py(self) -> None:
        llvm = coverage_report.load_llvm(self.write("rust.json", json.dumps(LLVM)))
        self.assertEqual(llvm["/w/crates/core/src/lib.rs"], (9, 10))
        py = coverage_report.load_py(self.write("py.json", json.dumps(PY)))
        self.assertEqual(py, {"pkg/mod.py": (19, 20)})

    def test_group_keys(self) -> None:
        self.assertEqual(coverage_report.group_key("llvm", "/w/crates/core/src/lib.rs"), "crates/core")
        self.assertEqual(coverage_report.group_key("llvm", "/w/tools/gen/src/main.rs"), "tools/gen")
        self.assertEqual(coverage_report.group_key("lcov", "sdk\\node\\a.js"), "node")
        self.assertEqual(coverage_report.group_key("jacoco", "io.example/Alpha"), "io.example")
        self.assertEqual(coverage_report.group_key("py", "top.py"), "top.py")

    def test_pct_of_an_empty_group_is_full(self) -> None:
        self.assertEqual(coverage_report.pct(0, 0), 100.0)


class MainTest(ReportFiles):
    def run_main(self, *argv: str) -> tuple[int, str, str]:
        out, err = io.StringIO(), io.StringIO()
        with mock.patch.object(sys, "argv", ["coverage_report.py", *argv]):
            with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
                code = coverage_report.main()
        return code, out.getvalue(), err.getvalue()

    def test_every_kind_prints_a_table(self) -> None:
        reports = {
            "go": self.write("go.out", GO_PROFILE),
            "lcov": self.write("node.lcov", LCOV),
            "jacoco": self.write("jacoco.csv", JACOCO),
            "py": self.write("py.json", json.dumps(PY)),
            "llvm": self.write("rust.json", json.dumps(LLVM)),
        }
        expected = {
            "go": "66.67% (4/6)",
            "lcov": "71.43% (10/14)",
            "jacoco": "96.00% (24/25)",
            "py": "95.00% (19/20)",
            # tests/ is not a subject of the measurement, tools/ is.
            "llvm": "90.91% (10/11)",
        }
        for kind, path in reports.items():
            with self.subTest(kind=kind):
                with mock.patch.dict(os.environ, {"GITHUB_STEP_SUMMARY": ""}):
                    code, out, _ = self.run_main(kind, str(path))
                self.assertEqual(code, 0)
                self.assertIn(f"## Line coverage ({kind}): {expected[kind]}", out)

    def test_shortfall_lists_files_and_fails_only_when_asked(self) -> None:
        path = self.write("go.out", GO_PROFILE)
        code, out, _ = self.run_main("go", str(path), "--target", "95")
        self.assertEqual(code, 0)
        self.assertIn("BELOW (>= 95%)", out)
        self.assertIn("0.00% (0/2) example.com/m/pkg/b.go", out)
        code, _, err = self.run_main("go", str(path), "--fail-under", "70")
        self.assertEqual(code, 1)
        self.assertIn("coverage 66.67% is under --fail-under 70%", err)
        code, _, _ = self.run_main("go", str(path), "--fail-under", "60")
        self.assertEqual(code, 0)

    def test_summary_is_appended_for_github(self) -> None:
        summary = self.root / "summary.md"
        path = self.write("node.lcov", LCOV)
        with mock.patch.dict(os.environ, {"GITHUB_STEP_SUMMARY": str(summary)}):
            self.run_main("lcov", str(path))
        self.assertIn("## Line coverage (lcov)", summary.read_text(encoding="utf-8"))

    def test_absolute_paths_are_printed_relative(self) -> None:
        absolute = str(Path.cwd() / "x" / "y.js")
        path = self.write("abs.lcov", f"SF:{absolute}\nLH:0\nLF:2\nend_of_record\n")
        with mock.patch.dict(os.environ, {"GITHUB_STEP_SUMMARY": ""}):
            _, out, _ = self.run_main("lcov", str(path))
        self.assertIn(f"- {0:6.2f}% (0/2) {os.path.join('x', 'y.js')}", out)


if __name__ == "__main__":
    unittest.main()
