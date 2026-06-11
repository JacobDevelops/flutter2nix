# iOS Testing & Signing Runbook

> **⚠️ SECURITY WARNING** — Do NOT run signing builds with shell `set -x`, `xcodebuild -verbose`, or similar tracing while `IOS2NIX_P12_PASSWORD` or `IOS2NIX_KEYCHAIN_PASSWORD` are in the environment — they will appear in plaintext in CI logs, leaking the signing key password and keychain password. Instead:
> - ios2nix must NEVER log environment values; pass passwords only via argv or stdin to `security` commands, never interpolated into shell scripts.
> - Never echo or trace commands that reference `IOS2NIX_*` variables.
> - Prefer reading password files from a file or secret store (e.g., GitHub Actions Secrets, HashiCorp Vault) rather than plain environment variables where your CI platform supports it.
> - Scrub `IOS2NIX_*` from the environment after the build.

---

## Prerequisites

- **macOS** with **Xcode ≥ 14** (to ensure modern provisioning profile support)
- **Apple Developer certificate** exported as a `.p12` file (either Apple Distribution for App Store/Ad Hoc builds, or Development for internal testing)
- **Provisioning profile** as a `.mobileprovision` file matching the certificate and team
- **Apple Developer Team ID** (10 characters, visible in developer.apple.com)
- **CocoaPods** installed (via `brew install cocoapods` or included in Xcode)

### Certificate & Profile Setup (Manual)

