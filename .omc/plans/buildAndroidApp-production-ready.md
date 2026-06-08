# Plan: buildAndroidApp production-ready + jfit integration

**Status:** pending approval  
**Created:** 2026-06-08  
**Consensus:** Architect APPROVE (4th iteration), Critic best-effort (4/5 iterations)

---

## Context

`buildAndroidApp` in `nix/gradle2nix-lib.nix` has three env var bugs. This plan fixes them, adds an internal eval check, then wires jfit mobile as the first real consumer.

**User constraint:** "I want this package to be as easy to use as possible — nothing extra required."

Target API:
```nix
flutter2nix.lib.buildAndroidApp {
  pkgs = pkgs;
  name = "jfit-mobile";
  src = ./android;
  lockFile = ./flutter2nix.lock;
  androidSdk = androidComposition.androidsdk;
}
```

---

## Step 1 — Fix env vars in `nix/gradle2nix-lib.nix`

Three changes to `buildAndroidApp`:

```diff
- ANDROID_HOME = "${androidSdk}";
+ ANDROID_HOME = "${androidSdk}/libexec/android-sdk";
- ANDROID_SDK_ROOT = "${androidSdk}";
+ ANDROID_SDK_ROOT = "${androidSdk}/libexec/android-sdk";
+ JAVA_HOME = "${jdk}";
```

`GRADLE_USER_HOME` stays as a shell export in `buildPhase` — `$TMPDIR` is sandbox-provided at build time and cannot be a derivation attribute literal. Add comment explaining why.

Also add a comment documenting that `androidSdk` must be a composition with `buildToolsVersions` and `platformVersions` matching the project's `build.gradle`.

**Verified:** nixpkgs `androidenv/build-app.nix:37` uses identical `"${androidsdk}/libexec/android-sdk"` pattern. `composeAndroidPackages` has default args (`buildToolsVersions ? [ "latest" ]`).

---

## Step 2 — Add type-check to `flake.nix`

```nix
# Type-only: verifies buildAndroidApp returns a derivation. Does not verify SDK content.
buildAndroidApp-eval = let
  drv = self.lib.buildAndroidApp {
    inherit pkgs;
    name = "eval-test";
    src = ./tests/fixtures/gradle;
    lockFile = ./tests/fixtures/gradle/android-minimal.lock;
    androidSdk = (pkgs.androidenv.composeAndroidPackages {}).androidsdk;
  };
in assert drv ? drvPath;
   pkgs.runCommand "buildAndroidApp-eval" {} "touch $out";
```

Uses existing `tests/fixtures/gradle/android-minimal.lock`. Works on all platforms (no build).

---

## Step 3 — jfit: add `flutter2nix` input

In `/Users/jacob/Documents/GitHub/jfit/flake.nix`:

```nix
# inputs section:
flutter2nix = {
  url = "path:../flutter2nix";
  inputs.nixpkgs.follows = "nixpkgs";
};
```

Add `flutter2nix` to the `outputs` destructured inputs.

```nix
# mobile callsite (currently line ~86):
mobile = import ./apps/mobile/package.nix {
  inherit pkgs lib;
  jfitCli = cli.package;
  flutter2nixLib = flutter2nix.lib;
};
```

---

## Step 4 — jfit: Maven repo check in `mobile/package.nix`

`buildGradleProject { }.mavenRepo` is the correct primitive for jfit (not `buildAndroidApp`).
jfit is a Flutter project — standalone `gradle assembleRelease` requires the Flutter CLI for Dart
compilation. `mavenRepo` verifies all 158 deps are fetchable and SHA256-correct, which is the
core value flutter2nix provides. Full Flutter+Android APK build via Nix is future work.

**Signature change:**
```nix
{ pkgs, lib, jfitCli, flutter2nixLib ? null }:
```

**New binding in let block:**
```nix
androidDepsCheck = lib.optionalAttrs (pkgs.stdenv.isLinux && flutter2nixLib != null) {
  android-deps = (flutter2nixLib.buildGradleProject {
    inherit pkgs;
    lockFile = ./flutter2nix.lock;
  }).mavenRepo;
};
```

**Return value:**
```nix
{
  checks = {
    lint = mkCheck { ... };   # unchanged
    test = mkCheck { ... };   # unchanged
  } // androidDepsCheck;
}
```

jfit's existing `checks.mobile = mobile.checks` in flake.nix picks this up automatically.

---

## Acceptance Criteria

1. `nix flake check` passes in flutter2nix (existing + `buildAndroidApp-eval`)
2. `ANDROID_HOME` and `ANDROID_SDK_ROOT` = `${androidSdk}/libexec/android-sdk`
3. `JAVA_HOME = "${jdk}"` set as mkDerivation attribute
4. `GRADLE_USER_HOME` set via shell export using `$TMPDIR`
5. jfit `flake.nix` adds `path:../flutter2nix` input
6. jfit `mobile.checks.android-deps` verifies all lockfile deps on Linux
7. All Rust tests pass (`cargo test --workspace`)

---

## Key Decisions (ADR)

**Decision:** Use `buildGradleProject { }.mavenRepo` for jfit, not `buildAndroidApp`.  
**Drivers:** Flutter projects cannot build APK via standalone Gradle (Dart compilation requires Flutter CLI).  
**Alternatives considered:** `buildAndroidApp { src = ./android }` — rejected, needs Flutter CLI.  
**Why chosen:** `mavenRepo` verifies the core flutter2nix value proposition (dep fetching) without requiring Flutter CLI in the Nix sandbox.  
**Consequences:** Full APK build via Nix deferred to future `buildFlutterAndroidApp` wrapper.  
**Follow-ups:** `buildFlutterAndroidApp` — wraps `buildDartApplication` + `buildAndroidApp` for Flutter projects.

**Decision:** Keep `GRADLE_USER_HOME` as shell export.  
**Drivers:** `$TMPDIR` is only available at build time; derivation attributes are evaluated before the sandbox is set up.  
**Why chosen:** Standard Nix pattern for sandbox-provided paths.

---

## Pre-mortem

- **SDK content**: `composeAndroidPackages {}` = minimal SDK; real consumers must specify `buildToolsVersions` + `platformVersions`. Documented in comment.
- **Gradle daemon**: `$TMPDIR/gradle-home` is sandbox-writable; `--no-daemon` prevents orphan processes.
- **macOS CI**: `buildAndroidApp-eval` evaluates on Darwin (no build). `android-deps` is Linux-only.

---

## Test Plan

- **Unit**: 6 existing Rust unit tests for `artifact_repo_url` routing
- **Integration**: `buildAndroidApp-eval` (type check) + `android-maven-repo-test` (4-artifact FOD fetch)
- **E2E jfit**: `android-deps` FOD verifies all 158 jfit lockfile deps on Linux CI
- **Future**: `buildFlutterAndroidApp` for full APK (out of scope)
