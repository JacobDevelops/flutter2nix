#[allow(unused_imports)]
use super::*;

/// Pure test: verify signing order computation
#[test]
fn test_compute_signing_order_basic() {
    use tempfile::TempDir;

    let tmpdir = TempDir::new().expect("failed to create tempdir");
    let app_path = tmpdir.path().join("Test.app");
    std::fs::create_dir(&app_path).expect("failed to create app dir");

    // Create a Frameworks directory with a stub framework
    let frameworks_dir = app_path.join("Frameworks");
    std::fs::create_dir(&frameworks_dir).expect("failed to create frameworks dir");
    let framework = frameworks_dir.join("Test.framework");
    std::fs::create_dir(&framework).expect("failed to create framework");

    let order = super::compute_signing_order(&app_path).expect("should compute signing order");

    assert_eq!(order.frameworks.len(), 1, "should find one framework");
    assert_eq!(order.extensions.len(), 0, "should find no extensions");
    assert_eq!(order.app, app_path, "main app path should match");
}

/// Pure test: verify signing order with extensions
#[test]
fn test_compute_signing_order_with_extension() {
    use tempfile::TempDir;

    let tmpdir = TempDir::new().expect("failed to create tempdir");
    let app_path = tmpdir.path().join("Test.app");
    std::fs::create_dir(&app_path).expect("failed to create app dir");

    // Create PlugIns directory with an extension
    let plugins_dir = app_path.join("PlugIns");
    std::fs::create_dir(&plugins_dir).expect("failed to create plugins dir");
    let appex = plugins_dir.join("Demo.appex");
    std::fs::create_dir(&appex).expect("failed to create appex dir");

    // Extension has its own frameworks
    let ext_frameworks_dir = appex.join("Frameworks");
    std::fs::create_dir(&ext_frameworks_dir).expect("failed to create extension frameworks");
    let ext_framework = ext_frameworks_dir.join("DemoFW.framework");
    std::fs::create_dir(&ext_framework).expect("failed to create extension framework");

    let order = super::compute_signing_order(&app_path).expect("should compute signing order");

    assert_eq!(
        order.frameworks.len(),
        0,
        "main app should have no frameworks"
    );
    assert_eq!(order.extensions.len(), 1, "should find one extension");
    assert_eq!(
        order.extensions[0].frameworks.len(),
        1,
        "extension should have one framework"
    );
    assert_eq!(order.app, app_path, "main app path should match");
}

/// Create a minimal fake bundle (Info.plist + executable copied from /bin/ls).
/// Returns false (caller should skip) if the executable can't be copied.
#[cfg(target_os = "macos")]
fn make_fake_bundle(bundle_dir: &std::path::Path, bundle_id: &str, executable: &str) -> bool {
    std::fs::create_dir_all(bundle_dir).expect("failed to create bundle dir");

    let plist_content = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleIdentifier</key>
    <string>{bundle_id}</string>
    <key>CFBundleExecutable</key>
    <string>{executable}</string>
    <key>CFBundleVersion</key>
    <string>1</string>
</dict>
</plist>"#
    );
    std::fs::write(bundle_dir.join("Info.plist"), plist_content)
        .expect("failed to write Info.plist");

    std::fs::copy("/bin/ls", bundle_dir.join(executable)).is_ok()
}

/// Zip a Payload directory into an .ipa. Returns false (caller should skip) on zip failure.
#[cfg(target_os = "macos")]
fn zip_payload(workdir: &std::path::Path, ipa_path: &std::path::Path) -> bool {
    std::process::Command::new("zip")
        .arg("-qry")
        .arg(ipa_path)
        .arg("Payload")
        .current_dir(workdir)
        .output()
        .is_ok_and(|o| o.status.success())
}

#[cfg(target_os = "macos")]
#[test]
fn test_sign_ipa_with_ad_hoc_identity() {
    use std::process::Command;
    use tempfile::TempDir;

    let tmpdir = TempDir::new().expect("failed to create tempdir");
    let app_dir = tmpdir.path().join("Payload/Test.app");

    if !make_fake_bundle(&app_dir, "com.example.test", "Test") {
        eprintln!("Failed to copy /bin/ls (OK in sandbox); skipping test");
        return;
    }

    let ipa_path = tmpdir.path().join("test.ipa");
    if !zip_payload(tmpdir.path(), &ipa_path) {
        eprintln!("zip failed; skipping test");
        return;
    }

    // Sign the IPA with the ad-hoc identity
    let cmd = super::SignCommand {
        ipa_path: ipa_path.clone(),
        identity: "-".to_string(),
        keychain: None,
        output: tmpdir.path().join("test-signed.ipa"),
    };

    let result = super::run(cmd);
    assert!(
        result.is_ok(),
        "should sign IPA with ad-hoc identity: {:?}",
        result
    );

    let output_path = result.unwrap();
    assert!(output_path.exists(), "output IPA should exist");

    // Verify it's a valid zip
    let unzip_check = match Command::new("unzip")
        .args(["-t"])
        .arg(&output_path)
        .output()
    {
        Err(e) => {
            eprintln!("unzip missing; skipping verification: {}", e);
            return;
        }
        Ok(output) => output,
    };
    assert!(
        unzip_check.status.success(),
        "signed IPA should be valid zip"
    );
}

#[cfg(target_os = "macos")]
#[test]
fn test_sign_ipa_with_extension() {
    use tempfile::TempDir;

    let tmpdir = TempDir::new().expect("failed to create tempdir");
    let app_dir = tmpdir.path().join("Payload/Test.app");

    if !make_fake_bundle(&app_dir, "com.example.test", "Test") {
        eprintln!("Failed to copy /bin/ls (OK in sandbox); skipping test");
        return;
    }

    // App extension with its own Info.plist + executable — exercises inside-out ordering
    let appex_dir = app_dir.join("PlugIns/Demo.appex");
    if !make_fake_bundle(&appex_dir, "com.example.test.Demo", "Demo") {
        eprintln!("Failed to copy /bin/ls (OK in sandbox); skipping test");
        return;
    }

    let ipa_path = tmpdir.path().join("test.ipa");
    if !zip_payload(tmpdir.path(), &ipa_path) {
        eprintln!("zip failed; skipping test");
        return;
    }

    let cmd = super::SignCommand {
        ipa_path,
        identity: "-".to_string(),
        keychain: None,
        output: tmpdir.path().join("test-signed.ipa"),
    };

    let result = super::run(cmd);
    assert!(
        result.is_ok(),
        "should sign IPA carrying an .appex: {:?}",
        result
    );
    assert!(result.unwrap().exists(), "output IPA should exist");
}

#[cfg(target_os = "macos")]
#[test]
fn test_sign_ipa_invalid_identity() {
    use tempfile::TempDir;

    let tmpdir = TempDir::new().expect("failed to create tempdir");
    let app_dir = tmpdir.path().join("Payload/Test.app");

    if !make_fake_bundle(&app_dir, "com.example.test", "Test") {
        eprintln!("Failed to copy /bin/ls; skipping test");
        return;
    }

    let ipa_path = tmpdir.path().join("test.ipa");
    if !zip_payload(tmpdir.path(), &ipa_path) {
        return;
    }

    // Try to sign with nonexistent identity
    let cmd = super::SignCommand {
        ipa_path,
        identity: "Nonexistent: Identity (XXX)".to_string(),
        keychain: None,
        output: tmpdir.path().join("test-signed.ipa"),
    };

    let result = super::run(cmd);
    assert!(result.is_err(), "should fail with invalid identity");
}
