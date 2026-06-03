# iOS Testing

> **Status:** Placeholder — full runbook added in Phase 3.

This document will contain the manual test procedure for ios2nix, including:

- Prerequisites (macOS, Xcode, signing certificate, provisioning profile)
- Step-by-step: `ios2nix lock` → `ios2nix archive` → `ios2nix export`
- Environment variable setup (`IOS2NIX_P12_PATH`, etc.)
- Troubleshooting common Xcode version mismatches and cert import errors
- Cleanup procedure

See `crates/ios2nix/` for implementation.
