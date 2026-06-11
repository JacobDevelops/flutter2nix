use super::*;

#[test]
fn test_export_missing_archive() {
    let tmpdir = tempfile::tempdir().expect("failed to create tempdir");

    let cmd = ExportCommand {
        archive_path: tmpdir.path().join("nonexistent.xcarchive"),
        export_opts_plist: tmpdir.path().join("ExportOptions.plist"),
        output_path: tmpdir.path().join("output"),
    };

    let result = run(cmd);
    assert!(result.is_err(), "should error on missing archive");
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("archive not found"));
}

#[test]
fn test_export_args() {
    let cmd = ExportCommand {
        archive_path: std::path::PathBuf::from("out.xcarchive"),
        export_opts_plist: std::path::PathBuf::from("ExportOptions.plist"),
        output_path: std::path::PathBuf::from("output"),
    };

    let args = xcodebuild_args(&cmd);
    assert!(args.contains(&"-exportArchive".to_string()));
    assert!(args.contains(&"-archivePath".to_string()));
    assert!(args.contains(&"-exportOptionsPlist".to_string()));
    assert!(args.contains(&"-exportPath".to_string()));
}
