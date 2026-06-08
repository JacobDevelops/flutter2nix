#[allow(unused_imports)]
use super::*;

#[test]
fn test_maven_coordinate_parse_basic() {
    let coord = MavenCoordinate::parse("com.google.guava:guava:31.1-jre").unwrap();
    assert_eq!(coord.group, "com.google.guava");
    assert_eq!(coord.artifact, "guava");
    assert_eq!(coord.version, "31.1-jre");
    assert_eq!(coord.classifier, None);
    assert_eq!(coord.extension, "jar");
}

#[test]
fn test_maven_coordinate_parse_with_classifier() {
    let coord = MavenCoordinate::parse("com.google.guava:guava:31.1-jre:sources").unwrap();
    assert_eq!(coord.classifier, Some("sources".to_string()));
    assert_eq!(coord.extension, "jar");
}

#[test]
fn test_maven_coordinate_parse_with_extension() {
    let coord = MavenCoordinate::parse("org.apache.maven:maven-core:3.9.0@aar").unwrap();
    assert_eq!(coord.extension, "aar");
    assert_eq!(coord.classifier, None);
}

#[test]
fn test_maven_coordinate_to_artifact_path() {
    let coord = MavenCoordinate::parse("com.google.guava:guava:31.1-jre").unwrap();
    assert_eq!(
        coord.to_artifact_path(),
        "com/google/guava/guava/31.1-jre/guava-31.1-jre.jar"
    );
}

#[test]
fn test_maven_coordinate_to_artifact_path_with_classifier() {
    let coord = MavenCoordinate::parse("com.google.guava:guava:31.1-jre:sources").unwrap();
    assert_eq!(
        coord.to_artifact_path(),
        "com/google/guava/guava/31.1-jre/guava-31.1-jre-sources.jar"
    );
}

#[test]
fn test_maven_coordinate_roundtrip_parse_to_path_to_parse() {
    let coord = MavenCoordinate::parse("com.google.guava:guava:31.1-jre").unwrap();
    let path1 = coord.to_artifact_path();
    let path2 = coord.to_artifact_path();
    assert_eq!(path1, path2, "to_artifact_path must be deterministic");
    assert!(!path1.is_empty());
}

#[test]
fn test_maven_coordinate_special_chars_in_group() {
    let coord = MavenCoordinate::parse("io.netty:netty-codec-http:4.1.100.Final").unwrap();
    assert_eq!(coord.group, "io.netty");
    assert_eq!(coord.artifact, "netty-codec-http");
    assert_eq!(coord.version, "4.1.100.Final");
    let path = coord.to_artifact_path();
    assert!(path.starts_with("io/netty/"), "group dots must become slashes, got: {path}");
}

#[test]
fn test_maven_coordinate_legacy_commons_lang() {
    let coord = MavenCoordinate::parse("commons-lang:commons-lang:2.6").unwrap();
    assert_eq!(coord.group, "commons-lang");
    assert_eq!(coord.artifact, "commons-lang");
    assert_eq!(
        coord.to_artifact_path(),
        "commons-lang/commons-lang/2.6/commons-lang-2.6.jar"
    );
}

#[tokio::test]
async fn test_resolve_artifact_sha256_from_local_cache() {
    let coord = MavenCoordinate::parse("com.google.guava:guava:31.1-jre").unwrap();
    let config = MavenResolverConfig {
        repository_urls: vec![],
        local_cache_dir: Some(PathBuf::from("tests/fixtures/maven-repos/maven-central-stub")),
        gradle_user_home: None,
        timeout_secs: 30,
        max_concurrency: 10,
    };
    let sha256 = resolve_artifact_sha256(&coord, &config).await.unwrap();
    assert_eq!(sha256, "c4b87ce48bf565fce5dfc5db0ce5f13f69e58e6b0f7b960e42ab2e42e7cef83c");
    assert_eq!(sha256.len(), 64);
    assert!(sha256.chars().all(|c| c.is_ascii_hexdigit()));
}

