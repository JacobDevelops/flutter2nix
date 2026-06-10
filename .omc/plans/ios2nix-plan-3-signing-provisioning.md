# ios2nix — Plan 3: Signing & Provisioning (macOS, fully specified)

> Reads with: `ios2nix-implementation-plan.md` (overview) and depends on **Plan 2** (the
> archive/export skeleton). This is the deep-dive that the scaffold under-models: the `export_opts`
> stub takes only `method + team_id`, and the `keychain` stub omits the steps that actually make
> non-interactive `codesign` work. Every command below is verified against current Apple/CI practice
> (sources at end). Status: pending approval.

**Scope:** the complete signing surface —
1. Temp-keychain lifecycle (create → unlock → import → **`set-key-partition-list`** → search-list →
   delete), the step whose absence silently hangs/fails codesign in CI.
2. Provisioning-profile install (UUID extraction → copy to the canonical dir).
3. The **full `ExportOptions.plist` model** (expanding `export_opts` far beyond the stub:
   `signingStyle`, `signingCertificate`, `provisioningProfiles` map, `destination`, symbol flags).
4. Signed `archive` → signed `export`; the `sign` re-sign subcommand (`codesign`).
5. The secret-input contract (`IOS2NIX_*` env vars), Nix-side impurity, and the runbook.

**Turns green:** `keychain_tests` (4), `export_opts_tests` (6 + new), `cli/sign_tests` (2),
`cli/export_tests` signed paths, and the macOS integration stubs `test_cli_export_ipa_with_codesign`
+ `test_cli_full_e2e_lock_to_ipa`.

**Non-goals:** TestFlight/App Store Connect *upload* (`notarytool`/`altool`) — out of v1; the model
leaves `destination: upload` reachable so it can be added later. Automatic (cloud) signing — v1 is
**manual signing only** (deterministic, offline-friendly, no `-allowProvisioningUpdates`).

---

## 1. Secret-input contract (how certs/profiles enter the process)

Signing material is secret and machine-specific → it is **never** committed and **never** a Nix
input by content (that would put a `.p12` in the store). It enters at *runtime* via env/args, and
the Nix derivation that signs is impure (`__noChroot`, reads env). Contract:

| Variable / arg | Meaning |
|---|---|
| `IOS2NIX_P12_PATH` | path to the signing cert+key `.p12` (Apple Distribution / Development). |
| `IOS2NIX_P12_PASSWORD` | password protecting the `.p12`. |
| `IOS2NIX_PROFILE_PATH` | path to the `.mobileprovision` (or a dir of them). |
| `IOS2NIX_KEYCHAIN_PASSWORD` | password for the *temporary* keychain we create (we own it). |
| `--team-id` / `IOS2NIX_TEAM_ID` | Apple Developer Team ID (10 chars). |
| `--signing-identity` | e.g. `"Apple Distribution: Example Corp (TEAM123456)"` (resolved from the imported cert if omitted). |

All are read at `export`/`sign` time. Tests inject fixtures + a throwaway keychain; the sidecar path
(Plan 2) lets the *non-signing* steps run on Linux, but the signing steps themselves are
`#[cfg(target_os="macos")]` and exercised only on a Mac.

---

## 2. `keychain.rs` — temporary keychain lifecycle (macOS)

The make-or-break sequence. `set-key-partition-list` is the step that lets `codesign` use the
private key **without a UI prompt** — its absence is the #1 cause of CI codesign hangs/
`errSecInternalComponent`.

