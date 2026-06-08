#[allow(unused_imports)]
use super::*;

#[test]
#[ignore = "TODO: ios2nix not yet implemented"]
fn test_setup_xcode_env_sets_developer_dir() {
    todo!("Phase 1: stub — expect: DEVELOPER_DIR env var is set to Xcode.app/Contents/Developer")
}

#[test]
#[ignore = "TODO: ios2nix not yet implemented"]
fn test_setup_xcode_env_sets_sdkroot() {
    todo!("Phase 1: stub — expect: SDKROOT env var is set to iPhoneOS.sdk path")
}

#[test]
#[ignore = "TODO: ios2nix not yet implemented"]
fn test_setup_xcode_env_preserves_user_vars() {
    todo!("Phase 1: stub — input: pre-set MY_VAR=foo, expect: MY_VAR still equals foo after setup")
}

#[test]
#[ignore = "TODO: ios2nix not yet implemented"]
fn test_setup_xcode_env_xcode_not_found() {
    todo!("Phase 1: stub — input: DEVELOPER_DIR points to non-existent path, expect: Err or graceful fallback")
}

#[test]
#[ignore = "TODO: ios2nix not yet implemented"]
fn test_setup_xcode_env_invalid_xcode_path() {
    todo!("Phase 1: stub — input: DEVELOPER_DIR=/tmp/not-xcode, expect: Err(invalid Xcode path)")
}
