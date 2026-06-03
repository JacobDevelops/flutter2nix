# Open Questions — flutter2nix Phase 0

## Phase 0: Monorepo Scaffold — 2026-06-03

### Resolved During Planning
- ✓ **Fenix vs rust-overlay**: Using fenix for Rust toolchain in flake.nix (standard in ecosystem).
- ✓ **Workspace resolver**: Using resolver = "2" (required for monorepo with path deps).
- ✓ **CI strategy**: Structural checks required (cargo, clippy, nix); e2e allowed-to-fail placeholder.

### Open for Phase 0 Execution
None — all scaffolding decisions are explicit in the plan.

### Open for Phase 1+
- **gradle2nix TAPI integration**: How deeply does gradle2nix integrate with tapi-shim? Bidirectional IPC?
- **Signing strategy for iOS**: Should keychain unlock be interactive or Nix-driven?
- **Nix package outputs**: Should gradle2nix/ios2nix/flutter2nix binaries be built from workspace Cargo, or separate derivations?
- **Test fixtures format**: What do real gradle/flutter projects in tests/fixtures/ look like? Sample projects from real repos?
- **Reproducibility boundary**: Where does Nix take over? After lockfiles generated? Full app build?
