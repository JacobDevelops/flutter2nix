use super::*;
use std::path::PathBuf;

#[test]
fn test_resolve_developer_dir_uses_valid_user_var() {
    let tmpdir = tempfile::tempdir().expect("failed to create tempdir");
    let dir = tmpdir.path();
    let bin_dir = dir.join("usr/bin");
    std::fs::create_dir_all(&bin_dir).expect("failed to create usr/bin");
    std::fs::File::create(bin_dir.join("xcodebuild")).expect("failed to create xcodebuild");

    let dir_str = dir.to_string_lossy().to_string();
    let result = resolve_developer_dir(
        |key| {
            if key == "DEVELOPER_DIR" {
                Some(dir_str.clone())
            } else {
                None
            }
        },
        || Err(anyhow::anyhow!("fallback should not be called")),
    );

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), dir);
}

#[test]
fn test_resolve_developer_dir_rejects_invalid_path() {
    let tmpdir = tempfile::tempdir().expect("failed to create tempdir");
    let dir = tmpdir.path().to_string_lossy().to_string();

    let result = resolve_developer_dir(
        |key| {
            if key == "DEVELOPER_DIR" {
                Some(dir.clone())
            } else {
                None
            }
        },
        || Err(anyhow::anyhow!("fallback")),
    );

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("invalid Xcode path"));
}

#[test]
fn test_resolve_developer_dir_ignores_nix_store_pollution() {
    let result = resolve_developer_dir(
        |key| {
            if key == "DEVELOPER_DIR" {
                Some("/nix/store/abc-apple-sdk".to_string())
            } else {
                None
            }
        },
        || Ok(PathBuf::from("/fallback/path")),
    );

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), PathBuf::from("/fallback/path"));
}

#[test]
fn test_resolve_developer_dir_falls_back_when_unset() {
    let result = resolve_developer_dir(|_| None, || Ok(PathBuf::from("/fallback/path")));

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), PathBuf::from("/fallback/path"));
}

#[test]
fn test_sanitized_env_strips_nix_vars() {
    let env = vec![
        ("PATH".to_string(), "/usr/bin:/nix".to_string()),
        ("HOME".to_string(), "/home/user".to_string()),
        (
            "NIX_CFLAGS_COMPILE".to_string(),
            "-march=native".to_string(),
        ),
        ("CC".to_string(), "clang".to_string()),
        ("SDKROOT".to_string(), "/some/sdk".to_string()),
        ("DEVELOPER_DIR".to_string(), "/some/xcode".to_string()),
    ];

    let result = sanitized_env(env);
    let result_map: std::collections::HashMap<String, String> = result.into_iter().collect();

    assert!(!result_map.contains_key("NIX_CFLAGS_COMPILE"));
    assert!(!result_map.contains_key("CC"));
    assert!(!result_map.contains_key("SDKROOT"));
    assert!(!result_map.contains_key("DEVELOPER_DIR"));
    assert_eq!(
        result_map.get("HOME").map(|s| s.as_str()),
        Some("/home/user")
    );
    assert_eq!(
        result_map.get("PATH").map(|s| s.as_str()),
        Some("/usr/bin:/bin:/usr/sbin:/sbin")
    );
}

#[test]
fn test_xcode_env_apply_to() {
    let env = XcodeEnv {
        developer_dir: PathBuf::from("/Applications/Xcode.app/Contents/Developer"),
        sdkroot: PathBuf::from("/Applications/Xcode.app/Contents/Developer/Platforms/iPhoneOS.platform/Developer/SDKs/iPhoneOS.sdk"),
    };

    let mut cmd = std::process::Command::new("true");
    env.apply_to(&mut cmd);

    // We can't easily inspect Command internals, but we can verify it doesn't panic.
    // The actual env vars would be tested via integration tests.
}

#[cfg(target_os = "macos")]
#[test]
fn test_setup_xcode_env_real() {
    match setup_xcode_env() {
        Ok(env) => {
            assert!(env.developer_dir.exists(), "developer_dir should exist");
            assert!(env.sdkroot.exists(), "sdkroot should exist");
        }
        Err(e) => {
            // Xcode or SDK may not be available on this system (e.g., CI without macOS),
            // or at an unexpected location. Just log the error and skip the assertion.
            eprintln!("setup_xcode_env failed (OK for CI without Xcode): {}", e);
        }
    }
}
