#[allow(unused_imports)]
use super::*;

#[test]
#[ignore = "TODO: ios2nix not yet implemented"]
fn test_parse_podfile_lock_simple() {
    todo!("Phase 1: stub — input: fixtures/podfile-locks/simple-2-pods.lock, expect: Ok(PodfileLock {{ pods: 2 }})")
}

#[test]
#[ignore = "TODO: ios2nix not yet implemented"]
fn test_parse_podfile_lock_complex() {
    todo!("Phase 1: stub — input: fixtures/podfile-locks/complex-20-pods.lock, expect: Ok(PodfileLock {{ pods: 20 }})")
}

#[test]
#[ignore = "TODO: ios2nix not yet implemented"]
fn test_parse_podfile_lock_invalid_yaml() {
    todo!("Phase 1: stub — input: fixtures/podfile-locks/malformed-invalid.lock, expect: Err(invalid YAML)")
}

#[test]
#[ignore = "TODO: ios2nix not yet implemented"]
fn test_parse_podfile_lock_missing_sha256() {
    todo!("Phase 1: stub — input: fixtures/podfile-locks/malformed-missing-sha256.lock, expect: Err(missing sha256)")
}

#[test]
#[ignore = "TODO: ios2nix not yet implemented"]
fn test_resolve_pod_url_valid() {
    todo!("Phase 1: stub — input: fixtures/cocoapods-specs/flutter.json, expect: Ok(url string)")
}

#[test]
#[ignore = "TODO: ios2nix not yet implemented"]
fn test_resolve_pod_url_missing_spec() {
    todo!("Phase 1: stub — input: unknown pod name, expect: Err(spec not found)")
}

#[test]
#[ignore = "TODO: ios2nix not yet implemented"]
fn test_resolve_pod_sha256_valid() {
    todo!("Phase 1: stub — input: fixtures/podfile-locks/simple-2-pods.lock, expect: Ok(sha256 hex string)")
}

#[test]
#[ignore = "TODO: ios2nix not yet implemented"]
fn test_resolve_pod_sha256_mismatch() {
    todo!("Phase 1: stub — input: fixtures/podfile-locks/simple-2-pods-stale.lock, expect: Err(hash mismatch)")
}
