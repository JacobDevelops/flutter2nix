//! Tests for nix/relocate-plugins.py — copies Android plugin packages out of
//! the read-only Nix store into a writable directory and rewrites their paths
//! in .flutter-plugins-dependencies.
//!
//! Gradle 9+ refuses to configure an included project whose projectDirectory
//! is not writable ("Configuring project ':x' without an existing directory
//! is not allowed. The configured projectDirectory '...' does not exist,
//! can't be written to or is not a directory"). Flutter's Gradle plugin sets
//! each plugin's projectDir to <package>/android from the paths in
//! .flutter-plugins-dependencies, so store paths must be relocated before
//! `flutter build` drives Gradle. iOS entries stay on store paths — CocoaPods
//! only reads them.

use std::path::{Path, PathBuf};
use std::process::Command;

fn script_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../nix/relocate-plugins.py")
}

fn run_script(dir: &Path, deps: &str, dest: &str) -> std::process::Output {
    Command::new("python3")
        .arg(script_path())
        .arg(deps)
        .arg(dest)
        .current_dir(dir)
        .output()
        .expect("python3 must be available to run relocate-plugins.py")
}

/// Creates a fake read-only store package with android/ and ios/ subdirs.
fn make_store_pkg(root: &Path, name: &str) -> PathBuf {
    let pkg = root.join(format!("store/pub-{name}-1.0.0"));
    std::fs::create_dir_all(pkg.join("android/src")).unwrap();
    std::fs::create_dir_all(pkg.join("ios")).unwrap();
    std::fs::write(pkg.join("pubspec.yaml"), format!("name: {name}\n")).unwrap();
    std::fs::write(pkg.join("android/build.gradle"), "// gradle\n").unwrap();
    // Store paths are read-only; the copy must come out writable anyway.
    let mut perms = std::fs::metadata(pkg.join("android/build.gradle"))
        .unwrap()
        .permissions();
    use std::os::unix::fs::PermissionsExt;
    perms.set_mode(0o444);
    std::fs::set_permissions(pkg.join("android/build.gradle"), perms).unwrap();
    pkg
}

fn deps_json(android_path: &Path, ios_path: &Path) -> String {
    serde_json::json!({
        "plugins": {
            "ios": [
                {"name": "my_plugin", "path": format!("{}/", ios_path.display()),
                 "native_build": true, "dependencies": [], "dev_dependency": false}
            ],
            "android": [
                {"name": "my_plugin", "path": format!("{}/", android_path.display()),
                 "native_build": true, "dependencies": [], "dev_dependency": false}
            ],
            "macos": [], "linux": [], "windows": [], "web": []
        },
        "dependencyGraph": [{"name": "my_plugin", "dependencies": []}]
    })
    .to_string()
}

#[test]
fn rewrites_android_paths_to_writable_copies() {
    let dir = tempfile::tempdir().unwrap();
    let pkg = make_store_pkg(dir.path(), "my_plugin");
    let deps = dir.path().join(".flutter-plugins-dependencies");
    std::fs::write(&deps, deps_json(&pkg, &pkg)).unwrap();

    let out = run_script(dir.path(), ".flutter-plugins-dependencies", "plugin-copies");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let result: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&deps).unwrap()).unwrap();
    let android_path = result["plugins"]["android"][0]["path"].as_str().unwrap();
    assert!(
        !android_path.contains("store/pub-"),
        "android path must leave the store: {android_path}"
    );

    let copied = Path::new(android_path.trim_end_matches('/'));
    assert!(
        copied.join("android/build.gradle").is_file(),
        "android project must exist in the copy at {}",
        copied.display()
    );

    use std::os::unix::fs::PermissionsExt;
    let mode = std::fs::metadata(copied.join("android"))
        .unwrap()
        .permissions()
        .mode();
    assert!(
        mode & 0o200 != 0,
        "copied android dir must be user-writable, mode was {mode:o}"
    );
    let file_mode = std::fs::metadata(copied.join("android/build.gradle"))
        .unwrap()
        .permissions()
        .mode();
    assert!(
        file_mode & 0o200 != 0,
        "read-only store file modes must be lifted in the copy, mode was {file_mode:o}"
    );
}

#[test]
fn leaves_ios_paths_on_store() {
    let dir = tempfile::tempdir().unwrap();
    let pkg = make_store_pkg(dir.path(), "my_plugin");
    let deps = dir.path().join(".flutter-plugins-dependencies");
    std::fs::write(&deps, deps_json(&pkg, &pkg)).unwrap();

    let out = run_script(dir.path(), ".flutter-plugins-dependencies", "plugin-copies");
    assert!(out.status.success());

    let result: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&deps).unwrap()).unwrap();
    let ios_path = result["plugins"]["ios"][0]["path"].as_str().unwrap();
    assert_eq!(
        ios_path,
        format!("{}/", pkg.display()),
        "ios entries must keep their original (store) paths"
    );
}

#[test]
fn no_android_plugins_is_a_noop() {
    let dir = tempfile::tempdir().unwrap();
    let deps = dir.path().join(".flutter-plugins-dependencies");
    let content = serde_json::json!({
        "plugins": {"ios": [], "android": [], "macos": [], "linux": [],
                     "windows": [], "web": []},
        "dependencyGraph": []
    })
    .to_string();
    std::fs::write(&deps, &content).unwrap();

    let out = run_script(dir.path(), ".flutter-plugins-dependencies", "plugin-copies");
    assert!(out.status.success());
    assert!(
        !dir.path().join("plugin-copies").exists()
            || std::fs::read_dir(dir.path().join("plugin-copies"))
                .unwrap()
                .next()
                .is_none(),
        "no copies should be made when there are no android plugins"
    );
}

#[test]
fn missing_deps_file_exits_cleanly() {
    let dir = tempfile::tempdir().unwrap();
    let out = run_script(dir.path(), "nonexistent.json", "plugin-copies");
    assert!(out.status.success(), "missing deps file must be a no-op");
}
