#[allow(unused_imports)]
use super::*;
use nix_core::dep::{DependencyGraph, LockedDependency};

fn load_fixture(name: &str) -> DependencyGraph {
    let path = std::path::Path::new("tests/fixtures/lockfiles").join(name);
    read_lockfile(&path).unwrap_or_else(|e| panic!("failed to load fixture {name}: {e}"))
}

fn make_dep(name: &str, version: &str, sha256: &str) -> LockedDependency {
    LockedDependency::new(
        name.to_string(),
        version.to_string(),
        format!("https://repo.maven.apache.org/maven2/{name}/{version}/{name}-{version}.jar"),
        sha256.to_string(),
    )
}

#[test]
fn test_write_lockfile_simple() {
    let graph = load_fixture("simple-2-deps.lock");
    let tmp = tempfile::NamedTempFile::new().unwrap();
    write_lockfile(tmp.path(), &graph).unwrap();

    let written: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(tmp.path()).unwrap()).unwrap();
    let fixture: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string("tests/fixtures/lockfiles/simple-2-deps.lock").unwrap(),
    )
    .unwrap();
    assert_eq!(written, fixture);
}

#[test]
fn test_read_lockfile_simple() {
    let graph = load_fixture("simple-2-deps.lock");
    assert_eq!(graph.nodes.len(), 2);
    assert!(graph
        .nodes
        .iter()
        .any(|n| n.name == "com.google.guava:guava:31.1-jre"));
    assert!(graph.nodes.iter().any(|n| n.name == "junit:junit:4.13.2"));
}

#[test]
fn test_lockfile_roundtrip_write_read_equal() {
    let original = load_fixture("simple-2-deps.lock");
    let tmp = tempfile::NamedTempFile::new().unwrap();
    write_lockfile(tmp.path(), &original).unwrap();
    let roundtripped = read_lockfile(tmp.path()).unwrap();
    assert_eq!(original, roundtripped);
}

#[test]
fn test_read_lockfile_malformed_invalid_json() {
    let path = std::path::Path::new("tests/fixtures/lockfiles/malformed-invalid-json.lock");
    let err = read_lockfile(path).unwrap_err();
    assert!(
        err.to_string().contains("invalid JSON"),
        "expected 'invalid JSON' in error, got: {err}"
    );
}

#[test]
fn test_read_lockfile_missing_required_field() {
    let path = std::path::Path::new("tests/fixtures/lockfiles/malformed-missing-sha256.lock");
    let err = read_lockfile(path).unwrap_err();
    assert!(
        err.to_string().contains("missing field"),
        "expected 'missing field' in error, got: {err}"
    );
}

#[test]
fn test_diff_lockfiles_fresh() {
    let graph = load_fixture("simple-2-deps.lock");
    let diff = diff_lockfiles(&graph, &graph);
    assert!(
        diff.is_empty(),
        "diff of identical graphs must be empty, got: {diff}"
    );
}

#[test]
fn test_diff_lockfiles_stale_added_one_dep() {
    let old = load_fixture("simple-2-deps.lock");
    let new = load_fixture("complex-20-deps.lock");
    let diff = diff_lockfiles(&old, &new);
    assert_eq!(
        diff.added.len(),
        18,
        "expected 18 added deps, got {}: {diff}",
        diff.added.len()
    );
    assert_eq!(diff.removed.len(), 0);
    assert_eq!(diff.modified.len(), 0);
}

#[test]
fn test_diff_lockfiles_stale_sha256_changed() {
    let old = load_fixture("simple-2-deps.lock");
    let new = load_fixture("simple-2-deps-stale.lock");
    let diff = diff_lockfiles(&old, &new);
    assert_eq!(
        diff.modified.len(),
        1,
        "expected 1 modified dep, got: {diff}"
    );
    assert_eq!(diff.added.len(), 0);
    assert_eq!(diff.removed.len(), 0);
    let (old_dep, _new_dep) = &diff.modified[0];
    assert_eq!(old_dep.name, "com.google.guava:guava:31.1-jre");
}

#[test]
fn test_diff_lockfiles_classifier_identity_distinct() {
    let sha = "c4b87ce48bf565fce5dfc5db0ce5f13f69e58e6b0f7b960e42ab2e42e7cef83c";
    let sha_src = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    let base_dep = make_dep("com.google.guava:guava:31.1-jre", "31.1-jre", sha);
    let sources_dep = make_dep(
        "com.google.guava:guava:31.1-jre:sources",
        "31.1-jre",
        sha_src,
    );

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
            make_dep("group:removed-dep:1.0", "1.0", sha_a),
            make_dep("group:modified-dep:1.0", "1.0", sha_b),
        ],
    };
    let new = DependencyGraph {
        format_version: "1".to_string(),
        nodes: vec![
            make_dep("group:added-dep:2.0", "2.0", sha_a),
            make_dep("group:modified-dep:1.0", "1.0", sha_c),
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
