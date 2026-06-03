# Phase 0: flutter2nix Monorepo Initialization (Scaffold Only)

> **Status: PENDING APPROVAL** — Consensus reached (Planner → Architect → Critic ITERATE → fixes applied). Ready for execution.

**Objective:** Create a valid, compilable Rust monorepo skeleton with a valid Nix flake and GitHub Actions CI, all with minimal stub content. **No implementation logic anywhere.**

**Success Condition:** `cargo check` passes, `nix flake check` passes, CI pipeline runs green on Day 1.

**All 8 acceptance criteria at end of document are REQUIRED before Phase 0 PR merge.**

---

## RALPLAN-DR Summary

### Principles
1. **Stub-first design**: Minimal content makes compilation explicit; implementation is Phase 1–3.
2. **Day-1 green CI**: Structural checks only (cargo, clippy, flake); e2e tests are allowed-to-fail placeholders.
3. **Tree-before-logic**: Directory structure exactly matches spec; no implementation details in stubs.
4. **Zero dependencies on unwritten code**: Each crate compiles as-is; no circular deps or incomplete traits.
5. **Nix + Rust parity**: flake.nix and Cargo.toml syntax valid; no runtime expectations.

### Decision Drivers
1. **Parallelizability**: Small, independent crates allow Phase 1 teams to work in parallel without merge conflicts.
2. **First PR integration**: CI must be green immediately; failing tests block later PRs.
3. **Onboarding clarity**: New contributors see the full tree, understand boundaries, and know where to add code.

### Viable Options

**Option A: Scaffold now, implement later (CHOSEN)**
- Day 0: Create all files with empty stubs, real Cargo/Nix syntax, working CI.
- Day 1: Teams start Phase 1 implementation in their crates.
- Trade-off: More files upfront, but zero integration surprises later; parallel work unlocked immediately.

**Option B: Implement gradle2nix first, scaffold others**
- Day 0: Build gradle2nix fully; scaffold rest as empty stubs.
- Day 1: Integrate gradle2nix into flake, then implement others.
- Trade-off: One crate ready earlier, but blocks parallel work; higher merge risk.

**Why Option A wins:**
- Eliminates async dependencies; teams start immediately.
- Single "Phase 0" PR with all structure; no incremental scaffolding PRs cluttering history.
- CI is green on Day 1, unblocking Phase 1 PRs with confidence.

---

## Work Plan: 10 Groups

### 1. Root Files (Cargo.toml, rust-toolchain.toml, .gitignore, README, CONTRIBUTING)

**File:** `/Cargo.toml`
```toml
[workspace]
resolver = "2"
members = [
  "crates/nix-core",
  "crates/gradle2nix",
  "crates/ios2nix",
  "crates/flutter2nix",
]

[workspace.package]
version = "0.1.0"
edition = "2021"
```

**File:** `/rust-toolchain.toml`
```toml
[toolchain]
channel = "stable"
```

**File:** `/.gitignore`
```
# Rust
/target/
**/*.rs.bk
*.pdb
*.swp
*.swo
*~
.DS_Store

# Nix
result/
result-*
.dirlocals

# IDEs
.vscode/
.idea/
*.iml
*.sublime-workspace

# Generated
/nix-core/generated/
```

**File:** `/README.md`
```markdown
# flutter2nix

A modular Nix toolchain for building Flutter apps on Android and iOS without Google/Apple's build tools.

## Crates

- **nix-core**: Dependency parsing (gradle, pub, cocoapods). Core models and code generation.
- **gradle2nix**: Android/Gradle lockfile → Nix derivations.
- **ios2nix**: iOS/CocoaPods lockfile → Nix derivations.
- **flutter2nix**: Pub/Flutter.yaml → Nix derivations. Main entry point.

## Quick Start

```bash
git clone https://github.com/loke/flutter2nix.git
cd flutter2nix
nix flake show
cargo check
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup.
```

**File:** `/CONTRIBUTING.md`
```markdown
# Contributing to flutter2nix

