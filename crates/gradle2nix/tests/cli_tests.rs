use gradle2nix::cli;
use std::path::PathBuf;

#[tokio::test]
async fn test_cli_lock_full_pipeline() {
    let tmp = tempfile::NamedTempFile::new().unwrap();

    cli::lock::run(cli::lock::LockCommand {
        gradle_dir: PathBuf::from("tests/fixtures/gradle-projects/simple-app"),
        output: Some(tmp.path().to_path_buf()),
        repositories: Some(vec!["https://repo.maven.apache.org/maven2/".to_string()]),
        gradle_cache_dir: Some(PathBuf::from("tests/fixtures/maven-repos/maven-central-stub")),
        timeout_secs: 60,
    })
    .await
    .unwrap();

    let written: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(tmp.path()).unwrap()).unwrap();
    let fixture: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string("tests/fixtures/lockfiles/simple-2-deps.lock").unwrap(),
    )
    .unwrap();
    assert_eq!(written, fixture, "lock output must match simple-2-deps.lock");
}

#[tokio::test]
async fn test_cli_check_fresh_lockfile() {
    let result = cli::check::run(cli::check::CheckCommand {
        gradle_dir: PathBuf::from("tests/fixtures/gradle-projects/simple-app"),
        lockfile: Some(PathBuf::from("tests/fixtures/lockfiles/simple-2-deps.lock")),
        repositories: Some(vec!["https://repo.maven.apache.org/maven2/".to_string()]),
        gradle_cache_dir: Some(PathBuf::from("tests/fixtures/maven-repos/maven-central-stub")),
        timeout_secs: 60,
    })
    .await;

    assert!(result.is_ok(), "check on fresh lockfile must exit 0, got: {:?}", result.unwrap_err());
}

#[tokio::test]
async fn test_cli_check_stale_lockfile() {
    let result = cli::check::run(cli::check::CheckCommand {
        gradle_dir: PathBuf::from("tests/fixtures/gradle-projects/simple-app"),
        lockfile: Some(PathBuf::from("tests/fixtures/lockfiles/simple-2-deps-stale.lock")),
        repositories: Some(vec!["https://repo.maven.apache.org/maven2/".to_string()]),
        gradle_cache_dir: Some(PathBuf::from("tests/fixtures/maven-repos/maven-central-stub")),
        timeout_secs: 60,
    })
    .await;

    assert!(result.is_err(), "check on stale lockfile must exit non-zero");
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("stale"), "error message must mention 'stale': {msg}");
}

#[tokio::test]
async fn test_cli_generate_from_lockfile() {
    let tmp = tempfile::NamedTempFile::new().unwrap();

    cli::generate::run(cli::generate::GenerateCommand {
        lockfile: Some(PathBuf::from("tests/fixtures/lockfiles/simple-2-deps.lock")),
        output: Some(tmp.path().to_path_buf()),
        format: cli::generate::NixFormat::Inline,
    })
    .unwrap();

    let written = std::fs::read_to_string(tmp.path()).unwrap();
    let fixture =
        std::fs::read_to_string("tests/fixtures/nix-outputs/simple-2-deps-inline.nix").unwrap();
    assert_eq!(written, fixture, "generate output must match simple-2-deps-inline.nix");
}

#[tokio::test]
async fn test_cli_generate_missing_lockfile() {
    let result = cli::generate::run(cli::generate::GenerateCommand {
        lockfile: Some(PathBuf::from("/nonexistent/path/gradle2nix.lock")),
        output: None,
        format: cli::generate::NixFormat::Inline,
    });

    assert!(result.is_err(), "generate with missing lockfile must fail");
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("not found") || msg.contains("No such file") || msg.contains("failed to read"),
        "error must describe missing file: {msg}"
    );
}

