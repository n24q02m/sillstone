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


def make_tgz(path: pathlib.Path, names: list[str], contents: dict[str, bytes] | None = None) -> pathlib.Path:
    payloads = contents or {}
    with tarfile.open(path, "w:gz") as t:
        for n in names:
            payload = payloads.get(n, b"x")
            info = tarfile.TarInfo(n)
            info.size = len(payload)
            t.addfile(info, io.BytesIO(payload))
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
        ] + list(cpc.NPM_DOCS)
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
        tgz = make_tgz(self.tmp / "sillstone-0.2.1.tgz", entries + list(cpc.NPM_DOCS))
        problems = cpc.check_npm(tgz, ALL, "sillstone_ffi")
        self.assertEqual(len(problems), len(ALL))
        self.assertTrue(all("sillstone_napi.node" in p for p in problems))

    def _prebuilds(self):
        return [
            f"package/prebuilds/{p}/{n}"
            for p in ALL
            for n in ("sillstone_napi.node", cpc.cdylib_name(p, "sillstone_ffi"))
        ]

    def test_npm_without_readme_and_license_fails(self):
        # The shipped 0.3.0 tarball: prebuilds only, `files` named two missing docs.
        tgz = make_tgz(self.tmp / "sillstone-0.3.0.tgz", self._prebuilds())
        problems = cpc.check_npm(tgz, ALL, "sillstone_ffi")
        self.assertEqual(len(problems), 2)
        self.assertTrue(any("README.md" in p for p in problems))
        self.assertTrue(any("LICENSE" in p for p in problems))

    def test_npm_docs_check_can_be_waived_for_old_releases(self):
        tgz = make_tgz(self.tmp / "sillstone-0.3.0.tgz", self._prebuilds())
        self.assertEqual(cpc.check_npm(tgz, ALL, "sillstone_ffi", require_docs=False), [])


class SdistTests(unittest.TestCase):
    def setUp(self):
        self.tmp = pathlib.Path(tempfile.mkdtemp())

    def manifest(self, members: list[str]) -> bytes:
        return ("members = [\n" + "".join(f'    "{m}",\n' for m in members) + "]\n").encode()

    def sdist(
        self, entries: list[str], members: list[str] | None = None, root: str = "sillstone-0.3.1b1"
    ) -> pathlib.Path:
        names = [f"{root}/{e}" for e in entries]
        contents = {f"{root}/rust/Cargo.toml": self.manifest(members or ["crates/ffi"])}
        return make_tgz(self.tmp / f"{root}.tar.gz", names, contents)

    def complete(self) -> list[str]:
        return list(cpc.SDIST_REQUIRED) + ["rust/crates/unicode/data/ucd.bin", "PKG-INFO"]

    def test_sdist_carrying_the_rust_workspace_passes(self):
        self.assertEqual(cpc.check_sdist(self.sdist(self.complete())), [])

    def test_sdist_members_are_read_from_the_archive_manifest(self):
        self.assertEqual(
            cpc.sdist_members(self.manifest(["crates/ffi", "crates/buffer"]).decode()),
            ["crates/ffi", "crates/buffer"],
        )
        self.assertEqual(cpc.sdist_members(""), [])

    def test_sdist_listing_a_member_it_does_not_carry_fails(self):
        # Cargo refuses to build when a listed member's directory is absent, so
        # a manifest staged with a member left behind has to fail here instead.
        problems = cpc.check_sdist(self.sdist(self.complete(), members=["crates/ffi", "crates/cli"]))
        self.assertEqual(len(problems), 1)
        self.assertIn("crates/cli", problems[0])

    def test_sdist_carrying_the_members_its_manifest_lists_passes(self):
        entries = self.complete() + ["rust/crates/buffer/Cargo.toml"]
        self.assertEqual(cpc.check_sdist(self.sdist(entries, members=["crates/ffi", "crates/buffer"])), [])

    def test_sdist_without_the_rust_workspace_fails(self):
        # What `python -m build --sdist sdk/python` writes: setuptools cannot
        # pack crates/ above the project root, so nothing here can compile.
        entries = [e for e in self.complete() if not e.startswith("rust/")]
        problems = cpc.check_sdist(self.sdist(entries))
        self.assertEqual(
            sorted(p.split("missing ", 1)[1] for p in problems),
            ["rust/Cargo.toml", "rust/crates/ffi/Cargo.toml"],
        )

    def test_sdist_carrying_a_prebuilt_cdylib_fails(self):
        problems = cpc.check_sdist(self.sdist(self.complete() + ["sillstone/sillstone_ffi.dll"]))
        self.assertEqual(len(problems), 1)
        self.assertIn("prebuilt", problems[0])

    def test_sdist_with_several_roots_fails(self):
        tgz = make_tgz(self.tmp / "sillstone-0.3.1b1.tar.gz", ["sillstone-0.3.1b1/setup.py", "other/setup.py"])
        self.assertTrue(any("top-level" in p for p in cpc.check_sdist(tgz)))

    def test_sdist_kind_needs_neither_platforms_nor_stem(self):
        # The sdist advertises no platform: it compiles the one it is installed on.
        tgz = self.sdist(self.complete())
        self.assertEqual(cpc.main(["sdist", str(tgz)]), 0)

    def test_sdist_kind_fails_a_broken_archive(self):
        entries = [e for e in self.complete() if not e.startswith("rust/")]
        self.assertEqual(cpc.main(["sdist", str(self.sdist(entries))]), 1)


if __name__ == "__main__":
    unittest.main()
