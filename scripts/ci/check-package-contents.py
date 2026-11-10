#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright (c) 2026 sillstone Contributors
"""Package-contents gate: a published artifact must carry a native library for
every platform it advertises.

Exact-version presence on a registry proves nothing about usability: a JAR
without its cdylib, a wheel set missing a platform, or an npm tarball missing a
prebuild all install and then fail at load time, and a version cannot be
replaced. The sdist has the same failure in a different shape: without the Rust
workspace it builds a Python package whose ctypes library is never compiled.
This gate only lists archive entries (nothing from an archive is executed) and
exits non-zero when anything is missing, so it can sit in front of every
publish job.

  check-package-contents.py wheels 'wheelhouse-flat/*.whl' --platforms linux-x64,win32-x64 --stem sillstone_ffi
  check-package-contents.py jar    'sdk/java/target/*.jar' --platforms ... --stem sillstone_ffi --prefix github/sillstone/native
  check-package-contents.py npm    'sdk/node/*.tgz'        --platforms ... --stem sillstone_ffi --addon-stem sillstone_napi
  check-package-contents.py sdist  'sdk/python/dist/*.tar.gz'
"""
from __future__ import annotations

import argparse
import fnmatch
import glob
import pathlib
import sys
import tarfile
import zipfile

#: Node-style platform id -> (cdylib file name format, wheel platform tag glob)
PLATFORMS: dict[str, tuple[str, str]] = {
    "linux-x64": ("lib{stem}.so", "manylinux*_x86_64"),
    "linux-arm64": ("lib{stem}.so", "manylinux*_aarch64"),
    "darwin-arm64": ("lib{stem}.dylib", "macosx_*_arm64"),
    "darwin-x64": ("lib{stem}.dylib", "macosx_*_x86_64"),
    "win32-x64": ("{stem}.dll", "win_amd64"),
}


def _known(platform: str) -> tuple[str, str]:
    if platform not in PLATFORMS:
        raise ValueError(f"unknown platform {platform!r}; known: {', '.join(sorted(PLATFORMS))}")
    return PLATFORMS[platform]


def cdylib_name(platform: str, stem: str) -> str:
    return _known(platform)[0].format(stem=stem)


def wheel_platform_patterns(platform: str) -> str:
    return "py3-none-" + _known(platform)[1]


def _wheel_tag(path: pathlib.Path) -> str:
    return "-".join(path.name[: -len(".whl")].split("-")[-3:])


def check_wheels(wheels: list[pathlib.Path], platforms: list[str], stem: str) -> list[str]:
    if not wheels:
        return ["no wheel files found"]
    problems: list[str] = []
    for w in wheels:
        tag = _wheel_tag(w)
        if not tag.startswith("py3-none-"):
            problems.append(
                f"{w.name}: tag {tag!r} is not py3-none-<platform> "
                "(an interpreter-specific wheel only installs on one Python)"
            )
    for platform in platforms:
        pattern = wheel_platform_patterns(platform)
        matching = [w for w in wheels if fnmatch.fnmatchcase(_wheel_tag(w), pattern)]
        if not matching:
            problems.append(f"{platform}: no wheel matches {pattern}")
            continue
        want = cdylib_name(platform, stem)
        for w in matching:
            with zipfile.ZipFile(w) as z:
                if not any(n.rsplit("/", 1)[-1] == want for n in z.namelist()):
                    problems.append(f"{w.name}: does not contain {want}")
    return problems


def check_jar(jar: pathlib.Path, platforms: list[str], stem: str, prefix: str) -> list[str]:
    if not jar.is_file():
        return [f"{jar}: not a file"]
    with zipfile.ZipFile(jar) as z:
        names = set(z.namelist())
    problems = []
    for platform in platforms:
        entry = f"{prefix.strip('/')}/{platform}/{cdylib_name(platform, stem)}"
        if entry not in names:
            problems.append(f"{jar.name}: missing {entry}")
    return problems


NPM_DOCS = ("package/README.md", "package/LICENSE")

#: Entries an installable sdist must carry: the setuptools project plus the Rust
#: workspace `setup.py` compiles the cdylib from when no prebuild is supplied.
SDIST_REQUIRED = (
    "pyproject.toml",
    "setup.py",
    "MANIFEST.in",
    "sillstone/__init__.py",
    "sillstone/_generated.py",
    "rust/Cargo.toml",
    "rust/crates/ffi/Cargo.toml",
)

NATIVE_SUFFIXES = (".dll", ".so", ".dylib", ".node")


