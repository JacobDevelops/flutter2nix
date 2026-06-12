#[allow(unused_imports)]
use super::*;

#[test]
fn test_lock_parse_podfile() {
    let podfile_lock_content = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/podfile-locks/simple-2-pods.lock"
    ))
    .expect("fixture must exist");
    let podfile_lock = crate::cocoapods::parse_podfile_lock(&podfile_lock_content)
        .expect("should parse simple-2-pods.lock");

    assert_eq!(podfile_lock.pods.len(), 2, "should have 2 pods");
    assert_eq!(podfile_lock.pods[0].name, "Flutter");
    assert_eq!(podfile_lock.pods[0].version, "1.0.0");
    assert_eq!(podfile_lock.pods[1].name, "firebase_core");
    assert_eq!(podfile_lock.pods[1].version, "10.0.0");
}

#[tokio::test]
async fn test_lock_sidecar_excludes_path_pods() {
    let dir = tempfile::TempDir::new().unwrap();
    let sidecar = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sidecars/simple.ios2nix-podspecs.json");
    std::fs::copy(sidecar, dir.path().join(".ios2nix-podspecs.json")).unwrap();

    let graph = build_dependency_graph(dir.path(), &[], None, 60)
        .await
        .expect("sidecar-driven lock must succeed");

    assert_eq!(graph.nodes.len(), 3, "path pod must be excluded");
    assert!(graph
        .nodes
        .iter()
        .all(|n| n.name != "path_provider_foundation"));
    let git_pod = graph
        .nodes
        .iter()
        .find(|n| n.name == "MBProgressHUD")
        .expect("git pod present");
    assert_eq!(git_pod.dep_source.as_deref(), Some("pod-git"));
    assert!(git_pod.url.starts_with("git+") && git_pod.url.contains('#'));
}
