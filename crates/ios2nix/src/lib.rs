// ios2nix is a macOS-only tool: its CLI shells out to xcodebuild/security, so on
// non-Darwin targets the macOS-gated helpers (xcodebuild_args, find_ipa_in_dir,
// compute_signing_order, failure_detail, …) and their imports are intentionally
// dead code. The cross-platform surface (lock resolution, podspec/cocoapods
// parsing) still compiles and is what flutter2nix consumes on Linux. Tolerate the
// macOS-only dead code off-Darwin instead of failing `clippy -D warnings`; full
// lints still apply on macOS, where the crate is actually built and exercised.
#![cfg_attr(
    not(target_os = "macos"),
    allow(dead_code, unused_imports, unused_variables)
)]

pub mod cli;
pub mod cocoapods;
pub mod export_opts;
pub mod keychain;
pub mod podspec;
pub mod provisioning;
pub mod resolve_cache;
pub mod xcode;