def check_sdist(tarball: pathlib.Path) -> list[str]:
    """An sdist installs by compiling its native library, so it must carry the
    Rust workspace — and must not smuggle in a prebuilt cdylib, because one
    binary cannot serve the five platforms the wheel set covers."""
    if not tarball.is_file():
        return [f"{tarball}: not a file"]
    with tarfile.open(tarball, "r:*") as t:
        names = [n for n in t.getnames() if not n.endswith("/")]
    roots = {n.split("/", 1)[0] for n in names}
    if len(roots) != 1:
        return [f"{tarball.name}: expected one top-level directory, found {', '.join(sorted(roots)) or 'none'}"]
    root = roots.pop()
    if "sillstone" not in root:
        return [f"{tarball.name}: top-level directory {root!r} does not name the project"]
    present = {n[len(root) + 1 :] for n in names if n.startswith(root + "/")}
    problems = [f"{tarball.name}: missing {entry}" for entry in SDIST_REQUIRED if entry not in present]
    binaries = sorted(entry for entry in present if entry.endswith(NATIVE_SUFFIXES))
    if binaries:
        listed = ", ".join(binaries[:3]) + ("…" if len(binaries) > 3 else "")
        problems.append(
            f"{tarball.name}: carries prebuilt binaries ({listed}); the sdist compiles the cdylib instead"
        )
    return problems


def check_npm(
    tgz: pathlib.Path,
    platforms: list[str],
    stem: str,
    addon_stem: str = "sillstone_napi",
    require_docs: bool = True,
) -> list[str]:
    """Every platform needs the N-API addon (the only file `require()` can load)
    and the cdylib it links against, side by side under prebuilds/<platform>/.
    A tarball that carries only the cdylib installs fine and then every native
    call fails with "addon not loaded". The package must also ship its README.md
    and LICENSE (`require_docs`; releases up to 0.3.0 shipped without them)."""
    if not tgz.is_file():
        return [f"{tgz}: not a file"]
    with tarfile.open(tgz, "r:gz") as t:
        names = set(t.getnames())
    problems = []
    if require_docs:
        problems += [f"{tgz.name}: missing {entry}" for entry in NPM_DOCS if entry not in names]
    for platform in platforms:
        for entry in (
            f"package/prebuilds/{platform}/{addon_stem}.node",
            f"package/prebuilds/{platform}/{cdylib_name(platform, stem)}",
        ):
            if entry not in names:
                problems.append(f"{tgz.name}: missing {entry}")
    return problems


def _expand(patterns: list[str]) -> list[pathlib.Path]:
    return sorted({pathlib.Path(p) for pat in patterns for p in glob.glob(pat)})


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("kind", choices=["wheels", "jar", "npm", "sdist"])
    ap.add_argument("paths", nargs="+", help="files or globs")
    ap.add_argument("--platforms", help="comma-separated platform ids (wheels, jar, npm)")
    ap.add_argument("--stem", help="cdylib stem, e.g. sillstone_ffi (wheels, jar, npm)")
    ap.add_argument("--prefix", default="github/sillstone/native", help="jar resource prefix")
    ap.add_argument("--addon-stem", default="sillstone_napi", help="npm N-API addon stem")
    args = ap.parse_args(argv)
    platforms: list[str] = []
    if args.kind != "sdist":
        if not args.platforms or not args.stem:
            ap.error(f"--platforms and --stem are required for {args.kind}")
        platforms = [p.strip() for p in args.platforms.split(",") if p.strip()]
        unknown = [p for p in platforms if p not in PLATFORMS]
        if unknown:
            ap.error(f"unknown platform: {', '.join(unknown)}")
    files = _expand(args.paths)
    if not files:
        print(f"FAIL: nothing matched {args.paths}")
        return 1
    if args.kind == "sdist":
        problems = [problem for f in files for problem in check_sdist(f)]
    elif args.kind == "wheels":
        problems = check_wheels(files, platforms, args.stem)
    else:
        problems = []
        for f in files:
            problems += (
                check_jar(f, platforms, args.stem, args.prefix)
                if args.kind == "jar"
                else check_npm(f, platforms, args.stem, args.addon_stem)
            )
    for p in problems:
        print(f"FAIL: {p}")
    if not problems:
        carries = (
            "the Rust workspace and no prebuilt binary"
            if args.kind == "sdist"
            else ", ".join(platforms)
        )
        print(f"ok: {len(files)} {args.kind} file(s) carry {carries}")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