## Setup

```bash
# Clone and enter nix develop
git clone ...
cd flutter2nix
nix flake update
nix develop

# Verify compilation
cargo check
cargo clippy -- -D warnings
nix flake check
```

## Structure

- `/crates`: Rust workspaces. Each is a separate crate.
- `/nix`: Nix library modules (gradle2nix-lib.nix, ios2nix-lib.nix, flutter2nix-lib.nix).
- `/tapi-shim`: Gradle plugin wrapper (Kotlin). Stateless; used as a build hook.
- `/tests/fixtures`: Test data (gradle builds, flutter projects).

## Adding Code

1. Choose your crate (`crates/gradle2nix`, `crates/ios2nix`, etc.).
2. Add modules under `src/` or in subdirectories.
3. Run `cargo check` and `cargo clippy`.
4. Commit with conventional message: `feat(gradle2nix): add parser`, `fix(ios2nix): handle variants`.

## Testing

e2e tests land in Phase 1. For now:
```bash
cargo test --all
```

## CI

Pushes to `main` and PRs run:
- `cargo check` (all workspace crates)
- `cargo clippy`
- `nix flake check`
- e2e tests (allowed to fail)

See `.github/workflows/ci.yml`.
```

**Acceptance:** All four files exist, syntax is valid (can read without error).

---

### 2. flake.nix + flake.lock

**File:** `/flake.nix`
```nix
{
  description = "flutter2nix: Nix toolchain for building Flutter apps";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, fenix }:
    # lib is top-level (not per-system) so consumers access lib.buildAndroidApp directly
    {
      lib = {
        buildAndroidApp = _: throw "buildAndroidApp: not implemented — see Phase 1";
        buildIOSApp = _: throw "buildIOSApp: not implemented — see Phase 1";
        buildFlutterApp = _: throw "buildFlutterApp: not implemented — see Phase 1";
      };
    } // flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        rust = fenix.packages.${system}.stable;
      in
      {
        # Dev shell for development
        devShells.default = pkgs.mkShell {
          buildInputs = [
            rust.toolchain
            pkgs.nixpkgs-fmt
          ];
        };

        # Packages: stubs — replaced in Phase 1 with real Rust binaries
        packages = {
          gradle2nix = pkgs.emptyDirectory;
          ios2nix = pkgs.emptyDirectory;
          flutter2nix = pkgs.emptyDirectory;
          default = self.packages.${system}.flutter2nix;
        };

        # Checks: cargo + clippy + nix flake syntax
        # Uses src = ./. (path literal) to avoid import-from-derivation issues
        checks = {
          cargo-check = pkgs.runCommand "cargo-check" {
            buildInputs = [ rust.toolchain ];
            src = ./.;
          } ''
            cp -r $src src
            cd src
            cargo check --workspace
            mkdir -p $out
          '';
          cargo-clippy = pkgs.runCommand "cargo-clippy" {
            buildInputs = [ rust.toolchain ];
            src = ./.;
          } ''
            cp -r $src src
            cd src
            cargo clippy --workspace -- -D warnings
            mkdir -p $out
          '';
          default = pkgs.runCommand "flake-check-ok" {} "echo ok > $out";
        };
      }
    );
}
```

**File:** `/flake.lock`

(Auto-generated by `nix flake update`; include in repo post-Phase-0. For now, touch it empty or generate via script.)

**Acceptance:** `nix flake show` runs without parse errors; `nix flake check` passes (echo ok); `lib.buildAndroidApp` accessible at flake root (not `lib.<system>.buildAndroidApp`).

---

### 3. nix-core Crate Scaffold

**File:** `/crates/nix-core/Cargo.toml`
```toml
[package]
name = "nix-core"
version.workspace = true
edition.workspace = true

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
anyhow = "1"
```

**File:** `/crates/nix-core/src/lib.rs`
```rust
pub mod dep;
pub mod codegen;
```

**File:** `/crates/nix-core/src/dep.rs`
```rust
use serde::{Deserialize, Serialize};

