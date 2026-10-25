#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright (c) 2026 sillstone Contributors
"""Offline tests for check-registries.py: the checks must read the registry's
own metadata, so an HTTP 200 carrying the wrong content has to fail.

Run: python -m unittest scripts/ci/test_check_registries.py
"""
from __future__ import annotations

import importlib.util
import json
import pathlib
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


if __name__ == "__main__":
    unittest.main()