```rust
#[cfg(target_os="macos")]
pub struct TempKeychain { path: PathBuf, password: String }   // RAII: Drop → `security delete-keychain`

#[cfg(target_os="macos")]
impl TempKeychain {
    pub fn create(password:&str) -> anyhow::Result<Self>;       // create + unlock + settings
    pub fn import_identity(&self, p12:&Path, p12_pw:&str) -> anyhow::Result<()>;  // import + partition-list
    pub fn add_to_search_list(&self) -> anyhow::Result<()>;     // so codesign/xcodebuild find it
    pub fn signing_identities(&self) -> anyhow::Result<Vec<String>>;  // `security find-identity -v -p codesigning`
}
```
Exact commands (each run via `std::process::Command`, errors surfaced with `anyhow::Context`):
```sh
# create (we own the password from the start — required for set-key-partition-list)
security create-keychain -p "$KPW" "$KC"
security set-keychain-settings -lut 21600 "$KC"        # 6h, avoid mid-build auto-lock
security unlock-keychain -p "$KPW" "$KC"

# import the cert+key; -T grants the listed tools access without prompting
security import "$P12" -P "$P12PW" -k "$KC" -T /usr/bin/codesign -T /usr/bin/security -f pkcs12

# THE CRITICAL STEP — authorize non-interactive key use for codesign:
security set-key-partition-list -S apple-tool:,apple:,codesign: -s -k "$KPW" "$KC"

# make codesign/xcodebuild search this keychain (prepend; preserve the existing list)
security list-keychains -d user -s "$KC" $(security list-keychains -d user | sed 's/"//g')

# optional but recommended on macOS 12+: also set it default so tools that consult only the
# default keychain (not the search list) still find the identity. Capture the prior default to
# restore on cleanup.
PRIOR_DEFAULT=$(security default-keychain -d user | tr -d ' "')
security default-keychain -d user -s "$KC"
```
Cleanup (RAII `Drop`, and an explicit `cli`-level finally): `security delete-keychain "$KC"`,
`security default-keychain -d user -s "$PRIOR_DEFAULT"`, and restore the prior search list. Green: `test_create_temp_keychain_{success,cleanup}`,
`test_import_certificate_to_keychain_{valid,invalid_format}`.

> **Why a *temporary* keychain:** we control its password from creation, which
> `set-key-partition-list` requires; it isolates from the login keychain; and `Drop` guarantees the
> private key doesn't linger on the build host.

---

## 3. Provisioning-profile install (macOS)

Xcode resolves profiles by UUID from a fixed directory. Install = extract UUID, copy by UUID name.
```rust
#[cfg(target_os="macos")]
pub fn install_provisioning_profile(profile:&Path) -> anyhow::Result<InstalledProfile>;
pub struct InstalledProfile { pub uuid:String, pub name:String, pub bundle_id:String, pub team_id:String, pub installed_path:PathBuf }
```
Commands:
```sh
# decode the CMS-signed plist, then read fields:
security cms -D -i "$PROFILE" -o /tmp/p.plist
UUID=$(/usr/libexec/PlistBuddy -c 'Print :UUID' /tmp/p.plist)
NAME=$(/usr/libexec/PlistBuddy -c 'Print :Name' /tmp/p.plist)
APPID=$(/usr/libexec/PlistBuddy -c 'Print :Entitlements:application-identifier' /tmp/p.plist) # TEAM.bundleID

DEST="$HOME/Library/MobileDevice/Provisioning Profiles"     # canonical location (still honored)
mkdir -p "$DEST"
cp "$PROFILE" "$DEST/$UUID.mobileprovision"
```
Parse the profile plist (pure → Linux-testable) to populate `InstalledProfile`; the copy step is
macOS-only. The existing `provisioning-profiles/*.mobileprovision` fixtures are stubs (`STUB_ADHOC`)
— **add a realistic decoded-plist fixture** so the parser (UUID/Name/bundleID/teamID/expiry
extraction) is unit-tested on Linux without needing `security cms`.

> **Xcode 16 note:** newer Xcode also reads
> `~/Library/Developer/Xcode/UserData/Provisioning Profiles/`. Install to **both** dirs to be safe;
> verify against the target Xcode in the Phase -1 / Plan-2 spike environment.

---

## 4. `export_opts.rs` — the full ExportOptions.plist model (expands the stub)

The stub only models `method + team_id`. Real manual signing needs the full schema. Verified key set
(`xcodebuild -help`):

