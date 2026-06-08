#[tokio::test]
#[ignore = "TODO: ios2nix not yet implemented"]
async fn test_cli_lock_full_pipeline() {
    todo!("Phase 1: stub — project_dir: fixtures/xcode-projects/simple-app (with .ios2nix-xcode-output.json sidecar), output: pods.json, expect: file created matching podfile-locks/simple-2-pods.lock, exit 0")
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
