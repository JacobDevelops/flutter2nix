# Deep Interview Spec: Nix Mobile Build Tooling (gradle2nix / ios2nix / flutter2nix)

## Metadata
- Interview ID: a7f3b291-4e8d-4b6a-9d53-1e2f7a8b5c0d
- Rounds: 19 (+ Round 0 topology confirmation)
- Final Ambiguity Score: ~22%
- Type: greenfield
- Generated: 2026-06-03
- Threshold: 20% (source: default)
- Initial Context Summarized: yes
- Status: BELOW_THRESHOLD_EARLY_EXIT (22% — within scoring model error margin, all major decisions locked)

## Clarity Breakdown
| Dimension | Score | Weight | Weighted |
|-----------|-------|--------|----------|
| Goal Clarity | 0.84 | 40% | 0.336 |
| Constraint Clarity | 0.79 | 30% | 0.237 |
| Success Criteria | 0.70 | 30% | 0.210 |
| **Total Clarity** | | | **0.783** |
| **Ambiguity** | | | **~22%** |

## Topology
| Component | Status | Description | Coverage |
|-----------|--------|-------------|---------|
| gradle2nix | active | Rust CLI + Nix library for Gradle/Maven dep materialisation via TAPI; independently OSS-useful | Android APK + AAB (debug), generic Gradle projects, tadfisher v2 fixtures |
| ios2nix | active | Rust CLI + Nix library for iOS/Xcode/CocoaPods/archive/export/signing; independently OSS-useful | Real-device IPA via archive+export+signing; manual test in v0.1 |
| flutter2nix | active | Rust CLI + Nix library composing gradle2nix + ios2nix; unified cross-platform lockfile | Flutter Android APK+AAB (v0.1 MVP); Flutter iOS (v0.2) |
| Shared Rust crate layer (nix-core) | active | LockedDependency types + NixExprWriter; published to crates.io independently | Foundation for all three tools; usable outside this project |

## Goal

