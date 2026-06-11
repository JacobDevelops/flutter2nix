# flutter2nix — Plan 4: Unified Build & Flutter Signing (macOS + Linux)

> Reads with: `ios2nix-implementation-plan.md` (overview) and follows **ios2nix Plans 1–3** (all
> landed) plus the unplanned-but-shipped `buildFlutterIOSApp` (commit `b3276fe1`). This plan covers
> what remains between "each piece works" and "one entry point builds a Flutter app for both
> platforms, signed where signing is possible". Status: pending approval.

**Already true (do not re-plan):** `flutter2nix lock` composes gradle2nix + ios2nix into one
dual-section lockfile (exercised by the minimal-app fixture); `buildFlutterAndroidApp` (Linux) and
`buildFlutterIOSApp` (Darwin, unsigned) both build from it; the signed iOS surface
(`sign-setup`/`archive`/`export`/`sign`) works end-to-end via the cargo signing e2e and the
`ios-build` benchmark; pub machinery is shared in `nix/pub-lib.nix`.

**Scope:**
1. `buildFlutterApp` — the unified per-platform dispatcher (replaces the throw).
2. Signed Flutter iOS `.ipa` — wiring the fixture's Flutter project through the proven
   ios2nix CLI signing pipeline (cargo e2e tier), and an optional `signing` param on
   `buildFlutterIOSApp` for catalog-free apps.
3. jfit — the real-world validation target (pods + catalogs + Firebase), CLI-pipeline based.
4. Explicit non-fixture decision: no synthetic pods-bearing Flutter fixture (rationale in §3).

**Non-goals:** TestFlight/App Store upload (unchanged from Plan 3); bit-reproducible `.ipa`s;
making actool/ibtool work inside Nix derivations (impossible for nixbld users — see
`ios-sandbox-constraints` memory and the KNOWN LIMITATION note in `flutter2nix-lib.nix`);
Android-on-Darwin builds (android stays `meta.platforms = linux`).

---

## 1. `buildFlutterApp` — unified dispatcher (`nix/flutter2nix-lib.nix`)

One call, per-platform outputs, honest about what the evaluating host can build:

```nix
buildFlutterApp =
  { pkgs
  , name
  , src
  , lockFile                       # unified flutter2nix.lock (android + ios sections)
  , platforms ? [ "android" "ios" ]
  , androidSdk ? null              # required iff "android" requested on Linux
  , signing ? null                 # forwarded to the iOS builder (§2b); Android signing is
                                   # out of scope here (keystore handling is its own plan)
  , ...                            # forwarded per-platform (pubspecLockFile, flutterSdk, …)
  }:
```

Semantics (drive the eval tests):
- Returns an attrset `{ android? ; ios? }` containing only the platforms that are BOTH requested
  and buildable on the evaluating system (`android` ⇒ `stdenv.isLinux && androidSdk != null`,
  `ios` ⇒ `stdenv.isDarwin`).
- A requested platform whose **lockfile section is missing** is a `throw` (a lockfile without an
  `ios` section fed to an iOS build is user error, not a skip).
- A requested platform that the **host can't build** is silently filtered (this is what makes one
  flake expression usable from both CI legs) — but if the filter leaves NOTHING, `throw` with a
  message naming the host system and the requested platforms.
- No `linkFarm` aggregate inside the function: the per-system aggregation (e.g. `.#e2e`) is the
  flake's job, as today.

Flake follow-ups: `buildFlutterApp-eval` check (instantiate both branches with
`builtins.seq drv.drvPath` — remember the laziness lesson) on each system; the existing
`buildFlutterAndroidApp-e2e` / `buildFlutterIOSApp-e2e` entries become thin
`(buildFlutterApp { … }).android` / `.ios` projections so the dispatcher itself is what e2e
exercises.

**Turns green:** new `buildFlutterApp-eval` flake check; both existing e2e entries unchanged in
behavior; `nix flake check` on darwin + linux.

---

## 2. Signed Flutter iOS `.ipa`

Two tiers, because the asset-catalog constraint splits the world:

### 2a. Primary: CLI-pipeline signed e2e (works for ANY app, runs as the real user)

Extend `crates/ios2nix/tests/cli_tests.rs` with `test_cli_flutter_e2e_to_signed_ipa`
(`#[ignore]`-gated on the `IOS2NIX_*` contract, run by `fnx check` via the signing e2e wiring):

