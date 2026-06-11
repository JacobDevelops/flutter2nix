#[allow(unused_imports)]
use super::*;

#[test]
fn test_parse_profile_plist_valid() {
    // This is a minimal but realistic decoded provisioning profile plist
    let plist_bytes = br#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>UUID</key><string>ef3d7190-5839-4429-ad81-c82cf90e444a</string>
  <key>Name</key><string>Example AdHoc Profile</string>
  <key>TeamIdentifier</key><array><string>TEAM123456</string></array>
  <key>ExpirationDate</key><string>2025-12-31T23:59:59Z</string>
  <key>Entitlements</key><dict>
    <key>application-identifier</key><string>TEAM123456.com.example.app</string>
    <key>get-task-allow</key><false/>
    <key>team-identifier</key><string>TEAM123456</string>
  </dict>
  <key>AppIDName</key><string>Example App</string>
</dict></plist>"#;

    let profile_info = parse_profile_plist(plist_bytes).expect("should parse valid plist");

    assert_eq!(profile_info.uuid, "ef3d7190-5839-4429-ad81-c82cf90e444a");
    assert_eq!(profile_info.name, "Example AdHoc Profile");
    assert_eq!(profile_info.bundle_id, "com.example.app");
    assert_eq!(profile_info.team_id, "TEAM123456");
    assert_eq!(
        profile_info.expiration_date,
        Some("2025-12-31T23:59:59Z".to_string())
    );
}

#[test]
fn test_parse_profile_plist_missing_uuid() {
    let plist_bytes = br#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>Name</key><string>Example AdHoc Profile</string>
  <key>TeamIdentifier</key><array><string>TEAM123456</string></array>
  <key>Entitlements</key><dict>
    <key>application-identifier</key><string>TEAM123456.com.example.app</string>
  </dict>
</dict></plist>"#;

    let result = parse_profile_plist(plist_bytes);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("missing UUID"));
}

#[test]
fn test_parse_profile_plist_missing_name() {
    let plist_bytes = br#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>UUID</key><string>ef3d7190-5839-4429-ad81-c82cf90e444a</string>
  <key>TeamIdentifier</key><array><string>TEAM123456</string></array>
  <key>Entitlements</key><dict>
    <key>application-identifier</key><string>TEAM123456.com.example.app</string>
  </dict>
</dict></plist>"#;

    let result = parse_profile_plist(plist_bytes);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("missing Name"));
}

#[test]
fn test_parse_profile_plist_invalid_plist() {
    let plist_bytes = b"not a plist at all";
    let result = parse_profile_plist(plist_bytes);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("failed to parse"));
}

#[test]
fn test_parse_profile_plist_missing_team_identifier() {
    let plist_bytes = br#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>UUID</key><string>ef3d7190-5839-4429-ad81-c82cf90e444a</string>
  <key>Name</key><string>Example AdHoc Profile</string>
  <key>Entitlements</key><dict>
    <key>application-identifier</key><string>TEAM123456.com.example.app</string>
  </dict>
</dict></plist>"#;

    let result = parse_profile_plist(plist_bytes);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("missing or invalid TeamIdentifier"));
}

#[test]
fn test_parse_profile_plist_missing_application_identifier() {
    let plist_bytes = br#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>UUID</key><string>ef3d7190-5839-4429-ad81-c82cf90e444a</string>
  <key>Name</key><string>Example AdHoc Profile</string>
  <key>TeamIdentifier</key><array><string>TEAM123456</string></array>
  <key>Entitlements</key><dict>
  </dict>
</dict></plist>"#;

    let result = parse_profile_plist(plist_bytes);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("missing application-identifier"));
}

#[test]
fn test_parse_profile_plist_no_expiration_date() {
    let plist_bytes = br#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>UUID</key><string>ef3d7190-5839-4429-ad81-c82cf90e444a</string>
  <key>Name</key><string>Example AdHoc Profile</string>
  <key>TeamIdentifier</key><array><string>TEAM123456</string></array>
  <key>Entitlements</key><dict>
    <key>application-identifier</key><string>TEAM123456.com.example.app</string>
  </dict>
</dict></plist>"#;

    let profile_info = parse_profile_plist(plist_bytes).expect("should parse plist");

    assert_eq!(profile_info.uuid, "ef3d7190-5839-4429-ad81-c82cf90e444a");
    assert_eq!(profile_info.expiration_date, None);
}

#[test]
fn test_parse_profile_plist_bundle_id_extraction() {
    let plist_bytes = br#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>UUID</key><string>ef3d7190-5839-4429-ad81-c82cf90e444a</string>
  <key>Name</key><string>Example AdHoc Profile</string>
  <key>TeamIdentifier</key><array><string>TEAM123456</string></array>
  <key>Entitlements</key><dict>
    <key>application-identifier</key><string>TEAM123456.com.example.app.nested.bundle</string>
  </dict>
</dict></plist>"#;

    let profile_info = parse_profile_plist(plist_bytes).expect("should parse plist");

    // Should strip TEAM123456. prefix
    assert_eq!(profile_info.bundle_id, "com.example.app.nested.bundle");
}

#[test]
fn test_parse_profile_plist_multiple_team_identifiers() {
    let plist_bytes = br#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>UUID</key><string>ef3d7190-5839-4429-ad81-c82cf90e444a</string>
  <key>Name</key><string>Example AdHoc Profile</string>
  <key>TeamIdentifier</key><array>
    <string>TEAM123456</string>
    <string>TEAM789012</string>
  </array>
  <key>Entitlements</key><dict>
    <key>application-identifier</key><string>TEAM123456.com.example.app</string>
  </dict>
</dict></plist>"#;

    let profile_info = parse_profile_plist(plist_bytes).expect("should parse plist");

    // Should take the first team identifier
    assert_eq!(profile_info.team_id, "TEAM123456");
}
