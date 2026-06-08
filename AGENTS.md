# AGENTS.md — flutter2nix

Agent instructions and conventions for AI-assisted work in this repository.

---

## CRITICAL: This project is not published anywhere

**Nothing in this repository has ever been released to any registry.**

- No crates are published to crates.io.
- No packages are published to npm, PyPI, or any other registry.
- No versioned releases have been cut.
- No external consumers exist.

**Consequence for agents:** Never factor in migration concerns, backwards-compatibility, semver bumps, deprecation shims, or "existing users" when choosing between implementation options. There are no existing users. Pick the technically superior approach without qualification. If you catch yourself writing phrases like "to avoid breaking existing consumers" or "for backwards compatibility with previous versions", stop — that concern does not apply here. Delete it and choose the better design.

---

## Repository overview

Cargo workspace providing a modular Nix toolchain for reproducible Flutter app builds.

```
flutter2nix/
├── crates/
│   ├── nix-core/       # Shared dep model + Nix expression codegen (library)
│   ├── gradle2nix/     # Gradle/Maven → Nix lockfile (binary + library)
│   ├── ios2nix/        # iOS/Xcode orchestration for Nix (binary)
│   └── flutter2nix/    # Unified cross-platform integration layer (binary)
├── tools/
│   └── fnx/            # Developer CLI (cargo check/test/fmt shortcuts)
├── tapi-shim/          # Kotlin/Gradle subproject — Gradle Tooling API shim (JAR)
├── nix/                # Nix library functions (buildAndroidApp, buildGradleProject, etc.)
├── tests/fixtures/     # Integration test fixtures (flutter, gradle)
├── docs/               # Guides (gradle2nix-standalone.md, ios-testing.md)
├── flake.nix           # Nix flake: dev shell, packages, checks
└── Cargo.toml          # Workspace root
```

### Crate roles

| Crate | Type | Purpose |
|-------|------|---------|
| `nix-core` | lib | Shared `LockedDep`, `Lockfile`, Nix codegen — no binaries |
| `gradle2nix` | lib + bin | Drives tapi-shim, resolves Maven deps, writes lockfile JSON |
| `ios2nix` | bin | CocoaPods resolution, Xcode archive/export, keychain ops |
| `flutter2nix` | bin | Composes gradle2nix + ios2nix into a unified lockfile |
| `fnx` | bin | Dev utility — thin wrappers around cargo/nix subcommands |

`tapi-shim` is a separate Kotlin/Gradle project under `tapi-shim/`. It is built via Gradle and its output JAR is bundled into `gradle2nix` at build time via `build.rs`.

---

## VCS

This project uses **jj (Jujutsu)**, not git. Use `jj` commands exclusively.

```
jj log                  # history
jj diff                 # working copy changes
jj new                  # new change
jj describe -m "msg"    # set commit message
jj squash               # fold into parent
```

Branch naming: `feat/`, `fix/`, `chore/`, `refactor/`.

**Never push or create PRs without explicit user approval in the current turn.**

---

## Build and check commands

```bash
cargo check --workspace --all-targets   # must pass
cargo clippy --workspace --all-targets -- -D warnings   # must pass (warnings = errors)
cargo test --workspace                  # unit + integration tests
nix flake check                         # Nix eval + structural checks
nix build .#gradle2nix                  # build a specific package
fnx check / fnx build / fnx test / fnx fmt   # convenience wrappers
```

CI runs `cargo check`, `cargo clippy`, and `nix flake check` as required gates. E2E tests are currently allowed to fail.

---

## Code conventions

- **Rust edition 2021**, workspace version `0.1.0` (currently — irrelevant as nothing is published).
- `anyhow` for error handling throughout; no custom error types unless the added precision is genuinely needed.
- `clap` with `derive` feature for all CLIs.
- `serde` + `serde_json` for all lockfile serialisation.
- Tests live alongside source in `*_tests.rs` modules, imported via `#[cfg(test)] mod foo_tests;`.
- No dead code, no commented-out code, no backwards-compatibility shims.

---

## What agents should and should not do

**Do:**
- Choose the cleanest, simplest design without worrying about what existed before.
- Rename types, restructure modules, change lockfile schemas freely — there are no external consumers.
- Delete code that is no longer needed.
- Add `#[allow(...)]` only for intentional suppressions with a clear reason.

**Do not:**
- Add `#[deprecated]` annotations or compatibility re-exports.
- Write "migration guide" sections in docs.
- Hedge implementation choices with "to maintain backwards compatibility".
- Consider semver when deciding whether a change is acceptable.
- Push to remote or open PRs without explicit user approval.
