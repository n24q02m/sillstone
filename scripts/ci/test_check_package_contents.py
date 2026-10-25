#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright (c) 2026 sillstone Contributors
"""Offline tests for check-package-contents.py: the gate must read the archive's
own entries, so a package that installs but lacks a platform's native library
has to fail, and a glob that matches nothing must never pass vacuously.

Run: python -m unittest scripts/ci/test_check_package_contents.py
"""
from __future__ import annotations

import importlib.util
import io
import pathlib
import tarfile
import tempfile
import unittest
import zipfile

_spec = importlib.util.spec_from_file_location(
    "check_package_contents", pathlib.Path(__file__).with_name("check-package-contents.py")
)
cpc = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(cpc)

ALL = ["linux-x64", "linux-arm64", "darwin-arm64", "darwin-x64", "win32-x64"]


def make_zip(path: pathlib.Path, names: list[str]) -> pathlib.Path:
    with zipfile.ZipFile(path, "w") as z:
        for n in names:
            z.writestr(n, b"x")
    return path


def make_tgz(path: pathlib.Path, names: list[str]) -> pathlib.Path:
    with tarfile.open(path, "w:gz") as t:
        for n in names:
            info = tarfile.TarInfo(n)
            info.size = 1
            t.addfile(info, io.BytesIO(b"x"))
    return path


class NamesTests(unittest.TestCase):
    def test_cdylib_names(self):
        self.assertEqual(cpc.cdylib_name("win32-x64", "sillstone_ffi"), "sillstone_ffi.dll")
        self.assertEqual(cpc.cdylib_name("darwin-arm64", "sillstone_ffi"), "libsillstone_ffi.dylib")
        self.assertEqual(cpc.cdylib_name("linux-arm64", "sillstone_ffi"), "libsillstone_ffi.so")

    def test_unknown_platform_raises(self):
        with self.assertRaises(ValueError):
            cpc.cdylib_name("freebsd-x64", "sillstone_ffi")
        with self.assertRaises(ValueError):
            cpc.wheel_platform_patterns("freebsd-x64")


class WheelTests(unittest.TestCase):
    def setUp(self):
        self.tmp = pathlib.Path(tempfile.mkdtemp())

    def wheel(self, tag: str, entry: str | None = "sillstone/libsillstone_ffi.so"):
        return make_zip(
            self.tmp / f"sillstone-0.2.0b1-{tag}.whl",
            [entry] if entry else ["sillstone/__init__.py"],
        )

    def test_complete_set_passes(self):
        wheels = [
            self.wheel("py3-none-manylinux2014_x86_64", "sillstone/libsillstone_ffi.so"),
            self.wheel("py3-none-manylinux2014_aarch64", "sillstone/libsillstone_ffi.so"),
            self.wheel("py3-none-macosx_11_0_arm64", "sillstone/libsillstone_ffi.dylib"),
            self.wheel("py3-none-macosx_10_9_x86_64", "sillstone/libsillstone_ffi.dylib"),
            self.wheel("py3-none-win_amd64", "sillstone/sillstone_ffi.dll"),
        ]
        self.assertEqual(cpc.check_wheels(wheels, ALL, "sillstone_ffi"), [])

    def test_missing_platform_fails(self):
        wheels = [self.wheel("py3-none-win_amd64", "sillstone/sillstone_ffi.dll")]
        problems = cpc.check_wheels(wheels, ALL, "sillstone_ffi")
        self.assertTrue(any("linux-x64" in p for p in problems))

    def test_missing_native_library_inside_wheel_fails(self):
        wheels = [self.wheel("py3-none-win_amd64", entry=None)]
        problems = cpc.check_wheels(wheels, ["win32-x64"], "sillstone_ffi")
        self.assertTrue(any("does not contain sillstone_ffi.dll" in p for p in problems))


class JarTests(unittest.TestCase):
    def setUp(self):
        self.tmp = pathlib.Path(tempfile.mkdtemp())

    def test_jar_with_all_platforms_passes(self):
        prefix = "github/sillstone/native"
        entries = [
            f"{prefix}/linux-x64/libsillstone_ffi.so",
            f"{prefix}/linux-arm64/libsillstone_ffi.so",
            f"{prefix}/darwin-arm64/libsillstone_ffi.dylib",
            f"{prefix}/darwin-x64/libsillstone_ffi.dylib",
            f"{prefix}/win32-x64/sillstone_ffi.dll",
        ]
        jar = make_zip(self.tmp / "sillstone-0.2.0-beta.1.jar", entries)
        self.assertEqual(cpc.check_jar(jar, ALL, "sillstone_ffi", prefix), [])

    def test_jar_missing_platform_fails(self):
        prefix = "github/sillstone/native"
        entries = [f"{prefix}/win32-x64/sillstone_ffi.dll"]
        jar = make_zip(self.tmp / "sillstone-0.2.0-beta.1.jar", entries)
        problems = cpc.check_jar(jar, ALL, "sillstone_ffi", prefix)
        self.assertTrue(any("linux-x64" in p for p in problems))


class NpmTests(unittest.TestCase):
    def setUp(self):
        self.tmp = pathlib.Path(tempfile.mkdtemp())

    def test_npm_tgz_with_all_platforms_passes(self):
        entries = [
            f"package/prebuilds/{p}/{n}"
            for p in ALL
            for n in ("sillstone_napi.node", cpc.cdylib_name(p, "sillstone_ffi"))
        ]
        tgz = make_tgz(self.tmp / "sillstone-0.2.0-beta.1.tgz", entries)
        self.assertEqual(cpc.check_npm(tgz, ALL, "sillstone_ffi"), [])

    def test_npm_missing_prebuild_fails(self):
        entries = [
            "package/prebuilds/win32-x64/sillstone_napi.node",
            "package/prebuilds/win32-x64/sillstone_ffi.dll",
        ]
        tgz = make_tgz(self.tmp / "sillstone-0.2.0-beta.1.tgz", entries)
        problems = cpc.check_npm(tgz, ALL, "sillstone_ffi")
        self.assertTrue(any("linux-x64" in p for p in problems))

    def test_npm_cdylib_without_addon_fails(self):
        # The shipped 0.2.1 tarball: raw cdylibs only, nothing require() can load.
        entries = [f"package/prebuilds/{p}/{cpc.cdylib_name(p, 'sillstone_ffi')}" for p in ALL]
        tgz = make_tgz(self.tmp / "sillstone-0.2.1.tgz", entries)
        problems = cpc.check_npm(tgz, ALL, "sillstone_ffi")
        self.assertEqual(len(problems), len(ALL))
        self.assertTrue(all("sillstone_napi.node" in p for p in problems))


if __name__ == "__main__":
    unittest.main()
