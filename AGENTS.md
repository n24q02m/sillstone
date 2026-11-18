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

Nobody picks the version. [better-semantic-release](https://github.com/n24q02m/better-semantic-release)
computes it from the conventional commits since the last `v*` tag (`fix:` bumps the
patch, `feat:` the minor), configured in `semantic-release.toml`.

Every manifest carries the same version: the workspace and crate `Cargo.toml` files,
`sdk/node/package.json`, `sdk/java/pom.xml`, the Python SDK (in PEP 440 form) and
`EXPECTED_VERSION` in `scripts/ci/check-registries.py`. The release runs
`scripts/ci/version_sync.py set <version>` to move all of them at once, then
`version_sync.py check <version>`, which refuses a release whose manifests disagree.

1. Merge `fix:`/`feat:` PRs to `main` as usual; do not edit versions or `CHANGELOG.md` by hand.
2. Dispatch `cd.yml` with `release_type` (`beta` first, then `stable`).
3. CD computes the version, stamps every manifest, updates `CHANGELOG.md`, commits and tags
   `v<version>` on `main`, builds the cdylib for five platforms from that tag, publishes to
   crates.io, PyPI, npm, Maven Central and the Go proxy, reads every registry back, then
   creates the GitHub Release. With no `fix:`/`feat:` commit since the last tag it releases
   nothing.
4. If a run fails after the tag exists, dispatch `cd.yml` again with `publish_existing_tag`
   set to that tag: it skips the version step and re-runs the build and publish jobs, each
   of which skips what its registry already has.

## Conventions

- Commits: only `feat:` and `fix:` prefixes (enforced by pre-commit `commit-msg` hook)
- Test coverage: ≥ 95% target; the Node.js and Java gates in `ci.yml` are ratchets
  that only move up
- Zero third-party Rust dependencies: `scripts/ci/check-zero-deps.py` fails on any
  registry or git source in `Cargo.lock`
- No secrets in code, commits, or memory — use [skret](https://skret.n24q02m.com) for secret retrieval
