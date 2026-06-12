use flutter2nix::lockfile::{read_lockfile, FlutterLockfile};
use std::path::PathBuf;

/// Level-2 E2E: real TAPI shim via JVM, real dependency resolution over the network.
/// No sidecar, no local cache. Mirrors the gradle2nix `test_e2e_real_gradle_no_mocks` pattern.
/// Run with: cargo test -p flutter2nix -- --ignored test_e2e_real_android
///
/// Phase timings are printed to stderr to help identify bottlenecks:
///   [bench] tapi+resolve: Xs   ← Gradle daemon startup + SHA fetch
///   [bench] total:         Xs
#[tokio::test]
#[ignore = "requires java, Gradle distribution download, and network access to Maven Central"]
async fn test_e2e_real_android() {
    if std::process::Command::new("java")
        .arg("-version")
        .stderr(std::process::Stdio::null())
        .output()
        .is_err()
    {
        eprintln!("Skipping: java not available in PATH");
        return;
    }

    let tmp = tempfile::NamedTempFile::new().unwrap();
    let t_total = std::time::Instant::now();

    let t_lock = std::time::Instant::now();
    let result = flutter2nix::cli::lock::run(flutter2nix::cli::lock::LockCommand {
        project_dir: PathBuf::from("tests/fixtures/flutter-projects/real-android-flutter"),
        output: Some(tmp.path().to_path_buf()),
        repositories: Some(vec!["https://repo.maven.apache.org/maven2/".to_string()]),
        gradle_cache_dir: None,
        gradle_user_home: None,
        timeout_secs: 300,
    })
    .await;
    eprintln!("[bench] tapi+resolve: {:.2?}", t_lock.elapsed());
    eprintln!("[bench] total:        {:.2?}", t_total.elapsed());

    assert!(
        result.is_ok(),
        "flutter lock must succeed against real Gradle project: {:?}",
        result.unwrap_err()
    );

    let content = std::fs::read_to_string(tmp.path()).unwrap();
    let lock: FlutterLockfile = serde_json::from_str(&content).unwrap();

    let android = lock.android.expect("android section must be present");
    assert!(!android.nodes.is_empty(), "android nodes must not be empty");

    let has_junit = android
        .nodes
        .iter()
        .any(|n| n.name.starts_with("junit:junit:"));
    assert!(
        has_junit,
        "junit must appear in android nodes — got: {:?}",
        android.nodes
    );

    for node in &android.nodes {
        let sha = node.sha256_hex();
        assert_eq!(sha.len(), 64, "sha256 must be 64 hex chars, got: {sha}");
        assert!(
            sha.chars().all(|c| c.is_ascii_hexdigit()),
            "sha256 must be hex, got: {sha}"
        );
    }

    assert!(
        !content.contains("\"ios\""),
        "ios section must be absent, got:\n{content}"
    );
}

#[tokio::test]
async fn test_lock_simple_android_flutter() {
    let tmp = tempfile::NamedTempFile::new().unwrap();

    flutter2nix::cli::lock::run(flutter2nix::cli::lock::LockCommand {
        project_dir: PathBuf::from("tests/fixtures/flutter-projects/simple-android-flutter"),
        output: Some(tmp.path().to_path_buf()),
        repositories: Some(vec!["https://repo.maven.apache.org/maven2/".to_string()]),
        gradle_cache_dir: Some(PathBuf::from(
            "../gradle2nix/tests/fixtures/maven-repos/maven-central-stub",
        )),
        gradle_user_home: None,
        timeout_secs: 60,
    })
    .await
    .unwrap();

    let written: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(tmp.path()).unwrap()).unwrap();
    let expected: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(
            "tests/fixtures/flutter-projects/simple-android-flutter/flutter2nix.lock",
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        written, expected,
        "lock output must match fixture flutter2nix.lock"
    );
}

