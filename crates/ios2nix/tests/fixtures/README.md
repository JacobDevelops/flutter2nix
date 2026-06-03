# ios2nix Test Fixtures

## Recording Environment

All fixtures are recorded against:
- **Xcode 15.3** (iOS SDK 17.2)
- **CocoaPods 1.15.0**

Re-record fixtures when Xcode major version changes.

## Directory Layout

- **xcode-outputs/**: Xcode build tool output JSON stubs (basic.json, with-frameworks.json, etc.)
- **xcode-projects/**: Skeleton Xcode project directories — empty placeholder, reserved for Phase 2 when real xcodebuild invocations are mocked via the sidecar pattern.
- **podfile-locks/**: Podfile.lock YAML fixtures (`*.lock`). Uses **real Podfile.lock YAML format** (CocoaPods 1.15.0), not JSON.
- **cocoapods-specs/**: CocoaPods spec JSON stubs (flutter.json, firebase-core.json, etc.)
- **provisioning-profiles/**: Provisioning profile stub binaries (.mobileprovision files)
- **nix-outputs/**: Expected Nix derivation outputs (inline and modular formats)
- **xcode-schema.json**: JSON Schema stub describing the Xcode build output format

## Sidecar Pattern for Xcode Tool Invocation

When a test needs to stub an Xcode tool invocation (e.g., `xcodebuild`):

1. Create an Xcode project fixture directory (e.g., `xcode-projects/simple-app/`)
2. Place a `.ios2nix-xcode-output.json` sidecar file in that directory
3. At test time, the code under test reads the sidecar instead of invoking the real Xcode tool

Example:
```
xcode-projects/simple-app/
├── .ios2nix-xcode-output.json  ← points to xcode-outputs/basic.json content
└── <stub project files would go here>
```

This mirrors the gradle2nix pattern (`.gradle2nix-tapi-output.json`).

## CocoaPods Lockfile Format

`podfile-locks/*.lock` files use the real Podfile.lock YAML format as produced by CocoaPods 1.15.0.
They are **not** JSON — parsers must use a YAML library, not `serde_json`.

Example structure:
```yaml
PODS:
  - Flutter (1.0.0)

DEPENDENCIES:
  - Flutter

SPEC CHECKSUMS:
  Flutter: <sha256hex>

PODFILE CHECKSUM: <sha256hex>

COCOAPODS: 1.15.0
```
