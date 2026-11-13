#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright (c) 2026 sillstone Contributors
"""Offline tests for check-registries.py: the checks must read the registry's
own metadata, so an HTTP 200 carrying the wrong content has to fail.

Run: python -m unittest scripts/ci/test_check_registries.py
"""
from __future__ import annotations

import importlib.util
import io
import json
import pathlib
import tarfile
import unittest
from unittest import mock

_spec = importlib.util.spec_from_file_location(
    "check_registries", pathlib.Path(__file__).with_name("check-registries.py")
)
cr = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(cr)


def fake(routes: dict[str, tuple[int, object]]):
    def get(url: str):
        status, body = routes.get(url, (404, b"not found"))
        if not isinstance(body, bytes):
            body = json.dumps(body).encode()
        return status, body

    return get


class CratesTests(unittest.TestCase):
    URL = "https://crates.io/api/v1/crates/sillstone"

    def doc(self, num="0.2.0-beta.1", yanked=False, default="0.2.0-beta.1"):
        return {
            "crate": {"name": "sillstone", "default_version": default},
            "versions": [{"num": num, "yanked": yanked}],
        }

    def test_ok(self):
        self.assertEqual(cr.check_crate("sillstone", "0.2.0-beta.1", fake({self.URL: (200, self.doc())})), [])

    def test_wrong_version_with_http_200_fails(self):
        got = cr.check_crate("sillstone", "0.2.0-beta.1", fake({self.URL: (200, self.doc(num="0.1.0", default="0.1.0"))}))
        self.assertTrue(any("missing" in p for p in got))

    def test_yanked_fails(self):
        got = cr.check_crate("sillstone", "0.2.0-beta.1", fake({self.URL: (200, self.doc(yanked=True))}))
        self.assertTrue(any("yanked" in p for p in got))

    def test_absent_fails(self):
        self.assertTrue(cr.check_crate("sillstone", "0.2.0-beta.1", fake({})))

    def test_prerelease_is_not_required_to_be_default(self):
        doc = self.doc(num="0.3.0-beta.1", default="0.2.1")
        doc["versions"].append({"num": "0.2.1", "yanked": False})
        self.assertEqual(cr.check_crate("sillstone", "0.3.0-beta.1", fake({self.URL: (200, doc)})), [])

    def test_stable_must_be_default(self):
        doc = self.doc(num="0.3.0", default="0.2.1")
        got = cr.check_crate("sillstone", "0.3.0", fake({self.URL: (200, doc)}))
        self.assertTrue(any("default version" in p for p in got))


class PropagationTests(unittest.TestCase):
    def test_waits_until_the_release_appears(self):
        calls = []

        def check(name, version, get):
            calls.append(1)
            return [] if len(calls) >= 3 else [f"version {version} is missing (registry has: none)"]

        patcher = mock.patch.object(cr.time, "sleep")
        patcher.start()
        self.addCleanup(patcher.stop)
        got = cr._check_waiting(check, "x", "1.0.0", None, wait_seconds=60)
        self.assertEqual((got, len(calls)), ([], 3))

    def test_does_not_wait_for_real_defects(self):
        calls = []

        def check(name, version, get):
            calls.append(1)
            return ["version 1.0.0 is yanked"]

        patcher = mock.patch.object(cr.time, "sleep")
        patcher.start()
        self.addCleanup(patcher.stop)
        self.assertEqual(cr._check_waiting(check, "x", "1.0.0", None, wait_seconds=60), ["version 1.0.0 is yanked"])
        self.assertEqual(len(calls), 1)


class PypiTests(unittest.TestCase):
    URL = "https://pypi.org/pypi/sillstone/json"

    def test_ok(self):
        ok = {
            "info": {"name": "sillstone", "version": "0.2.0b1"},
            "releases": {"0.2.0b1": [{"yanked": False, "filename": "sillstone-0.2.0b1-py3-none-any.whl"}]},
        }
        self.assertEqual(cr.check_pypi("sillstone", "0.2.0-beta.1", fake({self.URL: (200, ok)})), [])

    def test_empty_release_fails(self):
        empty = {"info": {"name": "sillstone", "version": "0.2.0b1"}, "releases": {"0.2.0b1": []}}
        self.assertTrue(cr.check_pypi("sillstone", "0.2.0-beta.1", fake({self.URL: (200, empty)})))