/// Stub: represents a locked dependency (Maven, CocoaPods, Pub, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockedDependency {
    pub name: String,
    pub version: String,
}

/// Stub: represents a dependency graph (for analysis and codegen)
#[derive(Debug, Clone)]
pub struct DependencyGraph {
    pub nodes: Vec<LockedDependency>,
}
```

**File:** `/crates/nix-core/src/codegen/mod.rs`
```rust
pub mod nix_writer;
pub mod maven;
pub mod cocoapods;
pub mod pub_deps;
```

**File:** `/crates/nix-core/src/codegen/nix_writer.rs`
```rust
/// Stub: generates Nix expressions from dependency graphs
pub struct NixExprWriter;

impl NixExprWriter {
    pub fn new() -> Self {
        Self
    }
}
```

**File:** `/crates/nix-core/src/codegen/maven.rs`
```rust
/// Stub: Maven/Gradle dependency parsing
pub fn parse_gradle_lock() -> anyhow::Result<()> {
    Ok(())
}
```

**File:** `/crates/nix-core/src/codegen/cocoapods.rs`
```rust
/// Stub: CocoaPods dependency parsing
pub fn parse_podfile_lock() -> anyhow::Result<()> {
    Ok(())
}
```

**File:** `/crates/nix-core/src/codegen/pub_deps.rs`
```rust
/// Stub: Pub/Flutter dependency parsing
pub fn parse_pubspec_lock() -> anyhow::Result<()> {
    Ok(())
}
```

**Acceptance:** `cargo check -p nix-core` passes; `cargo clippy -p nix-core` passes.

---

### 4. gradle2nix Crate Scaffold

**File:** `/crates/gradle2nix/Cargo.toml`
```toml
[package]
name = "gradle2nix"
version.workspace = true
edition.workspace = true

[[bin]]
name = "gradle2nix"
path = "src/main.rs"

[dependencies]
clap = { version = "4", features = ["derive"] }
anyhow = "1"
nix-core = { path = "../nix-core" }
serde_json = "1"
```

**File:** `/crates/gradle2nix/src/main.rs`
```rust
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "gradle2nix")]
#[command(about = "Convert Gradle lockfiles to Nix", long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Lock Gradle project
    Lock,
    /// Check Gradle project
    Check,
    /// Generate Nix expressions
    Generate,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    
    match args.command {
        Some(Command::Lock) => cli::lock::run()?,
        Some(Command::Check) => cli::check::run()?,
        Some(Command::Generate) => cli::generate::run()?,
        None => println!("gradle2nix: use --help for subcommands"),
    }
    
    Ok(())
}

mod cli;
mod tapi;
mod maven;
mod lockfile;
```

**File:** `/crates/gradle2nix/src/cli/mod.rs`
```rust
pub mod lock;
pub mod check;
pub mod generate;
```

**File:** `/crates/gradle2nix/src/cli/lock.rs`
```rust
/// Stub: lock Gradle project
pub fn run() -> anyhow::Result<()> {
    println!("gradle2nix lock: not implemented");
    Ok(())
}
```

**File:** `/crates/gradle2nix/src/cli/check.rs`
```rust
/// Stub: check Gradle project
pub fn run() -> anyhow::Result<()> {
    println!("gradle2nix check: not implemented");
    Ok(())
}
```

**File:** `/crates/gradle2nix/src/cli/generate.rs`
```rust
/// Stub: generate Nix from Gradle
pub fn run() -> anyhow::Result<()> {
    println!("gradle2nix generate: not implemented");
    Ok(())
}
```

**File:** `/crates/gradle2nix/src/tapi/mod.rs`
```rust
pub mod shim;
pub mod model;
```

**File:** `/crates/gradle2nix/src/tapi/shim.rs`
```rust
/// Stub: TAPI shim integration
pub struct TapiShim;
```

**File:** `/crates/gradle2nix/src/tapi/model.rs`
```rust
/// Stub: TAPI data models
pub struct TapiModel;
```

**File:** `/crates/gradle2nix/src/maven.rs`
```rust
/// Stub: Maven-specific logic
pub fn parse_maven_metadata() -> anyhow::Result<()> {
    Ok(())
}
```

**File:** `/crates/gradle2nix/src/lockfile.rs`
```rust
/// Stub: Gradle lockfile parsing
pub fn parse_lockfile() -> anyhow::Result<()> {
    Ok(())
}
```

**Acceptance:** `cargo check -p gradle2nix` passes; binary compiles with `cargo build -p gradle2nix`.

---

### 5. ios2nix Crate Scaffold

**File:** `/crates/ios2nix/Cargo.toml`
```toml
[package]
name = "ios2nix"
version.workspace = true
edition.workspace = true

