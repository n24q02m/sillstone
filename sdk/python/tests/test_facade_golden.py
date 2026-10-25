"""The committed facade golden vectors are exactly what the Python SDK produces.

Node and Java load `tests/fixtures/facade/golden.json` and must reproduce it, so this
test is what makes the Python SDK the oracle for them: regenerate the file with
`python scripts/ci/gen_facade_golden.py` when an intended Python change moves a vector.
"""

import importlib.util
import json
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]


class FacadeGoldenTest(unittest.TestCase):
    def test_fixture_matches_python_sdk(self):
        spec = importlib.util.spec_from_file_location("gen_facade_golden", ROOT / "scripts" / "ci" / "gen_facade_golden.py")
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        committed = json.loads((ROOT / "tests" / "fixtures" / "facade" / "golden.json").read_text(encoding="utf-8"))
        self.assertEqual(module.build(), committed)


if __name__ == "__main__":
    unittest.main()
