# ios2nix — Plan 2: Build & Nix Integration (macOS)

> Reads with: `ios2nix-implementation-plan.md` (overview) and depends on **Plan 1** (the lockfile +
> crate skeleton must exist). Signing/provisioning is **Plan 3** — this plan builds the *unsigned*
> archive→export skeleton and the Nix offline-pod sandbox. Status: pending approval.

**Scope:** `xcode` env/assert/output modules; `build`/`archive`/`export` orchestration around
`xcodebuild`; the offline `pod install` sandbox; `nix/ios2nix-lib.nix` (`readPods`,
`buildPodsSandbox`, `buildIOSApp` — unsigned/dev path); flake `ios2nix` package + darwin-gated
checks + `flake.lib` merge. **Turns green:** the macOS `xcode`/`build`/`archive`/`export` unit +
integration stubs (sidecar-mocked on Linux, real on macOS).

**Hard prerequisite:** Plan 1 Phase -1 spike PASSED (or the resolver switched to Option A/C). If the
spike failed and Option A was chosen, replace §2 "buildPodsSandbox from `ios.nodes`" with "restore
the vendored `Pods/` snapshot" — the rest of this plan (xcodebuild orchestration) is unchanged.

---

## 1. `xcode` modules (macOS, cfg-gated; output model is pure/Linux-testable)

### 1a. `xcode/env.rs` — environment setup
```rust
#[cfg(target_os="macos")]
pub fn setup_xcode_env() -> anyhow::Result<()>;   // resolves DEVELOPER_DIR via `xcode-select -p`,
// sets DEVELOPER_DIR + SDKROOT (iPhoneOS.sdk), PRESERVES pre-set user vars, errs on invalid path.
```
Green: `test_setup_xcode_env_{sets_developer_dir,sets_sdkroot,preserves_user_vars,xcode_not_found,invalid_xcode_path}`.
On Linux: `bail!("requires macOS")`; the success-path tests are `#[cfg(target_os="macos")]`.

### 1b. `xcode/assert.rs` — version + tooling guards
```rust
pub fn assert_xcode_version(found:&str, minimum:&str) -> anyhow::Result<()>;  // pure semver-ish compare → Linux-testable
#[cfg(target_os="macos")] pub fn assert_xcode_tools_installed() -> anyhow::Result<()>; // `xcode-select -p` valid
```
Green: `test_assert_xcode_version_{valid,too_old}` (pure, run on Linux),
`test_assert_xcode_tools_installed` (macOS).

### 1c. `xcode/build_output.rs` — the parsed-output model (PURE — Linux-testable)
```rust
#[derive(Deserialize)] pub struct XcodeBuildOutput {
    pub version: String, pub architectures: Vec<String>,
    #[serde(default)] pub frameworks: Vec<String>,
    pub codesign_identity: Option<String>,
}
pub fn parse_xcode_build_output(json:&str) -> anyhow::Result<XcodeBuildOutput>;
```
Match `tests/fixtures/xcode-schema.json`. Green:
`test_parse_xcode_build_output_{valid,with_frameworks,malformed_missing_field,version_mismatch}`.
**Reconcile the schema-version concept:** the `version-mismatch.json` fixture expects "unsupported
schema version" — introduce an internal `schema_version` gate distinct from the Xcode `version`
string (the `malformed-unknown-fields` + `version-mismatch` fixtures define the validation contract;
use `#[serde(deny_unknown_fields)]` for the unknown-fields case and an explicit supported-version
check for version-mismatch).

---

## 2. The offline pod-install sandbox + xcodebuild orchestration (macOS)

### 2a. `cli::build` — pod install (offline) + xcodebuild build
```rust
#[cfg(target_os="macos")]
pub fn run(cmd: BuildCommand) -> anyhow::Result<XcodeBuildOutput>;
```
Behavior:
1. **Sidecar short-circuit:** if `<proj>/.ios2nix-xcode-output.json` exists, read+parse it instead
   of spawning xcodebuild (overview §2.3) — this is how the test runs without a Mac.
2. Else (macOS): assemble the offline pod tree (from Plan 1's `ios.nodes` / the Phase-1-validated
   layout), run `pod install --no-repo-update --no-repo-update` against a `file://` Specs repo, then
   `xcodebuild build -workspace Runner.xcworkspace -scheme <scheme> -configuration Release
   -destination 'generic/platform=iOS' -derivedDataPath <dd> CODE_SIGNING_ALLOWED=NO` (unsigned —
   signing is Plan 3), capturing output → `XcodeBuildOutput`.
Green: `test_build_invoke_xcodebuild` (sidecar), `test_build_capture_output`.

