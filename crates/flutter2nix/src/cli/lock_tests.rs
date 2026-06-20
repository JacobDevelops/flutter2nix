use super::*;

#[tokio::test]
async fn errors_when_no_lockable_platforms() {
    let tmp = tempfile::tempdir().unwrap();
    // pubspec.yaml present, but no android/ or ios/ → nothing to lock.
    std::fs::write(tmp.path().join("pubspec.yaml"), "name: demo\n").unwrap();
    let err = generate_lockfile(tmp.path(), &[], None, None, 5, 5)
        .await
        .expect_err("a project with no lockable platforms must error");
    assert!(
        err.to_string().contains("no lockable platforms"),
        "expected 'no lockable platforms' hint, got: {err}"
    );
}

#[tokio::test]
async fn ios_sidecar_alone_is_lockable() {
    // A `.ios2nix-podspecs.json` sidecar makes iOS lockable even without a
    // Podfile.lock; the sidecar path parses offline, so this needs no network.
    // Without the sidecar gate this would fail with "no lockable platforms".
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("pubspec.yaml"), "name: demo\n").unwrap();
    std::fs::create_dir(tmp.path().join("ios")).unwrap();
    std::fs::write(
        tmp.path().join("ios/.ios2nix-podspecs.json"),
        r#"{"pods":[]}"#,
    )
    .unwrap();
    let lock = generate_lockfile(tmp.path(), &[], None, None, 5, 5)
        .await
        .expect("sidecar-only iOS project must be lockable");
    assert!(
        lock.ios.is_some(),
        "expected an ios section from the sidecar"
    );
    assert!(lock.android.is_none(), "no android/ present");
}
