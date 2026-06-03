#[allow(unused_imports)]
use super::*;

#[test]
fn test_write_lockfile_simple() {
    todo!("Phase 1: stub — DependencyGraph with 2 nodes written to temp file, content matches fixtures/lockfiles/simple-2-deps.json")
}

#[test]
fn test_read_lockfile_simple() {
    todo!("Phase 1: stub — read fixtures/lockfiles/simple-2-deps.json, expect DependencyGraph with 2 nodes")
}

#[test]
fn test_lockfile_roundtrip_write_read_equal() {
    todo!("Phase 1: stub — deserialize fixture → write to temp → read back → assert exact equality")
}

#[test]
fn test_read_lockfile_malformed_invalid_json() {
    todo!("Phase 1: stub — read fixtures/lockfiles/malformed-invalid-json.json, expect Err containing 'invalid JSON'")
}

#[test]
fn test_read_lockfile_missing_required_field() {
    todo!("Phase 1: stub — read fixtures/lockfiles/malformed-missing-sha256.json, expect Err containing 'missing field'")
}

#[test]
fn test_diff_lockfiles_fresh() {
    todo!("Phase 1: stub — diff identical graphs, expect LockfileDiff{{added:[], removed:[], modified:[]}}")
}

#[test]
fn test_diff_lockfiles_stale_added_one_dep() {
    todo!("Phase 1: stub — old=simple-2-deps, new=complex-20-deps, expect added has 18 entries")
}

#[test]
fn test_diff_lockfiles_stale_sha256_changed() {
    todo!("Phase 1: stub — old=simple-2-deps, new=simple-2-deps-stale (guava sha256 changed), expect modified has 1 entry")
}

#[test]
fn test_diff_lockfiles_classifier_identity_distinct() {
    todo!("Phase 1: stub — old has guava:31.1-jre, new has guava:31.1-jre + guava:31.1-jre:sources; expect added:[sources], modified:[]")
}

#[test]
fn test_diff_output_is_readable() {
    todo!("Phase 1: stub — diff with differences produces Display output showing +added, -removed, ~modified lines")
}
