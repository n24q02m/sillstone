"""Tests for scripts/ci/check-forbidden-traces.py."""

from __future__ import annotations

import importlib.util
import pathlib
import unittest
from unittest import mock

_spec = importlib.util.spec_from_file_location(
    "check_forbidden_traces", pathlib.Path(__file__).with_name("check-forbidden-traces.py")
)
ft = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(ft)

ROOT = pathlib.Path(__file__).resolve().parents[2]


class UnreadableFile(unittest.TestCase):
    def test_a_file_that_cannot_be_read_fails_the_scan(self):
        # The walk lists a file the scan cannot open. Reporting PASS here would claim
        # a tree is clean without having read all of it.
        listing = [(str(ROOT), [], ["listed-but-missing-for-the-trace-scan"])]
        with mock.patch.object(ft.os, "walk", return_value=listing):
            with self.assertRaises(OSError):
                ft.main()


if __name__ == "__main__":
    unittest.main()
