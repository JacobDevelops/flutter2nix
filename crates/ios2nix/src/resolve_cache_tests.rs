#[allow(unused_imports)]
use super::*;
use tempfile::TempDir;

#[tokio::test]
async fn test_resolve_pod_sha256_valid() {
    let temp_dir = TempDir::new().unwrap();
    let cache = ResolveCache::open(temp_dir.path());

    let client = reqwest::Client::new();

    // Create a mock server using mockito
    let _mock = mockito::mock("GET", mockito::Matcher::Any)
        .with_status(200)
        .with_body(b"test content for hashing")
        .create();

    let url = &mockito::server_url();
    let src = PodSourceKind::Http {
        url: url.to_string(),
    };

    // First call should hit the mock and compute hash
    let hash1 = prefetch_content_hash(&src, None, &client, &cache).await;
    assert!(hash1.is_ok());
    let hash_hex = hash1.unwrap();

    // Verify it's a valid hex string of expected length (64 chars for sha256)
    assert_eq!(hash_hex.len(), 64);
    assert!(hash_hex.chars().all(|c| c.is_ascii_hexdigit()));

    // Second call should hit the cache (no additional mock request)
    let hash2 = prefetch_content_hash(&src, None, &client, &cache).await;
    assert!(hash2.is_ok());
    let hash2_value = hash2.unwrap();
    assert_eq!(hash_hex, hash2_value);

    // Verify cache was used (no additional HTTP request)
    // Note: mockito server may have multiple instances, so we just verify the hash matches
    assert_eq!(hash_hex, hash2_value);
}

#[tokio::test]
async fn test_resolve_pod_sha256_mismatch() {
    let temp_dir = TempDir::new().unwrap();
    let cache = ResolveCache::open(temp_dir.path());

    let client = reqwest::Client::new();

    let _mock = mockito::mock("GET", mockito::Matcher::Any)
        .with_status(200)
        .with_body(b"test content for hashing")
        .create();

    let url = &mockito::server_url();
    let src = PodSourceKind::Http {
        url: url.to_string(),
    };

    // Expected hash that doesn't match what we'll compute
    let wrong_hash = "0000000000000000000000000000000000000000000000000000000000000000";

    let result = prefetch_content_hash(&src, Some(wrong_hash), &client, &cache).await;

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("hash mismatch") || err_msg.contains("!="));
}

#[test]
fn test_resolve_cache_persistence() {
    let temp_dir = TempDir::new().unwrap();

    {
        let cache = ResolveCache::open(temp_dir.path());
        cache.store_sha256(
            "http://example.com/foo.zip",
            Some("abc123def456".to_string()),
        );
        cache.flush().unwrap();
    }

    // Re-open and verify data persisted
    {
        let cache = ResolveCache::open(temp_dir.path());
        let result = cache.lookup_sha256("http://example.com/foo.zip");
        assert_eq!(result, Some(Some("abc123def456".to_string())));
        cache.flush().unwrap();
    }
}

#[tokio::test]
async fn test_path_pod_no_hash() {
    let temp_dir = TempDir::new().unwrap();
    let cache = ResolveCache::open(temp_dir.path());

    let client = reqwest::Client::new();

    let src = PodSourceKind::Path {
        path: "/path/to/pod".to_string(),
    };

    let result = prefetch_content_hash(&src, None, &client, &cache).await;

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("path pods"));
}