```rust
pub enum ExportMethod { AppStore, AdHoc, Enterprise, Development, DeveloperId, Validation }
pub enum SigningStyle { Manual, Automatic }
pub enum Destination { Export, Upload }

pub struct ExportOptions {
    pub method: ExportMethod,
    pub team_id: String,
    pub signing_style: SigningStyle,                 // v1 default Manual
    pub signing_certificate: Option<String>,         // manual: "Apple Distribution" / full identity
    pub provisioning_profiles: BTreeMap<String,String>, // manual: bundleID -> profile UUID (NOT name — determinism)
    pub destination: Destination,                    // default Export
    pub strip_swift_symbols: bool,                   // default true
    pub upload_symbols: bool,                        // App Store only
    pub compile_bitcode: bool,                       // default false (deprecated by Apple)
    pub manage_app_version_and_build_number: bool,   // default false for offline/manual
}
pub fn generate_export_options_plist(opts:&ExportOptions) -> anyhow::Result<String>; // → XML plist
pub fn write_export_options(opts:&ExportOptions, path:&Path) -> anyhow::Result<()>;
```
**`method` → plist string, with Xcode-version auto-detection:** parse `xcodebuild -version` once;
emit **classic** names for Xcode ≤15, **Xcode-16 aliases** for Xcode ≥16 (both still accepted today,
but emitting the version-appropriate name avoids drift surprises). Override via env
`IOS2NIX_XCODE_METHOD_NAMES=classic|xcode16`. Mapping: `AppStore→app-store / app-store-connect`,
`AdHoc→ad-hoc / release-testing`, `Development→development / debugging`, `Enterprise→enterprise`,
`DeveloperId→developer-id`, `Validation→validation`. `signingStyle`: `manual|automatic`.
`destination`: `export|upload`.

**Validation rules (drive the error tests):** `team_id` required for all non-`development` methods;
manual style requires `signing_certificate` + ≥1 `provisioning_profiles` entry; **every
`provisioning_profiles` value must be a 36-char UUID** (reject names — error: "use the profile UUID,
not its name, for deterministic resolution"); unknown method string → error. **Multi-bundle apps:**
the map needs one entry per embedded bundle (main app **and each `.appex` extension**); the bundle-ID
list is read from the `.xcarchive`'s `Info.plist` at export time (Fable wires this in the
integration; the model already supports N entries).

Green: existing `test_generate_export_options_{adhoc,enterprise,appstore}`,
`test_export_options_roundtrip_write_read`, `test_export_options_{missing_team_id,invalid_export_method}`
**plus new**: manual-without-profile → err; non-UUID profile value → err; provisioningProfiles map
round-trips; signingStyle present in plist. All pure → **Linux-testable**. **Ownership (overview
§2.5):** the `export_opts` module and ALL its tests live in **Plan 3** (even though they run on
Linux) so the export/signing surface is cohesive; Plan 1 only ships the compiling cfg stub.

Example emitted plist (ad-hoc, manual):
```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>method</key><string>ad-hoc</string>
  <key>teamID</key><string>TEAM123456</string>
  <key>signingStyle</key><string>manual</string>
  <key>signingCertificate</key><string>Apple Distribution</string>
  <key>provisioningProfiles</key><dict>
    <key>com.example.app</key><string>ef3d7190-5839-4429-ad81-c82cf90e444a</string>
    <!-- one entry PER embedded bundle — main app + each .appex extension: -->
    <key>com.example.app.ShareExtension</key><string>a1b2c3d4-1111-2222-3333-444455556666</string>
  </dict>
  <key>stripSwiftSymbols</key><true/>
  <key>compileBitcode</key><false/>
</dict></plist>
```

---

## 5. Signed archive → signed export, and the `sign` subcommand

### 5a. Signed archive (extends Plan 2 §2b with manual-signing flags)
```sh
xcodebuild archive \
  -workspace Runner.xcworkspace -scheme Runner -configuration Release \
  -archivePath build/Runner.xcarchive -destination 'generic/platform=iOS' \
  DEVELOPMENT_TEAM="$TEAM" CODE_SIGN_STYLE=Manual \
  CODE_SIGN_IDENTITY="$IDENTITY" PROVISIONING_PROFILE_SPECIFIER="$PROFILE_NAME" \
  OTHER_CODE_SIGN_FLAGS="--keychain $KC"
```

### 5b. Signed export (Plan 2 §2c + the Plan-3 plist; manual ⇒ no network)
```sh
xcodebuild -exportArchive -archivePath build/Runner.xcarchive \
  -exportOptionsPlist build/ExportOptions.plist -exportPath build/ipa
# NOTE: deliberately NO -allowProvisioningUpdates → fully offline manual signing.
```

