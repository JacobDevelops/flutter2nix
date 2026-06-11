#[allow(unused_imports)]
use super::*;
use std::collections::BTreeMap;
use std::str::FromStr;
use tempfile::TempDir;

#[test]
fn test_generate_export_options_adhoc() {
    let mut opts = ExportOptions::new(ExportMethod::AdHoc, "TEAM123456".to_string());
    opts.signing_style = SigningStyle::Manual;
    opts.signing_certificate = Some("Apple Distribution".to_string());
    opts.provisioning_profiles.insert(
        "com.example.app".to_string(),
        "ef3d7190-5839-4429-ad81-c82cf90e444a".to_string(),
    );

    let plist = generate_export_options_plist(&opts, MethodNameStyle::Classic)
        .expect("should generate plist");
    assert!(plist.contains("<key>method</key><string>ad-hoc</string>"));
    assert!(plist.contains("<key>teamID</key><string>TEAM123456</string>"));
    assert!(plist.contains("<key>signingStyle</key><string>manual</string>"));
}

#[test]
fn test_generate_export_options_enterprise() {
    let mut opts = ExportOptions::new(ExportMethod::Enterprise, "TEAM123456".to_string());
    opts.signing_style = SigningStyle::Manual;
    opts.signing_certificate = Some("Apple Distribution".to_string());
    opts.provisioning_profiles.insert(
        "com.example.app".to_string(),
        "ef3d7190-5839-4429-ad81-c82cf90e444a".to_string(),
    );

    let plist = generate_export_options_plist(&opts, MethodNameStyle::Classic)
        .expect("should generate plist");
    assert!(plist.contains("<key>method</key><string>enterprise</string>"));
    assert!(plist.contains("<key>teamID</key><string>TEAM123456</string>"));
}

#[test]
fn test_generate_export_options_appstore() {
    let mut opts = ExportOptions::new(ExportMethod::AppStore, "TEAM123456".to_string());
    opts.signing_style = SigningStyle::Manual;
    opts.signing_certificate = Some("Apple Distribution".to_string());
    opts.provisioning_profiles.insert(
        "com.example.app".to_string(),
        "ef3d7190-5839-4429-ad81-c82cf90e444a".to_string(),
    );
    opts.upload_symbols = true;

    let plist = generate_export_options_plist(&opts, MethodNameStyle::Classic)
        .expect("should generate plist");
    assert!(plist.contains("<key>method</key><string>app-store</string>"));
    assert!(plist.contains("<key>uploadSymbols</key><true/>"));
}

#[test]
fn test_export_options_roundtrip_write_read() {
    let temp_dir = TempDir::new().expect("should create temp dir");
    let plist_path = temp_dir.path().join("ExportOptions.plist");

    let mut opts = ExportOptions::new(ExportMethod::AdHoc, "TEAM123456".to_string());
    opts.signing_style = SigningStyle::Manual;
    opts.signing_certificate = Some("Apple Distribution".to_string());
    opts.provisioning_profiles.insert(
        "com.example.app".to_string(),
        "ef3d7190-5839-4429-ad81-c82cf90e444a".to_string(),
    );
    opts.provisioning_profiles.insert(
        "com.example.app.ShareExtension".to_string(),
        "a1b2c3d4-1111-2222-3333-444455556666".to_string(),
    );
    opts.strip_swift_symbols = true;
    opts.compile_bitcode = false;

    // Write
    write_export_options(&opts, MethodNameStyle::Classic, &plist_path).expect("should write plist");

    // Read back
    let plist_bytes = std::fs::read(&plist_path).expect("should read plist");
    let plist_dict: plist::Dictionary =
        plist::from_bytes(&plist_bytes).expect("should parse plist");

    // Verify values match
    assert_eq!(
        plist_dict.get("method").and_then(|v| v.as_string()),
        Some("ad-hoc")
    );
    assert_eq!(
        plist_dict.get("teamID").and_then(|v| v.as_string()),
        Some("TEAM123456")
    );
    assert_eq!(
        plist_dict.get("signingStyle").and_then(|v| v.as_string()),
        Some("manual")
    );
    assert_eq!(
        plist_dict
            .get("stripSwiftSymbols")
            .and_then(|v| v.as_boolean()),
        Some(true)
    );
    assert_eq!(
        plist_dict
            .get("compileBitcode")
            .and_then(|v| v.as_boolean()),
        Some(false)
    );

    // Verify provisioning profiles map
    if let Some(profiles_value) = plist_dict.get("provisioningProfiles") {
        let profiles = profiles_value.as_dictionary().expect("should be a dict");
        assert_eq!(profiles.len(), 2);
        assert_eq!(
            profiles.get("com.example.app").and_then(|v| v.as_string()),
            Some("ef3d7190-5839-4429-ad81-c82cf90e444a")
        );
        assert_eq!(
            profiles
                .get("com.example.app.ShareExtension")
                .and_then(|v| v.as_string()),
            Some("a1b2c3d4-1111-2222-3333-444455556666")
        );
    } else {
        panic!("provisioningProfiles should exist in plist");
    }
}