#[tokio::test]
async fn test_resolve_artifact_sha256_not_found_404() {
    let coord = MavenCoordinate::parse("com.example:nonexistent-artifact:1.0.0").unwrap();
    let config = MavenResolverConfig {
        repository_urls: vec![],
        local_cache_dir: Some(PathBuf::from("tests/fixtures/maven-repos/missing-artifact")),
        gradle_user_home: None,
        timeout_secs: 30,
        max_concurrency: 10,
    };
    let err = resolve_artifact_sha256(&coord, &config).await.unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("not found") || msg.contains("404"),
        "expected 'not found' or '404' in error, got: {msg}"
    );
}

#[tokio::test]
async fn test_resolve_artifact_sha256_invalid_format_not_hex() {
    let coord = MavenCoordinate::parse("com.google.guava:guava:31.1-jre").unwrap();
    let config = MavenResolverConfig {
        repository_urls: vec![],
        local_cache_dir: Some(PathBuf::from("tests/fixtures/maven-repos/corrupt-sha256")),
        gradle_user_home: None,
        timeout_secs: 30,
        max_concurrency: 10,
    };
    let err = resolve_artifact_sha256(&coord, &config).await.unwrap_err();
    assert!(
        err.to_string().contains("invalid hex"),
        "expected 'invalid hex' in error, got: {err}"
    );
}

#[tokio::test]
async fn test_resolve_artifacts_batch() {
    let coords = vec![
        MavenCoordinate::parse("com.google.guava:guava:31.1-jre").unwrap(),
        MavenCoordinate::parse("junit:junit:4.13.2").unwrap(),
    ];
    let config = MavenResolverConfig {
        repository_urls: vec![],
        local_cache_dir: Some(PathBuf::from("tests/fixtures/maven-repos/maven-central-stub")),
        gradle_user_home: None,
        timeout_secs: 30,
        max_concurrency: 10,
    };
    let results = resolve_artifacts_batch(&coords, &config).await.unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].0, coords[0]);
    assert_eq!(results[0].1, "c4b87ce48bf565fce5dfc5db0ce5f13f69e58e6b0f7b960e42ab2e42e7cef83c");
    assert_eq!(results[1].0, coords[1]);
    assert_eq!(results[1].1, "8e495b634469d64fb8acfa3495a065cdc1e19432e3508bfc5cc1e73eaebc19b0");
}

#[test]
fn test_error_messages_are_actionable() {
    // Parse error names the problematic input
    let err = MavenCoordinate::parse("only-one-part").unwrap_err();
    assert!(
        err.to_string().contains("only-one-part"),
        "parse error must quote the bad input so the user knows what to fix: {err}"
    );

    // Coordinate path embeds all identifying info (group/artifact/version visible in errors)
    let coord = MavenCoordinate::parse("com.example:my-lib:1.0").unwrap();
    let path = coord.to_artifact_path();
    assert!(path.contains("com/example"), "artifact path must contain group");
    assert!(path.contains("my-lib"), "artifact path must contain artifact id");
    assert!(path.contains("1.0"), "artifact path must contain version");
}

#[test]
fn test_maven_resolver_config_default() {
    let config = MavenResolverConfig::default();
    assert_eq!(
        config.max_concurrency, 10,
        "default max_concurrency should be 10 for parallel HTTP requests"
    );
    assert_eq!(config.timeout_secs, 60, "default timeout should be 60 seconds");
    assert!(
        !config.repository_urls.is_empty(),
        "default repositories should be configured"
    );
}

#[test]
fn test_maven_resolver_config_custom_concurrency() {
    let config = MavenResolverConfig {
        repository_urls: vec!["https://custom.repo".to_string()],
        local_cache_dir: None,
        gradle_user_home: None,
        timeout_secs: 30,
        max_concurrency: 20,
    };
    assert_eq!(config.max_concurrency, 20, "custom max_concurrency should be respected");
    assert_eq!(config.timeout_secs, 30, "custom timeout should be respected");
}

// ─── HTTP resolver tests (M2) ────────────────────────────────────────────────

