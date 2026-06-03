#[tokio::test]
async fn test_cli_lock_full_pipeline() {
    todo!(
        "Phase 1: stub — \
        gradle_dir: fixtures/gradle-projects/simple-app (with .gradle2nix-tapi-output.json sidecar), \
        output: /tmp/gradle2nix-test-lock.json, \
        expect: file created, content matches lockfiles/simple-2-deps.json, exit 0"
    )
}

#[tokio::test]
async fn test_cli_check_fresh_lockfile() {
    todo!(
        "Phase 1: stub — \
        gradle_dir: fixtures/gradle-projects/simple-app, \
        lockfile: fixtures/lockfiles/simple-2-deps.json (matching content), \
        expect: exit 0, no stale output"
    )
}

#[tokio::test]
async fn test_cli_check_stale_lockfile() {
    todo!(
        "Phase 1: stub — \
        gradle_dir: fixtures/gradle-projects/simple-app, \
        lockfile: fixtures/lockfiles/simple-2-deps-stale.json (guava sha256 changed), \
        expect: exit 1, output contains diff showing modified guava entry"
    )
}

#[tokio::test]
async fn test_cli_generate_from_lockfile() {
    todo!(
        "Phase 1: stub — \
        lockfile: fixtures/lockfiles/simple-2-deps.json, \
        output: /tmp/gradle2nix-test-generate.nix, \
        format: Inline, \
        expect: file created, content matches nix-outputs/simple-2-deps-inline.nix, exit 0"
    )
}

#[tokio::test]
async fn test_cli_generate_missing_lockfile() {
    todo!(
        "Phase 1: stub — \
        lockfile: /nonexistent/path/gradle.nix, \
        expect: Err result containing 'not found' or 'No such file', exit non-zero"
    )
}