### 5c. `cli::sign` — re-sign an existing `.ipa`
```rust
#[cfg(target_os="macos")] pub fn run(cmd: SignCommand) -> anyhow::Result<PathBuf>;
```
Unzip → re-sign nested code **strictly inside-out** (deepest first), repackage. The order matters:
a parent signature is invalidated if a child is re-signed afterward, so frameworks → each
extension's frameworks → each extension → main app.
```sh
unzip -q "$IPA" -d work
APP=$(echo work/Payload/*.app)

# 1. main-app frameworks/dylibs
for fw in "$APP"/Frameworks/*; do
  codesign -f -s "$IDENTITY" --keychain "$KC" --timestamp=none "$fw"
done
# 2. app extensions (.appex) — sign THEIR frameworks first, then the extension itself.
#    Each .appex has its own bundle ID + its own embedded.mobileprovision + entitlements.
for ext in "$APP"/PlugIns/*.appex; do
  [ -e "$ext" ] || continue
  for efw in "$ext"/Frameworks/*; do
    [ -e "$efw" ] && codesign -f -s "$IDENTITY" --keychain "$KC" --timestamp=none "$efw"
  done
  EXT_ENT=$(extract_entitlements "$ext/embedded.mobileprovision")   # security cms -D → :Entitlements
  codesign -f -s "$IDENTITY" --keychain "$KC" --timestamp=none --entitlements "$EXT_ENT" "$ext"
done
# 3. main app last
codesign -f -s "$IDENTITY" --keychain "$KC" --timestamp=none --entitlements "$ENT" "$APP"

( cd work && zip -qry "../$OUT" Payload )
codesign --verify --deep --strict "$APP"   # sanity — fails loudly if any nested code is unsigned
```
Entitlements come from each bundle's embedded provisioning profile (`security cms -D` →
`:Entitlements`). Green: `test_sign_ipa_with_certificate`, `test_sign_ipa_invalid_cert`; **add**
`test_sign_ipa_with_extension` (a fixture `.ipa` carrying one `.appex`).

> **Known limitations (v1 scope — documented, not silently dropped):** the loop covers
> frameworks + `.appex` extensions + the main app. It does **not yet** handle nested watchOS apps
> (`Payload/*.app/Watch/*.app`), on-demand-resource asset packs (`OnDemandResources/`), or `.dSYM`
> signing. It does **not** replace `embedded.mobileprovision` (re-signing with a *different* profile
> requires swapping it in per bundle). Flag these in `docs/ios-testing.md`; add when a real app needs
> them. (Note: the primary signed path is `xcodebuild -exportArchive` in §5b, which handles all of
> these correctly — `cli::sign` is the secondary re-sign-an-existing-`.ipa` tool.)

---

## 6. `nix/ios2nix-lib.nix` — wiring signing into `buildIOSApp`

Extend Plan 2's `buildIOSApp` with optional signing, kept impure and env-driven:
```nix
buildIOSApp = { pkgs, name, src, lockFile, scheme ? "Runner", configuration ? "Release"
              , exportOptions                      # path to a generated ExportOptions.plist
              , signing ? null                     # null ⇒ unsigned/dev (Plan 2); attrset ⇒ signed
              , ... }:
  pkgs.stdenv.mkDerivation {
    inherit name src;
    __noChroot = true; meta.platforms = lib.platforms.darwin;
    buildInputs = [ pkgs.cocoapods ];
    # Signing material is read from the impure environment at build time, NEVER from the store:
    #   IOS2NIX_P12_PATH, IOS2NIX_P12_PASSWORD, IOS2NIX_PROFILE_PATH, IOS2NIX_KEYCHAIN_PASSWORD
    buildPhase = ''
      ${lib.optionalString (signing != null) ''
        # ios2nix sign-setup: create temp keychain, import $IOS2NIX_P12_PATH, set-key-partition-list,
        # install $IOS2NIX_PROFILE_PATH by UUID  (delegates to the `ios2nix` CLI subcommands)
      ''}
      # pod install --no-repo-update (offline sandbox from Plan 2 §2)
      # xcodebuild archive (signed flags if signing != null)
      # xcodebuild -exportArchive -exportOptionsPlist ${exportOptions}
    '';
    installPhase = '' mkdir -p $out; cp build/ipa/*.ipa $out/ '';
  };
```
**P4 honesty comment** in the file: the IPA is not bit-reproducible; signing depends on impure
keychain + profile state supplied via env; only pod inputs are content-addressed.

