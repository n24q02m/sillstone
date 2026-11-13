#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright (c) 2026 sillstone Contributors
"""Stage the Java SDK's native prebuilds before `mvn -Prelease deploy`.

`package-sdks` builds `sdk/java/target/sillstone-<version>.jar` with the
prebuilds under `io/github/sillstone/native/<platform>/`. The deploy job then
runs Maven again, and that rebuild compiles the jar from `src/`, so the
prebuilds have to exist as resources first; copying the finished jar into
`target/` alone does not survive it. Every earlier Maven version shipped
without them and threw SillstoneLibraryNotFoundException at runtime.

Run from the repository root: python scripts/ci/stage-java-natives.py
"""
from __future__ import annotations

import pathlib
import sys
import zipfile

PLATFORM_LIBS = {
    "linux-x64": "libsillstone_ffi.so",
    "linux-arm64": "libsillstone_ffi.so",
    "darwin-arm64": "libsillstone_ffi.dylib",
    "darwin-x64": "libsillstone_ffi.dylib",
    "win32-x64": "sillstone_ffi.dll",
}
PREFIX = "io/github/sillstone/native"
TARGET = pathlib.Path("sdk/java/target")
DEST_ROOT = pathlib.Path("sdk/java/src/main/resources")


def packaged_jar(target: pathlib.Path = TARGET) -> pathlib.Path:
    """The deployable jar: never a sources/javadoc classifier."""
    jars = [
        p
        for p in sorted(target.glob("sillstone-*.jar"))
        if not p.name.endswith(("-sources.jar", "-javadoc.jar", "-tests.jar"))
    ]
    if len(jars) != 1:
        raise SystemExit(f"expected exactly one packaged jar in {target}, found {[p.name for p in jars]}")
    return jars[0]


def missing_entries(jar: pathlib.Path) -> list[str]:
    with zipfile.ZipFile(jar) as zf:
        names = set(zf.namelist())
    return [f"{PREFIX}/{plat}/{lib}" for plat, lib in PLATFORM_LIBS.items() if f"{PREFIX}/{plat}/{lib}" not in names]


def stage(jar: pathlib.Path, dest_root: pathlib.Path = DEST_ROOT) -> list[pathlib.Path]:
    written = []
    with zipfile.ZipFile(jar) as zf:
        for info in zf.infolist():
            if info.is_dir() or not info.filename.startswith(PREFIX + "/"):
                continue
            out = dest_root / info.filename
            out.parent.mkdir(parents=True, exist_ok=True)
            out.write_bytes(zf.read(info))
            written.append(out)
    return written


def main() -> int:
    jar = packaged_jar()
    missing = missing_entries(jar)
    if missing:
        print(f"::error title=Java jar::{jar.name} carries no prebuilds: {', '.join(missing)}", file=sys.stderr)
        return 1
    for path in stage(jar):
        print(path)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
