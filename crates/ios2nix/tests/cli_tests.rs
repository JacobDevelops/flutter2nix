#[tokio::test]
async fn test_cli_lock_full_pipeline() {
    use std::path::PathBuf;

    // A tempdir plays the ios/ project dir; the committed sidecar fixture
    // short-circuits resolution so the pipeline runs hermetically.
    let ios_dir = tempfile::TempDir::new().unwrap();
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    std::fs::copy(
        fixtures.join("sidecars/simple.ios2nix-podspecs.json"),
        ios_dir.path().join(".ios2nix-podspecs.json"),
    )
    .unwrap();

    let output = ios_dir.path().join("ios2nix.lock");
    ios2nix::cli::lock::run(ios2nix::cli::lock::LockCommand {
        ios_dir: ios_dir.path().to_path_buf(),
        output: Some(output.clone()),
        spec_repos: None,
        cache_dir: None,
        timeout_secs: 60,
    })
    .await
    .unwrap();

    let written = std::fs::read_to_string(&output).unwrap();
    let expected =
        std::fs::read_to_string(fixtures.join("lockfiles/simple-expected.lock")).unwrap();
    assert_eq!(
        written, expected,
        "lock output must match simple-expected.lock fixture byte-for-byte"
    );
}

#[test]
fn test_cli_build_from_sidecar_hermetic() {
    let tmpdir = tempfile::TempDir::new().expect("failed to create tempdir");
    let sidecar = tmpdir.path().join(".ios2nix-xcode-output.json");

    let fixture = include_str!("fixtures/xcode-outputs/basic.json");
    std::fs::write(&sidecar, fixture).expect("failed to write sidecar");

    let cmd = ios2nix::cli::build::BuildCommand {
        project_dir: tmpdir.path().to_path_buf(),
        workspace: tmpdir.path().join("Runner.xcworkspace"),
        scheme: "Runner".to_string(),
        configuration: "Release".to_string(),
        derived_data: None,
    };

    let result = ios2nix::cli::build::run(cmd).expect("should build from sidecar");
    assert_eq!(result.version, "14.3");
}

#[cfg(target_os = "macos")]
fn copy_dir_all(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let file_name = entry.file_name();
        if path.is_dir() {
            copy_dir_all(&path, &dst.join(&file_name))?;
        } else {
            std::fs::copy(&path, dst.join(&file_name))?;
        }
    }
    Ok(())
}

/// Copy the native-app Xcode fixture into a tempdir; returns the destination root.
#[cfg(target_os = "macos")]
fn copy_native_app_fixture(tmpdir: &std::path::Path) -> std::path::PathBuf {
    let fixture_src = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/xcode-projects/native-app");
    let fixture_dst = tmpdir.join("native-app");
    copy_dir_all(&fixture_src, &fixture_dst).expect("failed to copy fixture");
    fixture_dst
}

#[cfg(target_os = "macos")]
#[test]
#[ignore = "invokes real xcodebuild (slow)"]
fn test_cli_archive_from_build() {
    let tmpdir = tempfile::TempDir::new().expect("failed to create tempdir");
    let fixture_dst = copy_native_app_fixture(tmpdir.path());

    let cmd = ios2nix::cli::archive::ArchiveCommand {
        workspace: fixture_dst.join("ExportTest.xcodeproj/project.xcworkspace"),
        scheme: "ExportTest".to_string(),
        configuration: "Release".to_string(),
        archive_path: fixture_dst.join("out.xcarchive"),
        signing: None,
        bundle_id: None,
    };

    let result = ios2nix::cli::archive::run(cmd);
    assert!(result.is_ok(), "should create archive: {:?}", result);
}

/// Signing material for the manual-run signing tests, sourced from the
/// IOS2NIX_* environment (plan 3 §1 contract). Runs `sign-setup` for real:
/// temp keychain + identity import + profile install.
#[cfg(target_os = "macos")]
struct SigningMaterial {
    team_id: String,
    identity: String,
    keychain: std::path::PathBuf,
    profile: ios2nix::provisioning::ProfileInfo,
}

/// `sign_setup::run` persists the keychain (it must outlive that process for
/// the Nix flow); inside the test process we own cleanup — delete it (which
/// also drops its search-list entry) even when an assertion panics.
#[cfg(target_os = "macos")]
impl Drop for SigningMaterial {
    fn drop(&mut self) {
        let _ = std::process::Command::new("security")
            .arg("delete-keychain")
            .arg(&self.keychain)
            .output();
    }
}

#[cfg(target_os = "macos")]
fn setup_signing_from_env() -> SigningMaterial {
    use std::path::PathBuf;

    let team_id = std::env::var("IOS2NIX_TEAM_ID").expect("IOS2NIX_TEAM_ID required for this test");
    let identity = std::env::var("IOS2NIX_SIGNING_IDENTITY")
        .expect("IOS2NIX_SIGNING_IDENTITY required for this test");
    let profile_path = PathBuf::from(
        std::env::var("IOS2NIX_PROFILE_PATH").expect("IOS2NIX_PROFILE_PATH required for this test"),
    );
    let p12_path = PathBuf::from(
        std::env::var("IOS2NIX_P12_PATH").expect("IOS2NIX_P12_PATH required for this test"),
    );

    let keychain = ios2nix::cli::sign_setup::run(ios2nix::cli::sign_setup::SignSetupCommand {
        p12: p12_path,
        profile: profile_path.clone(),
    })
    .expect("sign-setup should succeed");

    let decoded = ios2nix::provisioning::decode_cms_plist(&profile_path)
        .expect("should decode provisioning profile");
    let profile = ios2nix::provisioning::parse_profile_plist(&decoded)
        .expect("should parse provisioning profile");

    SigningMaterial {
        team_id,
        identity,
        keychain,
        profile,
    }
}

