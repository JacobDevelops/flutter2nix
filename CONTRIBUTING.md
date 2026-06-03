# Contributing to flutter2nix

## Prerequisites

- [Nix](https://nixos.org/download/) with flakes enabled
- Rust stable (provided by the dev shell)

## Setup

```bash
git clone https://github.com/JacobDevelops/flutter2nix
cd flutter2nix
nix develop        # enters dev shell with Rust + tools
cargo check        # verify workspace compiles
cargo clippy       # lint
nix flake check    # verify Nix outputs
```

## Repository Layout

- `crates/` — Rust workspace members (nix-core, gradle2nix, ios2nix, flutter2nix)
- `tools/fnx/` — `fnx` dev CLI for running common repo tasks
- `tapi-shim/` — Kotlin/Gradle project providing the TAPI JAR for gradle2nix
- `nix/` — Nix library functions (buildAndroidApp, buildIOSApp, buildFlutterApp)
- `tests/fixtures/` — Test fixture projects (gradle/, flutter/)
- `docs/` — User-facing documentation

## Development Workflow

```bash
fnx check          # run nix flake check
fnx test           # cargo test --workspace
fnx build          # nix build all packages
fnx fmt            # cargo fmt --all
cargo clippy --workspace -- -D warnings
```

## Commit Style

Use [Conventional Commits](https://www.conventionalcommits.org/):
- `feat(gradle2nix): add TAPI shim extraction`
- `fix(nix-core): handle empty dep graph`
- `docs: update iOS testing runbook`

## Testing

- Unit tests: `cargo test -p <crate>`
- e2e tests: land in Phase 1 (see PLAN.md)
- iOS tests: manual procedure documented in `docs/ios-testing.md`