[[bin]]
name = "ios2nix"
path = "src/main.rs"

[dependencies]
clap = { version = "4", features = ["derive"] }
anyhow = "1"
nix-core = { path = "../nix-core" }
serde_json = "1"
```

**File:** `/crates/ios2nix/src/main.rs`
```rust
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "ios2nix")]
#[command(about = "Convert iOS/CocoaPods lockfiles to Nix", long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Lock iOS project
    Lock,
    /// Build iOS project
    Build,
    /// Archive iOS app
    Archive,
    /// Export iOS app
    Export,
    /// Sign iOS app
    Sign,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    
    match args.command {
        Some(Command::Lock) => cli::lock::run()?,
        Some(Command::Build) => cli::build::run()?,
        Some(Command::Archive) => cli::archive::run()?,
        Some(Command::Export) => cli::export::run()?,
        Some(Command::Sign) => cli::sign::run()?,
        None => println!("ios2nix: use --help for subcommands"),
    }
    
    Ok(())
}

mod cli;
mod xcode;
mod cocoapods;
mod keychain;
mod export_opts;
```

**File:** `/crates/ios2nix/src/cli/mod.rs`
```rust
pub mod lock;
pub mod build;
pub mod archive;
pub mod export;
pub mod sign;
```

**File:** `/crates/ios2nix/src/cli/lock.rs`
```rust
pub fn run() -> anyhow::Result<()> {
    println!("ios2nix lock: not implemented");
    Ok(())
}
```

**File:** `/crates/ios2nix/src/cli/build.rs`
```rust
pub fn run() -> anyhow::Result<()> {
    println!("ios2nix build: not implemented");
    Ok(())
}
```

**File:** `/crates/ios2nix/src/cli/archive.rs`
```rust
pub fn run() -> anyhow::Result<()> {
    println!("ios2nix archive: not implemented");
    Ok(())
}
```

**File:** `/crates/ios2nix/src/cli/export.rs`
```rust
pub fn run() -> anyhow::Result<()> {
    println!("ios2nix export: not implemented");
    Ok(())
}
```

**File:** `/crates/ios2nix/src/cli/sign.rs`
```rust
pub fn run() -> anyhow::Result<()> {
    println!("ios2nix sign: not implemented");
    Ok(())
}
```

**File:** `/crates/ios2nix/src/xcode/mod.rs`
```rust
pub mod assert;
pub mod env;
```

**File:** `/crates/ios2nix/src/xcode/assert.rs`
```rust
/// Stub: Xcode assertions
pub fn check_xcode() -> anyhow::Result<()> {
    Ok(())
}
```

**File:** `/crates/ios2nix/src/xcode/env.rs`
```rust
/// Stub: Xcode environment setup
pub fn setup_env() -> anyhow::Result<()> {
    Ok(())
}
```

**File:** `/crates/ios2nix/src/cocoapods.rs`
```rust
/// Stub: CocoaPods integration
pub fn parse_podfile() -> anyhow::Result<()> {
    Ok(())
}
```

**File:** `/crates/ios2nix/src/keychain.rs`
```rust
/// Stub: Keychain integration
pub fn unlock_keychain() -> anyhow::Result<()> {
    Ok(())
}
```

**File:** `/crates/ios2nix/src/export_opts.rs`
```rust
/// Stub: ExportOptions.plist generation
pub fn generate_export_opts() -> anyhow::Result<()> {
    Ok(())
}
```

**Acceptance:** `cargo check -p ios2nix` passes; binary compiles with `cargo build -p ios2nix`.

---

### 6. flutter2nix Crate Scaffold

**File:** `/crates/flutter2nix/Cargo.toml`
```toml
[package]
name = "flutter2nix"
version.workspace = true
edition.workspace = true