class PypiSdistTests(unittest.TestCase):
    """Content mode must notice a release that published wheels only: the sdist
    is the only route on a platform outside the matrix, so it cannot be skipped."""

    URL = "https://pypi.org/pypi/sillstone/json"
    SDIST_URL = "https://files.example/sillstone-0.3.2b1.tar.gz"

    def release(self, files: list[dict]) -> dict:
        return {"info": {"name": "sillstone", "version": "0.3.2b1"}, "releases": {"0.3.2b1": files}}

    def sdist_bytes(self, entries: list[str]) -> bytes:
        buffer = io.BytesIO()
        with tarfile.open(fileobj=buffer, mode="w:gz") as t:
            for name in entries:
                info = tarfile.TarInfo(f"sillstone-0.3.2b1/{name}")
                info.size = 1
                t.addfile(info, io.BytesIO(b"x"))
        return buffer.getvalue()

    def served(self, files: list[dict], blobs: dict[str, bytes] | None = None):
        blobs = blobs or {}

        def get(url: str):
            if url in blobs:
                return 200, blobs[url]
            if url == self.URL:
                return 200, json.dumps(self.release(files)).encode()
            return 404, b""

        return get

    def test_wheels_only_release_fails(self):
        # A yanked wheel keeps the release non-empty while leaving no wheel to
        # inspect, so the sdist requirement is what answers.
        files = [{"packagetype": "bdist_wheel", "yanked": True, "filename": "sillstone-0.3.2b1-py3-none-win_amd64.whl"}]
        problems = cr.check_pypi("sillstone", "0.3.2-beta.1", self.served(files), content=True)
        self.assertTrue(any("no sdist is published" in p for p in problems))

    def test_sdist_without_the_rust_workspace_fails(self):
        files = [
            {
                "packagetype": "sdist",
                "yanked": False,
                "filename": "sillstone-0.3.2b1.tar.gz",
                "url": self.SDIST_URL,
            }
        ]
        blob = self.sdist_bytes(["pyproject.toml", "setup.py", "sillstone/__init__.py"])
        problems = cr.check_pypi("sillstone", "0.3.2-beta.1", self.served(files, {self.SDIST_URL: blob}), content=True)
        self.assertTrue(any("rust/Cargo.toml" in p for p in problems))

    def test_old_releases_are_not_required_to_have_one(self):
        doc = {"info": {"name": "sillstone", "version": "0.2.0b1"}, "releases": {"0.2.0b1": []}}
        problems = cr.check_pypi("sillstone", "0.2.0-beta.1", fake({self.URL: (200, doc)}), content=True)
        self.assertFalse(any("sdist" in p for p in problems))

    def test_the_already_published_0_3_1_release_is_exempt(self):
        # 0.3.1-beta.1 went out as wheels only and PyPI freezes a release's
        # files, so failing it here would fail a release that cannot change.
        doc = {"info": {"name": "sillstone", "version": "0.3.1b1"}, "releases": {"0.3.1b1": [
            {"packagetype": "bdist_wheel", "yanked": True, "filename": "sillstone-0.3.1b1-py3-none-win_amd64.whl"}
        ]}}
        problems = cr.check_pypi("sillstone", "0.3.1-beta.1", fake({self.URL: (200, doc)}), content=True)
        self.assertFalse(any("no sdist is published" in p for p in problems))


class NpmTests(unittest.TestCase):
    URL = "https://registry.npmjs.org/sillstone"

    def doc(self, version="0.2.0-beta.1", latest="0.2.0-beta.1"):
        return {
            "name": "sillstone",
            "dist-tags": {"latest": latest},
            "versions": {
                version: {
                    "name": "sillstone",
                    "version": version,
                    "dist": {
                        "tarball": f"https://registry.npmjs.org/sillstone/-/sillstone-{version}.tgz",
                        "integrity": "sha512-mockintegrity==",
                        "shasum": "0123456789abcdef0123456789abcdef01234567",
                    },
                }
            },
        }

    def test_ok(self):
        self.assertEqual(cr.check_npm("sillstone", "0.2.0-beta.1", fake({self.URL: (200, self.doc())})), [])

    def test_missing_version_fails(self):
        got = cr.check_npm("sillstone", "0.2.0-beta.1", fake({self.URL: (200, self.doc(version="0.1.0", latest="0.1.0"))}))
        self.assertTrue(any("missing" in p for p in got))


class JarPrefixTests(unittest.TestCase):
    """The gate, the packaging check and the Java loader must agree on where the
    native prebuilds live inside the jar; a drifting prefix lets a jar without
    the native library pass every check."""

    def test_prefix_matches_loader_and_packaging(self):
        root = pathlib.Path(__file__).resolve().parents[2]
        loader = (root / "sdk/java/src/main/java/io/github/sillstone/SillstoneNative.java").read_text()
        self.assertIn(f'"/{cr.JAR_PREFIX}/"', loader)
        workflow = (root / ".github/workflows/cd.yml").read_text()
        self.assertIn(f"--prefix {cr.JAR_PREFIX}", workflow)


if __name__ == "__main__":
    unittest.main()