### 2b. `cli::archive` — `.xcarchive`  (single command, signing-optional — overview §2.5)
```rust
#[cfg(target_os="macos")] pub fn run(cmd: ArchiveCommand) -> anyhow::Result<PathBuf>; // → .xcarchive
// ArchiveCommand { workspace, scheme, configuration, archive_path, signing: Option<SigningConfig> }
```
**Plan 2 owns `run()` and the `signing == None` (unsigned) path:**
`xcodebuild archive -workspace Runner.xcworkspace -scheme <scheme> -configuration Release
-archivePath <out>.xcarchive -destination 'generic/platform=iOS'`. When `signing == Some(s)`, the
function appends the manual-signing flags — **that branch's exact flags are specified by Plan 3 §5a**
(Plan 2 leaves a clearly-marked `if let Some(s) = &cmd.signing { /* Plan 3 §5a flags */ }` hook).
Then verify structure: `Products/Applications/<App>.app` + `Info.plist`. Green:
`test_archive_create_xcarchive`, `test_archive_verify_structure` (unsigned).

### 2c. `cli::export` — `.xcarchive` → `.ipa`  (Plan 2 owns the call; Plan 3 owns the plist)
```rust
#[cfg(target_os="macos")] pub fn run(cmd: ExportCommand) -> anyhow::Result<PathBuf>;  // → .ipa
// ExportCommand { archive_path, export_opts_plist: PathBuf, output_path }
```
**Plist flow (overview §2.5, unambiguous):** the caller (Plan 3, or a dev default for Plan 2's own
test) first writes an `ExportOptions.plist` via `export_opts::write_export_options(&opts, path)`, then
passes that path as `cmd.export_opts_plist`. Plan 2's `run()` only invokes
`xcodebuild -exportArchive -archivePath <archive> -exportOptionsPlist <cmd.export_opts_plist>
-exportPath <out>` and surfaces the missing-archive error. **Plan 2 never builds plist content;
Plan 3 never calls xcodebuild.** Green: `test_export_missing_archive` (pure/dev);
`test_export_xcarchive_to_ipa` — see §5 for whether this is an unsigned or signed-only path
(decided by Plan 1 Phase -1.5).

> **Scheme/workspace detection:** Flutter apps use `ios/Runner.xcworkspace` + scheme `Runner`;
> default to those, allow `--workspace`/`--scheme`/`--configuration` overrides. Document the default
> in `--help`.

---

## 3. `nix/ios2nix-lib.nix` (replace the `throw` stub)

Mirror `gradle2nix-lib.nix`'s helper/derivation split. Reads the **single source** `lock.ios.nodes`
(overview P2).
```nix
{ lib }:
let
  readPods = lockFile:
    let lock = builtins.fromJSON (builtins.readFile lockFile);
    in if lock ? ios then lock.ios.nodes
       else if lock ? nodes then lock.nodes
       else throw "ios2nix-lib: unrecognized lockfile format in ${toString lockFile}";

  # fetch each pod by hash into a CocoaPods-shaped store tree (analogue of buildMavenRepo).
  buildPodsSandbox = pkgs: nodes:
    let
      fetchPod = node:
        if lib.hasPrefix "git+" node.url
        then let m = … split "git+<url>#<rev>" …;            # pre-mortem #5: the round-trip split
             in pkgs.fetchgit { url = m.url; rev = m.rev; sha256 = node.sha256; }
        else pkgs.fetchurl { url = node.url; sha256 = node.sha256; };
      entries = map (n: { inherit (n) name; src = fetchPod n; }) nodes;
    in pkgs.runCommand "ios-pods-sandbox" {} '' … lay out Specs/ + sources for --no-repo-update … '';

  buildIOSApp =
    { pkgs, name, src, lockFile
    , scheme ? "Runner", configuration ? "Release"
    , exportOptions          # Plan 3 supplies the signed plist; here a dev/unsigned default
    , ... }:
    pkgs.stdenv.mkDerivation {
      inherit name src;
      __noChroot = true;                     # P4 — Xcode build is impure (signing, Apple network)
      meta.platforms = lib.platforms.darwin;
      buildInputs = [ pkgs.cocoapods … ];
      buildPhase = ''
        export LANG=en_US.UTF-8
        # link the offline pod sandbox into ios/Pods, pod install --no-repo-update,
        # xcodebuild archive, xcodebuild -exportArchive -exportOptionsPlist ${exportOptions}
      '';
      installPhase = '' mkdir -p $out; cp build/*.ipa $out/ '';
    };
in { inherit buildIOSApp buildPodsSandbox readPods; }
```
**Honesty (P4):** `__noChroot = true` and `meta.platforms = darwin` are deliberate. File comment:
*"Signing is impure — it depends on Apple network reachability, keychain state, installed
provisioning profiles, and embedded timestamps, none of which are content-addressed. Only the pod
inputs (fetched by hash) are deterministic; the `.ipa` is not bit-reproducible."*

