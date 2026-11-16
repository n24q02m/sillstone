# Agent Collaboration

## Quick reference

- Repo: `n24q02m/sillstone`
- Description: Pure-Rust, zero-dependency multimedia, hashing, and array computing suite.
- License: Apache-2.0

## Build & Test

See [README.md](README.md) for end-user install. For development:

```sh
mise run setup     # First-time dev environment
mise run lint      # cargo check + clippy, the same gates as CI
mise run test      # Build the text cdylib, then run the Rust workspace tests
mise run fix       # Apply clippy's automatic fixes
```

The language SDKs (Python, Node.js, Java, Go) load the `sillstone-ffi` cdylib. Build it with
`cargo build -p sillstone-ffi` before running an SDK suite; `.github/workflows/ci.yml` lists
the exact command for each one.

## Release

Every manifest carries the same version: the workspace and crate `Cargo.toml` files,
`sdk/node/package.json`, `sdk/java/pom.xml`, the Python SDK (in PEP 440 form), the
default `expected_version` of `cd.yml` and `EXPECTED_VERSION` in
`scripts/ci/check-registries.py`. `scripts/ci/version_sync.py set <version>` moves
all of them at once; `version_sync.py check <version>` refuses a release whose manifests
disagree.

1. Open a `fix:` PR that runs `version_sync.py set <version>` and adds the release's entries
   to `CHANGELOG.md`.
2. After it merges, dispatch `cd.yml` with `release_type` (`beta` first, then `stable`) and
   the same `expected_version`.
3. CD checks the manifests, builds the cdylib for five platforms, publishes to PyPI, npm,
   Maven Central and the Go proxy, then tags the commit and creates the GitHub Release.

## Conventions

- Commits: only `feat:` and `fix:` prefixes (enforced by pre-commit `commit-msg` hook)
- Test coverage: ≥ 95% target; the Go, Node.js and Java gates in `ci.yml` are ratchets
  that only move up
- Zero third-party Rust dependencies: `scripts/ci/check-zero-deps.py` fails on any
  registry or git source in `Cargo.lock`
- No secrets in code, commits, or memory — use [skret](https://skret.n24q02m.com) for secret retrieval