[[bin]]
name = "flutter2nix"
path = "src/main.rs"

[dependencies]
clap = { version = "4", features = ["derive"] }
anyhow = "1"
nix-core = { path = "../nix-core" }
serde_json = "1"
```

**File:** `/crates/flutter2nix/src/main.rs`
```rust
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "flutter2nix")]
#[command(about = "Convert Flutter projects to Nix", long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Lock Flutter project
    Lock,
    /// Build Flutter app
    Build,
    /// Check Flutter project
    Check,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    match args.command {
        Some(Command::Lock) => cli::lock::run()?,
        Some(Command::Build) => cli::build::run()?,
        Some(Command::Check) => cli::check::run()?,
        None => println!("flutter2nix: use --help for subcommands"),
    }

    Ok(())
}

mod cli;
mod pub_deps;   // src/pub_deps/mod.rs  (note: `pub` is a reserved keyword in Rust)
mod detect;
mod sdk;
mod compose;
```

**File:** `/crates/flutter2nix/src/cli/mod.rs`
```rust
pub mod lock;
pub mod build;
pub mod check;
```

**File:** `/crates/flutter2nix/src/cli/lock.rs`
```rust
pub fn run() -> anyhow::Result<()> {
    println!("flutter2nix lock: not implemented");
    Ok(())
}
```

**File:** `/crates/flutter2nix/src/cli/build.rs`
```rust
pub fn run() -> anyhow::Result<()> {
    println!("flutter2nix build: not implemented");
    Ok(())
}
```

**File:** `/crates/flutter2nix/src/cli/check.rs`
```rust
pub fn run() -> anyhow::Result<()> {
    println!("flutter2nix check: not implemented");
    Ok(())
}
```

**File:** `/crates/flutter2nix/src/pub_deps/mod.rs`
```rust
// Note: directory is named `pub_deps/` because `pub` is a Rust keyword.
// The spec refers to this as the `pub/` module; Rust requires a non-keyword name.
pub mod resolver;
pub mod codegen;
```

**File:** `/crates/flutter2nix/src/pub_deps/resolver.rs`
```rust
/// Stub: pubspec.lock parser → LockedDependency[]
pub fn resolve_pub_deps() -> anyhow::Result<()> {
    Ok(())
}
```

**File:** `/crates/flutter2nix/src/pub_deps/codegen.rs`
```rust
/// Stub: pub section codegen → delegates to nix-core
pub fn generate_pub_section() -> anyhow::Result<()> {
    Ok(())
}
```

**File:** `/crates/flutter2nix/src/detect.rs`
```rust
/// Stub: Flutter project detection
pub fn detect_flutter_project() -> anyhow::Result<()> {
    Ok(())
}
```

**File:** `/crates/flutter2nix/src/sdk.rs`
```rust
/// Stub: Flutter SDK integration
pub fn check_flutter_sdk() -> anyhow::Result<()> {
    Ok(())
}
```

**File:** `/crates/flutter2nix/src/compose.rs`
```rust
/// Stub: Nix composition
pub fn compose_derivation() -> anyhow::Result<()> {
    Ok(())
}
```

**Acceptance:** `cargo check -p flutter2nix` passes; binary compiles with `cargo build -p flutter2nix`.

---

### 7. tapi-shim Skeleton (Kotlin/Gradle)

**File:** `/tapi-shim/settings.gradle.kts`
```kotlin
rootProject.name = "tapi-shim"
```

**File:** `/tapi-shim/build.gradle.kts`
```kotlin
plugins {
    kotlin("jvm") version "1.9.0"
    application
}

