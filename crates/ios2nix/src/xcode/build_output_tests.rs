use super::*;

#[test]
fn test_parse_xcode_build_output_valid() {
    let json = r#"{
        "version": "14.3",
        "architectures": ["arm64", "x86_64"],
        "frameworks": [],
        "codesign_identity": null
    }"#;

    let output = parse_xcode_build_output(json).expect("should parse valid output");
    assert_eq!(output.version, "14.3");
    assert_eq!(output.architectures, vec!["arm64", "x86_64"]);
    assert!(output.frameworks.is_empty());
    assert!(output.codesign_identity.is_none());
}

#[test]
fn test_parse_xcode_build_output_with_frameworks() {
    let json = r#"{
        "version": "14.3",
        "architectures": ["arm64"],
        "frameworks": ["Flutter", "FirebaseCore", "GoogleUtilities"],
        "codesign_identity": null
    }"#;

    let output = parse_xcode_build_output(json).expect("should parse with frameworks");
    assert_eq!(output.frameworks.len(), 3);
    assert_eq!(output.frameworks[0], "Flutter");
}

#[test]
fn test_parse_xcode_build_output_with_code_signing() {
    let json = r#"{
        "version": "14.3",
        "architectures": ["arm64"],
        "frameworks": [],
        "codesign_identity": "Apple Distribution: Example Corp (TEAM123456)"
    }"#;

    let output = parse_xcode_build_output(json).expect("should parse with codesign_identity");
    assert_eq!(
        output.codesign_identity,
        Some("Apple Distribution: Example Corp (TEAM123456)".to_string())
    );
}

#[test]
fn test_parse_xcode_build_output_malformed_missing_field() {
    let json = r#"{
        "architectures": ["arm64"],
        "frameworks": [],
        "codesign_identity": null
    }"#;

    let result = parse_xcode_build_output(json);
    assert!(result.is_err(), "should error on missing 'version' field");
}

#[test]
fn test_parse_xcode_build_output_malformed_unknown_fields() {
    let json = r#"{
        "version": "14.3",
        "architectures": ["arm64"],
        "frameworks": [],
        "codesign_identity": null,
        "unknown_future_field": "should cause error",
        "another_unknown": 42
    }"#;

    let result = parse_xcode_build_output(json);
    assert!(
        result.is_err(),
        "should error on unknown fields due to deny_unknown_fields"
    );
}

#[test]
fn test_parse_xcode_build_output_version_mismatch() {
    let json = r#"{
        "version": "99.0",
        "architectures": ["arm64"],
        "frameworks": [],
        "codesign_identity": null
    }"#;

    let result = parse_xcode_build_output(json);
    assert!(result.is_err(), "should error on unsupported version");
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("unsupported schema version"),
        "error message should mention unsupported schema version: {}",
        err_msg
    );
}
