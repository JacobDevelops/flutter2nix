use super::*;

#[test]
fn roundtrip_positive_negative_and_pom_entries() {
    let dir = tempfile::tempdir().unwrap();
    let cache = ResolveCache::open(dir.path());

    assert_eq!(cache.lookup_sha256("https://repo/a.jar"), None);
    cache.store_sha256("https://repo/a.jar", Some("ab".repeat(32)));
    cache.store_sha256("https://repo/missing.jar", None);
    cache.store_pom("https://repo/a.pom", Some("<project/>".to_string()));

    cache.persist().unwrap();

    // Reopen from disk — entries survive, including the negative (404) marker.
    let cache = ResolveCache::open(dir.path());
    assert_eq!(
        cache.lookup_sha256("https://repo/a.jar"),
        Some(Some("ab".repeat(32)))
    );
    assert_eq!(cache.lookup_sha256("https://repo/missing.jar"), Some(None));
    assert_eq!(
        cache.lookup_pom("https://repo/a.pom"),
        Some(Some("<project/>".to_string()))
    );
    assert_eq!(cache.lookup_sha256("https://repo/never-seen.jar"), None);
}

#[test]
fn url_exists_roundtrips_through_persist() {
    let dir = tempfile::tempdir().unwrap();
    let cache = ResolveCache::open(dir.path());

    cache.store_url_exists("https://repo/a.jar", true);
    cache.store_url_exists("https://repo/missing.jar", false);
    cache.persist().unwrap();

    // Reopen from disk — warm lock runs must skip re-HEADing verified URLs.
    let cache = ResolveCache::open(dir.path());
    assert_eq!(cache.lookup_url_exists("https://repo/a.jar"), Some(true));
    assert_eq!(
        cache.lookup_url_exists("https://repo/missing.jar"),
        Some(false)
    );
    assert_eq!(cache.lookup_url_exists("https://repo/never-seen.jar"), None);
}

#[test]
fn corrupt_cache_file_is_discarded_not_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("caches/gradle2nix/resolve-cache.json");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, b"{not json").unwrap();

    let cache = ResolveCache::open(dir.path());
    assert_eq!(cache.lookup_sha256("anything"), None);
}

#[test]
fn file_sha256_invalidated_when_file_changes() {
    let dir = tempfile::tempdir().unwrap();
    let cache = ResolveCache::open(dir.path());
    let file = dir.path().join("artifact.jar");
    std::fs::write(&file, b"original bytes").unwrap();

    cache.store_file_sha256(&file, &"cd".repeat(32));
    assert_eq!(cache.lookup_file_sha256(&file), Some("cd".repeat(32)));

    // Change size → cached hash must not be returned.
    std::fs::write(&file, b"different content with another length").unwrap();
    assert_eq!(cache.lookup_file_sha256(&file), None);
}

#[test]
fn persist_skips_write_when_clean() {
    let dir = tempfile::tempdir().unwrap();
    let cache = ResolveCache::open(dir.path());
    cache.persist().unwrap();
    // Nothing stored, nothing dirty — no file should have been created.
    assert!(!dir
        .path()
        .join("caches/gradle2nix/resolve-cache.json")
        .exists());
}
