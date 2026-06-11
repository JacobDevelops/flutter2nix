#[tokio::test]
async fn test_cli_lock_full_pipeline() {
    use std::path::PathBuf;

    // A tempdir plays the ios/ project dir; the committed sidecar fixture
    // short-circuits resolution so the pipeline runs hermetically.
    let ios_dir = tempfile::TempDir::new().unwrap();
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    std::fs::copy(
        fixtures.join("sidecars/simple.ios2nix-podspecs.json"),
        ios_dir.path().join(".ios2nix-podspecs.json"),
    )
    .unwrap();

    let output = ios_dir.path().join("ios2nix.lock");
    ios2nix::cli::lock::run(ios2nix::cli::lock::LockCommand {
        ios_dir: ios_dir.path().to_path_buf(),
        output: Some(output.clone()),
        spec_repos: None,
        cache_dir: None,
        timeout_secs: 60,
    })
    .await
    .unwrap();

    let written = std::fs::read_to_string(&output).unwrap();
    let expected =
        std::fs::read_to_string(fixtures.join("lockfiles/simple-expected.lock")).unwrap();
    assert_eq!(
        written, expected,
        "lock output must match simple-expected.lock fixture byte-for-byte"
    );
}

#[tokio::test]
#[ignore = "TODO: ios2nix not yet implemented"]
async fn test_cli_build_from_podfile() {
    todo!("Phase 1: stub — project_dir: fixtures/xcode-projects/simple-app, podfile: fixtures/podfile-locks/simple-2-pods.lock, expect: xcodebuild invoked via sidecar, archive created, exit 0")
}

#[tokio::test]
#[ignore = "TODO: ios2nix not yet implemented"]
async fn test_cli_archive_from_build() {
    todo!("Phase 1: stub — build_dir: /tmp/ios2nix-test-build, output: /tmp/ios2nix-test.xcarchive, expect: archive created, structure verified, exit 0")
}

#[tokio::test]
#[ignore = "TODO: ios2nix not yet implemented"]
async fn test_cli_export_ipa_with_codesign() {
    todo!("Phase 1: stub — archive: /tmp/ios2nix-test.xcarchive, export_opts: ExportOptions.plist (adhoc, team TEAM123), keychain: temp keychain with cert fixture, output: /tmp/ios2nix-test.ipa, expect: ipa created and signed, exit 0")
}

#[tokio::test]
#[ignore = "TODO: ios2nix not yet implemented"]
async fn test_cli_full_e2e_lock_to_ipa() {
    todo!("Phase 1: stub — project_dir: fixtures/xcode-projects/simple-app, sequence: lock -> build -> archive -> export -> sign, expect: all steps succeed, final .ipa exists, exit 0")
}
