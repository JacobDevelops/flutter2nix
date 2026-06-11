#[allow(unused_imports)]
use super::*;

#[cfg(target_os = "macos")]
#[test]
fn test_create_temp_keychain_success() {
    let result = TempKeychain::create("test-password");
    assert!(result.is_ok(), "should create temp keychain");
    let kc = result.unwrap();
    assert!(kc.path().exists(), "keychain file should exist");
}

#[cfg(target_os = "macos")]
#[test]
fn test_create_temp_keychain_cleanup() {
    let kc_path = {
        let result = TempKeychain::create("test-password");
        assert!(result.is_ok());
        let kc = result.unwrap();
        let p = kc.path().to_path_buf();
        assert!(p.exists(), "keychain should exist");
        p
    };
    // kc dropped here; cleanup should occur
    assert!(!kc_path.exists(), "keychain should be deleted after drop");
}

#[cfg(target_os = "macos")]
#[test]
fn test_import_certificate_to_keychain_valid() {
    // Generate a self-signed cert + key at runtime
    let tmpdir = tempfile::TempDir::new().expect("failed to create tempdir");
    let key_file = tmpdir.path().join("test_key.pem");
    let cert_file = tmpdir.path().join("test_cert.pem");
    let p12_file = tmpdir.path().join("test.p12");

    // Try to generate a self-signed cert. macOS ships LibreSSL, which has quirks with -legacy and -addext.
    // Attempt with modern openssl, fall back gracefully if it fails.
    let gen_result = std::process::Command::new("openssl")
        .args(["req", "-x509", "-newkey", "rsa:2048", "-keyout"])
        .arg(&key_file)
        .args(["-out"])
        .arg(&cert_file)
        .args(["-days", "1", "-nodes", "-subj", "/CN=ios2nix-test"])
        .output();

    if !gen_result.is_ok_and(|o| o.status.success()) {
        eprintln!("openssl cert generation failed (OK on systems without openssl)");
        return;
    }

    // Convert to PKCS12. OpenSSL 3 needs -legacy for keychain-importable
    // encryption; LibreSSL (macOS default) has no -legacy flag, so fall back.
    let pkcs12_export = |extra_args: &[&str]| {
        std::process::Command::new("openssl")
            .args(["pkcs12", "-export", "-out"])
            .arg(&p12_file)
            .arg("-inkey")
            .arg(&key_file)
            .arg("-in")
            .arg(&cert_file)
            .args(["-passout", "pass:testpw"])
            .args(extra_args)
            .output()
            .is_ok_and(|o| o.status.success())
    };

    if !pkcs12_export(&["-legacy"]) && !pkcs12_export(&[]) {
        eprintln!("openssl pkcs12 generation failed (OK on systems without modern openssl)");
        return;
    }

    // Create temp keychain
    let kc = TempKeychain::create("kc-password").expect("failed to create keychain");

    // Import the cert
    let result = kc.import_identity(&p12_file, "testpw");
    assert!(
        result.is_ok(),
        "should import certificate to keychain: {:?}",
        result
    );
}

#[cfg(target_os = "macos")]
#[test]
fn test_import_certificate_to_keychain_invalid_format() {
    let tmpdir = tempfile::TempDir::new().expect("failed to create tempdir");
    let invalid_file = tmpdir.path().join("not-a-cert.p12");
    std::fs::write(&invalid_file, b"not a p12").expect("failed to write file");

    let kc = TempKeychain::create("kc-password").expect("failed to create keychain");
    let result = kc.import_identity(&invalid_file, "wrong-password");

    assert!(result.is_err(), "should reject invalid certificate format");
}
