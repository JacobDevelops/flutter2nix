use super::*;

#[test]
fn test_build_invoke_xcodebuild() {
    let tmpdir = tempfile::tempdir().expect("failed to create tempdir");
    let sidecar = tmpdir.path().join(".ios2nix-xcode-output.json");

    let fixture = include_str!("../../tests/fixtures/xcode-outputs/basic.json");
    std::fs::write(&sidecar, fixture).expect("failed to write sidecar");

    let cmd = BuildCommand {
        project_dir: tmpdir.path().to_path_buf(),
        workspace: tmpdir.path().join("Runner.xcworkspace"),
        scheme: "Runner".to_string(),
        configuration: "Release".to_string(),
        derived_data: None,
    };

    let result = run(cmd).expect("should parse sidecar");
    assert_eq!(result.version, "14.3");
}

#[test]
fn test_build_capture_output() {
    let tmpdir = tempfile::tempdir().expect("failed to create tempdir");
    let sidecar = tmpdir.path().join(".ios2nix-xcode-output.json");

    let fixture = include_str!("../../tests/fixtures/xcode-outputs/with-frameworks.json");
    std::fs::write(&sidecar, fixture).expect("failed to write sidecar");

    let cmd = BuildCommand {
        project_dir: tmpdir.path().to_path_buf(),
        workspace: tmpdir.path().join("Runner.xcworkspace"),
        scheme: "Runner".to_string(),
        configuration: "Release".to_string(),
        derived_data: None,
    };

    let result = run(cmd).expect("should parse sidecar");
    assert_eq!(result.frameworks.len(), 3);
}

#[test]
fn test_build_args_unsigned() {
    let cmd = BuildCommand {
        project_dir: std::path::PathBuf::from("."),
        workspace: std::path::PathBuf::from("./Runner.xcworkspace"),
        scheme: "Runner".to_string(),
        configuration: "Release".to_string(),
        derived_data: None,
    };

    let args = xcodebuild_args(&cmd);
    assert!(args.contains(&"build".to_string()));
    assert!(args.contains(&"CODE_SIGNING_ALLOWED=NO".to_string()));
    assert!(args.contains(&"generic/platform=iOS".to_string()));
}