#[test]
fn test_export_options_missing_team_id() {
    let opts = ExportOptions::new(ExportMethod::AdHoc, String::new());

    let result = generate_export_options_plist(&opts, MethodNameStyle::Classic);
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("team_id is required"));
}

#[test]
fn test_export_options_invalid_export_method() {
    let result = ExportMethod::from_str("invalid_method");
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("unrecognized export method"));
}

#[test]
fn test_export_options_manual_without_certificate() {
    let opts = ExportOptions {
        method: ExportMethod::AdHoc,
        team_id: "TEAM123456".to_string(),
        signing_style: SigningStyle::Manual,
        signing_certificate: None,
        provisioning_profiles: {
            let mut m = BTreeMap::new();
            m.insert(
                "com.example.app".to_string(),
                "ef3d7190-5839-4429-ad81-c82cf90e444a".to_string(),
            );
            m
        },
        destination: Destination::Export,
        strip_swift_symbols: true,
        upload_symbols: false,
        compile_bitcode: false,
        manage_app_version_and_build_number: false,
    };

    let result = generate_export_options_plist(&opts, MethodNameStyle::Classic);
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("signing_certificate is required"));
}

#[test]
fn test_export_options_manual_without_provisioning_profile() {
    let opts = ExportOptions {
        method: ExportMethod::AdHoc,
        team_id: "TEAM123456".to_string(),
        signing_style: SigningStyle::Manual,
        signing_certificate: Some("Apple Distribution".to_string()),
        provisioning_profiles: BTreeMap::new(),
        destination: Destination::Export,
        strip_swift_symbols: true,
        upload_symbols: false,
        compile_bitcode: false,
        manage_app_version_and_build_number: false,
    };

    let result = generate_export_options_plist(&opts, MethodNameStyle::Classic);
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("provisioning_profiles must have at least one entry"));
}

#[test]
fn test_export_options_invalid_uuid_format() {
    let opts = ExportOptions {
        method: ExportMethod::AdHoc,
        team_id: "TEAM123456".to_string(),
        signing_style: SigningStyle::Manual,
        signing_certificate: Some("Apple Distribution".to_string()),
        provisioning_profiles: {
            let mut m = BTreeMap::new();
            m.insert("com.example.app".to_string(), "not-a-uuid".to_string());
            m
        },
        destination: Destination::Export,
        strip_swift_symbols: true,
        upload_symbols: false,
        compile_bitcode: false,
        manage_app_version_and_build_number: false,
    };

    let result = generate_export_options_plist(&opts, MethodNameStyle::Classic);
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("not a valid UUID"));
    assert!(err_msg.contains("use the profile UUID, not its name"));
}

#[test]
fn test_export_options_xcode16_method_names() {
    let mut opts = ExportOptions::new(ExportMethod::AdHoc, "TEAM123456".to_string());
    opts.signing_style = SigningStyle::Manual;
    opts.signing_certificate = Some("Apple Distribution".to_string());
    opts.provisioning_profiles.insert(
        "com.example.app".to_string(),
        "ef3d7190-5839-4429-ad81-c82cf90e444a".to_string(),
    );

    let plist = generate_export_options_plist(&opts, MethodNameStyle::Xcode16)
        .expect("should generate plist");
    assert!(plist.contains("<key>method</key><string>release-testing</string>"));
}

#[test]
fn test_export_options_development_method() {
    let mut opts = ExportOptions::new(ExportMethod::Development, "TEAM123456".to_string());
    opts.signing_style = SigningStyle::Manual;
    opts.signing_certificate = Some("Apple Development".to_string());
    opts.provisioning_profiles.insert(
        "com.example.app".to_string(),
        "ef3d7190-5839-4429-ad81-c82cf90e444a".to_string(),
    );

    let plist = generate_export_options_plist(&opts, MethodNameStyle::Classic)
        .expect("should generate plist");
    assert!(plist.contains("<key>method</key><string>development</string>"));
}

