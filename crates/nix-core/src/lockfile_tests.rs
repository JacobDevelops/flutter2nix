#[allow(unused_imports)]
use super::*;
use tempfile::TempDir;

// Embedded fixture: simple-2-deps.lock from gradle2nix test fixtures
const SIMPLE_2_DEPS_JSON: &str = r#"{
  "version": "1",
  "nodes": [
    {
      "name": "com.google.guava:guava:31.1-jre",
      "version": "31.1-jre",
      "url": "https://repo.maven.apache.org/maven2/com/google/guava/guava/31.1-jre/guava-31.1-jre.jar",
      "sha256": "c4b87ce48bf565fce5dfc5db0ce5f13f69e58e6b0f7b960e42ab2e42e7cef83c"
    },
    {
      "name": "junit:junit:4.13.2",
      "version": "4.13.2",
      "url": "https://repo.maven.apache.org/maven2/junit/junit/4.13.2/junit-4.13.2.jar",
      "sha256": "8e495b634469d64fb8acfa3495a065cdc1e19432e3508bfc5cc1e73eaebc19b0"
    }
  ]
}"#;

// JSON missing sha256 field for error testing
const MALFORMED_MISSING_SHA256_JSON: &str = r#"{
  "version": "1",
  "nodes": [
    {
      "name": "com.google.guava:guava:31.1-jre",
      "version": "31.1-jre",
      "url": "https://repo.maven.apache.org/maven2/com/google/guava/guava/31.1-jre/guava-31.1-jre.jar"
    }
  ]
}"#;

// Basic helper to create a dependency
fn make_dep(name: &str, version: &str, url: &str, sha256: &str) -> LockedDependency {
    LockedDependency::new(
        name.to_string(),
        version.to_string(),
        url.to_string(),
        sha256.to_string(),
    )
}

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

    write_lockfile(&lockfile_path, &original_graph).unwrap();
    let restored_graph = read_lockfile(&lockfile_path).unwrap();
    assert_eq!(original_graph, restored_graph);
}

#[test]
fn test_read_lockfile_missing_file() {
    let temp_dir = TempDir::new().unwrap();
    let lockfile_path = temp_dir.path().join("nonexistent.lock");

    let result = read_lockfile(&lockfile_path);
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("failed to read"));
}

#[test]
fn test_read_lockfile_invalid_json() {
    let temp_dir = TempDir::new().unwrap();
    let lockfile_path = temp_dir.path().join("invalid.lock");
    std::fs::write(&lockfile_path, "{ invalid json }").unwrap();

    let result = read_lockfile(&lockfile_path);
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("invalid JSON"),
        "expected 'invalid JSON' in error, got: {err_msg}"
    );
}

#[test]
fn test_read_lockfile_missing_required_field() {
    let temp_dir = TempDir::new().unwrap();
    let lockfile_path = temp_dir.path().join("malformed.lock");
    std::fs::write(&lockfile_path, MALFORMED_MISSING_SHA256_JSON).unwrap();

    let result = read_lockfile(&lockfile_path);
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("missing field"),
        "expected 'missing field' in error, got: {err_msg}"
    );
}

#[test]
fn test_write_lockfile_format_stable() {
    let temp_dir = TempDir::new().unwrap();
    let lockfile_path = temp_dir.path().join("test.lock");

    // Write the embedded fixture JSON to a temp file, then read it and write it back
    std::fs::write(&lockfile_path, SIMPLE_2_DEPS_JSON).unwrap();
    let graph = read_lockfile(&lockfile_path).unwrap();

    // Write it back
    let output_path = temp_dir.path().join("output.lock");
    write_lockfile(&output_path, &graph).unwrap();

    // Parse both as JSON and compare structure (not byte-for-byte, as pretty-printing may differ)
    let original_json: serde_json::Value = serde_json::from_str(SIMPLE_2_DEPS_JSON).unwrap();
    let written_json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&output_path).unwrap()).unwrap();
    assert_eq!(written_json, original_json);
}

#[test]
fn test_lockfile_diff_added() {
    let old = DependencyGraph {
        format_version: "1".to_string(),
        nodes: vec![LockedDependency::new(
            "firebase_core".to_string(),
            "10.0.0".to_string(),
            "https://example.com/firebase_core.zip".to_string(),
            "cafebabe1234567890abcdefdeadbeefcafebabe1234567890abcdef12345678".to_string(),
        )],
    };

    let new = DependencyGraph {
        format_version: "1".to_string(),
        nodes: vec![
            LockedDependency::new(
                "firebase_core".to_string(),
                "10.0.0".to_string(),
                "https://example.com/firebase_core.zip".to_string(),
                "cafebabe1234567890abcdefdeadbeefcafebabe1234567890abcdef12345678".to_string(),
            ),
            LockedDependency::new(
                "firebase_auth".to_string(),
                "10.0.0".to_string(),
                "https://example.com/firebase_auth.zip".to_string(),
                "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef".to_string(),
            ),
        ],
    };

    let diff = diff_lockfiles(&old, &new);
    assert_eq!(diff.added.len(), 1);
    assert_eq!(diff.added[0].name, "firebase_auth");
    assert!(diff.removed.is_empty());
    assert!(diff.modified.is_empty());
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
                "cafebabe1234567890abcdefdeadbeefcafebabe1234567890abcdef12345678".to_string(),
            ),
            LockedDependency::new(
                "firebase_auth".to_string(),
                "10.0.0".to_string(),
                "https://example.com/firebase_auth.zip".to_string(),
                "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef".to_string(),
            ),
        ],
    };

    let new = DependencyGraph {
        format_version: "1".to_string(),
        nodes: vec![LockedDependency::new(
            "firebase_core".to_string(),
            "10.0.0".to_string(),
            "https://example.com/firebase_core.zip".to_string(),
            "cafebabe1234567890abcdefdeadbeefcafebabe1234567890abcdef12345678".to_string(),
        )],
    };

    let diff = diff_lockfiles(&old, &new);
    assert!(diff.added.is_empty());
    assert_eq!(diff.removed.len(), 1);
    assert_eq!(diff.removed[0].name, "firebase_auth");
    assert!(diff.modified.is_empty());
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
    assert!(diff.added.is_empty());
    assert!(diff.removed.is_empty());
    assert_eq!(diff.modified.len(), 1);
    assert_eq!(diff.modified[0].0.name, "firebase_core");
    assert_eq!(diff.modified[0].0.version, "10.0.0");
    assert_eq!(diff.modified[0].1.version, "10.1.0");
}

