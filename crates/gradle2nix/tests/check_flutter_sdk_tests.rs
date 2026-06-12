//! Tests for nix/check-flutter-sdk.py — the buildPhase preflight that verifies
//! a Flutter SDK is usable for offline builds before `flutter build` runs.
//!
//! flutter_tools' PubDependencies artifact check requires
//! packages/flutter_tools/.dart_tool/package_config.json to exist and contain
//! at least one package whose root has a pubspec.yaml. An SDK packaged from
//! the raw Google tarball ships no such file, so flutter_tools runs an ONLINE
//! `pub get` for itself — even under --no-pub — which dies in the Nix sandbox
//! with an opaque "Got socket error trying to find package test" failure.
//! The preflight turns that into an immediate, actionable error.

use std::path::{Path, PathBuf};
use std::process::Command;

fn script_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../nix/check-flutter-sdk.py")
}

fn run_check(sdk: &Path) -> std::process::Output {
    Command::new("python3")
        .arg(script_path())
        .arg(sdk)
        .output()
        .expect("python3 must be available to run check-flutter-sdk.py")
}

/// Lays out a minimal SDK: packages/flutter_tools/pubspec.yaml always exists;
/// package_config content is injected by each test (None = file absent).
fn make_sdk(dir: &Path, package_config: Option<&str>) {
    let tools = dir.join("packages/flutter_tools");
    std::fs::create_dir_all(tools.join(".dart_tool")).unwrap();
    std::fs::write(tools.join("pubspec.yaml"), "name: flutter_tools\n").unwrap();
    if let Some(content) = package_config {
        std::fs::write(tools.join(".dart_tool/package_config.json"), content).unwrap();
    }
}

const VALID_CONFIG: &str = r#"{
  "configVersion": 2,
  "packages": [
    {
      "name": "flutter_tools",
      "rootUri": "../",
      "packageUri": "lib/",
      "languageVersion": "3.10"
    }
  ]
}"#;

#[test]
fn passes_for_sdk_with_resolved_tool_config() {
    let dir = tempfile::tempdir().unwrap();
    make_sdk(dir.path(), Some(VALID_CONFIG));

    let out = run_check(dir.path());
    assert!(
        out.status.success(),
        "valid SDK must pass preflight, stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn fails_for_sdk_missing_tool_config() {
    let dir = tempfile::tempdir().unwrap();
    make_sdk(dir.path(), None);

    let out = run_check(dir.path());
    assert!(
        !out.status.success(),
        "SDK without flutter_tools package_config.json must fail preflight"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("package_config.json"),
        "error must name the missing file, got: {stderr}"
    );
    assert!(
        stderr.contains("pub get"),
        "error must explain the consequence (online pub get), got: {stderr}"
    );
}

#[test]
fn fails_for_sdk_with_empty_package_list() {
    // PackageConfig.empty is explicitly rejected by flutter_tools' isUpToDate,
    // so an empty packages array still triggers the online pub get.
    let dir = tempfile::tempdir().unwrap();
    make_sdk(dir.path(), Some(r#"{"configVersion": 2, "packages": []}"#));

    let out = run_check(dir.path());
    assert!(
        !out.status.success(),
        "empty package list must fail preflight (flutter_tools rejects PackageConfig.empty)"
    );
}

#[test]
fn fails_when_listed_package_root_has_no_pubspec() {
    // isUpToDate verifies every listed package root contains a pubspec.yaml.
    let dir = tempfile::tempdir().unwrap();
    make_sdk(
        dir.path(),
        Some(
            r#"{
  "configVersion": 2,
  "packages": [
    {"name": "ghost", "rootUri": "../../ghost/", "packageUri": "lib/", "languageVersion": "3.10"}
  ]
}"#,
        ),
    );

    let out = run_check(dir.path());
    assert!(
        !out.status.success(),
        "package root without pubspec.yaml must fail preflight"
    );
}

#[test]
fn resolves_relative_and_absolute_root_uris() {
    // file:// absolute rootUri (what pub2nix-style configs emit) must resolve.
    let dir = tempfile::tempdir().unwrap();
    let dep_root = dir.path().join("deps/some_pkg");
    std::fs::create_dir_all(&dep_root).unwrap();
    std::fs::write(dep_root.join("pubspec.yaml"), "name: some_pkg\n").unwrap();
    let config = format!(
        r#"{{
  "configVersion": 2,
  "packages": [
    {{"name": "flutter_tools", "rootUri": "../", "packageUri": "lib/", "languageVersion": "3.10"}},
    {{"name": "some_pkg", "rootUri": "file://{}/", "packageUri": "lib/", "languageVersion": "3.0"}}
  ]
}}"#,
        dep_root.display()
    );
    make_sdk(dir.path(), Some(&config));

    let out = run_check(dir.path());
    assert!(
        out.status.success(),
        "absolute file:// rootUri must resolve, stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}