---

## 7. `docs/ios-testing.md` — the runbook (replace the placeholder)

Concrete sections:
- **⚠️ SECURITY (top of the doc):** **never** run signing with shell `set -x` or
  `xcodebuild -verbose`/`-quiet=false` while `IOS2NIX_P12_PASSWORD`/`IOS2NIX_KEYCHAIN_PASSWORD` are in
  the environment — they will be echoed to CI logs in plaintext. ios2nix itself must NOT log env
  values, must pass passwords via argv/stdin to `security` (not interpolated into a traced shell),
  and should scrub them from any captured command output. Prefer reading the `.p12` password from a
  file/secret store over a plain env var where the CI supports it.
- **Prereqs:** macOS + Xcode ≥ min; an Apple Distribution (or Development) cert as `.p12`; a matching
  `.mobileprovision`; the Team ID.
- **Env setup:** export the `IOS2NIX_*` vars (§1) — with a worked example.
- **Walkthrough:** `ios2nix lock` → `ios2nix build` → `ios2nix archive` → `ios2nix export` (and
  `ios2nix sign` for re-signing), each with expected output.
- **Troubleshooting matrix** (the high-frequency failures):
  | Symptom | Cause | Fix |
  |---|---|---|
  | codesign hangs / `errSecInternalComponent` | missing `set-key-partition-list` | run §2 step; ensure we own `$KPW` |
  | `No profiles for 'com.x' were found` | profile not installed / bundleID mismatch | §3 install; check `provisioningProfiles` map keys |
  | `No signing certificate "iOS Distribution" found` | cert not in search-list keychain | `add_to_search_list`; verify `find-identity -v -p codesigning` |
  | export fails on `method` | Xcode-version method-name drift | toggle classic vs Xcode16 names (§4) |
  | `pod install` hits network | trunk CDN reachable / not `--no-repo-update` | §Plan2 offline sandbox; block CDN in spike |
- **Cleanup:** `security delete-keychain`, remove installed profile by UUID, scrub `IOS2NIX_*`.

---

## 8. Tests & acceptance

**Unit (Linux):** `export_opts` full model (all methods, signingStyle, provisioningProfiles map,
validation errors) — pure; provisioning-profile **plist parser** (UUID/Name/bundleID/teamID) against
a new decoded-plist fixture.
**Unit/integration (macOS):** `keychain` lifecycle (create/import/partition-list/cleanup); `sign`
re-sign; signed `export`.
**E2E (macOS, allowed-to-fail):** `test_cli_full_e2e_lock_to_ipa` — `lock → build → archive →
export → sign` produces a signed `.ipa`; assert: valid ZIP; `Payload/<App>.app/Info.plist` has
`CFBundleVersion`; `codesign --verify --deep --strict` passes; embedded `embedded.mobileprovision`
present. **NOT** asserted: byte-reproducibility, notarization.

**Acceptance:** on macOS, all keychain/export_opts/sign tests + the signed e2e are green; the runbook
reproduces a signed `.ipa` from a real cert+profile. On Linux, the `export_opts` + profile-parser
units stay green and `cargo check/clippy --workspace` is unaffected.

---

### Consensus footer
Round-2 review applied: signing modeled in full (no longer a stub) — keychain `set-key-partition-list`,
profile UUID install, complete `ExportOptions.plist` schema with `provisioningProfiles`/`signingStyle`/
`signingCertificate`, inside-out `codesign`, env-driven secret contract, impure-but-honest Nix wiring,
and a troubleshooting runbook. Method-name + profile-dir version drift flagged for Fable to confirm
against the target Xcode. Status: pending approval.

### Sources (signing specifics verified)
- ExportOptions.plist keys/values: <https://gist.github.com/jessedc/12a74aff88d06e669cf1c9999408c62c>
- `set-key-partition-list` for non-interactive codesign: <https://developer.apple.com/forums/thread/666107>, <https://dev.to/kylefoo/install-p12-certificate-on-the-cicds-macos-executor-470k>
- Provisioning-profile UUID install: <https://gist.github.com/benvium/2568707>, <https://developer.apple.com/documentation/devicemanagement/install-provisioning-profile-command>