If you don't have the signing material yet:
1. Visit [developer.apple.com/account](https://developer.apple.com/account)
2. In **Certificates, Identifiers & Profiles**, create or download an Apple Distribution certificate
3. Export it from Keychain as a `.p12` file with a strong password (you'll set `IOS2NIX_P12_PASSWORD`)
4. Create a provisioning profile for your app's bundle ID, download it as a `.mobileprovision` file
5. Copy your Team ID from the Membership page

---

## Secret Input Contract

The signing material is **not** committed to version control and **not** stored in the Nix store. Instead, it is supplied at build time via environment variables and file paths:

| Variable | Value | Notes |
|----------|-------|-------|
| `IOS2NIX_P12_PATH` | `/path/to/cert.p12` | Path to the signing certificate+key file. Readable by the build process. |
| `IOS2NIX_P12_PASSWORD` | `<password>` | Password protecting the `.p12` file. **Never log this.** |
| `IOS2NIX_PROFILE_PATH` | `/path/to/profile.mobileprovision` | Path to the provisioning profile file. |
| `IOS2NIX_KEYCHAIN_PASSWORD` | `<password>` | Password for the temporary keychain we create and own (you choose this). **Never log this.** |
| `IOS2NIX_TEAM_ID` | `TEAM123456` | Apple Developer Team ID (shown in developer.apple.com). |

**Example (local shell session):**
```bash
export IOS2NIX_P12_PATH="$HOME/Downloads/AppleDistribution.p12"
export IOS2NIX_P12_PASSWORD="my-secure-cert-password"
export IOS2NIX_PROFILE_PATH="$HOME/Downloads/FlutterApp.mobileprovision"
export IOS2NIX_KEYCHAIN_PASSWORD="my-temp-keychain-password"
export IOS2NIX_TEAM_ID="TEAM123456"

# Build with signing enabled (see Nix section below)
```

In **CI** (e.g., GitHub Actions):
```yaml
env:
  IOS2NIX_P12_PATH: "/tmp/cert.p12"
  IOS2NIX_PROFILE_PATH: "/tmp/profile.mobileprovision"
  IOS2NIX_KEYCHAIN_PASSWORD: ${{ secrets.KEYCHAIN_PASSWORD }}
  IOS2NIX_TEAM_ID: ${{ secrets.APPLE_TEAM_ID }}

before_script:
  # Securely write secrets to files (not env vars)
  - echo "${{ secrets.P12_PASSWORD }}" > /tmp/p12_pw.txt
  - echo "${{ secrets.P12_BYTES }}" | base64 -d > "$IOS2NIX_P12_PATH"
  - echo "${{ secrets.PROFILE_BYTES }}" | base64 -d > "$IOS2NIX_PROFILE_PATH"
  - export IOS2NIX_P12_PASSWORD="$(cat /tmp/p12_pw.txt)"
  # Never log these secrets; remove after use
```

---

## Walkthrough: Lock → Build → Archive → Export → Sign

Assume an iOS Flutter app in `flutter-app/` with the env vars set (see above).

### 1. `ios2nix lock` — Resolve CocoaPods dependencies

```bash
cd flutter-app
ios2nix lock --output flutter2nix.lock
```

**Expected output:**
```
Resolving CocoaPods dependencies...
Downloaded 42 pods (Firebase, GoogleUtilities, etc.)
Wrote ios2nix.lock
```

This produces a lockfile (`flutter2nix.lock`) listing all pods with pinned versions, hashes, and git revisions for deterministic installation.

### 2. `ios2nix build` — Verify pod and archive compatibility

```bash
ios2nix build \
  --lock flutter2nix.lock \
  --workspace ios/Runner.xcworkspace \
  --scheme Runner \
  --output build/analysis.json
```

**Expected output:**
```
Analyzed workspace ios/Runner.xcworkspace
Scheme: Runner
Configuration: Release
Wrote build/analysis.json
```

### 3. `ios2nix archive` — Create the unsigned `.xcarchive`

```bash
ios2nix archive \
  --lock flutter2nix.lock \
  --workspace ios/Runner.xcworkspace \
  --scheme Runner \
  --configuration Release \
  --output build/Runner.xcarchive
```

**Expected output:**
```
Installing pods...
Building archive: build/Runner.xcarchive
Done. Archive size: 542 MB
```

### 4. `ios2nix export` — Create the `.ipa` with signing

```bash
ios2nix export \
  --archive build/Runner.xcarchive \
  --method ad-hoc \
  --team-id "$IOS2NIX_TEAM_ID" \
  --signing-identity "Apple Distribution: My Company (TEAM123456)" \
  --provision-profile-uuid "a1b2c3d4-1234-5678-9abc-def012345678" \
  --output build/app.ipa
```

**Expected output:**
```
Setting up temporary keychain...
Importing certificate from $IOS2NIX_P12_PATH...
Installing provisioning profile...
Exporting archive with manual signing...
Signed IPA: build/app.ipa (18.5 MB)
```

The keychain is automatically cleaned up after export.

### 5. `ios2nix sign` — Re-sign an existing `.ipa` (optional)

If you need to re-sign an existing `.ipa` with a different certificate/profile:

```bash
ios2nix sign \
  --ipa build/app.ipa \
  --signing-identity "Apple Distribution: Other Company (OTHER12345)" \
  --provision-profile-uuid "e5f6a7b8-5678-90ab-cdef-012345678901" \
  --output build/app-resigned.ipa
```

**Expected output:**
```
Re-signing nested code (frameworks first, then extensions, then main app)...
Resigned IPA: build/app-resigned.ipa
Verifying signature...
Valid.
```

---

## Troubleshooting Matrix

| Symptom | Cause | Fix |
|---------|-------|-----|
| Build hangs during `xcodebuild archive` with no output for 5+ minutes | Missing `security set-key-partition-list` step; codesign is waiting for a UI prompt | Ensure `ios2nix sign-setup` is called before archive. Verify `IOS2NIX_KEYCHAIN_PASSWORD` is set. Check `security dump-keychain "$IOS2NIX_KEYCHAIN_PATH"` shows the imported cert. |
| Error: `errSecInternalComponent` during codesign | Keychain locked or partition list not configured | Run `security unlock-keychain "$IOS2NIX_KEYCHAIN_PATH"` manually; ensure `IOS2NIX_KEYCHAIN_PASSWORD` matches the one used in setup. |
| Error: `No profiles for 'com.example.app' were found` | Provisioning profile not installed or bundle ID mismatch | (1) Extract the profile's bundle ID: `security cms -D -i "$IOS2NIX_PROFILE_PATH" \| grep -A1 'application-identifier'`. (2) Verify it matches your app's bundle ID in Xcode. (3) Ensure `ios2nix` installed the profile to `~/Library/MobileDevice/Provisioning Profiles/`. |
| Error: `No signing certificate "Apple Distribution" found in keychain` | Certificate not in the search-list keychain | (1) Verify the cert is in the temp keychain: `security find-identity -v -p codesigning "$IOS2NIX_KEYCHAIN_PATH"`. (2) Ensure the keychain is in the search list: `security list-keychains -d user`. (3) Re-run `ios2nix sign-setup` if needed. |
| `xcodebuild -exportArchive` fails with method name not recognized (e.g., `"release-testing"` not found) | Xcode version mismatch; Xcode ≥16 uses new export method names | Set `IOS2NIX_XCODE_METHOD_NAMES=classic` to force classic names (ad-hoc, app-store, etc.) or `IOS2NIX_XCODE_METHOD_NAMES=xcode16` for new names (release-testing, app-store-connect, etc.). Run `xcodebuild -version` to confirm your Xcode version. |
| `pod install` hits network or timeout | CocoaPods CDN is reachable or internet connectivity issue | The build is NOT offline-sandboxed; network requests reach CocoaPods CDN. To fully offline sandbox, use the Nix integration (`nix build .#buildIOSApp-signed`) which provides a locked pod cache. For local builds, ensure internet is available or use a pre-downloaded pod cache. |

---

## Known Limitations (Plan 3, v1)

The `ios2nix sign` command for re-signing existing `.ipa` files does NOT yet handle:

- **Nested watchOS apps** (`Payload/*.app/Watch/*.app`) — re-signing skips these; use `xcodebuild -exportArchive` (primary signed path) if your app includes watchOS.
- **On-Demand Resources** (`OnDemandResources/` bundles) — not re-signed; again, rely on export.
- **Embedded `.dSYM` symbol files** — `.dSYM` bundles inside the `.ipa` are not signed; re-sign manually if needed for crash symbolication.
- **Replacing `embedded.mobileprovision`** — the re-sign step preserves the original provisioning profile; to change it, swap it in per bundle manually before calling `ios2nix sign`, or use the primary path (`xcodebuild -exportArchive`).

**Recommendation:** Use `xcodebuild -exportArchive` (the primary signed path) for app builds; it handles all of the above correctly. Use `ios2nix sign` only for re-signing existing IPAs when you need offline control over the signing cert/profile.

---

## Cleanup

After a successful or failed signing build, clean up the sensitive material:

```bash
# Delete the temporary keychain (should be automatic, but verify)
security delete-keychain "$IOS2NIX_KEYCHAIN_PATH" 2>/dev/null || true

# Remove the installed provisioning profile by UUID
rm -f "$HOME/Library/MobileDevice/Provisioning Profiles/$PROFILE_UUID.mobileprovision"

# Scrub the environment variables
unset IOS2NIX_P12_PATH IOS2NIX_P12_PASSWORD IOS2NIX_PROFILE_PATH IOS2NIX_KEYCHAIN_PASSWORD IOS2NIX_TEAM_ID
```

---

## Nix Integration

To build a signed iOS app using the Nix flake:

```nix
# flake.nix or a wrapper
{
  outputs = { self, nixpkgs, ... }:
    {
      lib.buildIOSApp {
        inherit pkgs;
        name = "my-ios-app";
        src = ./app-source;
        lockFile = ./app-source/ios/flutter2nix.lock;
        exportOptions = ./app-source/ios/ExportOptions.plist;
        # Unsigned (Plan 2 behavior):
        signing = null;
      };
      
      # Or, with signing enabled (impure, reads env):
      lib.buildIOSApp {
        inherit pkgs;
        name = "my-ios-app-signed";
        src = ./app-source;
        lockFile = ./app-source/ios/flutter2nix.lock;
        exportOptions = ./app-source/ios/ExportOptions.plist;
        # Signed path:
        signing = {
          teamId = "TEAM123456";
          identity = "Apple Distribution: My Company (TEAM123456)";
          profileSpecifier = "FlutterApp Production";
          # Optional: ios2nix = pkgs.ios2nix;  # defaults to pkgs.ios2nix
        };
      };
    };
}
```

When `signing != null`, the Nix derivation:
1. Sets `__noChroot = true` (impure; accesses local keychain, environment)
2. Calls `ios2nix sign-setup` to create a temp keychain, import the cert, and set partition list
3. Archives with manual-signing flags (`DEVELOPMENT_TEAM`, `CODE_SIGN_STYLE=Manual`, `CODE_SIGN_IDENTITY`, etc.)
4. Exports with the provided `ExportOptions.plist` (no `-allowProvisioningUpdates` — fully offline manual signing)
5. Cleans up the temp keychain on exit

**Important:** The IPA is NOT bit-reproducible. Signing depends on:
- Impure keychain state (cert, partition list)
- Provisioning profile installation state
- Embedded timestamps from codesign
- Apple network (currently none, but future versions may call Apple services)

Only the pod inputs (fetched by hash) are content-addressed. Useful for CI/CD workflows; not suitable for reproducible builds that require determinism.

---

## References

- [Apple Developer: Provisioning Profiles](https://developer.apple.com/help/account/manage-profiles/create-provisioning-profiles/)
- [Apple Developer: Certificates](https://developer.apple.com/help/account/manage-certificates/)
- [xcodebuild man page](https://developer.apple.com/library/archive/technotes/tn2339/_index.html) — ExportOptions.plist schema
- [security(1) man page](https://man.archlinux.org/man/security.1p.en.html) — keychain commands
- [Apple Forum: set-key-partition-list for non-interactive codesign](https://developer.apple.com/forums/thread/666107)
