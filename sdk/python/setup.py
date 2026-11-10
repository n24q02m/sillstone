# SPDX-License-Identifier: Apache-2.0
# Copyright (c) 2026 Sillstone Contributors
"""Build backend glue for the sillstone ctypes wheel.

The wheel carries the Rust cdylib built by ``cargo build -p sillstone-ffi --release``.
Two routes produce that binary:

* **Release matrix.** ``SILLSTONE_CDYLIB_DIR`` names a directory that already
  holds the cdylib for the platform being packaged (the CD workflow builds one
  per platform, cross-compiled for darwin-x64). When it is set it is the only
  source consulted: a missing artifact fails the job instead of quietly packing
  the build host's own library under another platform's tag.
* **Source install.** With no ``SILLSTONE_CDYLIB_DIR`` the cdylib is looked for
  in ``target/release`` and, when absent, compiled from the Rust workspace that
  sits beside this file (``rust/`` in the sdist, ``../..`` in the monorepo).
  ``pip install sillstone --no-binary :all:`` therefore works, and a machine
  without ``cargo`` gets an explanation instead of a package that installs and
  then fails at ``import sillstone``.

``BinaryDistribution`` marks the distribution non-pure so the wheel is tagged
per platform, and ``bdist_wheel_py3_none`` narrows the tag to
``py3-none-<platform>``. ``sdist_with_rust_workspace`` refuses to write an
archive without the Rust sources — which is exactly what a bare
``python -m build --sdist sdk/python`` produces, because setuptools cannot pack
files above the project root.
"""

from __future__ import annotations

import os
import platform as host
import shutil
import subprocess
from pathlib import Path

from setuptools import Distribution, setup
from setuptools.command.bdist_wheel import bdist_wheel
from setuptools.command.build_py import build_py
from setuptools.command.sdist import sdist

HERE = Path(__file__).resolve().parent

#: Every cdylib file name cargo may drop into the build directory, per
#: platform (windows / linux / macOS).
CDYLIB_NAMES = ("sillstone_ffi.dll", "libsillstone_ffi.so", "libsillstone_ffi.dylib")

#: The Rust workspace that owns `crates/ffi`: `../..` when building inside the
#: monorepo, `rust/` when installing the sdist that scripts/build-sdist.py
#: staged. Anything else is a tree that cannot produce a working package.
WORKSPACE_CANDIDATES = (HERE.parent.parent, HERE / "rust")

NO_CARGO = """\
sillstone: no cdylib was found, and this tree has to compile the Rust ctypes
library to produce a working package — but no `cargo` is on PATH.

  * Just want the package? Install a prebuilt wheel:
        pip install --only-binary :all: sillstone
    Wheels are published for linux-x64, linux-arm64, darwin-arm64,
    darwin-x64 and win32-x64.
  * Want to build it yourself? Install a Rust toolchain (https://rustup.rs),
    then rerun `pip install sillstone`.
    Rust workspace: {workspace}
"""

NO_WORKSPACE = """\
sillstone: no cdylib (any of {names}) in {candidates}, and no Rust workspace
beside {here} to compile one from.

  * A release build sets SILLSTONE_CDYLIB_DIR to the directory that already
    holds the cdylib for the platform being packaged.
  * A source distribution is built by scripts/build-sdist.py, which stages
    the Rust workspace as `rust/` next to setup.py.
"""


class BinaryDistribution(Distribution):
    """A distribution that always carries a binary, so wheels get a
    platform tag instead of ``py3-none-any``."""

    def has_ext_modules(self) -> bool:
        return True


def native_platform_tag(platform: str, machine: str) -> str:
    """Narrow a macOS tag or handle cross-compiled architecture."""
    target_arch = os.environ.get("SILLSTONE_TARGET_ARCH")
    if target_arch:
        machine = target_arch
    if target_arch and platform.startswith("macosx_"):
        parts = platform.split("_")
        parts[-1] = target_arch
        return "_".join(parts)
    if not (platform.startswith("macosx_") and platform.endswith("_universal2")):
        return platform
    arch = {"arm64": "arm64", "aarch64": "arm64", "x86_64": "x86_64", "amd64": "x86_64"}.get(machine.lower())
    if arch is None:
        raise RuntimeError(f"sillstone: cannot narrow {platform} for host architecture {machine!r}")
    return platform[: -len("universal2")] + arch


class bdist_wheel_py3_none(bdist_wheel):
    """Tags the wheel ``py3-none-<platform>``.

    The cdylib is loaded through ctypes and never links the Python C API, so
    one wheel per platform is valid for every CPython 3.x. The platform half
    of the tag is what setuptools computed for the build host, with a macOS
    ``universal2`` narrowed to the single architecture the cdylib was built for.
    """

    def get_tag(self):
        _python, _abi, platform = super().get_tag()
        return "py3", "none", native_platform_tag(platform, host.machine())


