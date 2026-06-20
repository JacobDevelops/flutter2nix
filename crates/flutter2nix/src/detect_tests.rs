use super::*;

#[test]
fn detect_ios_requires_podfile_lock() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    // No ios/ dir at all → not lockable.
    assert!(!detect_ios(root));
    // ios/ dir but no Podfile.lock → the "skip iOS lock + warn" case.
    std::fs::create_dir(root.join("ios")).unwrap();
    assert!(!detect_ios(root));
    // ios/Podfile.lock present → iOS is lockable.
    std::fs::write(root.join("ios/Podfile.lock"), "PODFILE CHECKSUM: x\n").unwrap();
    assert!(detect_ios(root));
}
