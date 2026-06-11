# iOS Pod Install Offline Feasibility Spike (Phase -1)

**Date:** 2026-06-11
**Environment:** macOS 26.4.1, Xcode 26.3 (17C529), Flutter 3.41.9 (nixpkgs-wrapped), CocoaPods 1.16.2 (nixpkgs)
**Objective:** Empirically validate Option B (podspec-driven offline resolution) before any resolver code, per plan 1 Phase -1. All results below were produced by actually running the commands — network isolation was enforced with poisoned proxies (`http_proxy=https_proxy=ALL_PROXY=http://127.0.0.1:9`), and every xcodebuild ran in a sanitized environment (see Finding 4).

---

## Verdict

**Option B is FEASIBLE and CHOSEN for v1.** A `pod install` reconstructed offline from prefetched podspecs and source artifacts produces a `Pods/` tree that is structurally identical to the online install **and compiles for the device SDK with zero network access**.

**Phase -1.5: unsigned export = not feasible** (empirical, not assumed — see below).

---

## Test app

Flutter app (`flutter create`) with `firebase_core`, `firebase_auth`, `firebase_analytics`, `path_provider`, `shared_preferences` plus one git-source pod added to the Podfile (`MBProgressHUD`, `:git` + `:tag => '1.2.0'`), platform iOS 15.0. Resulting Podfile.lock covers every pod kind:

| Pod kind | Examples | Offline install | Offline device build |
|---|---|---|---|
| Subspecs | Firebase/Auth, Firebase/CoreOnly, GoogleUtilities/* | ✓ expanded | ✓ |
| Binary xcframework (http zip) | FirebaseAnalytics, GoogleAppMeasurement | ✓ present | ✓ (vendored, linked) |
| Git source + tag | MBProgressHUD | ✓ from cache | ✓ compiled |
| Path pods (Flutter plugins) | firebase_*, shared_preferences_foundation | ✓ via `.symlinks` | ✓ |

22 third-party pods + 6 path pods total.

## Phase -1 evidence

1. **Online baseline:** `pod install` with a fresh `CP_HOME_DIR` resolved and installed everything; Podfile.lock contains `PODS`, `DEPENDENCIES`, `EXTERNAL SOURCES` (`:path` entries for plugins), `CHECKOUT OPTIONS` (`:git`/`:tag` for MBProgressHUD), `SPEC CHECKSUMS`, `PODFILE CHECKSUM`, `COCOAPODS`.
2. **Offline reconstruction:** deleted `Pods/`, kept `Podfile.lock`, poisoned all proxy env vars, re-ran `pod install --no-repo-update` against warm caches only (the CDN-spec cache under `CP_HOME_DIR` + the artifact/git download cache under `~/Library/Caches/CocoaPods` — exactly the two stores the Rust pipeline will pre-seed). **Result: all 22 pods installed with zero external network**, including the git pod. `Manifest.lock` byte-identical to `Podfile.lock` expectations; subspecs expanded; xcframeworks present; the 3 run-script phases persisted in `Pods.xcodeproj`.
3. **Offline build (the success criterion):** `xcodebuild -project Pods/Pods.xcodeproj -alltargets -sdk iphoneos -configuration Release CODE_SIGNING_ALLOWED=NO build` under poisoned proxies: **BUILD SUCCEEDED**, identical to the online baseline. The offline-assembled pods compile and link for device.
4. **Full-app caveat (not a pods problem):** `flutter build ios --release --no-codesign` fails on this machine in `release_unpack_ios`: *"Failed to codesign .../Flutter.framework with identity -"* — the nixpkgs-wrapped Flutter copies its engine framework out of the read-only Nix store and the ad-hoc `codesign` rewrite fails. This reproduces with and without pods and is a nixpkgs-flutter packaging defect, orthogonal to CocoaPods resolution.

## Findings that bind later plans

1. **Spec checksums are not artifact hashes** (confirmed): the lockfile's `SPEC CHECKSUMS` hash podspecs. Artifact `sha256` must come from prefetching the artifact itself.
2. **The two caches to pre-seed** for offline `pod install`: the CDN spec cache (under `CP_HOME_DIR`, populated per-pod as `Specs/{md5-shard}/{Name}/{Version}/{Name}.podspec.json`) and the download cache (`~/Library/Caches/CocoaPods/Pods/`, holding extracted http zips and git checkouts keyed by pod name+version/options). Pre-seeding both, `pod install --no-repo-update` makes no network calls.
3. **Git pods**: CocoaPods serves them from the download cache when warm; cold, it would `git clone` — the pipeline must inject the pre-cloned checkout into the cache (or run inside the Nix sandbox where the fetch already happened).
4. **xcodebuild requires a sanitized environment** (Plan 2, `xcode::env`): a Nix dev shell exports `CC`, `CXX`, `LD`, `SDKROOT` (Nix macOS SDK!), `DEVELOPER_DIR` (Nix apple-sdk!), `NIX_CFLAGS_COMPILE`, `NIX_LDFLAGS`, and puts a clang/ld wrapper first on `PATH`. Any of these reaching `xcodebuild` breaks device builds (observed: Nix `ld` rejecting Apple linker flags; Xcode picking the 14.4 Nix SDK). Plan 2's xcodebuild wrapper must strip `NIX_*`, `CC`, `CXX`, `LD`, `SDKROOT`, `DEVELOPER_DIR` and use a system `PATH`.
5. **nixpkgs-flutter iOS app builds are broken upstream** (read-only engine framework vs. ad-hoc codesign; reproduced for both device and simulator targets). Plan 2's `buildIOSApp` must either make the unpacked `Flutter.framework` writable before the Xcode script phase or use a non-store Flutter SDK; track as a Plan 2 risk.

## Phase -1.5: unsigned export (empirical)

Using a minimal **native** iOS app (no Flutter, to isolate the question):

- `xcodebuild archive CODE_SIGNING_ALLOWED=NO` → **ARCHIVE SUCCEEDED** (unsigned archives are possible).
- `xcodebuild -exportArchive` with a minimal ExportOptions.plist (`method=development` only, no `signingCertificate`/`provisioningProfiles`) → **EXPORT FAILED: `error: exportArchive No Team Found in Archive`**.
- Xcode 26 additionally warns: *"Command line name 'development' is deprecated. Use 'debugging' instead."* — confirming the plan's Xcode-version method-name auto-detection requirement.

**`unsigned export = not feasible`.** Plan 2's e2e is therefore "archive (unsigned) + export *plumbing* only"; the first functional signed `.ipa` is Plan 3's e2e, and Plans 2/3 are validated together on macOS.

## ADR outcome

Option B confirmed; proceed with plan 1 Phases 0–2 as specified. Option C (`--from-pod-install` capture) remains the documented escape hatch for apps whose pods misbehave offline; the `ios.nodes`/`dep_source` schema already accommodates it.