#[tokio::test]
async fn test_lock_android_section_present() {
    let tmp = tempfile::NamedTempFile::new().unwrap();

    flutter2nix::cli::lock::run(flutter2nix::cli::lock::LockCommand {
        project_dir: PathBuf::from("tests/fixtures/flutter-projects/simple-android-flutter"),
        output: Some(tmp.path().to_path_buf()),
        repositories: Some(vec!["https://repo.maven.apache.org/maven2/".to_string()]),
        gradle_cache_dir: Some(PathBuf::from(
            "../gradle2nix/tests/fixtures/maven-repos/maven-central-stub",
        )),
        gradle_user_home: None,
        timeout_secs: 60,
    })
    .await
    .unwrap();

    let lock: FlutterLockfile =
        read_lockfile(tmp.path()).expect("written lockfile must be readable");
    assert!(
        lock.android.is_some(),
        "android section must be present for Android project"
    );
    let android = lock.android.unwrap();
    assert!(!android.nodes.is_empty(), "android nodes must not be empty");
    assert!(
        android.nodes.iter().any(|n| n.name.contains("guava")),
        "guava must appear in android nodes"
    );
}

#[tokio::test]
async fn test_lock_ios_absent_for_android_only_project() {
    let tmp = tempfile::NamedTempFile::new().unwrap();

    flutter2nix::cli::lock::run(flutter2nix::cli::lock::LockCommand {
        project_dir: PathBuf::from("tests/fixtures/flutter-projects/simple-android-flutter"),
        output: Some(tmp.path().to_path_buf()),
        repositories: Some(vec!["https://repo.maven.apache.org/maven2/".to_string()]),
        gradle_cache_dir: Some(PathBuf::from(
            "../gradle2nix/tests/fixtures/maven-repos/maven-central-stub",
        )),
        gradle_user_home: None,
        timeout_secs: 60,
    })
    .await
    .unwrap();

    let json = std::fs::read_to_string(tmp.path()).unwrap();
    assert!(
        !json.contains("\"ios\""),
        "ios field must be absent for Android-only project, got:\n{json}"
    );
}

/// M4 canonical E2E test: full lock pipeline against the simple-android-flutter fixture.
#[tokio::test]
async fn test_flutter_lock_android_only() {
    let tmp = tempfile::TempDir::new().unwrap();
    let lockfile_path = tmp.path().join("flutter2nix.lock");

    flutter2nix::cli::lock::run(flutter2nix::cli::lock::LockCommand {
        project_dir: PathBuf::from("tests/fixtures/flutter-projects/simple-android-flutter"),
        output: Some(lockfile_path.clone()),
        repositories: Some(vec!["https://repo.maven.apache.org/maven2/".to_string()]),
        gradle_cache_dir: Some(PathBuf::from(
            "../gradle2nix/tests/fixtures/maven-repos/maven-central-stub",
        )),
        gradle_user_home: None,
        timeout_secs: 60,
    })
    .await
    .expect("flutter lock must succeed");

    let content = std::fs::read_to_string(&lockfile_path).unwrap();
    let lock: FlutterLockfile = serde_json::from_str(&content).unwrap();

    assert!(lock.android.is_some(), "android section must be present");
    assert!(
        !content.contains("\"ios\""),
        "ios section must be absent, got:\n{content}"
    );
    let nodes = &lock.android.unwrap().nodes;
    assert!(!nodes.is_empty(), "android nodes must be non-empty");
    assert!(
        nodes.iter().all(|n| !n.sha256_hex().is_empty()),
        "every node must have a sha256 hash"
    );
}

#[tokio::test]
async fn test_lock_fails_without_pubspec() {
    let tmp_dir = tempfile::TempDir::new().unwrap();
    // No pubspec.yaml — not a Flutter project
    let result = flutter2nix::cli::lock::run(flutter2nix::cli::lock::LockCommand {
        project_dir: tmp_dir.path().to_path_buf(),
        output: None,
        repositories: None,
        gradle_cache_dir: None,
        gradle_user_home: None,
        timeout_secs: 60,
    })
    .await;

    assert!(
        result.is_err(),
        "lock must fail when pubspec.yaml is absent"
    );
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("pubspec.yaml"),
        "error must mention pubspec.yaml, got: {msg}"
    );
}

