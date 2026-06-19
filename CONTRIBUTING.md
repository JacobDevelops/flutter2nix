# Contributing to flutter2nix

## Prerequisites

- [Nix](https://nixos.org/download/) with flakes enabled
- [devenv](https://devenv.sh/) + [direnv](https://direnv.net/) for the dev shell
  (`nix profile install nixpkgs#devenv`). Rust and all build tools (Flutter,
  Android SDK, JDK, Gradle) come from it.

## Setup

The dev environment is defined with **devenv** (`devenv.nix`) and entered via
**direnv** (`.envrc`):

```bash
git clone https://github.com/JacobDevelops/flutter2nix
cd flutter2nix
direnv allow       # enters the devenv shell (or run `devenv shell` directly)
cargo check        # verify workspace compiles
cargo clippy       # lint
cargo test         # unit + integration tests
nix flake check    # verify Nix outputs
```

Run tests from inside the devenv shell, not a bare `nix develop`: the shell's
`enterShell` exports `LD_LIBRARY_PATH` for OpenSSL, which the test binaries link
(via reqwest) and fail to load (`libssl.so.3: cannot open shared object file`)
without.

## Building the tapi-shim JAR (required for gradle2nix)

gradle2nix embeds the tapi-shim JAR at compile time via `include_bytes!`. You must build
the JAR before running `cargo build -p gradle2nix`:

```bash
(cd crates/gradle2nix/tapi-shim && gradle build)
cargo build -p gradle2nix
```

### Updating the JAR hash in flake.nix

When the tapi-shim Kotlin source changes, rebuild the JAR and update the hash in `flake.nix`:

```bash
(cd crates/gradle2nix/tapi-shim && gradle clean build)
nix hash file crates/gradle2nix/tapi-shim/build/libs/tapi-shim.jar
# Copy the sha256-... value and update outputHash in flake.nix:
#   tapi-shim-jar = pkgs.runCommand "tapi-shim-jar" {
#     outputHash = "sha256-<new-hash-here>";
#     ...
```

Then verify the updated flake builds correctly:

```bash
nix build .#tapi-shim-jar
nix build .#gradle2nix
```

## Repository Layout

- `crates/` — Rust workspace members (nix-core, gradle2nix, ios2nix, flutter2nix)
- `crates/gradle2nix/tapi-shim/` — Kotlin/Gradle project providing the TAPI JAR for gradle2nix
- `nix/` — Nix library functions (buildGradleProject, buildAndroidApp, buildIOSApp, buildFlutterApp)
- `docs/` — User-facing documentation
- `tests/fixtures/` — Test fixture projects (gradle/, flutter/)

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
- e2e tests: land in Phase 1 (see `tests/` directory)
- iOS tests: manual procedure documented in `docs/ios-testing.md`