1. Copy the minimal-app fixture; write `ios/Flutter/Generated.xcconfig` + `.dart_tool` the same
   way `buildFlutterIOSApp` does (extract that setup into a small shared shell script or just
   inline — the test runs on the real user where `flutter pub get --offline` also works).
2. `ios2nix sign-setup` → keychain; decode profile for name/UUID/bundle-id (existing helpers).
3. `ios2nix archive` on `ios/Runner.xcworkspace` with manual-signing flags. Bundle ID comes from
   stamping the pbxproj copy (the pwa-wrapper lesson: NEVER the global `--bundle-id` override for
   workspaces with pods/frameworks — Flutter.framework is embedded even pod-less).
4. `ios2nix export` with Manual ExportOptions (cert + bundleID→UUID map) → assert signed `.ipa`,
   `codesign --verify --deep --strict`.

This is the "flutter app → signed .ipa" acceptance path and needs no new product code.

### 2b. Secondary: `signing ? null` on `buildFlutterIOSApp` (catalog-free apps only)

Mirror `buildIOSApp`'s contract exactly (same env-driven `IOS2NIX_*` secrets, same
`sign-setup`/trap-cleanup script, same manual-flag archive + no-`|| true` export). Differences:
- The archive uses the Flutter workspace/scheme and the sanitized-env + DerivedData +
  HOME/PATH-build-setting machinery already in the unsigned path — factor the xcodebuild
  invocation into one place rather than duplicating the four sandbox workarounds.
- Document prominently: only viable for storyboard/catalog-free apps (the e2e fixture qualifies;
  jfit and the LOKE shell do NOT — they use tier 2a).

**Turns green:** `test_cli_flutter_e2e_to_signed_ipa` with the jfit material (manual/fnx tier);
`buildFlutterIOSApp { signing = …; }` eval check; optionally a signed sandbox build of the
fixture as an allowed-to-fail experiment.

---

## 3. No synthetic pods-bearing Flutter fixture — decision, not omission

Adding e.g. `firebase_core` to minimal-app would exercise pods-through-Flutter, but:
- it regenerates the **Android** lockfile too (Firebase Android artifacts → committed
  `android/flutter2nix.lock` + the offline Maven repo checks churn, Linux re-validation needed);
- pod-sandbox correctness is already covered twice (pwa-wrapper-app benchmark + minimal-pods
  e2e fixture);
- the genuinely new surface (Flutter plugin pods are `:path` pods into `.symlinks`) is exactly
  what jfit exercises for real in §4.

So: minimal-app stays plugin-less; pods-through-Flutter is validated on jfit. Revisit only if a
plugin-pod-specific bug class appears that jfit can't reproduce hermetically.

---

## 4. jfit validation (machine-local, the acceptance bar for "fully compatible")

Runbook-tier (documented in `docs/ios-testing.md`, executed manually or via a future fnx target):
1. `flutter2nix lock --project-dir <jfit>/apps/mobile --gradle-user-home <fresh tmp>` — MUST use a
   fresh gradle home (poisoned-`~/.gradle` lesson; the resolve cache caches 404s). Assert both
   sections present; ios section carries the Firebase pod tree (trunk pods) + plugin path-pods
   classified out.
2. iOS: tier-2a CLI pipeline with the existing jfit signing material
   (`.ios2nix-signing.env`) → signed `.ipa` of the real app.
3. Android: existing `buildFlutterAndroidApp` path against the jfit lockfile (Linux machine/CI).
4. Record findings; any failure here is a P1 bug in the corresponding crate, not a jfit problem.

**Acceptance for the whole plan:** `buildFlutterApp` dispatches both platforms from one lockfile
with eval checks green on both systems; the Flutter signed-`.ipa` cargo e2e passes with real
material; jfit locks dual-platform and produces a signed iOS `.ipa` through the CLI pipeline;
`cargo check/clippy/test --workspace`, `nix flake check`, and `fnx check` all stay green.

---

### Notes / carried constraints
- Sandbox constraints are load-bearing and documented in the `ios-sandbox-constraints` memory +
  `flutter2nix-lib.nix` comments: DerivedData via `-derivedDataPath`; HOME/PATH as build
  settings; codesign shim (parent-dir writability); actool/ibtool impossible for nixbld.
- Never pass `PRODUCT_BUNDLE_IDENTIFIER` as a global xcodebuild arg for workspace builds — stamp
  the pbxproj.
- Secrets contract unchanged from Plan 3 §1 (`IOS2NIX_*`, password-by-file, never logged).