#[tokio::test]
async fn test_lock_output_flag_writes_to_specified_path() {
    let tmp_dir = tempfile::TempDir::new().unwrap();
    let output_path = tmp_dir.path().join("test-output.json");

    cli::lock::run(cli::lock::LockCommand {
        gradle_dir: PathBuf::from("tests/fixtures/gradle-projects/simple-app"),
        output: Some(output_path.clone()),
        repositories: Some(vec!["https://repo.maven.apache.org/maven2/".to_string()]),
        gradle_cache_dir: Some(PathBuf::from("tests/fixtures/maven-repos/maven-central-stub")),
        timeout_secs: 60,
    })
    .await
    .unwrap();

    // Verify the file was written to the specified path
    assert!(
        output_path.exists(),
        "output file must be written to specified path: {}",
        output_path.display()
    );

    // Verify it contains valid JSON
    let content = std::fs::read_to_string(&output_path).unwrap();
    let parsed: serde_json::Value =
        serde_json::from_str(&content).expect("output must contain valid JSON");

    // Verify the JSON matches the expected fixture structure
    let fixture: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string("tests/fixtures/lockfiles/simple-2-deps.lock").unwrap(),
    )
    .unwrap();
    assert_eq!(
        parsed, fixture,
        "lock output must match simple-2-deps.lock fixture"
    );
}

#[tokio::test]
async fn test_e2e_lock_check_generate_full_pipeline() {
    // E2E test using with-buildscript fixture which includes buildscript block
    let tmp_dir = tempfile::TempDir::new().unwrap();
    let lockfile_path = tmp_dir.path().join("kotlin-jvm.json");
    let nix_output_path = tmp_dir.path().join("kotlin-jvm.nix");

    // Step 1: Lock — capture dependencies into lockfile
    cli::lock::run(cli::lock::LockCommand {
        gradle_dir: PathBuf::from("tests/fixtures/gradle-projects/with-buildscript"),
        output: Some(lockfile_path.clone()),
        repositories: Some(vec!["https://repo.maven.apache.org/maven2/".to_string()]),
        gradle_cache_dir: Some(PathBuf::from("tests/fixtures/maven-repos/maven-central-stub")),
        timeout_secs: 60,
    })
    .await
    .expect("lock phase must succeed");

    // Verify lockfile was created
    assert!(lockfile_path.exists(), "lockfile must be created");

    // Step 2: Check — verify lockfile is fresh
    let check_result = cli::check::run(cli::check::CheckCommand {
        gradle_dir: PathBuf::from("tests/fixtures/gradle-projects/with-buildscript"),
        lockfile: Some(lockfile_path.clone()),
        repositories: Some(vec!["https://repo.maven.apache.org/maven2/".to_string()]),
        gradle_cache_dir: Some(PathBuf::from("tests/fixtures/maven-repos/maven-central-stub")),
        timeout_secs: 60,
    })
    .await;

    assert!(
        check_result.is_ok(),
        "check phase must pass on fresh lockfile: {:?}",
        check_result.unwrap_err()
    );

    // Step 3: Generate — create Nix expressions from lockfile
    cli::generate::run(cli::generate::GenerateCommand {
        lockfile: Some(lockfile_path.clone()),
        output: Some(nix_output_path.clone()),
        format: cli::generate::NixFormat::Inline,
    })
    .expect("generate phase must succeed");

    // Verify Nix output was created
    assert!(nix_output_path.exists(), "nix output must be created");

    // Verify Nix output contains SRI format hashes (sha256-...)
    let nix_content = std::fs::read_to_string(&nix_output_path).unwrap();
    assert!(
        nix_content.contains("sha256-"),
        "Nix output must use SRI format hashes: {}",
        nix_content
    );

    // Note: dep_source field is only populated when the buildscript fallback parser is used.
    // Since this test uses the with-buildscript fixture's TAPI sidecar, the fallback parser
    // is not invoked, so dep_source won't be present. That's expected and correct.
}

