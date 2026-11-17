#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright (c) 2026 sillstone Contributors
"""Offline tests for version_sync.py, the stamp step of the release.

Run: python -m unittest scripts/ci/test_version_sync.py
"""
from __future__ import annotations

import contextlib
import importlib.util
import io
import pathlib
import tempfile
import unittest
from unittest import mock

_spec = importlib.util.spec_from_file_location(
    "version_sync", pathlib.Path(__file__).with_name("version_sync.py")
)
vs = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(vs)

# One file per kind of version line version_sync.py rewrites, as the repository
# lays them out. There is deliberately no .github/workflows/cd.yml: the release
# version comes from better-semantic-release, not from a workflow input default.
MANIFESTS = {
    "Cargo.toml": '[workspace.package]\nversion = "0.4.0"\nedition = "2024"\n',
    "crates/buffer/Cargo.toml": '[package]\nname = "sillstone-buffer"\nversion.workspace = true\n',
    "crates/ffi/Cargo.toml": (
        '[package]\nname = "sillstone-ffi"\nversion.workspace = true\n\n'
        '[dependencies]\nsillstone-buffer = { version = "0.4.0", path = "../buffer" }\n'
    ),
    "sdk/node/package.json": '{\n  "name": "sillstone",\n  "version": "0.4.0",\n  "main": "index.js"\n}\n',
    "sdk/java/pom.xml": "<project>\n    <version>0.4.0</version>\n    <dep><version>2.15.0</version></dep>\n</project>\n",
    "sdk/python/pyproject.toml": '[project]\nname = "sillstone"\nversion = "0.4.0"\n',
    "sdk/python/sillstone/__init__.py": '__version__ = "0.4.0"\n',
    "scripts/ci/check-registries.py": 'EXPECTED_VERSION = "0.4.0"\n',
}


def make_tree(root: pathlib.Path) -> None:
    for rel, body in MANIFESTS.items():
        path = root / rel
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(body, encoding="utf-8")


def run(*args: str) -> tuple[int, str]:
    out = io.StringIO()
    with contextlib.redirect_stdout(out):
        code = vs.main(["version_sync.py", *args])
    return code, out.getvalue()


class VersionSyncTests(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.root = pathlib.Path(tmp.name)
        make_tree(self.root)
        patcher = mock.patch.object(vs, "ROOT", self.root)
        patcher.start()
        self.addCleanup(patcher.stop)

    def read(self, rel: str) -> str:
        return (self.root / rel).read_text(encoding="utf-8")

    def test_set_stamps_every_manifest_without_a_workflow_default(self):
        self.assertEqual(run("set", "0.4.1-beta.1"), (0, ""))
        self.assertIn('version = "0.4.1-beta.1"', self.read("Cargo.toml"))
        self.assertIn('sillstone-buffer = { version = "0.4.1-beta.1", path = "../buffer" }', self.read("crates/ffi/Cargo.toml"))
        self.assertIn('"version": "0.4.1-beta.1"', self.read("sdk/node/package.json"))
        self.assertIn("<version>0.4.1-beta.1</version>", self.read("sdk/java/pom.xml"))
        self.assertIn("<version>2.15.0</version>", self.read("sdk/java/pom.xml"))
        self.assertIn('version = "0.4.1b1"', self.read("sdk/python/pyproject.toml"))
        self.assertIn('__version__ = "0.4.1b1"', self.read("sdk/python/sillstone/__init__.py"))
        self.assertIn('EXPECTED_VERSION = "0.4.1-beta.1"', self.read("scripts/ci/check-registries.py"))
        self.assertEqual(run("check", "0.4.1-beta.1"), (0, "all manifests carry 0.4.1-beta.1\n"))

    def test_check_names_a_manifest_the_stamp_missed(self):
        self.assertEqual(run("set", "0.4.1")[0], 0)
        (self.root / "sdk/node/package.json").write_text(MANIFESTS["sdk/node/package.json"], encoding="utf-8")
        self.assertEqual(run("check", "0.4.1"), (1, "version 0.4.1 missing/mixed in: sdk/node/package.json\n"))


class RepositoryTargetsTests(unittest.TestCase):
    def test_targets_are_the_repository_manifests_and_not_the_release_workflow(self):
        current = vs.current_version()
        paths = [path for path, _, _ in vs.targets(current, current)]
        missing = [p.relative_to(vs.ROOT).as_posix() for p in paths if not p.is_file()]
        self.assertEqual(missing, [])
        workflows = [p.relative_to(vs.ROOT).as_posix() for p in paths if ".github" in p.relative_to(vs.ROOT).parts]
        self.assertEqual(workflows, [])


if __name__ == "__main__":
    unittest.main()