#[tokio::test]
async fn test_check_fresh_lockfile() {
    let result = flutter2nix::cli::check::run(flutter2nix::cli::check::CheckCommand {
        project_dir: PathBuf::from("tests/fixtures/flutter-projects/simple-android-flutter"),
        lockfile: None,
        repositories: Some(vec!["https://repo.maven.apache.org/maven2/".to_string()]),
        gradle_cache_dir: Some(PathBuf::from(
            "../gradle2nix/tests/fixtures/maven-repos/maven-central-stub",
        )),
        gradle_user_home: None,
        timeout_secs: 60,
    })
    .await;
    assert!(
        result.is_ok(),
        "check must pass for a fresh lockfile: {:?}",
        result.unwrap_err()
    );
}

#[tokio::test]
async fn test_check_stale_lockfile() {
    // A lockfile with no android nodes cannot match the regenerated graph.
    let stale = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(stale.path(), r#"{ "android": { "nodes": [] } }"#).unwrap();

    let err = flutter2nix::cli::check::run(flutter2nix::cli::check::CheckCommand {
        project_dir: PathBuf::from("tests/fixtures/flutter-projects/simple-android-flutter"),
        lockfile: Some(stale.path().to_path_buf()),
        repositories: Some(vec!["https://repo.maven.apache.org/maven2/".to_string()]),
        gradle_cache_dir: Some(PathBuf::from(
            "../gradle2nix/tests/fixtures/maven-repos/maven-central-stub",
        )),
        gradle_user_home: None,
        timeout_secs: 60,
    })
    .await
    .expect_err("check must fail for a stale lockfile");
    assert!(
        err.to_string().contains("stale"),
        "error must say stale, got: {err}"
    );
}

/// Unified composition: a project with BOTH an android/ dir (TAPI sidecar) and
/// an ios/ dir (Podfile.lock + ios2nix sidecar) locks into `{ android, ios }`.
#[tokio::test]
async fn test_lock_android_and_ios_sections() {
    let project = tempfile::TempDir::new().unwrap();
    let fixture = PathBuf::from("tests/fixtures/flutter-projects/simple-android-flutter");

    std::fs::copy(
        fixture.join("pubspec.yaml"),
        project.path().join("pubspec.yaml"),
    )
    .unwrap();
    std::fs::create_dir(project.path().join("android")).unwrap();
    std::fs::copy(
        fixture.join("android/.gradle2nix-tapi-output.json"),
        project.path().join("android/.gradle2nix-tapi-output.json"),
    )
    .unwrap();
    std::fs::copy(
        fixture.join("android/build.gradle.kts"),
        project.path().join("android/build.gradle.kts"),
    )
    .unwrap();

    let ios2nix_fixtures = PathBuf::from("../ios2nix/tests/fixtures");
    std::fs::create_dir(project.path().join("ios")).unwrap();
    std::fs::copy(
        ios2nix_fixtures.join("podfile-locks/simple-2-pods.lock"),
        project.path().join("ios/Podfile.lock"),
    )
    .unwrap();
    std::fs::copy(
        ios2nix_fixtures.join("sidecars/simple.ios2nix-podspecs.json"),
        project.path().join("ios/.ios2nix-podspecs.json"),
    )
    .unwrap();

    let lock = flutter2nix::cli::lock::generate_lockfile(
        project.path(),
        &["https://repo.maven.apache.org/maven2/".to_string()],
        Some(std::path::Path::new(
            "../gradle2nix/tests/fixtures/maven-repos/maven-central-stub",
        )),
        None,
        60,
    )
    .await
    .unwrap();

    let android = lock
        .android
        .as_ref()
        .expect("android section must be present");
    assert!(!android.nodes.is_empty(), "android nodes must not be empty");

    let ios = lock.ios.as_ref().expect("ios section must be present");
    assert_eq!(
        ios.nodes.len(),
        3,
        "ios nodes: 2 http + 1 git, path pod excluded"
    );
    assert!(ios
        .nodes
        .iter()
        .all(|n| n.name != "path_provider_foundation"));

    let json = serde_json::to_string(&lock).unwrap();
    assert!(json.contains("\"android\""));
    assert!(json.contains("\"ios\""));
}