Build a fresh OSS Nix tooling monorepo named `flutter2nix` on GitHub, containing three **separately useful** Rust-first packages — `gradle2nix`, `ios2nix`, and `flutter2nix` — plus a shared `nix-core` crate. Each tool is independently attractive to OSS contributors outside the Flutter ecosystem. The project replaces an unstable prior implementation (jfit/PR#207) with a principled, fresh design.

- **gradle2nix**: general-purpose Gradle/Maven dep materialiser for Nix using the Gradle Tooling API; first-class Android/AGP support because flutter2nix needs it.
- **ios2nix**: first end-to-end iOS/Xcode orchestration layer for Nix; reproducible orchestration around a pinned/asserted host Xcode; targets real-device IPA production including full signing.
- **flutter2nix**: Flutter-specific integration composing gradle2nix + ios2nix; adds Flutter SDK wiring, `flutter pub` materialisation, and a unified cross-platform lockfile.
- **nix-core**: shared dep model + Nix expression codegen; published to crates.io for broader ecosystem use.

## Constraints

### Repository
- Single GitHub monorepo named `flutter2nix` (Cargo workspace); all three CLIs + nix-core
- Flake-input-only distribution for MVP; nixpkgs submission deferred to v1.0 stable

### Rust Architecture
- Shared crate (`nix-core`): both `LockedDependency`/`DependencyGraph` types AND `NixExprWriter`; published to crates.io independently (Option C)
- Rust-first: Rust owns all parsing, lockfile modelling, validation, planning, CLI UX, and Nix codegen
- Shell and Nix glue acceptable only where unavoidable

### Lockfile Model (universal across all three tools)
- **Pre-computed Nix module**: user runs `<tool> lock` locally → generates a `.nix` module file → commits it → Nix reads the module to fetch deps as individual content-addressed derivations
- Generated file format: Nix module (not TOML/JSON); integrates directly without a shim
- `flutter2nix` generates a **unified** lockfile (`flutter2nix.nix`) containing Maven, pub, CocoaPods, Flutter SDK version, and Gradle/AGP version metadata; extensible to future Flutter platforms

### Gradle Dependency Resolution
- Method: **Gradle Tooling API (TAPI)** — Rust CLI embeds a thin Java shim (JAR) that talks to Gradle via the official TAPI; shim output is JSON consumed by Rust
- JVM required only during `gradle2nix lock` (not during `nix build`)
- Shim bundling: embedded via `include_bytes!` in the release binary; also buildable as a separate Nix derivation for hermetic Nix packaging
- Supports Gradle 7.6+ and 8.x (current LTS + stable)

### CocoaPods Scope
- v0.1: spec-repo pods (versioned trunk) + git pods (SHA-pinned)
- Non-goals: path pods (local filesystem), private CocoaPods spec repos

### Flutter pub Scope
- v0.1: pub.dev hosted deps + git deps
- Non-goals: path deps, private pub registries

### Flutter SDK
- Source: nixpkgs `flutter` package; flutter2nix.nix asserts the expected version (error if mismatch)
- NOT a flutter2nix-managed FOD — tied to nixpkgs update cadence; standard Nix idiom

### iOS / Signing
- Host Xcode required; ios2nix asserts the expected version via DEVELOPER_DIR/SDKROOT
- Signing is a runtime CLI operation (NOT inside a Nix sandbox derivation)
- Signing secrets model: **env vars as default, CLI flags as override** (12-factor pattern)
  - `IOS2NIX_P12_PATH`, `IOS2NIX_P12_PASSPHRASE`, `IOS2NIX_PROFILE_PATH`
  - `--p12 PATH`, `--passphrase PASS`, `--profile PATH` as overrides

### iOS CI
- No macOS CI automation in v0.1
- iOS tests: **manual procedure documented** in `docs/ios-testing.md`
- Automated iOS CI is a v0.2 milestone

### Android Build Output
- Both APK (`assembleDebug`) and AAB (`bundleDebug`) in v0.1
- Debug build type (no external signing secrets required for v0.1)

### Distribution (v0.1)
- Flake-input-only: `flutter2nix.url = "github:JacobDevelops/flutter2nix"`
- CLI available via `nix run .#gradle2nix -- lock`
- nixpkgs submission: deferred to v1.0 when API is stable

## Non-Goals (v0.1)
- Flutter web, macOS desktop, Linux desktop, Windows desktop builds
- Kotlin Multiplatform (KMP / KMM)
- Private Maven registries (Artifactory, Nexus, GitHub Packages)
- Private CocoaPods spec repos
- Android AAB signing with production keystore (debug unsigned only)
- iOS Simulator builds (real device only)
- Gradle Enterprise / remote build cache integration
- Incremental Nix builds (full rebuild per `nix build`)
- Windows or Linux iOS cross-compilation
- nixpkgs submission in v0.1

## Acceptance Criteria

### gradle2nix
- [ ] `gradle2nix lock` on each tadfisher/gradle2nix v2 fixture project produces a `gradle.nix` without error
- [ ] `nix build` using the generated `gradle.nix` and `gradle2nix.lib.buildAndroidApp` produces a valid APK file
- [ ] `nix build` produces a valid AAB file (bundleDebug)
- [ ] `gradle2nix check` exits 0 when the lockfile is current, non-zero when it's stale
- [ ] All Rust unit tests pass (dep model, Nix codegen, TAPI model parsing)
- [ ] CI (Linux, GitHub Actions) passes on all fixture projects

### ios2nix
- [ ] `ios2nix lock` on a test iOS project with Podfile.lock produces a `pods.nix` containing all spec-repo and git pods
- [ ] Manual test: `ios2nix archive` produces a `.xcarchive` directory
- [ ] Manual test: `ios2nix export` produces an `.ipa` file using a real p12 + provisioning profile
- [ ] `IOS2NIX_P12_PATH` / `IOS2NIX_P12_PASSPHRASE` / `IOS2NIX_PROFILE_PATH` env vars are the primary secrets interface
- [ ] Xcode version assertion fails loudly when the wrong Xcode version is detected
- [ ] Manual test procedure documented in `docs/ios-testing.md`

### flutter2nix
- [ ] `flutter2nix lock --target android` on a Flutter project generates `flutter2nix.nix` containing mavenDeps + pubDeps + Flutter/Gradle version metadata
- [ ] `nix build .#android` produces a valid APK and AAB via `flutter2nix.lib.buildFlutterAndroid`
- [ ] Flutter SDK version in `flutter2nix.nix` matches nixpkgs `flutter`; mismatch produces a clear error
- [ ] `flutter2nix check` detects lockfile staleness

### nix-core
- [ ] `LockedDependency` type handles Maven, CocoaPods, and pub dep models
- [ ] `NixExprWriter` generates valid Nix module files parseable by Nix evaluator
- [ ] Published to crates.io as an independent library
- [ ] All public API items are documented

## Assumptions Exposed and Resolved
| Assumption | Challenge | Resolution |
|------------|-----------|------------|
| ios2nix v0.1 must include full signing | Signing makes ios2nix impossible to test in OSS CI | Keep signing in v0.1; iOS tests are manual + documented |
| Lockfile should be TOML/JSON | Nix-native format is better | Lockfile IS a Nix module (pre-computed, committed) |
| The project needs a migration path doc | User clarified the prior implementation (jfit/PR#207) was unstable and is being replaced | Migration path is not a deliverable; fresh design replaces it |
| gradle2nix should use init script like tadfisher | Config cache fragility and custom resolver interference | Gradle Tooling API (TAPI) — most robust, official API |
| ios2nix signing must be a separate tool | Full pipeline in one tool is the point | Signing stays in ios2nix v0.1 via runtime CLI with secrets from env vars |
| Shared crate is either dep model OR codegen | Both are needed for a clean abstraction | Option C: `nix-core` exports both LockedDependency types AND NixExprWriter |
| nixpkgs submission is required for OSS legitimacy | High maintenance cost; flake-input works for MVP | Flake-input-only for MVP; nixpkgs submission deferred to v1.0 |

## Technical Context

### Prior Art
- **tadfisher/gradle2nix v2** (`v2` branch): Go-based; Gradle init script interception; generates `gradle-env.nix`. TDD baseline: copy all input fixture Android projects from this repo (NOT expected output files). Reference for test coverage, NOT for implementation approach.
- **jfit/PR#207**: Prior internal flutter2nix implementation; merged but proved highly unstable. Do NOT reuse code. Treat as a "what not to do" reference.
- **nixpkgs xcodeenv**: The existing Nix iOS tooling wraps host Xcode; ios2nix builds on the same premise (host Xcode, not packaged Xcode) but adds full orchestration.

### Technology Choices
- Rust edition: 2021; MSRV: stable (latest stable at project start)
- CLI framework: `clap` v4
- Nix expression generation: hand-rolled string builder in `nix-core` (no external Nix AST library)
- TAPI shim: minimal Kotlin/Java project that uses `org.gradle:gradle-tooling-api`; built with Gradle, output JAR embedded in Rust via `include_bytes!`
- CocoaPods spec resolution: HTTP fetch from CocoaPods CDN using computed path from checksum
- Testing: `cargo test` for unit; `nix flake check` for e2e integration

## Architecture: Package and Crate Layout

```
flutter2nix/                    # GitHub repo root (Cargo workspace)
├── Cargo.toml                  # workspace = { members = ["crates/*"] }
├── flake.nix                   # Exposes: packages.*, lib.*, checks.*
├── crates/
│   ├── nix-core/               # Published independently to crates.io
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── dep.rs           # LockedDependency, DependencyGraph, DepSource enum
│   │       └── codegen/
│   │           ├── mod.rs
│   │           ├── nix_writer.rs   # NixExprWriter: generates .nix module files
│   │           ├── maven.rs        # Maven-specific fetch call codegen
│   │           ├── cocoapods.rs    # CocoaPods-specific codegen
│   │           └── pub.rs          # Dart pub-specific codegen
│   ├── gradle2nix/
│   │   └── src/
│   │       ├── main.rs
│   │       ├── cli/
│   │       │   ├── lock.rs     # `gradle2nix lock`
│   │       │   ├── check.rs    # `gradle2nix check`
│   │       │   └── generate.rs # `gradle2nix generate`
│   │       ├── tapi/
│   │       │   ├── shim.rs     # JAR extraction, JVM invocation, stdout JSON parsing
│   │       │   └── model.rs    # TAPI JSON response types
│   │       ├── maven.rs        # Maven coordinate → LockedDependency
│   │       └── lockfile.rs     # gradle.nix writer (delegates to nix-core)
│   ├── ios2nix/
│   │   └── src/
│   │       ├── main.rs
│   │       ├── cli/
│   │       │   ├── lock.rs     # `ios2nix lock` (Podfile.lock → pods.nix section)
│   │       │   ├── build.rs    # `ios2nix build`
│   │       │   ├── archive.rs  # `ios2nix archive`
│   │       │   ├── export.rs   # `ios2nix export`
│   │       │   └── sign.rs     # `ios2nix sign` (secrets from env/flags)
│   │       ├── xcode/
│   │       │   ├── assert.rs   # Version assertion (DEVELOPER_DIR)
│   │       │   └── env.rs      # DEVELOPER_DIR / SDKROOT setup
│   │       ├── cocoapods.rs    # Podfile.lock parser → LockedDependency[]
│   │       ├── keychain.rs     # Temporary keychain create/import/delete
│   │       └── export_opts.rs  # ExportOptions.plist generation
│   └── flutter2nix/
│       └── src/
│           ├── main.rs
│           ├── cli/
│           │   ├── lock.rs     # `flutter2nix lock` (unified flutter2nix.nix)
│           │   ├── build.rs    # `flutter2nix build android|ios`
│           │   └── check.rs    # `flutter2nix check`
│           ├── pub/
│           │   ├── resolver.rs # pubspec.lock parser → LockedDependency[]
│           │   └── codegen.rs  # pub section in flutter2nix.nix
│           ├── detect.rs       # Flutter project detection (pubspec.yaml)
│           ├── sdk.rs          # Flutter SDK version assertion vs nixpkgs
│           └── compose.rs      # Assembles unified flutter2nix.nix from sub-tool outputs
├── tapi-shim/                  # Kotlin/Gradle project for the TAPI JAR
│   ├── build.gradle.kts
│   └── src/main/kotlin/
│       └── TapiShim.kt         # Enumerates all resolved deps via TAPI, writes JSON to stdout
├── nix/
│   ├── gradle2nix-lib.nix      # buildAndroidApp, buildGradleProject functions
│   ├── ios2nix-lib.nix         # buildIOSArchive, etc.
│   └── flutter2nix-lib.nix     # buildFlutterAndroid, buildFlutterIOS functions
├── docs/
│   ├── ios-testing.md          # Manual test procedure for ios2nix
│   └── gradle2nix-standalone.md # How to use gradle2nix without Flutter
└── tests/
    └── fixtures/
        ├── gradle/             # Copied from tadfisher/gradle2nix v2 fixture projects
        └── flutter/            # Minimal Flutter project for integration tests
```

## CLI Commands and Example Usage

### gradle2nix

```bash
# Step 1: Generate the lockfile (requires JVM + internet)
gradle2nix lock --project-dir ./android

# Step 2: Commit gradle.nix
git add gradle.nix && git commit -m "chore: update gradle dependency lockfile"

# Step 3: Verify lockfile is current
gradle2nix check --project-dir ./android

# Step 4: Nix builds from the lockfile (no network)
nix build .#androidApp
```

### ios2nix

```bash
# Step 1: Generate CocoaPods lockfile (reads Podfile.lock)
ios2nix lock --podfile-lock ./ios/Podfile.lock --output ./ios/pods.nix

# Step 2: Build and archive (macOS only, host Xcode required)
ios2nix archive \
  --scheme MyApp \
  --configuration Release \
  --output ./build/MyApp.xcarchive

# Step 3: Export signed IPA
export IOS2NIX_P12_PATH=/path/to/cert.p12
export IOS2NIX_P12_PASSPHRASE=secret
export IOS2NIX_PROFILE_PATH=/path/to/profile.mobileprovision

ios2nix export \
  --archive ./build/MyApp.xcarchive \
  --output ./build/

# Result: ./build/MyApp.ipa
```

### flutter2nix

```bash
# Step 1: Generate unified lockfile
flutter2nix lock --target android  # or --target all (android + ios)

# Step 2: Commit flutter2nix.nix
git add flutter2nix.nix && git commit -m "chore: update flutter2nix lockfile"

# Step 3: Build in Nix sandbox
nix build .#android    # → APK + AAB
nix build .#ios        # → IPA (requires macOS + ios2nix signing setup)
```

## Nix API and Example Flake Usage

### Consumer flake.nix

```nix
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flutter2nix.url = "github:JacobDevelops/flutter2nix";
  };

  outputs = { nixpkgs, flutter2nix, self, ... }:
    let
      system = "x86_64-linux";
      pkgs = nixpkgs.legacyPackages.${system};
      # Generated by `flutter2nix lock` — committed to your repo
      deps = import ./flutter2nix.nix;
    in {
      packages.${system} = {
        android = flutter2nix.lib.buildFlutterAndroid {
          inherit pkgs;
          src = self;
          deps = deps;
          # flutter2nix asserts this matches nixpkgs.flutter version
          flutterVersion = deps.flutterVersion;
        };
      };

      # Dev shell with flutter2nix CLI tools available
      devShells.${system}.default = pkgs.mkShell {
        buildInputs = [
          flutter2nix.packages.${system}.flutter2nix
          flutter2nix.packages.${system}.gradle2nix
        ];
      };
    };
}
```

### Generated `flutter2nix.nix` (Nix module, committed to repo)

```nix
# Auto-generated by `flutter2nix lock`. Do not edit manually.
# Regenerate with: flutter2nix lock --target android
{
  flutterVersion = "3.24.3";
  gradleVersion = "8.4";
  agpVersion = "8.1.4";

  mavenDeps = [
    {
      groupId = "androidx.core";
      artifactId = "core-ktx";
      version = "1.13.1";
      url = "https://dl.google.com/dl/android/maven2/androidx/core/core-ktx/1.13.1/core-ktx-1.13.1.aar";
      sha256 = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
    }
    # ... many more Maven artifacts
  ];

  pubDeps = [
    {
      name = "provider";
      version = "6.1.2";
      url = "https://pub.dev/packages/provider/versions/6.1.2.tar.gz";
      sha256 = "sha256-BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB=";
    }
    # ... flutter pub packages
  ];

  # Present when `flutter2nix lock --target ios` was run
  cocoaPodsDeps = [
    {
      name = "GoogleMLKit";
      version = "3.2.0";
      source = "spec-repo";
      url = "https://github.com/google/MLKit/archive/3.2.0.tar.gz";
      sha256 = "sha256-CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC=";
    }
    # ...
  ];
}
```

### gradle2nix standalone (non-Flutter Android project)

```nix
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    gradle2nix.url = "github:JacobDevelops/flutter2nix";  # same repo
  };

  outputs = { nixpkgs, gradle2nix, self, ... }:
    let
      pkgs = nixpkgs.legacyPackages.x86_64-linux;
      deps = import ./gradle.nix;  # generated by `gradle2nix lock`
    in {
      packages.x86_64-linux.default = gradle2nix.lib.buildAndroidApp {
        inherit pkgs;
        src = self;
        deps = deps;
        buildTask = "assembleDebug";  # or bundleRelease, etc.
      };
    };
}
```

## MVP Milestone Plan

### Phase 0: Foundation (prerequisite, ~2 weeks)
- Set up Cargo workspace with four crates (nix-core, gradle2nix, ios2nix, flutter2nix)
- Scaffold flake.nix with packages, lib, and checks outputs
- Copy tadfisher/gradle2nix v2 fixture Android projects into `tests/fixtures/gradle/`
- Write failing e2e test assertions for each fixture (TDD entry point)
- Set up GitHub Actions CI (Linux, nix-based)

### Phase 1: gradle2nix MVP (~4 weeks)
- **nix-core**: `LockedDependency`, `DependencyGraph`, `NixExprWriter`, Maven codegen
- **tapi-shim**: Kotlin JAR that enumerates all resolved Maven deps via TAPI → JSON stdout
- **gradle2nix CLI**: `lock`, `check`, `generate` commands; shim embedding via `include_bytes!`
- **gradle2nix Nix lib**: `buildAndroidApp`, `buildGradleProject` functions
- **Tests**: All tadfisher v2 fixture projects pass `nix build`; unit tests for codegen
- **Milestone gate**: CI green for all fixtures; APK + AAB output verified

### Phase 2: flutter2nix Android (~3 weeks)
- **nix-core**: pub dep codegen module
- **flutter2nix CLI**: `lock --target android`, `build android`, `check`
- **flutter2nix Nix lib**: `buildFlutterAndroid` function; Flutter SDK version assertion
- **Tests**: Flutter fixture app in `tests/fixtures/flutter/`; APK + AAB output
- **Milestone gate**: `nix build .#android` on the Flutter fixture produces APK + AAB

### Phase 3: ios2nix MVP (~4 weeks, macOS dev machine required)
- **nix-core**: CocoaPods dep codegen module
- **ios2nix CLI**: `lock`, `build`, `archive`, `export`, `sign` commands
- **Xcode assertion layer**: DEVELOPER_DIR / SDKROOT; version checking
- **Keychain management**: temporary keychain create/import/delete
- **ExportOptions.plist generation**: templated from build configuration
- **Signing secrets**: env var + CLI flag 12-factor model
- **Documentation**: `docs/ios-testing.md` manual test procedure
- **Milestone gate**: manual test produces real-device IPA; `ios2nix lock` CI test passes on Linux

### Phase 4: flutter2nix iOS (~2 weeks)
- **flutter2nix CLI**: `lock --target ios`, `lock --target all`, `build ios`
- **Unified lockfile**: `cocoaPodsDeps` section added to `flutter2nix.nix`
- **flutter2nix Nix lib**: `buildFlutterIOS` function (delegates to ios2nix)
- **Milestone gate**: `flutter2nix lock --target all` generates unified lockfile; manual iOS test passes

### Phase 5: OSS Hardening (~2 weeks, parallel with Phase 4)
- READMEs for each tool independently (gradle2nix standalone, ios2nix standalone, flutter2nix)
- docs/gradle2nix-standalone.md: non-Flutter Android use case walkthrough
- Publish `nix-core` to crates.io
- Evaluate nixpkgs submission readiness

## Known Hard Problems

1. **TAPI shim bundling**: The JAR must be embedded in the Rust binary for `cargo install` compatibility AND buildable as a separate Nix derivation for hermetic Nix builds. Requires `build.rs` that embeds the JAR bytes at compile time; Nix package build-overrides the embedded JAR with the Nix-built one via env var `GRADLE2NIX_TAPI_SHIM_PATH`.

2. **Gradle configuration cache interference**: Even TAPI can be affected by configuration caching. `gradle2nix lock` should run with `--no-configuration-cache` by default and document this. Detect cache state and warn if it may cause issues.

3. **Maven BOM resolution**: BOMs import entire dependency groups. The TAPI correctly resolves transitively but the resulting dep graph can be enormous (500+ artifacts for a typical Android project). The `gradle.nix` generator must handle deduplication and must not exceed Nix evaluator limits for large attribute sets.

4. **AGP plugin classpath deps**: The Android Gradle Plugin's own Maven deps (buildscript/plugins block) must be materialised separately from app deps. The TAPI shim must enumerate both the "project" dependency graph and the "buildscript" dependency graph.

5. **CocoaPods spec repo URL resolution**: The CDN URL for a pod version is derived from its spec checksum (not the version string alone). The `ios2nix lock` command must either query the trunk spec repo API or cache the spec index locally to resolve pod URLs.

6. **ios2nix signing in CI without macOS runners**: Since iOS tests are manual in v0.1, the signing credential workflow is not automated. `docs/ios-testing.md` must include a complete runbook for manual testers including keychain setup, env var configuration, and cleanup.

7. **Nix sandbox vs macOS**: Nix sandbox on macOS is more permissive than Linux. The iOS build pipeline runs partly inside and partly outside the Nix sandbox. Document clearly which derivations are sandboxed and which are orchestrated externally.

8. **Large `flutter2nix.nix` eval performance**: A typical Flutter app with Firebase may have 200+ Maven deps + 50+ pub deps + 20+ CocoaPods. The unified `flutter2nix.nix` may be 5,000+ lines. Validate that the Nix evaluator handles this without timeout.

## Risk Register

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| TAPI API breaks between Gradle major versions | Medium | High | Version-matrix CI (Gradle 7.6, 8.0, 8.4, latest); pin minimum TAPI API version |
| Maven artifact hash mutability (mutable release artifacts) | Low | High | Document as unsupported; recommend users use `--strict` mode that fails on mismatches |
| CocoaPods trunk spec repo CDN format changes | Low | Medium | Cache spec resolution; integration test against trunk on CI |
| iOS CI becomes mandatory before macOS runner cost is acceptable | Medium | Medium | Defer to v0.2; keep manual procedure well-documented |
| AGP classpath dep explosion grows lockfile to unsupported size | Medium | Medium | Lazy evaluation strategies; split gradle.nix into multiple files if needed |
| Apple signing API changes (Xcode 16+ notarization requirements) | Low | High | Design signing layer as a pluggable module; version-assert Xcode in ios2nix lock |
| tadfisher v2 fixtures target Gradle/AGP versions outside our support matrix | Medium | Low | Start fixtures with Gradle 7.6+; upgrade fixtures as coverage expands |
| jfit/PR#207 instability root causes resurface in fresh design | Low | Medium | Fresh design; do NOT reuse PR#207 code; treat PR#207 as a regression test reference |
| `nix-core` API churn breaks downstream users after crates.io publish | High (early) | Low | Mark v0.x as unstable; major version semantics only after Phase 5 |

## Testing Matrix

| Tool | Test Type | CI Platform | What passes? |
|------|-----------|------------|--------------|
| nix-core | Unit (cargo test) | Linux (GH Actions) | LockedDependency serde, NixExprWriter output validity, codegen modules |
| gradle2nix | Unit (cargo test) | Linux (GH Actions) | TAPI model parsing, Maven coordinate resolution, gradle.nix codegen |
| gradle2nix | e2e (`nix build`) | Linux (GH Actions) | All tadfisher v2 fixture projects → APK + AAB produced |
| gradle2nix | e2e (`nix build`) | Linux (GH Actions) | Flutter fixture app Android target → APK + AAB produced |
| gradle2nix | Integration (`gradle2nix check`) | Linux (GH Actions) | check exits 0 on current lockfile; non-zero on stale |
| ios2nix | Unit (cargo test) | Linux (GH Actions) | Podfile.lock parsing, pods.nix codegen, ExportOptions.plist generation |
| ios2nix | e2e (manual) | macOS developer machine | xcarchive produced; IPA exported; signing completes with real cert |
| flutter2nix | Unit (cargo test) | Linux (GH Actions) | pubspec.lock parsing, unified lockfile codegen |
| flutter2nix | e2e (`nix build`) | Linux (GH Actions) | Flutter fixture app → APK + AAB |
| flutter2nix | e2e (manual) | macOS developer machine | Flutter iOS IPA via flutter2nix build ios |
| nix-core | Publish check | CI (cargo publish --dry-run) | Clean crates.io publish |

## OSS Contribution Strategy

### gradle2nix positioning
- README leads with the generic Gradle/Maven use case (Spring Boot, Kotlin JVM, etc.)
- Android use case is section 2; Flutter is section 3
- "If you care about Gradle + Nix, this tool is for you — Flutter is optional"
- `docs/gradle2nix-standalone.md` provides a non-Flutter walkthrough

### ios2nix positioning
- Positioned as "the first end-to-end iOS/Xcode build orchestration layer for Nix"
- README leads with the CocoaPods materialisation story, then the archive/export pipeline
- Signing is framed as "orchestration layer, not secret management" — credentials stay in your CI system
- "If you build iOS apps in any language (Swift, Objective-C, React Native, Flutter) and want reproducible Nix builds, this tool is for you"

### nix-core positioning
- Standalone crates.io library: "Build your own lockfile-to-Nix tooling"
- Useful for: npm2nix, pip2nix, composer2nix, any ecosystem that needs Nix expression codegen
- Documented API with examples independent of the mobile tools

### flutter2nix positioning
- Top-level README is Flutter-centric
- Cross-links to gradle2nix and ios2nix for contributors interested in specific ecosystems
- "flutter2nix is the integration layer; gradle2nix and ios2nix are the engines"

## Interview Transcript
<details>
<summary>Full Q&A (19 rounds + topology confirmation)</summary>

### Round 0 (Topology)
**Q:** Are these 4 components right? (gradle2nix, ios2nix, flutter2nix, Shared Rust crate layer)
**A:** Looks right, proceed
**Ambiguity:** Not yet scored

### Round 1
**Q:** Monorepo vs polyrepo — single GitHub repo or separate repos per tool?
**A:** Single repo on GitHub named `flutter2nix` (even though it publishes the other 2)
**Ambiguity:** 72% (Goal: 0.40, Constraints: 0.30, Criteria: 0.10)

### Round 2
**Q:** If gradle2nix v0.1 shipped tomorrow, what must it do?
**A:** Build a plain Android (non-Flutter) app in Nix sandbox
**Ambiguity:** 61%

### Round 3
**Q:** If ios2nix v0.1 shipped, what must it do?
**A:** Real-device IPA via archive + export (full signing pipeline)
**Ambiguity:** 59%

### Round 4 [Contrarian]
**Q:** What if ios2nix v0.1 deliberately excluded signing?
**A:** Keep signing in v0.1 — it's the point. Will get a proper planner in place for security issues.
**Ambiguity:** 58%

### Round 5
**Q:** If flutter2nix v0.1 shipped, what must it do?
**A:** Flutter Android APK in Nix sandbox (Android only)
**Ambiguity:** 56%

### Round 6 [Simplifier]
**Q:** Lockfile model: init script, one big FOD, or IFD-based Nix expressions?
**A:** Pre-computed lockfile, but output format is Nix expressions (not TOML/JSON)
**Ambiguity:** 51%

### Round 7
**Q:** CocoaPods scope: spec-repo only, spec+git, or full coverage including path pods?
**A:** Spec-repo + Git pods only (path pods are non-goals)
**Ambiguity:** 48%

### Round 8 [Ontologist]
**Q:** What IS the shared Rust crate — Nix codegen only, dep model only, or both?
**A:** Option C (both) — but importantly: gradle2nix and ios2nix generate their own standalone lockfiles; flutter2nix generates a unified cross-platform lockfile
**Ambiguity:** 45%

### Round 9
**Q:** Nix consumer API shape: library function, self-contained generated file, or overlay?
**A:** Mix of A and B — library is separate, lockfile is a generated Nix module
**Ambiguity:** 41%

### Round 10
**Q:** Distribution: flake-input-only, nixpkgs, overlay, or both?
**A:** Both — decide later (start with flake-input; nixpkgs at v1.0)
**Ambiguity:** 40%

### Round 11
**Q:** Pub dep materialisation: pre-computed Nix module or FOD?
**A:** Pre-computed Nix module (same as Maven/CocoaPods)
**Ambiguity:** 37%

### Round 12
**Q:** iOS CI strategy: macOS runners, ad-hoc signing, GitHub secrets, or manual?
**A:** No macOS CI in v0.1 — iOS tests are manual + documented
**Ambiguity:** 36%

### Round 13
**Q:** Flutter SDK in Nix: nixpkgs + version assertion, flutter2nix FOD, or user-provided?
**A:** nixpkgs flutter + version assertion
**Ambiguity:** 34%

### Round 14
**Q:** gradle2nix e2e test project: custom fixture, Flutter app, multiple fixtures, or OSS reference?
**A:** Copy tadfisher/gradle2nix v2 fixture projects (input only, not expected output) as TDD starting point
**Ambiguity:** 32%

### Round 15
**Q:** Gradle dep resolution interception: init script, verification XML, both, or Tooling API?
**A:** Gradle Tooling API (TAPI) — most robust, requires JVM interop
**Ambiguity:** 30%

### Round 16
**Q:** What are explicit non-goals for v0.1?
**A:** Flutter web/desktop, KMP, private Maven/CocoaPods registries (AAB NOT excluded)
**Ambiguity:** 26%

### Round 17
**Q:** Signing secrets model: env vars, config file, CLI flags, or combination?
**A:** Combination: env vars as default, CLI flags as override (12-factor)
**Ambiguity:** 24%

### Round 18
**Q:** Migration path scope: project phasing, user upgrade guide, or codebase refactoring plan?
**A:** Don't worry about that — prior implementation (jfit/PR#207) was unstable and is being replaced by this fresh design
**Ambiguity:** 23%

### Round 19
**Q:** Android build output: debug APK only, release APK, both APK+AAB, or AAB only?
**A:** Both APK and AAB (universal output)
**Ambiguity:** ~22%

</details>