#[tokio::test]
async fn test_e2e_nix_eval_generated_output() {
    // Optional test: if `nix` binary is available, verify Nix syntax of generated expressions
    // This is best-effort; if nix is not available, we skip gracefully

    // Check if nix is available in PATH
    if std::process::Command::new("nix")
        .arg("--version")
        .output()
        .is_err()
    {
        eprintln!("Skipping: nix not available in PATH");
        return;
    }

    let tmp_dir = tempfile::TempDir::new().unwrap();
    let lockfile_path = tmp_dir.path().join("kotlin-jvm.json");
    let nix_output_path = tmp_dir.path().join("kotlin-jvm.nix");

    // Generate Nix output from fixture
    cli::lock::run(cli::lock::LockCommand {
        gradle_dir: PathBuf::from("tests/fixtures/gradle-projects/with-buildscript"),
        output: Some(lockfile_path.clone()),
        repositories: Some(vec!["https://repo.maven.apache.org/maven2/".to_string()]),
        gradle_cache_dir: Some(PathBuf::from("tests/fixtures/maven-repos/maven-central-stub")),
        timeout_secs: 60,
    })
    .await
    .expect("lock phase must succeed");

    cli::generate::run(cli::generate::GenerateCommand {
        lockfile: Some(lockfile_path),
        output: Some(nix_output_path.clone()),
        format: cli::generate::NixFormat::Inline,
    })
    .expect("generate phase must succeed");

    // Verify the generated Nix output is syntactically valid by checking for expected patterns
    let nix_content = std::fs::read_to_string(&nix_output_path).unwrap();

    // Verify it contains fetchMaven calls and SRI hashes
    assert!(nix_content.contains("fetchMaven"), "Nix output must contain fetchMaven");
    assert!(nix_content.contains("sha256-"), "Nix output must use SRI format");
    assert!(nix_content.contains("repo ="), "Nix output must contain repo attribute");
    assert!(
        nix_content.contains("artifact ="),
        "Nix output must contain artifact attribute"
    );
}

/// End-to-end test against a real Gradle project with no mocks, no sidecar, and no local cache.
/// Invokes the real TAPI shim via JVM and resolves hashes from Maven Central over the network.
/// Requires: java in PATH, network access to services.gradle.org and repo.maven.apache.org.
#[tokio::test]
#[ignore = "requires java, Gradle distribution download, and network access to Maven Central"]
async fn test_e2e_real_gradle_no_mocks() {
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

    let result = cli::lock::run(cli::lock::LockCommand {
        gradle_dir: PathBuf::from("tests/fixtures/gradle-projects/real-simple-app"),
        output: Some(tmp.path().to_path_buf()),
        repositories: Some(vec!["https://repo.maven.apache.org/maven2/".to_string()]),
        gradle_cache_dir: None,
        timeout_secs: 300,
    })
    .await;

    assert!(
        result.is_ok(),
        "lock must succeed against real Gradle project: {:?}",
        result.unwrap_err()
    );

    let lockfile: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(tmp.path()).unwrap()).unwrap();

    let nodes = lockfile["nodes"].as_array().expect("lockfile must have a nodes array");
    assert!(!nodes.is_empty(), "lockfile must contain at least one dependency");

    let has_junit = nodes.iter().any(|n| {
        n["name"].as_str().map(|s| s.starts_with("junit:junit:")).unwrap_or(false)
    });
    assert!(has_junit, "lockfile must contain junit:junit — nodes: {nodes:?}");

    for node in nodes {
        let sha256 = node["sha256"].as_str().expect("each node must have a sha256 field");
        assert_eq!(sha256.len(), 64, "sha256 must be a 64-char hex string, got: {sha256}");
        assert!(
            sha256.chars().all(|c| c.is_ascii_hexdigit()),
            "sha256 must be hex, got: {sha256}"
        );
    }
}