#[test]
fn test_export_options_development_method_no_team_required() {
    let mut opts = ExportOptions::new(ExportMethod::Development, String::new());
    opts.signing_style = SigningStyle::Manual;
    opts.signing_certificate = Some("Apple Development".to_string());
    opts.provisioning_profiles.insert(
        "com.example.app".to_string(),
        "ef3d7190-5839-4429-ad81-c82cf90e444a".to_string(),
    );

    // Development method should not require team_id
    let result = generate_export_options_plist(&opts, MethodNameStyle::Classic);
    assert!(result.is_ok());
}

#[test]
fn test_export_method_from_str_case_insensitive() {
    assert_eq!(
        ExportMethod::from_str("AppStore").unwrap(),
        ExportMethod::AppStore
    );
    assert_eq!(
        ExportMethod::from_str("ADHOC").unwrap(),
        ExportMethod::AdHoc
    );
    assert_eq!(
        ExportMethod::from_str("app-store").unwrap(),
        ExportMethod::AppStore
    );
    assert_eq!(
        ExportMethod::from_str("ad-hoc").unwrap(),
        ExportMethod::AdHoc
    );
}

#[test]
fn test_provisioning_profiles_map_deterministic_order() {
    let mut opts = ExportOptions::new(ExportMethod::AdHoc, "TEAM123456".to_string());
    opts.signing_style = SigningStyle::Manual;
    opts.signing_certificate = Some("Apple Distribution".to_string());

    // Insert in non-alphabetical order
    opts.provisioning_profiles.insert(
        "com.example.app.z".to_string(),
        "ef3d7190-5839-4429-ad81-c82cf90e444a".to_string(),
    );
    opts.provisioning_profiles.insert(
        "com.example.app.a".to_string(),
        "a1b2c3d4-1111-2222-3333-444455556666".to_string(),
    );
    opts.provisioning_profiles.insert(
        "com.example.app.m".to_string(),
        "b2c3d4e5-2222-3333-4444-555566667777".to_string(),
    );

    let plist1 = generate_export_options_plist(&opts, MethodNameStyle::Classic)
        .expect("should generate plist");

    // Generate again - should be identical (BTreeMap guarantees order)
    let plist2 = generate_export_options_plist(&opts, MethodNameStyle::Classic)
        .expect("should generate plist");

    assert_eq!(plist1, plist2);

    // Verify order in plist is alphabetical
    let lines: Vec<&str> = plist1.lines().collect();
    let com_app_a_idx = lines
        .iter()
        .position(|l| l.contains("com.example.app.a"))
        .expect("should find .a");
    let com_app_m_idx = lines
        .iter()
        .position(|l| l.contains("com.example.app.m"))
        .expect("should find .m");
    let com_app_z_idx = lines
        .iter()
        .position(|l| l.contains("com.example.app.z"))
        .expect("should find .z");

    assert!(com_app_a_idx < com_app_m_idx && com_app_m_idx < com_app_z_idx);
}

#[test]
fn test_export_options_xml_escaping() {
    let mut opts = ExportOptions::new(ExportMethod::AdHoc, "TEAM&123456".to_string());
    opts.signing_style = SigningStyle::Manual;
    opts.signing_certificate = Some("Apple <Distribution>".to_string());
    opts.provisioning_profiles.insert(
        "com.example.app".to_string(),
        "ef3d7190-5839-4429-ad81-c82cf90e444a".to_string(),
    );

    let plist = generate_export_options_plist(&opts, MethodNameStyle::Classic)
        .expect("should generate plist");
    assert!(plist.contains("TEAM&amp;123456"));
    assert!(plist.contains("Apple &lt;Distribution&gt;"));
}

#[test]
fn test_signing_style_as_str() {
    assert_eq!(SigningStyle::Manual.as_str(), "manual");
    assert_eq!(SigningStyle::Automatic.as_str(), "automatic");
}

#[test]
fn test_destination_as_str() {
    assert_eq!(Destination::Export.as_str(), "export");
    assert_eq!(Destination::Upload.as_str(), "upload");
}
