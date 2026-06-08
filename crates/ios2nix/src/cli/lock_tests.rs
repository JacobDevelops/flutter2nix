#[allow(unused_imports)]
use super::*;

#[test]
#[ignore = "TODO: ios2nix not yet implemented"]
fn test_lock_parse_podfile() {
    todo!("Phase 1: stub — input: fixtures/podfile-locks/simple-2-pods.lock, expect: Ok(2 pods extracted)")
}

#[test]
#[ignore = "TODO: ios2nix not yet implemented"]
fn test_lock_write_pods_nix() {
    todo!("Phase 1: stub — input: 2-pod lockfile, expect: pods.nix written matching fixtures/nix-outputs/simple-2-pods-inline.nix")
}
