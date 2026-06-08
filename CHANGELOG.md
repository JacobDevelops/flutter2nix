# Changelog

## Phase 2 — Nix Integration & Documentation (2026-06-07)

### Added
- `flake.nix`: `packages.gradle2nix` — real `rustPlatform.buildRustPackage` replacing the Phase 0 `emptyDirectory` stub
- `flake.nix`: `packages.tapi-shim-jar` — fixed-output derivation that copies the pre-built tapi-shim JAR with hash verification (`sha256-iXOmUJ7D3IfH1JJ6J4Mw8/KVwtEwlrAhEs6uVgqljJ0=`)
- `flake.nix`: `preBuild` hook that places the JAR at the path expected by `include_bytes!` before `cargo build`
- `flake.lib.buildGradleProject` — passthrough attribute stub (Phase 5 placeholder); consumers can write valid Nix integration code now
- `flake.lib.buildAndroidApp` — passthrough attribute stub (Phase 5 placeholder)
- `nix/gradle2nix-lib.nix`: replaced `throw "not implemented"` stubs with passthrough attribute functions including `_phase5Placeholder = true` sentinel
- `docs/gradle2nix-standalone.md`: complete getting-started guide (Installation, Usage, Integration, Troubleshooting)
- `CONTRIBUTING.md`: JAR build prerequisite and hash-update procedure documented
- `crates/gradle2nix/Cargo.toml`: `repository`, `homepage`, and `keywords` metadata

### Architecture
- JAR bootstrapping uses copy+hash approach: pre-built JAR from source tree is copied via `pkgs.runCommand` and hash-locked in `flake.nix`. No Gradle runs in the Nix sandbox. Phase 3 will implement full Kotlin-in-Nix JAR building with offline Gradle support.
- Library functions return structured attrs rather than throwing errors, enabling consumers to adopt the flake API before Phase 5 ships full build orchestration.

---

## Phase 1 — Core Rust Implementation (2026-06-06)

### Added
- `crates/gradle2nix/src/maven.rs`: `MavenCoordinate` parser, `MavenResolverConfig`, `resolve_artifact_sha256`, `resolve_artifacts_batch` (parallel, fail-fast via `buffer_unordered`)
- HTTP Maven resolver using `reqwest` + `tokio::time::timeout` for configurable timeouts
- Local filesystem cache support (reads `.sha256` sidecar files before network fetch)
- `crates/gradle2nix/src/tapi/jar_source.rs`: dual-mode JAR loading — embedded via `include_bytes!` or overridden via `GRADLE2NIX_TAPI_SHIM_PATH` env var
- `crates/nix-core/`: SRI hash format conversion (`sha256:hex` → `sha256-base64`)
- `tapi-shim/`: Kotlin/Gradle Tooling API shim that extracts Maven dependencies from an `IdeaProject` model and outputs JSON to stdout
- 57 passing tests across gradle2nix and nix-core crates

### Architecture
- gradle2nix binary is self-contained: embeds tapi-shim JAR at compile time via `include_bytes!`, extracts to a temp file at runtime
- All Maven SHA-256 resolution is parallel with configurable concurrency (`max_concurrency: 10` default)
- All-or-nothing batch semantics: if any dependency fails to resolve, the entire batch fails (no partial lockfiles)

---

## Phase 0 — Repository Scaffolding (2026-06-05)

### Added
- Cargo workspace with five crates: `gradle2nix`, `ios2nix`, `flutter2nix`, `nix-core`, `fnx`
- `flake.nix` with fenix Rust toolchain, devShell, and Phase 0 package stubs
- `tapi-shim/`: Kotlin/Gradle project scaffold
- CI-ready structure with `cargo check`, `cargo clippy`, and `nix flake check`
