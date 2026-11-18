"""Tests for the --check mode of the Pillow and NumPy vector generators, which CI runs.

The Rust parity suites replay committed vectors. `--check` regenerates them with the
installed oracle and names every case the committed file no longer agrees with.
"""

from __future__ import annotations

import importlib.util
import io
import json
import pathlib
import shutil
import tempfile
import unittest
import warnings
from contextlib import redirect_stderr, redirect_stdout
from unittest import mock

ROOT = pathlib.Path(__file__).resolve().parents[2]

# The packaging job discovers every test in this directory without the oracle
# installed. The coverage job installs it and holds both generators to the target,
# so a skip there fails the coverage gate instead of passing silently.
_MISSING = [name for name in ("numpy", "PIL") if importlib.util.find_spec(name) is None]
if _MISSING:
    raise unittest.SkipTest(f"the parity oracle is not installed (missing {', '.join(_MISSING)})")


def _load(name: str, relative: str):
    spec = importlib.util.spec_from_file_location(name, ROOT / relative)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


pillow = _load("gen_pillow_golden", "scripts/golden/gen_pillow_golden.py")
numpy_oracle = _load("numpy_oracle_generate", "crates/tensor/tests/oracle/generate.py")


def _run(module, argv: list[str]) -> tuple[int, str, str]:
    out, err = io.StringIO(), io.StringIO()
    with redirect_stdout(out), redirect_stderr(err):
        code = module.main(argv)
    return code, out.getvalue(), err.getvalue()


class _Fixture(unittest.TestCase):
    """Writes the generator's vectors to a temporary file and points the generator at it."""

    module = None

    def setUp(self):
        # Some oracle cases overflow on purpose (exp of 700 in float32, 7e4 in float16),
        # and NumPy warns about each; only what the generator itself reports is asserted.
        self.enterContext(warnings.catch_warnings())
        warnings.simplefilter("ignore", RuntimeWarning)
        self.dir = pathlib.Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, self.dir)
        self.path = self.dir / "vectors"
        patcher = mock.patch.object(self.module, "OUT", self.path)
        patcher.start()
        self.addCleanup(patcher.stop)
        code, out, _ = _run(self.module, [])
        self.assertEqual(code, 0)
        self.assertIn("wrote", out)


class PillowGoldenCheck(_Fixture):
    module = pillow

    def lines(self) -> list[dict]:
        return [json.loads(line) for line in self.path.read_text(encoding="utf-8").splitlines()]

    def write(self, records: list[dict]) -> None:
        self.path.write_text("".join(json.dumps(r) + "\n" for r in records), encoding="utf-8")

    def test_a_fresh_rendering_matches_the_oracle(self):
        self.assertEqual(pillow.check(self.path), [])
        code, out, err = _run(pillow, ["--check"])
        self.assertEqual((code, err), (0, ""))
        self.assertIn("matches Pillow ", out)

    def test_a_changed_expectation_is_named(self):
        records = self.lines()
        case = records[5]
        case["expected"] = ("0" if case["expected"][0] != "0" else "1") + case["expected"][1:]
        self.write(records)
        self.assertEqual(pillow.check(self.path), [f"case {case['id']} differs from the oracle in expected"])
        code, out, err = _run(pillow, ["--check"])
        self.assertEqual((code, out), (1, ""))
        self.assertIn(case["id"], err)
        self.assertIn("is stale against Pillow ", err)

    def test_missing_and_unexpected_cases_are_named(self):
        records = self.lines()
        dropped = records.pop(3)
        records.append(dict(dropped, id="resize/L/made-up"))
        self.write(records)
        self.assertEqual(pillow.check(self.path),
                         [f"missing case {dropped['id']}", "unexpected case resize/L/made-up"])

    def test_reordered_cases_are_reported(self):
        records = self.lines()
        records[1], records[2] = records[2], records[1]
        self.write(records)
        self.assertEqual(pillow.check(self.path), ["the cases are repeated or not in the generator's order"])

    def test_the_provenance_line_is_required_but_not_compared(self):
        # The provenance records what generated the file; the vectors are what the
        # parity suite replays, so only they have to agree with the installed oracle.
        records = self.lines()
        records[0]["_provenance"].update(numpy="0.0.0", python="0.0.0")
        self.write(records)
        self.assertEqual(pillow.check(self.path), [])

        self.write(records[1:])
        self.assertEqual(pillow.check(self.path)[0], "the first line is not the _provenance line")


class NumpyOracleCheck(_Fixture):
    module = numpy_oracle

    def payload(self) -> dict:
        return json.loads(self.path.read_text(encoding="utf-8"))

    def write(self, payload: dict) -> None:
        self.path.write_text(json.dumps(payload, indent=1), encoding="utf-8")

    def test_a_fresh_rendering_matches_the_oracle(self):
        self.assertEqual(numpy_oracle.check(self.path), [])
        code, out, err = _run(numpy_oracle, ["--check"])
        self.assertEqual((code, err), (0, ""))
        self.assertIn("matches numpy ", out)

    def test_a_changed_element_is_named(self):
        payload = self.payload()
        payload["cases"][0]["expect"][3] += 1e-12
        self.write(payload)
        name = payload["cases"][0]["name"]
        self.assertEqual(numpy_oracle.check(self.path), [f"case {name} differs from the oracle in expect"])
        code, out, err = _run(numpy_oracle, ["--check"])
        self.assertEqual((code, out), (1, ""))
        self.assertIn(name, err)
        self.assertIn("is stale against numpy ", err)

    def test_the_sign_of_zero_is_compared(self):
        # -0.0 == 0.0 in Python, so a comparison of parsed values would let a sign flip through.
        payload = self.payload()
        case = next(c for c in payload["cases"] if c["name"] == "sin_f64")
        self.assertEqual(json.dumps(case["expect"][0]), "0.0")
        case["expect"][0] = -0.0
        self.write(payload)
        self.assertEqual(numpy_oracle.check(self.path), ["case sin_f64 differs from the oracle in expect"])

    def test_missing_and_unexpected_cases_are_named(self):
        payload = self.payload()
        dropped = payload["cases"].pop(0)
        payload["cases"].append(dict(dropped, name="made_up"))
        self.write(payload)
        self.assertEqual(numpy_oracle.check(self.path),
                         [f"missing case {dropped['name']}", "unexpected case made_up"])

    def test_repeated_cases_are_reported(self):
        payload = self.payload()
        payload["cases"].append(payload["cases"][0])
        self.write(payload)
        self.assertEqual(numpy_oracle.check(self.path), ["the cases are repeated or not in the generator's order"])

    def test_the_numpy_version_is_not_compared(self):
        payload = self.payload()
        payload["numpy"] = "0.0.0"
        self.write(payload)
        self.assertEqual(numpy_oracle.check(self.path), [])


if __name__ == "__main__":
    unittest.main()