group = "com.loke"
version = "0.1.0"

repositories {
    mavenCentral()
}

dependencies {
    // Stub: no gradle plugin deps yet
}

application {
    mainClass.set("TapiShimKt")
}
```

**File:** `/tapi-shim/src/main/kotlin/TapiShim.kt`
```kotlin
fun main(args: Array<String>) {
    println("TapiShim: stub implementation")
}
```

**Acceptance:** File structure matches Gradle conventions; `gradle build` succeeds (or stub equivalent).

---

### 8. nix/ Library Stubs

**File:** `/nix/gradle2nix-lib.nix`
```nix
{
  buildAndroidApp = _: throw "buildAndroidApp: not implemented — see Phase 1";
}
```

**File:** `/nix/ios2nix-lib.nix`
```nix
{
  buildIOSApp = _: throw "buildIOSApp: not implemented — see Phase 1";
}
```

**File:** `/nix/flutter2nix-lib.nix`
```nix
{
  buildFlutterApp = _: throw "buildFlutterApp: not implemented — see Phase 1";
}
```

**Acceptance:** All three files exist; Nix syntax is valid (parseable by `nix eval`).

---

### 9. docs/ + tests/fixtures/ Placeholders

**File:** `/docs/ios-testing.md`
```markdown
# iOS Testing

TODO: iOS testing strategy lands in Phase 1.

See: `crates/ios2nix/src/`
```

**File:** `/docs/gradle2nix-standalone.md`
```markdown
# gradle2nix Standalone Mode

TODO: Standalone gradle2nix usage lands in Phase 1.

See: `crates/gradle2nix/src/`
```

**File:** `/tests/fixtures/gradle/.gitkeep`

(Empty file to preserve directory structure in git.)

**File:** `/tests/fixtures/flutter/.gitkeep`

(Empty file to preserve directory structure in git.)

**Acceptance:** All files exist; `.gitkeep` files force directory creation in git.

---

### 10. .github/workflows/ci.yml

**File:** `/.github/workflows/ci.yml`
```yaml
name: CI

on:
  push:
    branches: [ main ]
  pull_request:
    branches: [ main ]

jobs:
  structural:
    name: Structural Checks (Required)
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Cargo check
        run: cargo check --workspace --all-targets

      - name: Cargo clippy
        run: cargo clippy --workspace --all-targets -- -D warnings

      - name: Install Nix
        uses: cachix/install-nix-action@v25

      - name: Nix flake check
        run: nix flake check

  e2e:
    name: E2E Tests (Allowed to Fail)
    runs-on: ubuntu-latest
    continue-on-error: true
    steps:
      - uses: actions/checkout@v4

      - name: Placeholder e2e
        run: echo "TDD: e2e tests land in Phase 1" && exit 0