#[tokio::test]
async fn test_resolve_artifact_sha256_http_200_ok() {
    let coord = MavenCoordinate::parse("com.example:http-200-lib:1.0").unwrap();
    let sha256_hex = "a".repeat(64);
    let _m = mockito::mock(
        "GET",
        format!("/{}.sha256", coord.to_artifact_path()).as_str(),
    )
    .with_status(200)
    .with_body(sha256_hex.as_str())
    .create();

    let config = MavenResolverConfig {
        repository_urls: vec![mockito::server_url()],
        local_cache_dir: None,
        gradle_user_home: None,
        timeout_secs: 10,
        max_concurrency: 10,
    };

    let result = resolve_artifact_sha256(&coord, &config).await.unwrap();
    assert_eq!(result, sha256_hex);
}

#[tokio::test]
async fn test_resolve_artifact_sha256_http_404_all_repos_fail() {
    let coord = MavenCoordinate::parse("com.example:http-404-lib:1.0").unwrap();
    let _m = mockito::mock(
        "GET",
        format!("/{}.sha256", coord.to_artifact_path()).as_str(),
    )
    .with_status(404)
    .create();

    let config = MavenResolverConfig {
        repository_urls: vec![mockito::server_url()],
        local_cache_dir: None,
        gradle_user_home: None,
        timeout_secs: 10,
        max_concurrency: 10,
    };

    let err = resolve_artifact_sha256(&coord, &config).await.unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("not found") || msg.contains("404"),
        "expected 'not found' or '404' in error, got: {msg}"
    );
}

#[tokio::test]
async fn test_resolve_artifact_sha256_http_invalid_response_not_hex() {
    let coord = MavenCoordinate::parse("com.example:invalid-hex-lib:1.0").unwrap();
    let _m = mockito::mock(
        "GET",
        format!("/{}.sha256", coord.to_artifact_path()).as_str(),
    )
    .with_status(200)
    .with_body("not-a-valid-sha256-hex-string!!")
    .create();

    let config = MavenResolverConfig {
        repository_urls: vec![mockito::server_url()],
        local_cache_dir: None,
        gradle_user_home: None,
        timeout_secs: 10,
        max_concurrency: 10,
    };

    let err = resolve_artifact_sha256(&coord, &config).await.unwrap_err();
    assert!(
        err.to_string().contains("invalid hex"),
        "expected 'invalid hex' in error, got: {err}"
    );
}

#[tokio::test]
async fn test_resolve_artifact_sha256_http_falls_back_to_second_repo() {
    let coord = MavenCoordinate::parse("com.example:fallback-lib:1.0").unwrap();
    let sha256_hex = "b".repeat(64);
    let artifact_path = coord.to_artifact_path();
    let base = mockito::server_url();

    let _m1 = mockito::mock(
        "GET",
        format!("/repo-a/{}.sha256", artifact_path).as_str(),
    )
    .with_status(404)
    .create();
    let _m2 = mockito::mock(
        "GET",
        format!("/repo-b/{}.sha256", artifact_path).as_str(),
    )
    .with_status(200)
    .with_body(sha256_hex.as_str())
    .create();

    let config = MavenResolverConfig {
        repository_urls: vec![
            format!("{}/repo-a", base),
            format!("{}/repo-b", base),
        ],
        local_cache_dir: None,
        gradle_user_home: None,
        timeout_secs: 10,
        max_concurrency: 10,
    };

    let result = resolve_artifact_sha256(&coord, &config).await.unwrap();
    assert_eq!(result, sha256_hex, "should have resolved from second repo after first returned 404");
}

#[tokio::test]
async fn test_resolve_artifact_sha256_http_timeout() {
    use std::net::TcpListener as StdTcpListener;

    // Bind a TCP server that accepts the connection but never sends an HTTP response,
    // forcing tokio::time::timeout to fire after timeout_secs.
    let listener = StdTcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();

    std::thread::spawn(move || {
        if let Ok((_stream, _)) = listener.accept() {
            // Hold the connection open without responding so the HTTP client blocks.
            std::thread::sleep(std::time::Duration::from_secs(2));
        }
    });

    let coord = MavenCoordinate::parse("com.example:timeout-lib:1.0").unwrap();
    let config = MavenResolverConfig {
        repository_urls: vec![format!("http://{}", addr)],
        local_cache_dir: None,
        gradle_user_home: None,
        timeout_secs: 1,
        max_concurrency: 10,
    };

    let err = resolve_artifact_sha256(&coord, &config).await.unwrap_err();
    assert!(
        err.to_string().contains("timeout"),
        "expected 'timeout' in error, got: {err}"
    );
}