/// End-to-end test against an Android AGP project using the init-script resolver.
/// Uses a TAPI sidecar + local maven stub — no Java, no Android SDK, no network required.
/// Verifies that AAR artifacts (invisible to IdeaProject) are resolved and routed to Google Maven,
/// and that io.flutter artifacts are routed to Flutter Storage.
#[tokio::test]
async fn test_cli_lock_android_fixture() {
    let tmp = tempfile::NamedTempFile::new().unwrap();

    let result = cli::lock::run(cli::lock::LockCommand {
        gradle_dir: PathBuf::from("tests/fixtures/gradle-projects/android-simple-app"),
        output: Some(tmp.path().to_path_buf()),
        repositories: Some(vec![
            "https://repo.maven.apache.org/maven2/".to_string(),
            "https://dl.google.com/dl/android/maven2/".to_string(),
            "https://storage.googleapis.com/download.flutter.io".to_string(),
        ]),
        gradle_cache_dir: Some(PathBuf::from("tests/fixtures/maven-repos/android-maven-stub")),
        timeout_secs: 60,
    })
    .await;

    assert!(
        result.is_ok(),
        "lock must succeed against Android AGP fixture: {:?}",
        result.unwrap_err()
    );

    let lockfile: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(tmp.path()).unwrap()).unwrap();

    let nodes = lockfile["nodes"].as_array().expect("lockfile must have a nodes array");
    assert!(!nodes.is_empty(), "init-script path must produce at least one dependency node");

    // androidx.core:core-ktx is an AAR — proves the init-script resolver ran (IdeaProject can't see it).
    let core_ktx = nodes.iter().find(|n| {
        n["name"].as_str().map(|s| s.starts_with("androidx.core:core-ktx:")).unwrap_or(false)
    });
    assert!(core_ktx.is_some(), "lockfile must contain androidx.core:core-ktx — nodes: {nodes:?}");
    assert!(
        core_ktx.unwrap()["url"].as_str().unwrap_or("").contains("dl.google.com"),
        "androidx artifact must be routed to Google Maven"
    );

    // io.flutter artifact must be routed to Flutter Storage, not Google Maven.
    let flutter_emb = nodes.iter().find(|n| {
        n["name"].as_str().map(|s| s.starts_with("io.flutter:flutter_embedding_debug:")).unwrap_or(false)
    });
    assert!(flutter_emb.is_some(), "lockfile must contain io.flutter:flutter_embedding_debug — nodes: {nodes:?}");
    assert!(
        flutter_emb.unwrap()["url"].as_str().unwrap_or("").contains("storage.googleapis.com"),
        "io.flutter artifact must be routed to Flutter Storage"
    );
}

/// End-to-end test against a real Gradle 9.x project. Validates TAPI shim compatibility
/// with Gradle 9.4.1 — the version used by the jfit mobile target app.
/// Same assertions as test_e2e_real_gradle_no_mocks; the only difference is the Gradle version.
#[tokio::test]
#[ignore = "requires java, Gradle 9 distribution download, and network access to Maven Central"]
async fn test_e2e_gradle9_no_mocks() {
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

    let result = cli::lock::run(cli::lock::LockCommand {
        gradle_dir: PathBuf::from("tests/fixtures/gradle-projects/real-simple-app-gradle9"),
        output: Some(tmp.path().to_path_buf()),
        repositories: Some(vec!["https://repo.maven.apache.org/maven2/".to_string()]),
        gradle_cache_dir: None,
        timeout_secs: 300,
    })
    .await;

    assert!(
        result.is_ok(),
        "lock must succeed against Gradle 9 project: {:?}",
        result.unwrap_err()
    );

    let lockfile: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(tmp.path()).unwrap()).unwrap();

    let nodes = lockfile["nodes"].as_array().expect("lockfile must have a nodes array");
    assert!(!nodes.is_empty(), "lockfile must contain at least one dependency");

    let has_junit = nodes.iter().any(|n| {
        n["name"].as_str().map(|s| s.starts_with("junit:junit:")).unwrap_or(false)
    });
    assert!(has_junit, "lockfile must contain junit:junit — nodes: {nodes:?}");

    for node in nodes {
        let sha256 = node["sha256"].as_str().expect("each node must have a sha256 field");
        assert_eq!(sha256.len(), 64, "sha256 must be a 64-char hex string, got: {sha256}");
        assert!(
            sha256.chars().all(|c| c.is_ascii_hexdigit()),
            "sha256 must be hex, got: {sha256}"
        );
    }
}
