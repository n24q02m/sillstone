"""Tests for scripts/bench/bench_hash_parity.py, which the CI bench job runs."""

from __future__ import annotations

import importlib.util
import pathlib
import sys
import unittest
from unittest import mock

_spec = importlib.util.spec_from_file_location(
    "bench_hash_parity",
    pathlib.Path(__file__).resolve().parents[1] / "bench" / "bench_hash_parity.py",
)
bench = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(bench)


class MissingOracle(unittest.TestCase):
    def test_the_image_hash_section_needs_its_oracle(self):
        # An empty section would make main() count the image hashes as matching
        # imagehash although nothing was compared.
        with mock.patch.dict(sys.modules, {"imagehash": None}):
            with self.assertRaises(ImportError):
                bench.bench_image_hashes([(16, 16)])


if __name__ == "__main__":
    unittest.main()