#[cfg(target_os = "macos")]
fn signed_archive_command(
    fixture_dst: &std::path::Path,
    signing: &SigningMaterial,
) -> ios2nix::cli::archive::ArchiveCommand {
    ios2nix::cli::archive::ArchiveCommand {
        workspace: fixture_dst.join("ExportTest.xcodeproj/project.xcworkspace"),
        scheme: "ExportTest".to_string(),
        configuration: "Release".to_string(),
        archive_path: fixture_dst.join("out.xcarchive"),
        signing: Some(ios2nix::export_opts::SigningConfig {
            team_id: signing.team_id.clone(),
            identity: signing.identity.clone(),
            profile_specifier: signing.profile.name.clone(),
            keychain: signing.keychain.clone(),
        }),
        // The fixture's own bundle ID won't match the supplied profile's App ID;
        // override it so any exact-match profile works.
        bundle_id: Some(signing.profile.bundle_id.clone()),
    }
}

#[cfg(target_os = "macos")]
#[test]
#[ignore = "requires signing material via IOS2NIX_* env"]
fn test_cli_export_ipa_with_codesign() {
    let signing = setup_signing_from_env();

    let tmpdir = tempfile::TempDir::new().expect("failed to create tempdir");
    let fixture_dst = copy_native_app_fixture(tmpdir.path());

    let archive_result = ios2nix::cli::archive::run(signed_archive_command(&fixture_dst, &signing));
    assert!(
        archive_result.is_ok(),
        "archive with signing should succeed: {:?}",
        archive_result
    );

    let archive_path = archive_result.unwrap();
    assert!(archive_path.exists(), "signed archive should exist");
}

#[cfg(target_os = "macos")]
#[test]
#[ignore = "requires signing material via IOS2NIX_* env"]
fn test_cli_full_e2e_lock_to_ipa() {
    let signing = setup_signing_from_env();

    let tmpdir = tempfile::TempDir::new().expect("failed to create tempdir");
    let fixture_dst = copy_native_app_fixture(tmpdir.path());

    // Archive (signed)
    let archive_path = ios2nix::cli::archive::run(signed_archive_command(&fixture_dst, &signing))
        .expect("archive should succeed");

    // Export with a Manual-style ExportOptions.plist (cert + bundleID -> profile UUID).
    // The method must match the supplied profile's kind (ad-hoc vs app-store);
    // default ad-hoc, override via IOS2NIX_EXPORT_METHOD.
    let method = std::env::var("IOS2NIX_EXPORT_METHOD")
        .unwrap_or_else(|_| "ad-hoc".to_string())
        .parse::<ios2nix::export_opts::ExportMethod>()
        .expect("IOS2NIX_EXPORT_METHOD must be a valid export method");
    let mut export_opts = ios2nix::export_opts::ExportOptions::new(method, signing.team_id.clone());
    export_opts.signing_certificate = Some(signing.identity.clone());
    export_opts.provisioning_profiles.insert(
        signing.profile.bundle_id.clone(),
        signing.profile.uuid.clone(),
    );

    let export_opts_plist = fixture_dst.join("ExportOptions.plist");
    ios2nix::export_opts::write_export_options(
        &export_opts,
        ios2nix::export_opts::resolve_method_name_style(),
        &export_opts_plist,
    )
    .expect("should write ExportOptions.plist");

    let ipa_path = ios2nix::cli::export::run(ios2nix::cli::export::ExportCommand {
        archive_path,
        export_opts_plist,
        output_path: fixture_dst.join("ipa"),
    })
    .expect("export should succeed");
    assert!(ipa_path.exists(), "exported IPA should exist");

    // Re-sign the exported IPA via `sign` (exercises the inside-out codesign path)
    let signed_ipa = ios2nix::cli::sign::run(ios2nix::cli::sign::SignCommand {
        ipa_path: ipa_path.clone(),
        identity: signing.identity.clone(),
        keychain: Some(signing.keychain.clone()),
        output: fixture_dst.join("resigned.ipa"),
    })
    .expect("re-sign should succeed");

    // Final assertions (plan 3 §8): valid zip with Payload/<App>.app/Info.plist
    let unzip_check = std::process::Command::new("unzip")
        .args(["-l"])
        .arg(&signed_ipa)
        .output()
        .expect("failed to run unzip -l");
    assert!(unzip_check.status.success(), "IPA should be a valid zip");
    let listing = String::from_utf8_lossy(&unzip_check.stdout);
    assert!(
        listing.contains(".app/Info.plist"),
        "IPA should contain Payload/<App>.app/Info.plist"
    );
}
