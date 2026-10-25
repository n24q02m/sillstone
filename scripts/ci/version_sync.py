#!/usr/bin/env python3
"""Single source for the release version across every manifest.

    version_sync.py set <semver>    rewrite all manifests (e.g. 0.3.0-beta.1)
    version_sync.py check <semver>  exit 1 unless every manifest already carries it

Cargo, npm and Maven use the semver string; Python manifests use the PEP 440 form
(``0.3.0-beta.1`` -> ``0.3.0b1``). The current version is read from the workspace
``Cargo.toml``; every ``version = "<current>"`` line in the crate manifests (package
version and path-dependency versions alike) moves with it.
"""

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SEMVER = re.compile(r"^\d+\.\d+\.\d+(-(alpha|beta|rc)\.\d+)?$")


def to_pep440(v: str) -> str:
    return re.sub(r"-(alpha|beta|rc)\.(\d+)$", lambda m: {"alpha": "a", "beta": "b", "rc": "rc"}[m[1]] + m[2], v)


def current_version() -> str:
    m = re.search(r'^version = "([^"]+)"', (ROOT / "Cargo.toml").read_text(encoding="utf-8"), re.M)
    if not m:
        raise SystemExit("workspace Cargo.toml has no version")
    return m[1]


def targets(old: str, new: str):
    """Yield (path, regex, replacement) for every place a version lives."""
    py_old, py_new = to_pep440(old), to_pep440(new)
    yield ROOT / "Cargo.toml", rf'^version = "{re.escape(old)}"', f'version = "{new}"'
    for toml in sorted((ROOT / "crates").glob("*/Cargo.toml")):
        if re.search(rf'version = "{re.escape(old)}"', toml.read_text(encoding="utf-8")):
            yield toml, rf'version = "{re.escape(old)}"', f'version = "{new}"'
    yield ROOT / "sdk/node/package.json", rf'"version": "{re.escape(old)}"', f'"version": "{new}"'
    yield ROOT / "sdk/java/pom.xml", rf"<version>{re.escape(old)}</version>", f"<version>{new}</version>"
    yield ROOT / "sdk/python/pyproject.toml", rf'^version = "{re.escape(py_old)}"', f'version = "{py_new}"'
    yield ROOT / "sdk/python/sillstone/__init__.py", rf'__version__ = "{re.escape(py_old)}"', f'__version__ = "{py_new}"'
    yield ROOT / "scripts/ci/check-registries.py", rf'EXPECTED_VERSION = "{re.escape(old)}"', f'EXPECTED_VERSION = "{new}"'
    yield ROOT / ".github/workflows/cd.yml", rf'default: "{re.escape(old)}"', f'default: "{new}"'


def main(argv: list[str]) -> int:
    if len(argv) != 3 or argv[1] not in ("set", "check") or not SEMVER.match(argv[2]):
        print(__doc__)
        return 2
    mode, new = argv[1], argv[2]
    old = current_version()
    if mode == "set":
        for path, pat, repl in targets(old, new):
            text = path.read_text(encoding="utf-8")
            out, n = re.subn(pat, repl, text, flags=re.M)
            if n == 0 and old != new:
                print(f"FAIL: {path.relative_to(ROOT)} has no version {old}")
                return 1
            path.write_bytes(out.encode("utf-8"))
        return 0
    bad = []
    for path, pat, _ in targets(new, new):
        if not re.search(pat, path.read_text(encoding="utf-8"), re.M):
            bad.append(path.relative_to(ROOT).as_posix())
    # crates must not carry a stray older version
    for toml in (ROOT / "crates").glob("*/Cargo.toml"):
        for v in re.findall(r'version = "(\d+\.\d+\.\d+[^"]*)"', toml.read_text(encoding="utf-8")):
            if v != new and toml.relative_to(ROOT).as_posix() not in bad:
                bad.append(toml.relative_to(ROOT).as_posix())
    if bad:
        print(f"version {new} missing/mixed in: " + ", ".join(sorted(set(bad))))
        return 1
    print(f"all manifests carry {new}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
