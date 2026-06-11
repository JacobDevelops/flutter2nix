use super::*;

#[test]
fn test_archive_verify_structure_valid() {
    let tmpdir = tempfile::tempdir().expect("failed to create tempdir");
    let archive = tmpdir.path().join("out.xcarchive");
    let app_dir = archive.join("Products/Applications/Foo.app");
    std::fs::create_dir_all(&app_dir).expect("failed to create app dir");
    std::fs::File::create(app_dir.join("Info.plist")).expect("failed to create Info.plist");

    let result = verify_archive_structure(&archive).expect("should verify structure");
    assert_eq!(result, app_dir);
}

#[test]
fn test_archive_verify_structure_missing_app() {
    let tmpdir = tempfile::tempdir().expect("failed to create tempdir");
    let archive = tmpdir.path().join("out.xcarchive");
    std::fs::create_dir_all(archive.join("Products/Applications")).expect("failed to create dir");

    let result = verify_archive_structure(&archive);
    assert!(result.is_err(), "should error on missing .app");
}

#[test]
fn test_archive_args_unsigned() {
    let cmd = ArchiveCommand {
        workspace: std::path::PathBuf::from("ios/Runner.xcworkspace"),
        scheme: "Runner".to_string(),
        configuration: "Release".to_string(),
        archive_path: std::path::PathBuf::from("out.xcarchive"),
        signing: None,
        bundle_id: None,
        derived_data: None,
    };

    let args = xcodebuild_args(&cmd);
    assert!(args.contains(&"archive".to_string()));
    assert!(args.contains(&"CODE_SIGNING_ALLOWED=NO".to_string()));
}

#[test]
fn test_archive_args_signed() {
    use crate::export_opts::SigningConfig;

    let cmd = ArchiveCommand {
        workspace: std::path::PathBuf::from("ios/Runner.xcworkspace"),
        scheme: "Runner".to_string(),
        configuration: "Release".to_string(),
        archive_path: std::path::PathBuf::from("out.xcarchive"),
        bundle_id: None,
        derived_data: None,
        signing: Some(SigningConfig {
            team_id: "TEAM123456".to_string(),
            identity: "Apple Distribution: Example Corp (TEAM123456)".to_string(),
            profile_specifier: "My Profile".to_string(),
            keychain: std::path::PathBuf::from("/path/to/keychain"),
        }),
    };

    let args = xcodebuild_args(&cmd);
    assert!(args.contains(&"archive".to_string()));
    assert!(args.contains(&"DEVELOPMENT_TEAM=TEAM123456".to_string()));
    assert!(args.contains(&"CODE_SIGN_STYLE=Manual".to_string()));
    assert!(args
        .iter()
        .any(|a| a.starts_with("CODE_SIGN_IDENTITY=Apple Distribution")));
    assert!(args
        .iter()
        .any(|a| a.starts_with("PROVISIONING_PROFILE_SPECIFIER=My Profile")));
    assert!(args
        .iter()
        .any(|a| a.starts_with("OTHER_CODE_SIGN_FLAGS=--keychain")));
    // Make sure we don't have CODE_SIGNING_ALLOWED=NO for signed
    assert!(!args.contains(&"CODE_SIGNING_ALLOWED=NO".to_string()));
}

#[cfg(target_os = "macos")]
#[test]
#[ignore = "invokes real xcodebuild (slow)"]
fn test_archive_create_xcarchive() {
    let tmpdir = tempfile::tempdir().expect("failed to create tempdir");

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

    let cmd = ArchiveCommand {
        workspace: fixture_dst.join("ExportTest.xcodeproj/project.xcworkspace"),
        scheme: "ExportTest".to_string(),
        configuration: "Release".to_string(),
        archive_path: fixture_dst.join("out.xcarchive"),
        signing: None,
        bundle_id: None,
        derived_data: None,
    };

    let result = run(cmd);
    assert!(result.is_ok(), "should create archive: {:?}", result);
}
