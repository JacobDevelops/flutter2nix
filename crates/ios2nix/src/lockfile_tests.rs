#[allow(unused_imports)]
use super::*;
use nix_core::dep::{DependencyGraph, LockedDependency};
use tempfile::TempDir;

#[test]
fn test_lockfile_roundtrip() {
    let temp_dir = TempDir::new().unwrap();
    let lockfile_path = temp_dir.path().join("test.lock");

    let original_graph = DependencyGraph {
        format_version: "1".to_string(),
        nodes: vec![
            LockedDependency::new(
                "firebase_core".to_string(),
                "10.0.0".to_string(),
                "https://github.com/firebase/firebase-ios-sdk/releases/download/10.0.0/firebase_core.zip".to_string(),
                "cafebabe1234567890abcdefdeadbeefcafebabe1234567890abcdef12345678".to_string(),
            ),
            {
                let mut dep = LockedDependency::new(
                    "MBProgressHUD".to_string(),
                    "1.2.0".to_string(),
                    "git+https://github.com/jdg/MBProgressHUD.git#1.2.0".to_string(),
                    "0000111122223333444455556666777788889999aaaabbbbccccddddeeeeeeef".to_string(),
                );
                dep.dep_source = Some("pod-git".to_string());
                dep
            },
        ],
    };

    // Write lockfile
    write_lockfile(&lockfile_path, &original_graph).unwrap();

    // Read it back
    let restored_graph = read_lockfile(&lockfile_path).unwrap();

    // Verify roundtrip equality
    assert_eq!(original_graph, restored_graph);
}

#[test]
fn test_lockfile_diff_added() {
    let old = DependencyGraph {
        format_version: "1".to_string(),
        nodes: vec![LockedDependency::new(
            "firebase_core".to_string(),
            "10.0.0".to_string(),
            "https://example.com/firebase_core.zip".to_string(),
            "abc123".to_string(),
        )],
    };

    let new = DependencyGraph {
        format_version: "1".to_string(),
        nodes: vec![
            LockedDependency::new(
                "firebase_core".to_string(),
                "10.0.0".to_string(),
                "https://example.com/firebase_core.zip".to_string(),
                "abc123".to_string(),
            ),
            LockedDependency::new(
                "firebase_auth".to_string(),
                "10.0.0".to_string(),
                "https://example.com/firebase_auth.zip".to_string(),
                "def456".to_string(),
            ),
        ],
    };

    let diff = diff_lockfiles(&old, &new);
    assert_eq!(diff.added.len(), 1);
    assert_eq!(diff.removed.len(), 0);
    assert_eq!(diff.modified.len(), 0);
    assert_eq!(diff.added[0].name, "firebase_auth");
}

#[test]
fn test_lockfile_diff_removed() {
    let old = DependencyGraph {
        format_version: "1".to_string(),
        nodes: vec![
            LockedDependency::new(
                "firebase_core".to_string(),
                "10.0.0".to_string(),
                "https://example.com/firebase_core.zip".to_string(),
                "abc123".to_string(),
            ),
            LockedDependency::new(
                "firebase_auth".to_string(),
                "10.0.0".to_string(),
                "https://example.com/firebase_auth.zip".to_string(),
                "def456".to_string(),
            ),
        ],
    };

    let new = DependencyGraph {
        format_version: "1".to_string(),
        nodes: vec![LockedDependency::new(
            "firebase_core".to_string(),
            "10.0.0".to_string(),
            "https://example.com/firebase_core.zip".to_string(),
            "abc123".to_string(),
        )],
    };

    let diff = diff_lockfiles(&old, &new);
    assert_eq!(diff.added.len(), 0);
    assert_eq!(diff.removed.len(), 1);
    assert_eq!(diff.modified.len(), 0);
    assert_eq!(diff.removed[0].name, "firebase_auth");
}

#[test]
fn test_lockfile_diff_modified() {
    let old = DependencyGraph {
        format_version: "1".to_string(),
        nodes: vec![LockedDependency::new(
            "firebase_core".to_string(),
            "10.0.0".to_string(),
            "https://example.com/firebase_core.zip".to_string(),
            "abc123".to_string(),
        )],
    };

    let new = DependencyGraph {
        format_version: "1".to_string(),
        nodes: vec![LockedDependency::new(
            "firebase_core".to_string(),
            "10.1.0".to_string(),
            "https://example.com/firebase_core-10.1.0.zip".to_string(),
            "newhashhexvalue1234567890abcdef1234567890abcdef1234567890ab".to_string(),
        )],
    };

    let diff = diff_lockfiles(&old, &new);
    assert_eq!(diff.added.len(), 0);
    assert_eq!(diff.removed.len(), 0);
    assert_eq!(diff.modified.len(), 1);
    assert_eq!(diff.modified[0].0.name, "firebase_core");
    assert_eq!(diff.modified[0].0.version, "10.0.0");
    assert_eq!(diff.modified[0].1.version, "10.1.0");
}

#[test]
fn test_lockfile_read_missing_file() {
    let temp_dir = TempDir::new().unwrap();
    let lockfile_path = temp_dir.path().join("nonexistent.lock");

    let result = read_lockfile(&lockfile_path);
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("failed to read"));
}

#[test]
fn test_lockfile_read_invalid_json() {
    let temp_dir = TempDir::new().unwrap();
    let lockfile_path = temp_dir.path().join("invalid.lock");
    std::fs::write(&lockfile_path, "{ invalid json }").unwrap();

    let result = read_lockfile(&lockfile_path);
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("invalid JSON") || err_msg.contains("JSON"));
}
