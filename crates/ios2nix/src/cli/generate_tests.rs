#[allow(unused_imports)]
use super::*;
use std::path::PathBuf;

fn fixture(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(rel)
}

#[test]
fn test_generate_pods_nix_inline() {
    let tmp = tempfile::NamedTempFile::new().unwrap();

    run(GenerateCommand {
        lockfile: Some(fixture("lockfiles/simple-2-pods.lock")),
        output: Some(tmp.path().to_path_buf()),
        format: "inline".to_string(),
    })
    .expect("generate should succeed");

    let written = std::fs::read_to_string(tmp.path()).unwrap();
    let expected = std::fs::read_to_string(fixture("nix-outputs/simple-2-pods-inline.nix"))
        .expect("fixture must exist");
    assert_eq!(
        written, expected,
        "generate output must match simple-2-pods-inline.nix"
    );
}

#[test]
fn test_generate_pods_nix_modular() {
    let tmp = tempfile::NamedTempFile::new().unwrap();

    run(GenerateCommand {
        lockfile: Some(fixture("lockfiles/simple-expected.lock")),
        output: Some(tmp.path().to_path_buf()),
        format: "modular".to_string(),
    })
    .expect("generate should succeed");

    let written = std::fs::read_to_string(tmp.path()).unwrap();
    assert!(
        written.contains("mkPod"),
        "modular format must contain mkPod function"
    );
    assert!(
        written.contains("Flutter"),
        "output must contain Flutter pod"
    );
    // The git pod bypasses mkPod and uses fetchgit, pulling it into the header.
    assert!(written.contains("{ lib, fetchurl, fetchgit }:"));
    assert!(written.contains("MBProgressHUD = fetchgit {"));
}

#[test]
fn test_generate_rejects_unknown_format() {
    let result = run(GenerateCommand {
        lockfile: Some(fixture("lockfiles/simple-2-pods.lock")),
        output: None,
        format: "bogus".to_string(),
    });
    assert!(result.is_err());
}
