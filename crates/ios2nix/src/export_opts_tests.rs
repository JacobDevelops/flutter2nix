#[allow(unused_imports)]
use super::*;

#[test]
#[ignore = "TODO: ios2nix not yet implemented"]
fn test_generate_export_options_adhoc() {
    todo!("Phase 1: stub — input: method=adhoc team_id=TEAM123, expect: Ok(ExportOptions.plist with method=ad-hoc)")
}

#[test]
#[ignore = "TODO: ios2nix not yet implemented"]
fn test_generate_export_options_enterprise() {
    todo!("Phase 1: stub — input: method=enterprise team_id=TEAM123, expect: Ok(ExportOptions.plist with method=enterprise)")
}

#[test]
#[ignore = "TODO: ios2nix not yet implemented"]
fn test_generate_export_options_appstore() {
    todo!("Phase 1: stub — input: method=app-store team_id=TEAM123, expect: Ok(ExportOptions.plist with method=app-store)")
}

#[test]
#[ignore = "TODO: ios2nix not yet implemented"]
fn test_export_options_roundtrip_write_read() {
    todo!("Phase 1: stub — write ExportOptions.plist to temp dir, read back, expect: values match")
}

#[test]
#[ignore = "TODO: ios2nix not yet implemented"]
fn test_export_options_missing_team_id() {
    todo!("Phase 1: stub — input: method=adhoc team_id=None, expect: Err(missing team_id)")
}

#[test]
#[ignore = "TODO: ios2nix not yet implemented"]
fn test_export_options_invalid_export_method() {
    todo!("Phase 1: stub — input: method=invalid, expect: Err(unrecognized export method)")
}