#[tokio::test]
async fn test_resolve_artifacts_batch_parallel_concurrent() {
    let coords: Vec<MavenCoordinate> = (0..5)
        .map(|i| {
            MavenCoordinate::parse(&format!("com.example:concurrent-lib-{}:1.0", i)).unwrap()
        })
        .collect();
    let sha256_hex = "c".repeat(64);
    let base = mockito::server_url();

    let _mocks: Vec<_> = coords
        .iter()
        .map(|coord| {
            mockito::mock(
                "GET",
                format!("/{}.sha256", coord.to_artifact_path()).as_str(),
            )
            .with_status(200)
            .with_body(sha256_hex.as_str())
            .create()
        })
        .collect();

    let config = MavenResolverConfig {
        repository_urls: vec![base],
        local_cache_dir: None,
        gradle_user_home: None,
        timeout_secs: 10,
        max_concurrency: 5,
    };

    let results = resolve_artifacts_batch(&coords, &config).await.unwrap();
    assert_eq!(results.len(), 5, "all 5 concurrent artifacts must resolve");
    for (_, sha256) in &results {
        assert_eq!(sha256, &sha256_hex);
    }
}

#[tokio::test]
async fn test_resolve_artifacts_batch_parallel_fail_fast() {
    let good = MavenCoordinate::parse("com.example:fail-fast-good:1.0").unwrap();
    let bad = MavenCoordinate::parse("com.example:fail-fast-bad:1.0").unwrap();
    let sha256_hex = "d".repeat(64);
    let base = mockito::server_url();

    let _m1 = mockito::mock(
        "GET",
        format!("/{}.sha256", good.to_artifact_path()).as_str(),
    )
    .with_status(200)
    .with_body(sha256_hex.as_str())
    .create();
    let _m2 = mockito::mock(
        "GET",
        format!("/{}.sha256", bad.to_artifact_path()).as_str(),
    )
    .with_status(404)
    .create();

    let config = MavenResolverConfig {
        repository_urls: vec![base],
        local_cache_dir: None,
        gradle_user_home: None,
        timeout_secs: 10,
        max_concurrency: 10,
    };

    let result = resolve_artifacts_batch(&[good, bad], &config).await;
    assert!(result.is_err(), "batch must fail entirely when any artifact is missing");
    // Use {:#} to get the full error chain — the outer context is "batch resolve failed at '...'"
    // while the inner cause contains "not found"/"404" from resolve_artifact_sha256.
    let chain = format!("{:#}", result.unwrap_err());
    assert!(
        chain.contains("not found") || chain.contains("404"),
        "error chain must identify the missing artifact: {chain}"
    );
}

#[tokio::test]
async fn test_resolve_artifacts_batch_20_under_2s() {
    let coords: Vec<MavenCoordinate> = (0..20)
        .map(|i| MavenCoordinate::parse(&format!("com.example:perf-lib-{}:1.0", i)).unwrap())
        .collect();
    let sha256_hex = "e".repeat(64);
    let base = mockito::server_url();

    let _mocks: Vec<_> = coords
        .iter()
        .map(|coord| {
            mockito::mock(
                "GET",
                format!("/{}.sha256", coord.to_artifact_path()).as_str(),
            )
            .with_status(200)
            .with_body(sha256_hex.as_str())
            .create()
        })
        .collect();

    let config = MavenResolverConfig {
        repository_urls: vec![base],
        local_cache_dir: None,
        gradle_user_home: None,
        timeout_secs: 30,
        max_concurrency: 10,
    };

    let start = std::time::Instant::now();
    let results = resolve_artifacts_batch(&coords, &config).await.unwrap();
    let elapsed = start.elapsed();

    assert_eq!(results.len(), 20, "all 20 artifacts must resolve");
    assert!(
        elapsed.as_millis() < 2000,
        "20 artifacts took {}ms, expected <2000ms (parallel batch may be too slow)",
        elapsed.as_millis()
    );
    eprintln!("Performance gate: 20 artifacts resolved in {}ms", elapsed.as_millis());
}
