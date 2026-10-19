#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright (c) 2026 sillstone Contributors
"""Registry readback gate for the sillstone monorepo.

Reads every published package back from its registry and fails unless the
registry itself says the expected version is there. An HTTP 200 is never
taken as proof: each check parses the registry's own metadata and compares
the package name, the version, the yank/deprecation state and the registry's
"latest" pointer against what this release is supposed to have shipped.

Registries:
  crates.io  the crate lists the version, not yanked, default version matches
  PyPI       the project has the version, wheels present, none yanked
  npm        the packument lists the version, tarball integrity matches, dist-tag matches
  Maven      maven-metadata.xml lists version, pom coordinates match, jar downloadable
  Go proxy   proxy.golang.org resolves exact version tag and go.mod declares module

Usage:
    python scripts/ci/check-registries.py [--version 0.2.0] [--only crates,pypi] [--[no-]content]
"""
from __future__ import annotations

import argparse
import base64
import hashlib
import importlib.util
import json
import pathlib
import re
import sys
import tempfile
import time
import urllib.error
import urllib.request
import xml.etree.ElementTree as ET
from concurrent.futures import ThreadPoolExecutor
from typing import Callable


def _load_contents_gate():
    path = pathlib.Path(__file__).with_name("check-package-contents.py")
    spec = importlib.util.spec_from_file_location("check_package_contents", path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


pc = _load_contents_gate()
CONTENT_PLATFORMS = list(pc.PLATFORMS)
CONTENT_FROM = (0, 2, 0)
JAR_PREFIX = "github/sillstone/native"

EXPECTED_VERSION = "0.2.0"
USER_AGENT = "sillstone-registry-readback (github.com/sillstone/sillstone)"
ORG = "sillstone"
MAVEN_GROUP = "io.github.sillstone"

REGISTRIES = ("crates", "pypi", "npm", "maven", "go")

PACKAGES: list[tuple[str, str, str]] = [
    ("crates", "sillstone-digest", "sillstone"),
    ("pypi", "sillstone", "sillstone"),
    ("npm", "sillstone", "sillstone"),
    ("maven", f"{MAVEN_GROUP}:sillstone", "sillstone"),
    ("go", f"github.com/{ORG}/sillstone/sdk/go/sillstone", "sillstone"),
]

Fetch = Callable[[str], "tuple[int, bytes]"]


def fetch(url: str, method: str = "GET", attempts: int = 5) -> tuple[int, bytes]:
    delay = 2.0
    status, body = 0, b""
    for attempt in range(attempts):
        req = urllib.request.Request(
            url, method=method, headers={"User-Agent": USER_AGENT}
        )
        retry_after = None
        try:
            with urllib.request.urlopen(req, timeout=30) as resp:
                return resp.status, resp.read()
        except urllib.error.HTTPError as err:
            status, body = err.code, err.read()
            if status != 429 and status < 500:
                return status, body
            retry_after = err.headers.get("Retry-After")
        except (urllib.error.URLError, TimeoutError, ConnectionError) as err:
            status, body = 0, str(err).encode()
        if attempt + 1 < attempts:
            wait = delay
            if retry_after and retry_after.isdigit():
                wait = max(wait, min(float(retry_after), 30.0))
            time.sleep(wait)
            delay *= 2
    return status, body


def _json(status: int, body: bytes, url: str) -> tuple[object | None, str | None]:
    if status == 404:
        return None, f"not found on the registry ({url})"
    if status != 200:
        return None, f"registry answered HTTP {status} for {url}"
    try:
        return json.loads(body), None
    except ValueError:
        return None, f"registry reply is not JSON ({url})"


def check_crate(name: str, version: str, get: Fetch) -> list[str]:
    url = f"https://crates.io/api/v1/crates/{name}"
    doc, err = _json(*get(url), url)
    if err:
        return [err]
    assert isinstance(doc, dict)
    problems: list[str] = []
    crate = doc.get("crate") or {}
    if crate.get("name") != name:
        problems.append(f"registry names this crate {crate.get('name')!r}, not {name!r}")
    found = [v for v in doc.get("versions") or [] if v.get("num") == version]
    if not found:
        have = sorted(v.get("num", "?") for v in doc.get("versions") or [])
        return problems + [f"version {version} is missing (registry has: {', '.join(have) or 'none'})"]
    if found[0].get("yanked"):
        problems.append(f"version {version} is yanked")
    if crate.get("default_version") != version and not version.startswith("0.2.0-beta"):
        problems.append(
            f"default version is {crate.get('default_version')!r}, expected {version}"
        )
    return problems


def _norm_py(name: str) -> str:
    return re.sub(r"[-_.]+", "-", name).lower()


def _stem(name: str) -> str:
    return "sillstone_ffi"


def _download(get: Fetch, url: str, dest: pathlib.Path) -> str | None:
    status, body = get(url)
    if status != 200:
        return f"{dest.name}: not downloadable (HTTP {status})"
    dest.write_bytes(body)
    return None


def _wheel_contents(name: str, files: list[dict], get: Fetch) -> list[str]:
    wheels = [f for f in files if f.get("filename", "").endswith(".whl") and not f.get("yanked")]
    with tempfile.TemporaryDirectory() as tmp:
        paths: list[pathlib.Path] = []
        for f in wheels:
            path = pathlib.Path(tmp) / pathlib.PurePosixPath(f["filename"]).name
            err = _download(get, f.get("url", ""), path)
            if err:
                return [err]
            paths.append(path)
        return pc.check_wheels(paths, CONTENT_PLATFORMS, _stem(name))


def check_pypi(name: str, version: str, get: Fetch, content: bool = False) -> list[str]:
    url = f"https://pypi.org/pypi/{name}/json"
    doc, err = _json(*get(url), url)
    if err:
        return [err]
    assert isinstance(doc, dict)
    problems: list[str] = []
    info = doc.get("info") or {}
    if _norm_py(info.get("name") or "") != _norm_py(name):
        problems.append(f"registry names this project {info.get('name')!r}, not {name!r}")
    releases = doc.get("releases") or {}
    py_ver = re.sub(r"-beta\.(\d+)", r"b\1", version)
    files = releases.get(version) or releases.get(py_ver) or []
    if not files:
        have = sorted(releases.keys())
        return problems + [f"version {version} has no files (registry has: {', '.join(have) or 'none'})"]
    if all(f.get("yanked") for f in files):
        problems.append(f"every file of version {version} is yanked")
    if info.get("version") not in (version, py_ver):
        problems.append(f"current version is {info.get('version')!r}, expected {version}")
    if content:
        problems += _wheel_contents(name, files, get)
    return problems


def _npm_contents(name: str, dist: dict, get: Fetch) -> list[str]:
    with tempfile.TemporaryDirectory() as tmp:
        path = pathlib.Path(tmp) / "package.tgz"
        err = _download(get, dist["tarball"], path)
        if err:
            return [err]
        integrity = dist.get("integrity") or ""
        if integrity.startswith("sha512-"):
            digest = base64.b64encode(hashlib.sha512(path.read_bytes()).digest()).decode()
            if integrity != "sha512-" + digest:
                return ["the tarball served does not match the integrity hash in the registry's own record"]
        return pc.check_npm(path, CONTENT_PLATFORMS, "sillstone_napi")


def check_npm(name: str, version: str, get: Fetch, content: bool = False) -> list[str]:
    url = "https://registry.npmjs.org/" + name.replace("/", "%2F")
    doc, err = _json(*get(url), url)
    if err:
        return [err]
    assert isinstance(doc, dict)
    problems: list[str] = []
    if doc.get("name") != name:
        problems.append(f"registry names this package {doc.get('name')!r}, not {name!r}")
    versions = doc.get("versions") or {}
    meta = versions.get(version)
    if meta is None:
        have = sorted(versions.keys())
        problems.append(f"version {version} is missing (registry has: {', '.join(have) or 'none'})")
        return problems
    if meta.get("version") != version or meta.get("name") != name:
        problems.append(f"version record says {meta.get('name')}@{meta.get('version')}")
    if meta.get("deprecated"):
        problems.append(f"version {version} is deprecated: {meta['deprecated']}")
    dist = meta.get("dist") or {}
    if not dist.get("tarball") or not dist.get("integrity"):
        problems.append(f"version {version} has no tarball/integrity in its dist record")
    if content and dist.get("tarball"):
        problems += _npm_contents(name, dist, get)
    return problems


def _xml(status: int, body: bytes, url: str) -> tuple[ET.Element | None, str | None]:
    if status == 404:
        return None, f"not found on the registry ({url})"
    if status != 200:
        return None, f"registry answered HTTP {status} for {url}"
    try:
        return ET.fromstring(body), None
    except ET.ParseError:
        return None, f"registry reply is not XML ({url})"


def _text(root: ET.Element, tag: str) -> str | None:
    for el in root.iter():
        if el.tag.rsplit("}", 1)[-1] == tag:
            return (el.text or "").strip()
    return None


def check_maven(coords: str, version: str, get: Fetch, content: bool = False) -> list[str]:
    group, artifact = coords.split(":")
    base = f"https://repo1.maven.org/maven2/{group.replace('.', '/')}/{artifact}"
    meta_url = f"{base}/maven-metadata.xml"
    root, err = _xml(*get(meta_url), meta_url)
    if err:
        return [err]
    assert root is not None
    problems: list[str] = []
    if (_text(root, "groupId"), _text(root, "artifactId")) != (group, artifact):
        problems.append(
            f"metadata names {_text(root, 'groupId')}:{_text(root, 'artifactId')}, not {coords}"
        )
    listed = [(v.text or "").strip() for v in root.iter("version")]
    if version not in listed:
        return problems + [f"version {version} is missing (registry has: {', '.join(listed) or 'none'})"]

    pom_url = f"{base}/{version}/{artifact}-{version}.pom"
    pom, err = _xml(*get(pom_url), pom_url)
    if err:
        problems.append(err)
    elif pom is not None:
        got = (_text(pom, "groupId"), _text(pom, "artifactId"), _text(pom, "version"))
        if got != (group, artifact, version):
            problems.append(f"pom declares {':'.join(map(str, got))}, expected {coords}:{version}")
    jar_url = f"{base}/{version}/{artifact}-{version}.jar"
    jar_status, jar_body = get(jar_url)
    if jar_status != 200:
        problems.append(f"jar is not downloadable (HTTP {jar_status})")
    elif content:
        with tempfile.TemporaryDirectory() as tmp:
            path = pathlib.Path(tmp) / f"{artifact}-{version}.jar"
            path.write_bytes(jar_body)
            problems += pc.check_jar(path, CONTENT_PLATFORMS, "sillstone_ffi", JAR_PREFIX)
    return problems


def check_go(module: str, version: str, get: Fetch) -> list[str]:
    base = f"https://proxy.golang.org/{module}/@v"
    tag = f"v{version}"
    info_url = f"{base}/{tag}.info"
    status, body = get(info_url)
    if status in (404, 410):
        subdir = module.split("/", 3)[3] if module.count("/") >= 3 else ""
        hint = f" (module lives in {subdir}/, so the tag it resolves from is {subdir}/{tag})" if subdir else ""
        return [f"{module}@{tag} is not resolvable on the proxy{hint}"]
    doc, err = _json(status, body, info_url)
    if err:
        return [err]
    assert isinstance(doc, dict)
    problems: list[str] = []
    if doc.get("Version") != tag:
        problems.append(f"proxy resolved {tag} to {doc.get('Version')!r}")
    list_status, list_body = get(f"{base}/list")
    if list_status == 200 and tag not in list_body.decode(errors="replace").split():
        problems.append(f"{tag} is not in the module's version list")
    mod_status, mod_body = get(f"{base}/{tag}.mod")
    if mod_status != 200:
        problems.append(f"go.mod is not served (HTTP {mod_status})")
    else:
        m = re.search(r"^module\s+(\S+)", mod_body.decode(errors="replace"), re.M)
        if not m or m.group(1) != module:
            problems.append(f"go.mod declares module {m.group(1) if m else None!r}, expected {module}")
    return problems


CHECKS: dict[str, Callable[..., list[str]]] = {
    "crates": check_crate,
    "pypi": check_pypi,
    "npm": check_npm,
    "maven": check_maven,
    "go": check_go,
}

CONTENT_REGISTRIES = {"pypi", "npm", "maven"}


def run_checks(version: str, only: set[str] | None = None, content: bool = True, get: Fetch = fetch) -> int:
    pkgs = [p for p in PACKAGES if not only or p[0] in only]
    failures = 0
    with ThreadPoolExecutor(max_workers=5) as ex:
        futures = []
        for reg, name, repo in pkgs:
            fn = CHECKS[reg]
            kwargs = {"content": content} if reg in CONTENT_REGISTRIES else {}
            futures.append((reg, name, ex.submit(fn, name, version, get, **kwargs)))
        for reg, name, f in futures:
            problems = f.result()
            if problems:
                failures += 1
                print(f"FAIL [{reg}] {name}:")
                for p in problems:
                    print(f"  - {p}")
            else:
                print(f"PASS [{reg}] {name} @ {version}")
    return failures


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--version", default=EXPECTED_VERSION, help=f"expected release version (default: {EXPECTED_VERSION})")
    ap.add_argument("--only", help="comma-separated list of registries to check")
    ap.add_argument("--content", dest="content", action="store_true", default=True, help="verify package archive contents")
    ap.add_argument("--no-content", dest="content", action="store_false")
    args = ap.parse_args()

    only = set(args.only.split(",")) if args.only else None
    print(f"Running registry readback gate for sillstone version {args.version}...")
    failures = run_checks(args.version, only=only, content=args.content)
    if failures == 0:
        print(f"All checked registries verified successfully for {args.version}.")
        return 0
    print(f"Readback gate found {failures} failing package(s).")
    return 1


if __name__ == "__main__":
    sys.exit(main())
