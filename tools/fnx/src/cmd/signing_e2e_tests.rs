use super::*;

#[test]
fn test_parse_env_file_basic() {
    let tmpdir = tempfile::TempDir::new().expect("failed to create tempdir");
    let path = tmpdir.path().join("signing.env");
    std::fs::write(
        &path,
        "# comment\n\nIOS2NIX_TEAM_ID=TEAM123456\nIOS2NIX_P12_PATH=/some/path.p12\n",
    )
    .expect("failed to write env file");

    let vars = parse_env_file(&path).expect("should parse");
    assert_eq!(vars.get("IOS2NIX_TEAM_ID").unwrap(), "TEAM123456");
    assert_eq!(vars.get("IOS2NIX_P12_PATH").unwrap(), "/some/path.p12");
    assert_eq!(vars.len(), 2, "comments and blanks are not entries");
}

#[test]
fn test_parse_env_file_value_with_equals_and_spaces() {
    let tmpdir = tempfile::TempDir::new().expect("failed to create tempdir");
    let path = tmpdir.path().join("signing.env");
    std::fs::write(
        &path,
        "IOS2NIX_SIGNING_IDENTITY=Apple Distribution: Example Corp (TEAM123456)\nX=a=b\n",
    )
    .expect("failed to write env file");

    let vars = parse_env_file(&path).expect("should parse");
    assert_eq!(
        vars.get("IOS2NIX_SIGNING_IDENTITY").unwrap(),
        "Apple Distribution: Example Corp (TEAM123456)"
    );
    assert_eq!(vars.get("X").unwrap(), "a=b", "split on first '=' only");
}

#[test]
fn test_parse_env_file_rejects_malformed_line() {
    let tmpdir = tempfile::TempDir::new().expect("failed to create tempdir");
    let path = tmpdir.path().join("signing.env");
    std::fs::write(&path, "NOT A KEY VALUE LINE\n").expect("failed to write env file");

    let err = parse_env_file(&path).unwrap_err();
    assert!(err.to_string().contains("expected KEY=VALUE"));
}

#[test]
fn test_validate_required_reports_missing_keys() {
    let mut vars = BTreeMap::new();
    vars.insert("IOS2NIX_TEAM_ID".to_string(), "TEAM123456".to_string());

    let err = validate_required(&vars, Path::new(".ios2nix-signing.env")).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("IOS2NIX_P12_PATH"));
    assert!(msg.contains("IOS2NIX_SIGNING_IDENTITY"));
    assert!(!msg.contains("IOS2NIX_TEAM_ID,"), "present key not listed");
}

#[test]
fn test_validate_required_accepts_complete_set() {
    let mut vars = BTreeMap::new();
    for key in REQUIRED_KEYS {
        vars.insert(key.to_string(), "x".to_string());
    }
    validate_required(&vars, Path::new(".ios2nix-signing.env")).expect("complete set is valid");
}