#[test]
fn test_lockfile_diff_empty() {
    let graph = DependencyGraph {
        format_version: "1".to_string(),
        nodes: vec![LockedDependency::new(
            "firebase_core".to_string(),
            "10.0.0".to_string(),
            "https://example.com/firebase_core.zip".to_string(),
            "cafebabe1234567890abcdefdeadbeefcafebabe1234567890abcdef12345678".to_string(),
        )],
    };

    let diff = diff_lockfiles(&graph, &graph);
    assert!(diff.is_empty());
}

#[test]
fn test_lockfile_diff_classifier_identity_distinct() {
    let sha = "c4b87ce48bf565fce5dfc5db0ce5f13f69e58e6b0f7b960e42ab2e42e7cef83c";
    let sha_src = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    let base_dep = make_dep(
        "com.google.guava:guava:31.1-jre",
        "31.1-jre",
        "https://repo.maven.apache.org/maven2/com/google/guava/guava/31.1-jre/guava-31.1-jre.jar",
        sha,
    );
    let sources_dep = make_dep("com.google.guava:guava:31.1-jre:sources", "31.1-jre",
        "https://repo.maven.apache.org/maven2/com/google/guava/guava/31.1-jre/guava-31.1-jre-sources.jar", sha_src);

    let old = DependencyGraph {
        format_version: "1".to_string(),
        nodes: vec![base_dep.clone()],
    };
    let new = DependencyGraph {
        format_version: "1".to_string(),
        nodes: vec![base_dep, sources_dep.clone()],
    };

    let diff = diff_lockfiles(&old, &new);
    assert_eq!(
        diff.added.len(),
        1,
        "expected 1 added (sources variant), got: {diff}"
    );
    assert_eq!(
        diff.added[0].name,
        "com.google.guava:guava:31.1-jre:sources"
    );
    assert_eq!(
        diff.modified.len(),
        0,
        "classifier variant must not appear as modified"
    );
    assert_eq!(diff.removed.len(), 0);
}

#[test]
fn test_diff_output_is_readable() {
    let sha_a = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let sha_b = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    let sha_c = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

    let old = DependencyGraph {
        format_version: "1".to_string(),
        nodes: vec![
            make_dep(
                "group:removed-dep:1.0",
                "1.0",
                "https://example.com/removed.jar",
                sha_a,
            ),
            make_dep(
                "group:modified-dep:1.0",
                "1.0",
                "https://example.com/modified.jar",
                sha_b,
            ),
        ],
    };
    let new = DependencyGraph {
        format_version: "1".to_string(),
        nodes: vec![
            make_dep(
                "group:added-dep:2.0",
                "2.0",
                "https://example.com/added.jar",
                sha_a,
            ),
            make_dep(
                "group:modified-dep:1.0",
                "1.0",
                "https://example.com/modified.jar",
                sha_c,
            ),
        ],
    };

    let diff = diff_lockfiles(&old, &new);
    let display = diff.to_string();

    assert!(
        display.contains('+'),
        "display must contain '+' for added deps, got:\n{display}"
    );
    assert!(
        display.contains('-'),
        "display must contain '-' for removed deps, got:\n{display}"
    );
    assert!(
        display.contains('~'),
        "display must contain '~' for modified deps, got:\n{display}"
    );
    assert!(
        display.contains("added-dep"),
        "display must name the added dep, got:\n{display}"
    );
    assert!(
        display.contains("removed-dep"),
        "display must name the removed dep, got:\n{display}"
    );
    assert!(
        display.contains("modified-dep"),
        "display must name the modified dep, got:\n{display}"
    );
}

#[test]
fn test_locked_dependency_source_field_serialization() {
    let sha = "c4b87ce48bf565fce5dfc5db0ce5f13f69e58e6b0f7b960e42ab2e42e7cef83c";
    let mut dep = LockedDependency::new(
        "com.android.tools.build:gradle".to_string(),
        "7.4.0".to_string(),
        "https://dl.google.com/dl/android/maven2/com/android/tools/build/gradle/7.4.0/gradle-7.4.0.jar".to_string(),
        sha.to_string(),
    );

    // Verify dep_source defaults to None and is not serialized
    let json_without_source = serde_json::to_string(&dep).unwrap();
    assert!(
        !json_without_source.contains("dep_source"),
        "dep_source should not appear in JSON when None"
    );

    // Add dep_source and verify it's serialized
    dep.dep_source = Some("buildscript-fallback".to_string());
    let json_with_source = serde_json::to_string(&dep).unwrap();
    assert!(
        json_with_source.contains("buildscript-fallback"),
        "dep_source should appear in JSON when Some"
    );

    // Roundtrip: serialize and deserialize
    let roundtripped: LockedDependency = serde_json::from_str(&json_with_source).unwrap();
    assert_eq!(
        roundtripped.dep_source,
        Some("buildscript-fallback".to_string()),
        "dep_source should roundtrip correctly"
    );
}
