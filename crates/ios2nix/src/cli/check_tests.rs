#[allow(unused_imports)]
use super::*;
use std::path::{Path, PathBuf};

/// Tempdir acting as an ios/ project dir whose resolution is driven by the
/// committed sidecar fixture (no network).
fn sidecar_ios_dir() -> tempfile::TempDir {
    let dir = tempfile::TempDir::new().unwrap();
    let sidecar = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sidecars/simple.ios2nix-podspecs.json");
    std::fs::copy(sidecar, dir.path().join(".ios2nix-podspecs.json")).unwrap();
    dir
}

fn expected_lockfile() -> &'static Path {
    Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/lockfiles/simple-expected.lock"
    ))
}

#[tokio::test]
async fn test_check_fresh_lockfile() {
    let ios_dir = sidecar_ios_dir();

    let result = run(CheckCommand {
        ios_dir: ios_dir.path().to_path_buf(),
        lockfile: Some(expected_lockfile().to_path_buf()),
        spec_repos: None,
        cache_dir: None,
        timeout_secs: 60,
    })
    .await;

    assert!(
        result.is_ok(),
        "check on fresh lockfile must exit 0, got: {:?}",
        result.unwrap_err()
    );
}

#[tokio::test]
async fn test_check_stale_lockfile() {
    let ios_dir = sidecar_ios_dir();
    let stale_lockfile = ios_dir.path().join("stale.lock");

    let fresh_content = std::fs::read_to_string(expected_lockfile()).expect("fixture must exist");
    let stale_content = fresh_content.replace(
        "deadbeefcafebabe1234567890abcdef1234567890abcdef1234567890abcdef",
        "badbadbadbadbadbadbadbadbadbadbadbadbadbadbadbadbadbadbadbadbadb",
    );
    assert_ne!(
        fresh_content, stale_content,
        "corruption must change the lockfile"
    );
    std::fs::write(&stale_lockfile, &stale_content).expect("should write stale lockfile");

    let result = run(CheckCommand {
        ios_dir: ios_dir.path().to_path_buf(),
        lockfile: Some(stale_lockfile),
        spec_repos: None,
        cache_dir: None,
        timeout_secs: 60,
    })
    .await;

    assert!(
        result.is_err(),
        "check on stale lockfile must exit non-zero"
    );
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("stale"),
        "error message must mention 'stale': {msg}"
    );
}
