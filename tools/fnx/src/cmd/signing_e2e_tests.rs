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

#[test]
fn test_load_signing_vars_none_when_env_file_absent() {
    // An empty repo root has no .ios2nix-signing.env → gate closed (no real
    // material), regardless of host OS.
    let tmpdir = tempfile::TempDir::new().expect("failed to create tempdir");
    let result = load_signing_vars(tmpdir.path()).expect("absent env file is not an error");
    assert!(result.is_none(), "absent env file => no real material");
}

#[cfg(target_os = "macos")]
#[test]
fn test_load_signing_vars_none_when_password_file_dangling() {
    // Env file present but its IOS2NIX_P12_PASSWORD_FILE points at a missing
    // file → treated as "no real material" (Ok(None)), not an error, so the
    // signing e2e can fall back to simulate.
    let tmpdir = tempfile::TempDir::new().expect("failed to create tempdir");
    std::fs::write(
        tmpdir.path().join(SIGNING_ENV_FILE),
        "IOS2NIX_P12_PATH=/nope/x.p12\n\
         IOS2NIX_P12_PASSWORD_FILE=/nope/x.password\n\
         IOS2NIX_PROFILE_PATH=/nope/x.mobileprovision\n\
         IOS2NIX_TEAM_ID=TEAM123456\n\
         IOS2NIX_SIGNING_IDENTITY=Apple Distribution: X (TEAM123456)\n",
    )
    .expect("failed to write env file");

    let result = load_signing_vars(tmpdir.path()).expect("dangling refs must not error");
    assert!(
        result.is_none(),
        "dangling password file => no real material"
    );
}

#[cfg(target_os = "macos")]
#[test]
fn test_load_signing_vars_none_when_referenced_paths_missing() {
    // Password supplied inline (no password file) but the p12/profile paths do
    // not exist → no real material.
    let tmpdir = tempfile::TempDir::new().expect("failed to create tempdir");
    std::fs::write(
        tmpdir.path().join(SIGNING_ENV_FILE),
        "IOS2NIX_P12_PATH=/nope/x.p12\n\
         IOS2NIX_P12_PASSWORD=secret\n\
         IOS2NIX_PROFILE_PATH=/nope/x.mobileprovision\n\
         IOS2NIX_TEAM_ID=TEAM123456\n\
         IOS2NIX_SIGNING_IDENTITY=Apple Distribution: X (TEAM123456)\n",
    )
    .expect("failed to write env file");

    let result = load_signing_vars(tmpdir.path()).expect("dangling refs must not error");
    assert!(result.is_none(), "missing p12/profile => no real material");
}

#[cfg(target_os = "macos")]
#[test]
fn test_load_signing_vars_some_when_real_paths_exist() {
    // All referenced paths exist (their *contents* are irrelevant to this
    // gate) → real material is reported, with the inline password preserved
    // and a throwaway keychain password synthesized.
    let tmpdir = tempfile::TempDir::new().expect("failed to create tempdir");
    let p12 = tmpdir.path().join("dist.p12");
    let profile = tmpdir.path().join("dist.mobileprovision");
    std::fs::write(&p12, "not-a-real-p12").expect("write p12");
    std::fs::write(&profile, "not-a-real-profile").expect("write profile");
    std::fs::write(
        tmpdir.path().join(SIGNING_ENV_FILE),
        format!(
            "IOS2NIX_P12_PATH={}\n\
             IOS2NIX_P12_PASSWORD=secret\n\
             IOS2NIX_PROFILE_PATH={}\n\
             IOS2NIX_TEAM_ID=TEAM123456\n\
             IOS2NIX_SIGNING_IDENTITY=Apple Distribution: X (TEAM123456)\n",
            p12.display(),
            profile.display()
        ),
    )
    .expect("failed to write env file");

    let vars = load_signing_vars(tmpdir.path())
        .expect("present paths must not error")
        .expect("present paths => real material");
    assert_eq!(vars.get("IOS2NIX_P12_PASSWORD").unwrap(), "secret");
    assert!(
        vars.contains_key("IOS2NIX_KEYCHAIN_PASSWORD"),
        "a throwaway keychain password is synthesized"
    );
}
