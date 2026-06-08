# Plan: buildFlutterAndroidApp — Nix Library Function

**Status:** PENDING APPROVAL  
**Scope:** Add `buildFlutterAndroidApp` to `nix/gradle2nix-lib.nix`  
**Complexity:** MEDIUM  
**Decision Mode:** DELIBERATE (pre-mortem + expanded test plan required)

---

## RALPLAN-DR: Structured Deliberation

### PRINCIPLES (3–5)

1. **No Network in Derivations** — All dependencies (Maven, pub, binaries) must be deterministically locked before sandbox evaluation. Network fetches only at Nix eval time, not build time.

2. **Build Isolation** — Flutter's internal Gradle must use the locked Maven repo without discovering network repos. Gradle's init script is the canonical isolation mechanism.

3. **Pub Cache is Pre-Built** — Dart pub dependencies are not resolved inside the derivation; they must be supplied as a pre-built store path. Nix sandbox cannot run `dart pub get` (no network).

4. **Linux-Only Android Build** — The Android SDK (NDK, build-tools) only works on Linux. The Nix function must either error on Darwin or evaluate safely without sandboxing Android tooling.

5. **Minimal Scope** — This function does **not** solve iOS (`buildFlutterIosApp`), Pub cache generation, or multi-platform orchestration. It solves Android only, for a caller who brings a pre-built pub cache.

### DECISION DRIVERS (Top 3)

1. **Pub Cache Provenance** — *How does the function receive the Dart pub cache?*
   - Driver: Caller (jfit) already uses `buildDartApplication` with `autoPubspecLock` + `gitHashes`. Forcing re-derivation of the same cache wastes computation and creates a second source of truth.
   - Resolution: Accept pub cache as a parameter (path or derivation). Caller builds it independently.

2. **Gradle Init Script Delivery** — *How does Flutter's internal Gradle access the locked Maven repo?*
   - Driver: Flutter invokes `gradle` without explicit flags. Gradle must discover the init script via environment or standard load paths (`$GRADLE_USER_HOME/init.d/`).
   - Resolution: Place the init script in `$GRADLE_USER_HOME/init.d/` and set `GRADLE_USER_HOME` to a writable tmpdir. Gradle auto-loads all `*.gradle` files from this directory.

