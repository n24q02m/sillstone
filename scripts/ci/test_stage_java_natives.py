#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright (c) 2026 sillstone Contributors
"""Offline tests for stage-java-natives.py.

Run: python -m unittest scripts/ci/test_stage_java_natives.py
"""
from __future__ import annotations

import importlib.util
import pathlib
import tempfile
import unittest
import zipfile

_spec = importlib.util.spec_from_file_location(
    "stage_java_natives", pathlib.Path(__file__).with_name("stage-java-natives.py")
)
sj = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(sj)


def make_jar(path: pathlib.Path, entries: dict[str, bytes]) -> pathlib.Path:
    with zipfile.ZipFile(path, "w") as zf:
        for name, body in entries.items():
            zf.writestr(name, body)
    return path


class StageTests(unittest.TestCase):
    def test_stages_every_platform(self):
        with tempfile.TemporaryDirectory() as tmp:
            tmp = pathlib.Path(tmp)
            jar = make_jar(
                tmp / "sillstone-0.3.2.jar",
                {f"{sj.PREFIX}/{plat}/{lib}": b"native" for plat, lib in sj.PLATFORM_LIBS.items()}
                | {"io/github/sillstone/Main.class": b"class"},
            )
            self.assertEqual(sj.missing_entries(jar), [])
            written = sj.stage(jar, tmp / "resources")
            self.assertEqual(len(written), len(sj.PLATFORM_LIBS))
            self.assertTrue((tmp / "resources" / sj.PREFIX / "win32-x64" / "sillstone_ffi.dll").is_file())

    def test_missing_prebuild_is_reported(self):
        with tempfile.TemporaryDirectory() as tmp:
            tmp = pathlib.Path(tmp)
            jar = make_jar(tmp / "sillstone-0.3.2.jar", {f"{sj.PREFIX}/linux-x64/libsillstone_ffi.so": b"native"})
            self.assertEqual(
                sorted(sj.missing_entries(jar)),
                sorted(f"{sj.PREFIX}/{p}/{lib}" for p, lib in sj.PLATFORM_LIBS.items() if p != "linux-x64"),
            )

    def test_classifier_jars_are_ignored(self):
        with tempfile.TemporaryDirectory() as tmp:
            tmp = pathlib.Path(tmp)
            make_jar(tmp / "sillstone-0.3.2-sources.jar", {"a": b""})
            main = make_jar(tmp / "sillstone-0.3.2.jar", {"a": b""})
            self.assertEqual(sj.packaged_jar(tmp), main)


if __name__ == "__main__":
    unittest.main()