def cargo_workspace() -> Path | None:
    """The staged or monorepo Rust workspace, or ``None`` when neither exists."""
    for candidate in WORKSPACE_CANDIDATES:
        manifest = candidate / "Cargo.toml"
        if manifest.is_file() and (candidate / "crates" / "ffi" / "Cargo.toml").is_file():
            return candidate.resolve()
    return None


def cargo_release_dir(workspace: Path) -> Path:
    """Where ``cargo build --release`` puts the cdylib for this workspace."""
    target = Path(os.environ.get("CARGO_TARGET_DIR") or "target")
    return (target if target.is_absolute() else workspace / target) / "release"


def copy_cdylib(candidates: list[Path], pkg_dir: Path) -> int:
    """Copy the first cdylib found; ``0`` when no candidate holds one."""
    for candidate in candidates:
        for name in CDYLIB_NAMES:
            src = candidate / name
            if src.is_file():
                pkg_dir.mkdir(parents=True, exist_ok=True)
                shutil.copy2(src, pkg_dir / name)
                print(f"sillstone: packed {name} from {candidate}", flush=True)
                return 1
    return 0


def build_cdylib_from_source(workspace: Path) -> None:
    """Compile the cdylib the wheel needs, failing loudly when that is impossible."""
    cargo = shutil.which("cargo")
    if cargo is None:
        raise SystemExit(NO_CARGO.format(workspace=workspace))
    command = [cargo, "build", "--release", "-p", "sillstone-ffi"]
    print(f"sillstone: compiling the ctypes cdylib: {' '.join(command)} (cwd={workspace})", flush=True)
    result = subprocess.run(command, cwd=str(workspace))
    if result.returncode != 0:
        raise SystemExit(
            f"sillstone: `{' '.join(command)}` failed in {workspace}; "
            "a source install needs a working Rust toolchain"
        )


class build_py_with_cdylib(build_py):
    """Packs the cdylib, compiling it from source when no prebuild is available."""

    def run(self) -> None:
        super().run()
        env_dir = os.environ.get("SILLSTONE_CDYLIB_DIR")
        workspace = cargo_workspace()
        candidates: list[Path] = []
        if env_dir:
            path = Path(env_dir).expanduser()
            candidates.append(path if path.is_absolute() else HERE.parent.parent / path)
        if workspace is not None:
            candidates.append(cargo_release_dir(workspace))
        candidates.append(HERE.parent.parent / "target" / "release")
        pkg_dir = Path(self.build_lib) / "sillstone"
        if copy_cdylib(candidates, pkg_dir):
            return
        names = ", ".join(CDYLIB_NAMES)
        if env_dir:
            # The release matrix named its artifact directory; compiling here
            # would tag this host's library as the target platform.
            raise SystemExit(
                f"sillstone: SILLSTONE_CDYLIB_DIR={env_dir} holds no cdylib ({names}). "
                "The release job must build the cdylib before packaging the wheel."
            )
        if workspace is None:
            raise SystemExit(
                NO_WORKSPACE.format(names=names, candidates=", ".join(str(c) for c in candidates), here=HERE)
            )
        build_cdylib_from_source(workspace)
        if not copy_cdylib([cargo_release_dir(workspace)], pkg_dir):
            raise SystemExit(
                f"sillstone: cargo finished but {cargo_release_dir(workspace)} still holds none of {names}"
            )


class sdist_with_rust_workspace(sdist):
    """Refuses to write an sdist that could not rebuild the cdylib.

    setuptools only packs files below the project root, so
    ``python -m build --sdist sdk/python`` produces an archive with the ctypes
    wrapper and no library: it installs, then every call fails at load time.
    scripts/build-sdist.py stages the Rust workspace as ``rust/`` next to this
    file; an archive without it is refused before anything is written.
    """

    def make_release_tree(self, base_dir, files):
        super().make_release_tree(base_dir, files)
        staged = Path(base_dir) / "rust" / "crates" / "ffi" / "Cargo.toml"
        if not staged.is_file():
            raise SystemExit(
                "sillstone: refusing to write an sdist without the Rust workspace "
                f"({staged} is missing). Build it with `python scripts/build-sdist.py`, "
                "which stages rust/ beside setup.py; a bare `python -m build --sdist sdk/python` "
                "cannot reach the crates above this project root."
            )


setup(
    cmdclass={
        "build_py": build_py_with_cdylib,
        "bdist_wheel": bdist_wheel_py3_none,
        "sdist": sdist_with_rust_workspace,
    },
    distclass=BinaryDistribution,
)