3. **buildAndroidApp Disposition** — *Do we deprecate the old `buildAndroidApp`, rename it, or keep both?*
   - Driver: `buildAndroidApp` is fundamentally broken for Flutter (runs raw `gradle assembleRelease` without Dart artifacts). It only works for pure-Gradle Android projects.
   - Resolution: Keep `buildAndroidApp` unchanged (no external consumers yet; it's safe to leave as-is). Add `buildFlutterAndroidApp` as a new function. Document the difference clearly in comments.

---

### VIABLE OPTIONS

#### OPTION A: Pub Cache as Derivation Parameter (RECOMMENDED)

**Description:** Function signature includes `pubCacheDir: path` pointing to a pre-built pub cache derivation. Caller (jfit) produces this using `buildDartApplication` or equivalent, then passes it to `buildFlutterAndroidApp`.

```nix
buildFlutterAndroidApp = {
  pkgs, name, src, lockFile, pubCacheDir,
  flutterSdk ? pkgs.flutter, jdk ? pkgs.jdk17, androidSdk,
  gradleFlags ? [], ...
}:
```

**Pros:**
- Clean separation of concerns: pub cache is caller's responsibility.
- Reuses jfit's existing `buildDartApplication` infrastructure (no new Nix logic).
- Caller can version/lock the cache independently from the Android build.

**Cons:**
- Requires caller to supply `pubCacheDir` as a separate derivation input.
- If caller forgets to build it, the error message is late (at Android build time).

---

#### OPTION B (REJECTED): Architect's `buildPubCache` Helper

**Description:** Architect proposed adding a `buildPubCache` helper function that uses `flutter pub get --offline` to populate a pub cache inside the Nix derivation.

**Why rejected:**

`flutter pub get --offline` **cannot populate an empty pub cache in a Nix sandbox** (no network access). This would fail at build time with an error like "pub get: no such package". The Critic correctly identified this as broken.

**Correct approach:** Caller pre-builds the pub cache independently using nixpkgs' `buildDartApplication` with `autoPubspecLock`, which is the proven pattern (already used by jfit for lint/test checks). This is Option A.

---

#### OPTION C (REJECTED): Accept pubspecLock + gitHashes, Build Internally

**Description:** Function accepts `pubspecLock` and `gitHashes` as parameters, then uses `fetchPubCache` or similar nixpkgs primitives to build the pub cache inside the function.

```nix
buildFlutterAndroidApp = {
  pkgs, name, src, lockFile, pubspecLock, gitHashes ? {},
  flutterSdk ? pkgs.flutter, jdk ? pkgs.jdk17, androidSdk,
  ...
}:
```

**Pros:**
- Single-stage build: caller provides minimal inputs, function orchestrates everything.
- More "batteries included" — looks simpler from the outside.

**Cons:**
- Re-derives the pub cache every time (wasteful; jfit already does this for lint/test checks).
- Requires discovering and exposing nixpkgs' pub2nix APIs (may be internal, unstable).
- Doubles the Nix complexity inside the function (now it must handle both pub and Maven lockfiles).

---

### SELECTED OPTION: A (Pub Cache as Derivation Parameter)

**Rationale:**

- jfit already builds and caches the pub cache via `buildDartApplication` for lint/test checks. Option A eliminates redundant re-derivation.
- Minimal Nix complexity inside `buildFlutterAndroidApp`: focus on orchestrating Flutter + Gradle + Android SDK, not pub resolution.
- Clear interface: caller is explicitly responsible for the pub cache quality. If it's stale or broken, the error is easy to trace.
- Option B (Architect's `buildPubCache`) is broken: `flutter pub get --offline` cannot bootstrap an empty cache in a sandbox.
- Option C would require stabilizing internal nixpkgs APIs that are not part of the public interface and subject to change.
- **Pub cache validation** (see Step 1 implementation details) adds an early fail-fast check to catch missing/stale packages before the full build runs.

---

### PRE-MORTEM: Three Failure Scenarios (Deliberate Mode)

**Scenario 1: "Flutter finds network repos despite init script"**
- **Symptom:** Build succeeds locally but fails in CI with "Cannot resolve io.flutter:flutter_android:1.5.0 — network unreachable."
- **Root cause:** Init script is not loaded, or Flutter/Gradle is using a different `GRADLE_USER_HOME`, or Gradle's repository discovery order places network repos first.
- **Prevention:** (a) Verify init script is placed in `$GRADLE_USER_HOME/init.d/` before build starts. (b) Assert that the Maven repo path exists and is readable. (c) Test with a network-isolated fixture (no DNS). (d) Check Gradle's buildscript blocks in source `build.gradle.kts` for hardcoded `mavenCentral()` without `allprojects` wrapper.
- **Detection:** Inspect Gradle's debug output (`--info` flag) to confirm init script was loaded.

---

**Scenario 2: "Pub cache is stale or missing packages"**
- **Symptom:** Flutter's build phase fails with "pub get: cannot find package XYZ."
- **Root cause:** `pubCacheDir` parameter points to an old or incomplete pub cache. Pub packages are missing or have the wrong version.
- **Prevention:** (a) Caller's contract is to supply a valid, complete pub cache derived from the same `pubspec.lock`. (b) Add a validation step early in the build phase: check that key packages (flutter, flutter_test, firebase_core) exist in the cache.
- **Detection:** Fail early with a clear error: "pubCacheDir does not contain expected package XYZ. Ensure pubCacheDir is built from the same pubspec.lock."

---

**Scenario 3: "AAB/APK is not found or is in unexpected location"**
- **Symptom:** Build runs without error, but `find . -name "*.aab"` returns nothing. `$out` is empty.
- **Root cause:** `flutter build appbundle` output structure changed in a Flutter version, or the source repo has a custom `build.gradle.kts` that redirects output to a non-standard path.
- **Prevention:** (a) Query the output of `flutter build appbundle --help` to document the expected output directory. (b) Add debug logging before the `find` step to print the build directory structure. (c) Assert that at least one `*.aab` or `*.apk` was found before calling `cp`.
- **Detection:** Add a post-build check: `if [ -z "$(find . -name "*.aab" -o -name "*.apk")" ]; then echo "ERROR: No AAB/APK found"; exit 1; fi`.

---

### EXPANDED TEST PLAN (Deliberate Mode)

#### Layer 1: Unit Tests (Pure Nix Eval, No Sandbox)

**What:** Test the function signature, parameter validation, and derivation structure without building anything.

**Tests:**
- `buildFlutterAndroidApp` returns a derivation with `drvPath` attribute.
- Function accepts `pubCacheDir` as a path or derivation.
- Function rejects missing required parameters (`pkgs`, `name`, `src`, `lockFile`, `pubCacheDir`, `androidSdk`) with a clear error.
- Derivation includes correct `buildInputs` (Flutter SDK, JDK, Android SDK).
- Environment variables are set: `ANDROID_HOME`, `ANDROID_SDK_ROOT`, `JAVA_HOME`, `GRADLE_USER_HOME`, `PUB_CACHE`.

**Nix code location:** `tests/nix/buildFlutterAndroidApp-eval.nix`

---

#### Layer 2: Integration Tests (nix build / nix flake check in Sandbox)

**What:** Build the derivation in the Nix sandbox, but use a **stub Flutter project** (directory structure + minimal pubspec.yaml + android/build.gradle.kts) to keep build time reasonable. The stub does NOT attempt to compile Dart or produce an APK; it tests infrastructure only.

**Stub vs. Real Project:**
- **Stub (RECOMMENDED for integration tests):** Directory structure that Gradle/Flutter can read without compiling Dart or producing an APK. Tests verify: (a) Gradle init script is loaded, (b) Maven repo is present, (c) pub cache is wired, (d) early validation checks work. Faster CI.
- **Real (for E2E only):** Full jfit app source with actual compilation. Should be deferred to jfit's own CI or optional Layer 3 tests (not required for this PR).

**Tests:**
1. **Maven Repo Isolation & Gradle Init Script** — Verify that the locked Maven repo is built correctly from the flutter2nix.lock fixture, and that Gradle's init script is placed in `$GRADLE_USER_HOME/init.d/gradle2nix-flutter.gradle`.
   - Fixture: `tests/fixtures/flutter/flutter-minimal.lock` (3 io.flutter artifacts + minimal transitive deps) + stub Flutter project.
   - Checks:
     - Init script exists and contains correct `file://` URL to Maven repo.
     - Gradle loads init script (confirmed by log marker like `[gradle2nix]`).
     - Build uses `--offline` mode (no network attempts).

2. **Pub Cache Wiring & Early Validation** — Verify that `PUB_CACHE` is set and early validation check works.
   - Fixture: Minimal pub cache from `buildDartApplication` (separate derivation).
   - Checks:
     - `PUB_CACHE=$pubCacheDir/` is exported in `buildPhase`.
     - Early validation loop checks that key packages (flutter, flutter_test) exist at `$PUB_CACHE/hosted/pub.dev/`.
     - If packages are missing, build fails with clear error: `"ERROR: pubCacheDir missing package: flutter"`.

3. **Stub Build Phase Execution** — Verify that build phase can execute without crashing, even with stub source that doesn't produce an APK.
   - Fixture: Minimal stub with:
     ```
     stub-app/
     ├── pubspec.yaml (minimal, declares flutter + android)
     ├── android/
     │   ├── app/build.gradle.kts (stub with compileSdk, targetSdk)
     │   └── settings.gradle.kts
     └── lib/main.dart (minimal Dart file)
     ```
   - Check: Build phase exits cleanly (status 0 or expected failure documented).

4. **Graceful Artifact Handling** — Verify that install phase handles the case when stub produces no APK/AAB.
   - Check: Install phase does NOT crash; fails with clear error message if no artifact found (not a silent failure).

5. **Composability** — Verify that `buildFlutterAndroidApp` internally calls `buildGradleProject` and reuses its outputs.
   - Check: Inspect generated Nix derivation to confirm it references `buildGradleProject`'s outputs (not re-computing them).

**Nix code location:** `flake.nix` checks section, new entries:
- `buildFlutterAndroidApp-eval` (eval-phase, no build)
- `buildFlutterAndroidApp-integration-stub` (stub fixture, infrastructure tests)

---

#### Layer 3: E2E Tests (Real AAB/APK Built from Minimal Fixture App — No jfit Required)

**What:** Commit a minimal self-contained Flutter hello-world app to `tests/fixtures/flutter/minimal-app/`, generate a real `flutter2nix.lock` from it, and run `flutter build appbundle` against it inside the Nix sandbox. Produces a real AAB. Linux-only. No jfit source required.

**Fixture structure:**
```
tests/fixtures/flutter/minimal-app/
├── pubspec.yaml         # minimal Flutter app (flutter + flutter_test only)
├── pubspec.lock         # locked pub deps
├── flutter2nix.lock     # full Maven dep graph from gradle2nix lock
├── android/
│   ├── app/
│   │   └── build.gradle.kts   # minimal compileSdk/targetSdk/minSdk
│   ├── settings.gradle.kts
│   └── gradle/wrapper/
│       └── gradle-wrapper.properties
└── lib/
    └── main.dart        # void main() => runApp(const Text('hi'));
```

**Tests:**
1. **AAB Is Produced** — `$out` contains exactly one `*.aab` file.
   - Command: `find $out -name "*.aab" | wc -l`
   - Expectation: exactly 1.

2. **AAB Is a Valid ZIP** — Output file is a well-formed ZIP archive.
   - Command: `unzip -t $out/*.aab`
   - Expectation: `unzip` reports no errors.

3. **AAB Contains Expected Structure** — Minimal Flutter AAB has known top-level entries.
   - Command: `unzip -l $out/*.aab | grep -E "base/manifest|resources.pb"`
   - Expectation: both entries present.

4. **Gradle Offline Verification** — Build succeeds with `--offline`; no network attempts.
   - Verified by Nix sandbox isolation (sandbox enforces no outbound connections).
   - Additional: Gradle output must not contain "Downloading" or "Could not resolve" lines.

**Test environment:** Linux CI only (`pkgs.stdenv.isLinux` guard in flake.nix). Skipped silently on Darwin.

**Nix code location:** `flake.nix` checks section, new entry: `buildFlutterAndroidApp-e2e` (Linux-only, wrapped in `lib.optionalAttrs pkgs.stdenv.isLinux { ... }`)

---

#### Layer 4: Observability (Production Use Signals)

**What:** Signals that tell us the function is working correctly when consumers (like jfit) use it in their CI/CD.

**Signals:**
1. **Build Log Signals:**
   - `flutter build appbundle` completes with exit status 0.
   - Gradle's `[gradle2nix]` init script prints to stdout (proves load).
   - No "Cannot resolve io.flutter:*" errors in Gradle output.
   - No "pub get: unresolved" errors in Flutter's build output.

2. **Output Artifacts:**
   - `$out` contains exactly one `.aab` file (or multiple if multi-APK).
   - AAB file size is >10 MB (sanity check: not an empty stub).
   - AAB modification time is recent (built in this derivation, not stale).

3. **Gradle Cache State:**
   - `$GRADLE_USER_HOME/.gradle/caches/modules-2/` does not grow (all deps already in Maven repo).
   - No network-related errors in Gradle's cache invalidation logic.

4. **Flutter Config State:**
   - `flutter config --version` output matches expected Flutter SDK version.
   - `flutter config --android-sdk` points to `$ANDROID_SDK_ROOT`.

5. **Reproducibility:**
   - Same inputs (pubCacheDir, lockFile, src, flutterSdk version) always produce byte-identical AABs.
   - Hash of output AAB is deterministic (for Nix cache reuse).

**How to emit signals:** Add informational `echo` statements and structured logs in the `buildPhase` and `installPhase`.

---

## IMPLEMENTATION PLAN

### Step 1: Add `buildFlutterAndroidApp` function to `nix/gradle2nix-lib.nix`

**File:** `/Users/jacob/Documents/GitHub/flutter2nix/nix/gradle2nix-lib.nix`

**What to add:**
- New function `buildFlutterAndroidApp` with signature:
  ```nix
  buildFlutterAndroidApp = {
    pkgs, name, src, lockFile, pubCacheDir,
    flutterSdk ? pkgs.flutter,
    jdk ? pkgs.jdk17,
    androidSdk,
    gradleFlags ? [],
    ...
  }: ...
  ```

**Key implementation details:**

1. **Linux-only guard (fail-fast):**
   ```nix
   assert pkgs.stdenv.isLinux 
     or (throw "buildFlutterAndroidApp only works on Linux; Android SDK is not available on Darwin");
   ```

2. **Compose from `buildGradleProject` (NOT inline like `buildAndroidApp`):**
   ```nix
   let
     gradle = buildGradleProject { inherit pkgs lockFile jdk; };
     # gradle exports: { mavenRepo, initScript, buildInputs, baseGradleFlags }
   in
   # Now use gradle.mavenRepo, gradle.initScript, gradle.buildInputs, gradle.baseGradleFlags
   ```
   This ensures `buildFlutterAndroidApp` reuses the same Maven repo + init script pattern without duplicating the logic from `buildGradleProject`.

3. **Wrap init script into `$GRADLE_USER_HOME/init.d/`:**
   ```nix
   let
     initDir = pkgs.runCommand "gradle-init-d" { } ''
       mkdir -p $out/init.d
       cat > $out/init.d/gradle2nix-flutter.gradle << 'EOF'
       ${gradle.initScript}
       EOF
     '';
   ```
   Gradle auto-loads all `*.gradle` files from `$GRADLE_USER_HOME/init.d/`.

4. **Export `PUB_CACHE` pointing to the pre-built `pubCacheDir`:**
   ```nix
   buildPhase = ''
     export GRADLE_USER_HOME=$(mktemp -d)
     mkdir -p $GRADLE_USER_HOME/init.d
     cp ${initDir}/init.d/* $GRADLE_USER_HOME/init.d/
     export PUB_CACHE=${pubCacheDir}
     # Validate pub cache early: check that key packages exist
     for pkg in flutter flutter_test; do
       if [ ! -d "$PUB_CACHE/hosted/pub.dev/$pkg-"* ]; then
         echo "ERROR: pubCacheDir missing package: $pkg"
         echo "Expected: $PUB_CACHE/hosted/pub.dev/$pkg-*/lib"
         exit 1
       fi
     done
     # NOTE: baseGradleFlags contains Gradle-specific flags (--no-daemon, --no-configuration-cache,
     # --init-script) that flutter build does NOT accept — passing them causes immediate failure.
     # The init script is auto-loaded from $GRADLE_USER_HOME/init.d/; only --offline applies here.
     flutter build appbundle --offline
   '';
   ```

5. **Install phase:** Find and copy `*.aab` and `*.apk` from release output to `$out`:
   ```nix
   installPhase = ''
     mkdir -p $out
     find build/app/outputs -name "*.aab" -o -name "*.apk" | while read artifact; do
       cp "$artifact" $out/
     done
     if [ -z "$(find $out -name "*.aab" -o -name "*.apk")" ]; then
       echo "ERROR: No AAB/APK found in build output"
       exit 1
     fi
   '';
   ```

**Acceptance criteria:**
- ✓ Function is syntactically valid Nix and passes `nix eval`.
- ✓ Function returns a valid derivation (has `drvPath`, `type = "derivation"`).
- ✓ Derivation includes expected `buildInputs` and environment variables.
- ✓ Function is exported in `flake.nix` as `self.lib.buildFlutterAndroidApp`.
- ✓ **Composability**: `buildFlutterAndroidApp` calls `buildGradleProject` and uses its outputs (`mavenRepo`, `initScript`, `buildInputs`, `baseGradleFlags`), not inline code.
- ✓ **Pub cache validation**: Early build-phase check that iterates key packages and fails with clear error if missing.

---

### Step 2: Add eval-phase test to `flake.nix`

**File:** `/Users/jacob/Documents/GitHub/flutter2nix/flake.nix`

**What to add:**
- New check: `buildFlutterAndroidApp-eval`
  ```nix
  buildFlutterAndroidApp-eval = let
    drv = self.lib.buildFlutterAndroidApp {
      inherit pkgs;
      name = "test-flutter-android";
      src = ./tests/fixtures/flutter;
      lockFile = ./tests/fixtures/flutter/flutter-minimal.lock;
      pubCacheDir = ./tests/fixtures/flutter/pub-cache; # or a derivation
      androidSdk = (pkgs.androidenv.composeAndroidPackages { }).androidsdk;
    };
  in assert drv ? drvPath;
     pkgs.runCommand "buildFlutterAndroidApp-eval" { } "touch $out";
  ```

**Acceptance criteria:**
- `nix flake check` includes this new check and it passes.
- Check verifies that the derivation is well-formed without attempting to build.

---

### Step 3: Create minimal Flutter fixture with pub cache

**File:** `/Users/jacob/Documents/GitHub/flutter2nix/tests/fixtures/flutter/pub-cache` (or a separate derivation)

**What to create:**
- A minimal pub cache directory with structure:
  ```
  pub-cache/
  └── hosted/pub.dev/
      ├── flutter-X.Y.Z/
      ├── flutter_test-X.Y.Z/
      ├── firebase_core-X.Y.Z/
      └── ... (other deps from flutter-minimal.lock)
  ```

**Option A:** Commit a pre-built, minimal pub cache to the repo (static, version-locked).
**Option B:** Generate it via a derivation that uses `buildDartApplication` to fetch deps, then extract to a store path.

*Recommendation:* Option B (as a derivation in `tests/nix/` called from `flake.nix`). This keeps the repo lightweight and makes the cache reproducible.

**Acceptance criteria:**
- Pub cache contains all packages required by the fixture (determined by scanning flutter-minimal.lock for Dart package names).
- Running `PUB_CACHE=/path/to/cache flutter pub get --offline` completes without error.

---

### Step 4: Add integration test: Maven repo + init script

**File:** `/Users/jacob/Documents/GitHub/flutter2nix/flake.nix` or `/Users/jacob/Documents/GitHub/flutter2nix/tests/nix/buildFlutterAndroidApp-integration.nix`

**What to test:**
- Call `buildFlutterAndroidApp` with flutter-minimal.lock fixture.
- Verify that `$out` contains the correct structure:
  - `init.d/gradle2nix-flutter.gradle` exists and points to the Maven repo.
  - Maven repo has all locked artifacts.
- (Optional) Run a minimal `flutter build apk --no-shrink` (faster than appbundle) against a dummy Flutter app source.

**Check definition in flake.nix:**
```nix
buildFlutterAndroidApp-integration-test = self.lib.buildFlutterAndroidApp {
  inherit pkgs;
  name = "test-flutter-build-integration";
  src = ./tests/fixtures/flutter/minimal-app; # minimal Flutter project
  lockFile = ./tests/fixtures/flutter/flutter-minimal.lock;
  pubCacheDir = /* from Step 3 */;
  androidSdk = (pkgs.androidenv.composeAndroidPackages { ... }).androidsdk;
};
```

**Acceptance criteria:**
- Check runs and produces an output (even if it's just the init script and Maven repo, not a full APK/AAB).
- No network access during build (verified by sandbox isolation).
- Gradle init script is loaded (confirmed by log output containing `[gradle2nix]` or similar marker).

---

### Step 5: Add documentation to `nix/gradle2nix-lib.nix`

**File:** `/Users/jacob/Documents/GitHub/flutter2nix/nix/gradle2nix-lib.nix`

**What to add:**
- Comment block above `buildFlutterAndroidApp` explaining:
  - Purpose: Build Flutter Android apps (APK/AAB) offline using locked Maven + pub caches.
  - Key difference from `buildAndroidApp`: Runs `flutter build appbundle`, not raw `gradle`.
  - Parameters: What each one means, defaults, type constraints.
  - `pubCacheDir`: Must be a path to a pre-built pub cache (caller's responsibility).
  - Example usage (pseudo-code, reference jfit structure).
  - Known limitations: Linux-only, requires Android SDK, requires pre-built pub cache.

**Example documentation:**
```nix
  # Builds a Flutter Android app (AAB/APK) offline using locked Maven and pub caches.
  #
  # Unlike buildAndroidApp (which runs raw gradle), this function:
  # - Invokes `flutter build appbundle` (or `apk`), which compiles Dart first.
  # - Wires the offline Maven repo into Flutter's internal Gradle via GRADLE_USER_HOME/init.d/.
  # - Requires pubCacheDir: a pre-built pub cache path (caller's responsibility).
  # - Only works on Linux (Android SDK is Linux-native).
  #
  # Parameters:
  #   pkgs: nixpkgs
  #   name: derivation name
  #   src: Flutter app source (must contain pubspec.yaml, android/, lib/, etc.)
  #   lockFile: flutter2nix.lock (output from gradle2nix, contains android.nodes)
  #   pubCacheDir: Path to pre-built pub cache (from buildDartApplication or similar)
  #   flutterSdk: Flutter SDK (defaults to pkgs.flutter)
  #   jdk: JDK version (defaults to pkgs.jdk17)
  #   androidSdk: Android SDK from androidenv.composeAndroidPackages (required)
  #   gradleFlags: Extra flags to pass to gradle (e.g., ["-x" "test"])
  #
  # Example:
  #   let
  #     pubCache = buildDartApplication { ... }.pubCache;
  #   in
  #   buildFlutterAndroidApp {
  #     inherit pkgs name src lockFile pubCacheDir;
  #     androidSdk = androidComposition.androidsdk;
  #   }
  #
  # Returns: stdenv.mkDerivation with $out containing the built AAB/APK.
  buildFlutterAndroidApp = { ... }: ...
```

**Acceptance criteria:**
- Documentation is clear and examples are copy-paste-ready (for jfit consumers).
- Limitations are explicit (Linux-only, pre-built pub cache requirement).
- No ambiguity about who is responsible for the pub cache.

---

### Step 6: Document `buildAndroidApp` vs `buildFlutterAndroidApp`

**File:** `/Users/jacob/Documents/GitHub/flutter2nix/docs/` (if exists) or inline comment in `nix/gradle2nix-lib.nix`

**What to add:**
- A brief comparison table or section explaining when to use each:
  - `buildAndroidApp`: Pure Gradle/Maven projects (no Flutter, no Dart).
  - `buildFlutterAndroidApp`: Flutter apps (Dart + Gradle + Android SDK).

**Acceptance criteria:**
- New consumers (like jfit) can quickly identify which function to use.
- No confusion between the two.

---

## DETAILED TODOS WITH ACCEPTANCE CRITERIA

### TODO 1: Implement `buildFlutterAndroidApp` in `nix/gradle2nix-lib.nix`

**Task:** Write the function body.

**Steps:**
1. Add function signature with all parameters (see Step 1 above).
2. Call `readNodes lockFile` to extract Maven nodes.
3. Call `buildMavenRepo pkgs nodes` to generate offline Maven repo.
4. Call `makeInitScript pkgs mavenRepo` to generate Gradle init script.
5. Create `$GRADLE_USER_HOME/init.d/` directory structure in a `let` binding:
   ```nix
   let
     initDir = pkgs.runCommand "gradle-init-d" { } ''
       mkdir -p $out/init.d
       cat > $out/init.d/gradle2nix-flutter.gradle << 'EOF'
       ${initScript}
       EOF
     '';
   ```
6. Build derivation using `pkgs.stdenv.mkDerivation` with:
   - `buildInputs = [ flutterSdk jdk androidSdk pkgs.gradle ]`
   - `buildPhase`: Export env vars, run `flutter build appbundle --offline ...`
   - `installPhase`: Find and copy `*.aab` and `*.apk` to `$out`
7. Guard for Linux: `assert pkgs.stdenv.isLinux` or conditional error.

**Acceptance criteria:**
- ✓ Function compiles (no Nix eval errors).
- ✓ Derivation has correct `buildInputs`.
- ✓ Derivation has correct environment variables set.
- ✓ Build phase exports `GRADLE_USER_HOME`, `PUB_CACHE`, `ANDROID_HOME`, etc.
- ✓ Install phase includes a fallback message if no AAB/APK is found.

---

### TODO 2: Export `buildFlutterAndroidApp` in `flake.nix`

**Task:** Add the function to `self.lib` so consumers can call `flake.lib.buildFlutterAndroidApp`.

**Steps:**
1. Open `flake.nix` line 18 (where `self.lib =` is defined).
2. Replace the import statement to include the new function (it should be auto-exported from `nix/gradle2nix-lib.nix`).
3. Verify `nix flake check` passes.

**Acceptance criteria:**
- ✓ `nix eval .#lib.buildFlutterAndroidApp` succeeds.
- ✓ Function is accessible as `self.lib.buildFlutterAndroidApp` from consumers.

---

### TODO 3: Create `buildFlutterAndroidApp-eval` check

**Task:** Add an eval-phase test to `flake.nix` that verifies the derivation structure without building.

**Steps:**
1. Add a new check in `flake.nix` `checks` section.
2. Call `buildFlutterAndroidApp` with minimal fixture parameters.
3. Assert `drv ? drvPath` to verify it's a valid derivation.
4. Return a dummy output (e.g., `pkgs.runCommand "buildFlutterAndroidApp-eval" { } "touch $out"`).

**Acceptance criteria:**
- ✓ `nix flake check` includes this check.
- ✓ Check passes without sandbox/build.
- ✓ Clear error message if derivation is malformed.

---

### TODO 4: Create minimal pub cache fixture or derivation

**Task:** Provide a pre-built pub cache that the integration tests can use.

**Steps (Option B: Derivation):**
1. Create `tests/nix/flutter-minimal-pub-cache.nix`:
   ```nix
   { pkgs, flutterSdk, pubspecLock }:
   let
     # Use buildDartApplication to fetch pub packages
     pubFetch = pkgs.buildDartApplication {
       pname = "flutter-minimal-pub";
       version = "1.0.0";
       src = pkgs.runCommand "dummy" { } "mkdir $out";
       pubspecLock = pubspecLock; # e.g., tests/fixtures/flutter/pubspec.lock
       ... # other attrs
     };
   in
   # Extract the pub cache from pubFetch and return as a path
   pkgs.runCommand "flutter-minimal-pub-cache" { } ''
     cp -r ${pubFetch}/.pub-cache $out
   '';
   ```
2. Call this derivation from `flake.nix` checks or fixture setup.

**Acceptance criteria:**
- ✓ Pub cache derivation builds successfully.
- ✓ Cache contains expected packages (flutter, flutter_test, firebase_core, etc.).
- ✓ Cache is small enough to commit/cache in CI (<100 MB for minimal fixture).
- ✓ `PUB_CACHE=/path/to/cache flutter pub get --offline` works.

---

### TODO 5b: Create minimal fixture Flutter app for E2E testing

**Task:** Commit a real (but tiny) Flutter hello-world app to `tests/fixtures/flutter/minimal-app/` so E2E tests can run entirely within flutter2nix.

**Steps:**
1. Create `tests/fixtures/flutter/minimal-app/lib/main.dart`:
   ```dart
   import 'package:flutter/material.dart';
   void main() => runApp(const MaterialApp(home: Text('hi')));
   ```
2. Create `tests/fixtures/flutter/minimal-app/pubspec.yaml` with minimal deps (flutter SDK only, no firebase, no plugins).
3. Run `flutter pub get` locally to generate `pubspec.lock`. Commit both.
4. Create `tests/fixtures/flutter/minimal-app/android/` with minimal `settings.gradle.kts`, `app/build.gradle.kts`, and `gradle/wrapper/gradle-wrapper.properties`.
5. Run `gradle2nix lock --project-dir tests/fixtures/flutter/minimal-app/android` to generate a real `flutter2nix.lock` with the full Maven dep graph. Commit it.
6. Create a pub cache derivation for the minimal app's deps in `flake.nix` (using `pkgs.buildDartApplication` with `autoPubspecLock`).

**Why this matters:** The Layer 2 stub never calls `flutter build`. This fixture lets the E2E check prove the full pipeline (Dart compile → Gradle → AAB) without requiring jfit source.

**Acceptance criteria:**
- ✓ `tests/fixtures/flutter/minimal-app/` is self-contained (no external deps beyond Flutter SDK and standard Maven repos).
- ✓ `flutter2nix.lock` for the minimal app covers all Maven artifacts needed for `flutter build appbundle`.
- ✓ Pub cache derivation builds successfully from the minimal app's `pubspec.lock`.

---

### TODO 5c: Add E2E check to `flake.nix`

**Task:** Add a Linux-only `buildFlutterAndroidApp-e2e` check that builds the minimal fixture app and asserts a real AAB is produced.

**Steps:**
1. In `flake.nix` checks, add under `lib.optionalAttrs pkgs.stdenv.isLinux`:
   ```nix
   buildFlutterAndroidApp-e2e = self.lib.buildFlutterAndroidApp {
     inherit pkgs;
     name = "test-flutter-android-e2e";
     src = ./tests/fixtures/flutter/minimal-app;
     lockFile = ./tests/fixtures/flutter/minimal-app/flutter2nix.lock;
     pubCacheDir = minimalAppPubCache; # from TODO 5b pub cache derivation
     androidSdk = (pkgs.androidenv.composeAndroidPackages {
       buildToolsVersions = [ "34.0.0" ];
       platformVersions = [ "34" ];
     }).androidsdk;
   };
   ```
2. Add a post-build assertion step that runs `unzip -t $out/*.aab` to verify the AAB is valid.
3. Wrap the entire check in `lib.optionalAttrs pkgs.stdenv.isLinux { ... }` so it is silently skipped on Darwin.

**Acceptance criteria:**
- ✓ Check runs on Linux CI and produces a `*.aab` in `$out`.
- ✓ `unzip -t $out/*.aab` passes (valid ZIP).
- ✓ Check is absent (not just skipped) on Darwin — no eval error.
- ✓ Build output does not contain "Downloading" or "Could not resolve" (offline verified).

---

### TODO 5: Add integration test check to `flake.nix`

**Task:** Create a check that builds a minimal Flutter app (or just verifies Maven + Gradle setup).

**Steps:**
1. In `flake.nix` checks, add `buildFlutterAndroidApp-integration`:
   ```nix
   buildFlutterAndroidApp-integration = self.lib.buildFlutterAndroidApp {
     inherit pkgs;
     name = "test-flutter-integration";
     src = ./tests/fixtures/flutter/minimal-app;
     lockFile = ./tests/fixtures/flutter/flutter-minimal.lock;
     pubCacheDir = /* pub cache from TODO 4 */;
     androidSdk = (pkgs.androidenv.composeAndroidPackages { ... }).androidsdk;
   };
   ```
2. If building the full APK is too slow for CI, create a dummy check that only verifies the init script and Maven repo are correctly set up (without running `flutter build`).

**Acceptance criteria:**
- ✓ Check runs in `nix flake check`.
- ✓ No network access is attempted (sandbox isolation verified).
- ✓ Gradle init script is loaded (log output confirms).
- ✓ Maven repo is present and readable.
- ✓ (Optional) A minimal APK or AAB is produced in `$out`.

---

### TODO 6: Add documentation to `nix/gradle2nix-lib.nix`

**Task:** Write comprehensive comments explaining the function, parameters, and usage.

**Steps:**
1. Add a comment block above `buildFlutterAndroidApp` (see Step 5 in Implementation Plan).
2. Document each parameter's purpose, type, and defaults.
3. Include an example usage.
4. Document known limitations (Linux-only, pre-built pub cache required, etc.).

**Acceptance criteria:**
- ✓ Comments are clear and grammatically correct.
- ✓ Example is copy-paste-ready for jfit consumers.
- ✓ Limitations are explicit (no ambiguity).

---

## SUCCESS CRITERIA (Final Acceptance)

The plan is complete when:

1. ✓ `nix/gradle2nix-lib.nix` contains `buildFlutterAndroidApp` function.
2. ✓ Function signature is: `{ pkgs, name, src, lockFile, pubCacheDir, flutterSdk, jdk, androidSdk, gradleFlags, ... }`
3. ✓ Function exports from `flake.nix` as `self.lib.buildFlutterAndroidApp`.
4. ✓ **Composability (REQUIRED)**: `buildFlutterAndroidApp` internally calls `buildGradleProject` and reuses its outputs (`mavenRepo`, `initScript`, `buildInputs`, `baseGradleFlags`). Does NOT inline `readNodes`, `buildMavenRepo`, or `makeInitScript` like `buildAndroidApp` does.
5. ✓ **Linux-only guard**: Function asserts `pkgs.stdenv.isLinux` with clear error message at eval time (fail-fast).
6. ✓ **Pub cache validation (REQUIRED)**: Early in `buildPhase`, validation loop checks that key packages (flutter, flutter_test) exist at `$PUB_CACHE/hosted/pub.dev/`. Fails with clear error message if packages are missing: `"ERROR: pubCacheDir missing package: XYZ"`.
6b. ✓ **Flag discipline (REQUIRED)**: `flutter build appbundle` is called with `--offline` only. Gradle-specific flags (`--no-daemon`, `--no-configuration-cache`, `--init-script`) are NOT passed to Flutter — they are accepted by Gradle, not Flutter. Init script delivery is via `$GRADLE_USER_HOME/init.d/` auto-load exclusively.
7. ✓ `nix flake check` includes and passes all of:
   - `buildFlutterAndroidApp-eval` (eval-phase test, no build)
   - `buildFlutterAndroidApp-integration-stub` (stub fixture, infrastructure tests)
   - `buildFlutterAndroidApp-e2e` (Linux-only; real `flutter build appbundle` → real AAB; silently absent on Darwin)
8. ✓ `tests/fixtures/flutter/minimal-app/` committed to repo — self-contained Flutter hello-world with `pubspec.lock`, `flutter2nix.lock` (full Maven dep graph from `gradle2nix lock`), and minimal `android/` structure.
9. ✓ E2E check verifies:
   - `$out` contains exactly one `*.aab` file.
   - `unzip -t $out/*.aab` passes (valid ZIP structure).
   - AAB contains `base/manifest/` and `resources.pb` entries.
   - Build output has no "Downloading" or "Could not resolve" lines (offline verified).
   - No jfit source required.
10. ✓ Integration test verifies:
   - Gradle init script is placed in `$GRADLE_USER_HOME/init.d/gradle2nix-flutter.gradle` and loaded by Gradle (confirmed by log output).
   - Maven repo is present and contains all locked artifacts.
   - Pub cache is wired correctly (`PUB_CACHE` environment variable).
   - Early validation check runs and detects missing pub cache packages.
   - No network access is attempted (sandbox isolation, `--offline` mode).
   - Stub source (no compilation) is handled gracefully.
11. ✓ Documentation in `nix/gradle2nix-lib.nix` is comprehensive (purpose, parameters, example, known limitations).
12. ✓ Explicit documentation that `pubCacheDir` is caller's responsibility. Documented as: "Caller should produce via `buildDartApplication` with `autoPubspecLock` + `gitHashes` (proven pattern used by jfit)."
13. ✓ All existing tests (`cargo check`, `cargo clippy`, `nix flake check`) still pass.

---

## OPEN QUESTIONS

See `.omc/plans/open-questions.md` for tracking.

Key decision points for user confirmation:

- [ ] **Q1: Pub cache input strategy** — Confirmed as Option A (derivation parameter, not Option B). ✓
- [ ] **Q2: Gradle init script delivery** — Confirmed as `$GRADLE_USER_HOME/init.d/` auto-load strategy. ✓
- [ ] **Q3: buildAndroidApp fate** — Confirmed as keep unchanged; no deprecation. ✓
- [ ] **Q4: Android SDK guard** — Use `assert pkgs.stdenv.isLinux` or `if !isLinux then throw "..."`?
  - *Recommendation:* Throw with clear message (`"buildFlutterAndroidApp only works on Linux; Android SDK is not available on Darwin"`).
- [x] **Q5: Test fixture size** — Resolved: minimal stub for Layer 2 (infra-only, no Dart compile) + minimal fixture app committed to repo for Layer 3 E2E (real AAB, no jfit required). ✓
- [ ] **Q6: Pub cache provisioning** — Build it as a separate Nix derivation (Option B from TODO 4), or commit a pre-built one?
  - *Recommendation:* Build as a derivation (reproducible, no repo bloat).

---

## CONSENSUS SUMMARY (Iteration 2 — Architect + Critic Feedback Integrated)

**Architect feedback — ADDRESSED:**
1. Composability violation fixed: `buildFlutterAndroidApp` now calls `buildGradleProject` and composes from its outputs, not inline logic.
2. Pub cache hidden contract — **RESOLVED by Critic's correction:** Architect proposed `buildPubCache` helper using `flutter pub get --offline`, but Critic correctly identified this is broken (cannot bootstrap empty cache in Nix sandbox without network). Plan confirms Option A (caller-provided `pubCacheDir`) is correct, with added pub cache validation (early check for missing packages).
3. Linux-only guard — NOW INCLUDED in Step 1 implementation outline.

**Critic feedback — ADDRESSED:**
1. Rejected Architect's `buildPubCache` synthesis: Cannot populate pub cache via `flutter pub get --offline` in sandbox. Documented explicit rejection in VIABLE OPTIONS section.
2. Confirmed composability requirement: `buildFlutterAndroidApp` must call `buildGradleProject` and use its outputs. Added to acceptance criteria and SUCCESS CRITERIA.
3. Clarified integration test fixture type: Use stub (Option B) — directory structure + pubspec.yaml + android/build.gradle.kts. Does NOT attempt compilation. Tests infrastructure only (init script, Maven repo, pub cache wiring).
4. Specified pub cache validation: Early build-phase shell loop checks that key packages exist in cache. Fails fast with clear error.
5. Updated acceptance criteria to include composability and pub cache validation requirements.

**Post-consensus addition (user-directed, pre-approval):**
- Layer 3 E2E is no longer deferred to jfit. A minimal hello-world Flutter app (`tests/fixtures/flutter/minimal-app/`) is committed to the flutter2nix repo itself, with its own `flutter2nix.lock` (full Maven dep graph) and pub cache derivation. A Linux-only `buildFlutterAndroidApp-e2e` check in `flake.nix` runs a real `flutter build appbundle` and asserts a valid AAB is produced. No jfit source required at any test layer.
- Flag discipline criterion added: `flutter build appbundle` receives `--offline` only; Gradle-specific flags stay out of the Flutter invocation.

**Principle conflicts resolved:**
- Pub cache is caller's responsibility (not function's) to enable reuse of existing nixpkgs `buildDartApplication` infrastructure.
- Pub cache source documented: Caller should use `buildDartApplication` with `autoPubspecLock` + `gitHashes` (proven pattern, already used by jfit for lint/test checks).
- Gradle is isolated via init script injection into `$GRADLE_USER_HOME/init.d/`, not environment variables (Gradle's documented pattern).
- Linux-only restriction is enforced at derivation eval time, not build time (fail-fast).

**Alternatives considered and rejected:**

1. **Option B (REJECTED): Architect's `buildPubCache` synthesis**
   - Proposal: Use `flutter pub get --offline` to populate pub cache inside derivation.
   - Why rejected: `flutter pub get --offline` cannot bootstrap an empty pub cache in a Nix sandbox (no network access). Build would fail with "pub get: no such package" errors.
   - Correct approach: Use Option A (caller-provided `pubCacheDir` via `buildDartApplication`).

2. **Option C (REJECTED): Internal pub cache building with `pubspecLock` + `gitHashes`**
   - Proposal: Accept `pubspecLock` + `gitHashes` parameters and build cache inside function using nixpkgs' pub2nix APIs.
   - Why rejected: Re-derives the cache every time (wasteful; jfit already does this for lint/test checks). Requires unstable nixpkgs APIs.

3. **buildAndroidApp deprecation: Rejected**
   - No external consumers exist yet, so it's safe to leave unchanged. Future decision can be deferred.

**Contingencies (pre-mortem mitigation):**
- Pre-mortem Scenario 1 (network repos discovered): Mitigated by Gradle `--offline` flag enforcement and init script verification.
- Pre-mortem Scenario 2 (stale pub cache): Mitigated by early validation check for key packages in cache.
- Pre-mortem Scenario 3 (AAB not found): Mitigated by debug logging and early failure check in install phase.

**Risk level:** MEDIUM (depends on Flutter/Gradle version stability, pub cache correctness, Android SDK availability in CI).

---

## READY FOR REVIEW

This plan is complete and ready for:
1. User confirmation of assumptions and decisions.
2. Executor implementation using `/oh-my-claudecode:start-work buildFlutterAndroidApp`.

**Proceed?** Confirm the plan is actionable, then hand off to implementation.