```

**Acceptance:** Workflow file is valid YAML; `structural` job runs green; `e2e` job exits 0.

---

## Execution Order (Groups 1–10)

1. **Create root files** (README, CONTRIBUTING, .gitignore, workspace Cargo.toml, rust-toolchain.toml)
2. **Create flake.nix** (generate flake.lock via `nix flake update`)
3. **Create nix-core crate** (lib, lib.rs, dep.rs, codegen/ modules)
4. **Create gradle2nix crate** (all modules and CLI stubs)
5. **Create ios2nix crate** (all modules and CLI stubs)
6. **Create flutter2nix crate** (all modules and CLI stubs)
7. **Create tapi-shim** (Kotlin gradle project skeleton)
8. **Create nix/ library files** (gradle2nix-lib.nix, ios2nix-lib.nix, flutter2nix-lib.nix)
9. **Create docs/ and tests/fixtures/** (markdown placeholders, .gitkeep files)
10. **Create .github/workflows/ci.yml** (GitHub Actions pipeline)

Each group is **independent** and can be executed in parallel or sequentially. After all files exist:

```bash
cd /home/jacob/Documents/Developer/flutter2nix
nix flake update  # Generate flake.lock
cargo check       # Verify all crates compile
nix flake check   # Verify Nix syntax
```

---

## Acceptance Criteria (Phase 0 Complete)

Run these checks to confirm scaffold is valid:

1. **Cargo check passes:**
   ```bash
   cargo check --workspace
   ```
   Expected: No errors. (Warnings from unused stubs are OK.)

2. **Cargo clippy passes:**
   ```bash
   cargo clippy --workspace -- -D warnings
   ```
   Expected: Clean (or only allow dead-code/unused warnings as exemptions if needed).

3. **Nix flake check passes:**
   ```bash
   nix flake check
   ```
   Expected: No parse errors; `checks.default` returns true.

4. **All directories exist:**
   ```bash
   find . \( -type d -name "src" -o -name "cli" -o -name "codegen" -o -name "xcode" \) | wc -l
   ```
   Expected: >= 10 directories.

5. **All Cargo.toml files present and parseable:**
   ```bash
   find . -name "Cargo.toml" -type f | while read f; do cargo metadata --manifest-path "$f" > /dev/null 2>&1 && echo "✓ $f" || echo "✗ $f"; done
   ```
   Expected: All pass.

6. **CI workflow file exists and is valid YAML:**
   ```bash
   cat .github/workflows/ci.yml | python3 -c "import sys, yaml; yaml.safe_load(sys.stdin)" && echo "✓ CI valid"
   ```
   Expected: ✓ CI valid.

7. **README and CONTRIBUTING exist:**
   ```bash
   test -f README.md && test -f CONTRIBUTING.md && echo "✓ Docs present"
   ```
   Expected: ✓ Docs present.

8. **No implementation code exists** (only stubs):
   ```bash
   grep -r "TODO\|FIXME\|unimplemented\|panic" crates/*/src/*.rs 2>/dev/null | wc -l
   ```
   Expected: 0 (all unimplemented cases use `println!("...")` or throw errors in Nix).

---

## Summary

| Group | Files | Status |
|-------|-------|--------|
| 1. Root files | 5 | Stub (README, CONTRIBUTING, .gitignore, Cargo.toml, rust-toolchain.toml) |
| 2. flake.nix | 2 | Stub (flake.nix, flake.lock auto) |
| 3. nix-core | 6 | Stub (lib.rs, dep.rs, codegen modules) |
| 4. gradle2nix | 9 | Stub (main.rs, CLI, tapi, maven, lockfile modules) |
| 5. ios2nix | 10 | Stub (main.rs, CLI, xcode, cocoapods, keychain, export_opts) |
| 6. flutter2nix | 10 | Stub (main.rs, CLI, pub_deps/{mod,resolver,codegen}, detect, sdk, compose) |
| 7. tapi-shim | 3 | Stub (Kotlin gradle project) |
| 8. nix/ libs | 3 | Stub (gradle2nix-lib.nix, ios2nix-lib.nix, flutter2nix-lib.nix) |
| 9. docs/tests | 4 | Stub (markdown placeholders, .gitkeep) |
| 10. CI | 1 | GitHub Actions (structural + e2e) |
| **TOTAL** | **53** | **All compiles, all CI green Day 1** |

---

## Next Steps (Phase 1+)

Phase 0 complete → Phase 1 teams can immediately:
- Add parsing logic to `nix-core`
- Implement Gradle locking in `gradle2nix`
- Implement CocoaPods/iOS in `ios2nix`
- Implement Pub/Flutter resolution in `flutter2nix`
- All without merge conflicts on scaffold PRs.

CI blocks unimplemented code → teams ship incrementally, unblocking e2e tests.