---

## 4. flake.nix wiring

- Replace `ios2nix = pkgs.emptyDirectory;` with a real `rustPlatform.buildRustPackage` (same shape
  as `gradle2nix`, **no** tapi-shim `preBuild`). Add `meta.platforms = lib.platforms.darwin` so it
  only *builds as a package* on macOS (it still cross-checks under `cargo check --workspace`).
- `flake.lib` (line 17): merge `(import ./nix/ios2nix-lib.nix { lib = nixpkgs.lib; })` into the
  attrset and drop the `buildIOSApp = throw …` stub.
- Checks, darwin-gated via `pkgs.lib.optionalAttrs pkgs.stdenv.isDarwin { … }`:
  - `ios-pods-sandbox-test` = `(self.lib.buildPodsSandbox pkgs (readNodes fixtureLock))` fetches a
    real pod into a store tree (analogue of `android-maven-repo-test`).
  - `buildIOSApp-eval` = type-only `assert drv ? drvPath` (analogue of `buildAndroidApp-eval`).
  - `buildIOSApp-e2e` behind `optionalAttrs (isDarwin && pathExists <fixture ios lock>)` (mirror the
    android `-e2e` gating). Allowed-to-fail tier.
- The existing `cargo-check`/`cargo-clippy` checks already compile ios2nix's pure code on Linux —
  they stay green provided Plan 1's P1 cfg-gating holds.

---

## 5. Tests & acceptance

**Unit (Linux):** `xcode::build_output` (all xcode-output fixtures), `assert_xcode_version` (pure),
the git-url split helper used by `buildPodsSandbox` (a Rust-side unit + a Nix-eval test).
**Integration (macOS-gated):** `build`/`archive`/`export` via sidecar on Linux; real on macOS.
**Nix-eval (Linux):** `nix eval` that `buildPodsSandbox` splits `git+url#rev` into exact
`fetchgit { url; rev; }` args (pre-mortem #5, the Nix half).
**E2E (macOS, allowed-to-fail) — wording selected by Plan 1 Phase -1.5:**
- *If unsigned export is feasible:* fixture app `archive` → `export` → unsigned/dev `.ipa` exists and
  is a valid ZIP with `Payload/<App>.app/Info.plist`. (Full signed path is Plan 3's e2e.)
- *If unsigned export is NOT feasible (the likely case — Apple generally requires a signing identity
  at export):* Plan 2's e2e is **archive-only** — assert the `.xcarchive` exists with
  `Products/Applications/<App>.app` + `Info.plist`, and that `cli::export::run` invokes xcodebuild
  with the right args (plumbing assertion via a recorded command / dry-run). **The first functional
  `.ipa` is then Plan 3's signed e2e, and Plans 2+3 are validated together on macOS** — an honest
  validation-time coupling, recorded in the spike report, not a hidden defect.

**Acceptance:** on macOS, the `xcode`/`build`/`archive` tests are green and `cli::export` invokes
xcodebuild correctly; `nix flake check` on darwin passes `ios-pods-sandbox-test` + `buildIOSApp-eval`.
The "produces a standalone `.ipa`" criterion holds **only if** Phase -1.5 found unsigned export
feasible; otherwise the producing-an-`.ipa` acceptance moves to Plan 3. On Linux, `cargo
check/clippy --workspace` stay green and the pure/sidecar tests pass.

---

## Optional follow-on: `buildFlutterIOSApp` (Phase 4, after Plan 3)
Wrap `buildIOSApp` with the Dart/pub layer (`flutter build ipa`), analogous to
`buildFlutterAndroidApp`: generate `.dart_tool/package_config.json` from `pubspec.lock` via
`pub2nix`, point Flutter's internal CocoaPods at the offline sandbox, run `flutter build ipa
--no-pub --export-options-plist <Plan-3 plist>`. Deferred until signing (Plan 3) is solid.

---

### Consensus footer
Round-2 review applied: unsigned/dev archive→export skeleton cleanly separated from signing (Plan 3);
`__noChroot`/darwin honesty explicit (P4); `buildPodsSandbox` reads the single-source `ios.nodes`
(P2); git-url split validated on both Rust and Nix sides (pre-mortem #5); Option-A fallback note for
a failed Phase -1 spike. Status: pending approval.
