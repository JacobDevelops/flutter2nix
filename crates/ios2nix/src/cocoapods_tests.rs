#[allow(unused_imports)]
use super::*;

#[test]
fn test_parse_podfile_lock_simple() {
    let yaml = include_str!("../tests/fixtures/podfile-locks/simple-2-pods.lock");
    let result = parse_podfile_lock(yaml);

    assert!(result.is_ok());
    let lock = result.unwrap();
    assert_eq!(lock.pods.len(), 2);
    assert_eq!(lock.pods[0].name, "Flutter");
    assert_eq!(lock.pods[0].version, "1.0.0");
    assert_eq!(lock.pods[1].name, "firebase_core");
    assert_eq!(lock.pods[1].version, "10.0.0");
    assert!(lock.pods[0].deps.is_empty());
    assert!(lock.pods[1].deps.is_empty());
    assert_eq!(lock.cocoapods_version, "1.15.0");
}

#[test]
fn test_parse_podfile_lock_complex() {
    let yaml = include_str!("../tests/fixtures/podfile-locks/complex-20-pods.lock");
    let result = parse_podfile_lock(yaml);

    if let Err(ref e) = result {
        eprintln!("Parse error: {}", e);
    }
    assert!(result.is_ok());
    let lock = result.unwrap();
    assert_eq!(lock.pods.len(), 20);

    // Verify firebase_auth has dependency on firebase_core
    let firebase_auth = lock
        .pods
        .iter()
        .find(|p| p.name == "firebase_auth")
        .unwrap();
    assert_eq!(firebase_auth.version, "10.0.0");
    assert!(firebase_auth.deps.contains(&"firebase_core".to_string()));

    assert_eq!(lock.cocoapods_version, "1.15.0");
}

#[test]
fn test_parse_podfile_lock_invalid_yaml() {
    let yaml = include_str!("../tests/fixtures/podfile-locks/malformed-invalid.lock");
    let result = parse_podfile_lock(yaml);

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("YAML") || err_msg.contains("invalid"));
}

#[test]
fn test_parse_podfile_lock_missing_sha256() {
    let yaml = include_str!("../tests/fixtures/podfile-locks/malformed-missing-sha256.lock");
    let result = parse_podfile_lock(yaml);

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("SPEC CHECKSUMS") || err_msg.contains("checksum"));
}

#[test]
fn test_parse_podfile_lock_external_sources() {
    let yaml = include_str!("../tests/fixtures/podfile-locks/external-sources.lock");
    let lock = parse_podfile_lock(yaml).unwrap();

    assert_eq!(lock.pods.len(), 3);
    assert_eq!(lock.external_sources.len(), 3);

    let path_pod = &lock.external_sources["path_provider_foundation"];
    assert_eq!(
        path_pod.path.as_deref(),
        Some(".symlinks/plugins/path_provider_foundation/darwin")
    );
    assert!(path_pod.git.is_none());

    let git_pod = &lock.external_sources["MBProgressHUD"];
    assert_eq!(
        git_pod.git.as_deref(),
        Some("https://github.com/jdg/MBProgressHUD.git")
    );
    assert!(git_pod.path.is_none());

    let checkout = &lock.checkout_options["MBProgressHUD"];
    assert_eq!(checkout.git, "https://github.com/jdg/MBProgressHUD.git");
    assert_eq!(checkout.tag.as_deref(), Some("1.2.0"));
    assert!(checkout.commit.is_none());
    assert!(checkout.branch.is_none());
}

#[test]
fn test_parse_podfile_lock_one_pod_missing_checksum() {
    // A lockfile whose SPEC CHECKSUMS section exists but lacks one pod's entry
    // must be rejected — partial checksum coverage is a malformed lockfile.
    let yaml = "PODS:\n  - Flutter (1.0.0)\n  - firebase_core (10.0.0)\n\nDEPENDENCIES:\n  - Flutter\n  - firebase_core\n\nSPEC CHECKSUMS:\n  Flutter: deadbeefcafebabe1234567890abcdef1234567890abcdef1234567890abcdef\n\nPODFILE CHECKSUM: 0000000000000000000000000000000000000000\n\nCOCOAPODS: 1.15.0\n";
    let result = parse_podfile_lock(yaml);

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("firebase_core"));
    assert!(err_msg.contains("checksum"));
}
