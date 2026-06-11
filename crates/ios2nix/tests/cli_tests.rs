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

#[test]
fn test_cli_build_from_sidecar_hermetic() {
    let tmpdir = tempfile::TempDir::new().expect("failed to create tempdir");
    let sidecar = tmpdir.path().join(".ios2nix-xcode-output.json");

    let fixture = include_str!("fixtures/xcode-outputs/basic.json");
    std::fs::write(&sidecar, fixture).expect("failed to write sidecar");

    let cmd = ios2nix::cli::build::BuildCommand {
        project_dir: tmpdir.path().to_path_buf(),
        workspace: tmpdir.path().join("Runner.xcworkspace"),
        scheme: "Runner".to_string(),
        configuration: "Release".to_string(),
        derived_data: None,
    };

    let result = ios2nix::cli::build::run(cmd).expect("should build from sidecar");
    assert_eq!(result.version, "14.3");
}

#[cfg(target_os = "macos")]
#[test]
#[ignore = "invokes real xcodebuild (slow)"]
fn test_cli_archive_from_build() {
    let tmpdir = tempfile::TempDir::new().expect("failed to create tempdir");

    // Copy fixture project to tempdir
    let fixture_src = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/xcode-projects/native-app");
    let fixture_dst = tmpdir.path().join("native-app");

    fn copy_dir_all(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
        std::fs::create_dir_all(dst)?;
        for entry in std::fs::read_dir(src)? {
            let entry = entry?;
            let path = entry.path();
            let file_name = entry.file_name();
            if path.is_dir() {
                copy_dir_all(&path, &dst.join(&file_name))?;
            } else {
                std::fs::copy(&path, dst.join(&file_name))?;
            }
        }
        Ok(())
    }

    copy_dir_all(&fixture_src, &fixture_dst).expect("failed to copy fixture");

    let cmd = ios2nix::cli::archive::ArchiveCommand {
        workspace: fixture_dst.join("ExportTest.xcodeproj/project.xcworkspace"),
        scheme: "ExportTest".to_string(),
        configuration: "Release".to_string(),
        archive_path: fixture_dst.join("out.xcarchive"),
        signing: None,
    };

    let result = ios2nix::cli::archive::run(cmd);
    assert!(result.is_ok(), "should create archive: {:?}", result);
}

#[test]
#[ignore = "Plan 3: requires signing"]
fn test_cli_export_ipa_with_codesign() {
    todo!("Plan 3: export with signing — unimplemented")
}

#[test]
#[ignore = "Plan 3: requires signing"]
fn test_cli_full_e2e_lock_to_ipa() {
    todo!("Plan 3: full e2e from lock to signed ipa — unimplemented")
}
