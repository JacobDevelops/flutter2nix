#[allow(unused_imports)]
use super::*;

#[test]
#[ignore = "TODO: ios2nix not yet implemented"]
fn test_parse_xcode_build_output_valid() {
    todo!("Phase 1: stub — input: fixtures/xcode-outputs/basic.json, expect: Ok(XcodeBuildOutput {{ version: \"14.3\" }})")
}

#[test]
#[ignore = "TODO: ios2nix not yet implemented"]
fn test_parse_xcode_build_output_with_frameworks() {
    todo!("Phase 1: stub — input: fixtures/xcode-outputs/with-frameworks.json, expect: Ok(XcodeBuildOutput {{ frameworks: non-empty }})")
}

#[test]
#[ignore = "TODO: ios2nix not yet implemented"]
fn test_parse_xcode_build_output_malformed_missing_field() {
    todo!("Phase 1: stub — input: fixtures/xcode-outputs/malformed-missing-field.json, expect: Err(missing required field)")
}

#[test]
#[ignore = "TODO: ios2nix not yet implemented"]
fn test_parse_xcode_build_output_version_mismatch() {
    todo!("Phase 1: stub — input: fixtures/xcode-outputs/version-mismatch.json, expect: Err(unsupported schema version)")
}
