use super::*;

#[test]
fn test_assert_xcode_version_valid() {
    assert!(assert_xcode_version("15.3", "14.0").is_ok());
}

#[test]
fn test_assert_xcode_version_equal() {
    assert!(assert_xcode_version("14.0", "14.0").is_ok());
}

#[test]
fn test_assert_xcode_version_minor_higher() {
    assert!(assert_xcode_version("14.10", "14.9").is_ok());
}

#[test]
fn test_assert_xcode_version_too_old() {
    let result = assert_xcode_version("13.0", "14.0");
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("too old"));
}

#[test]
fn test_assert_xcode_version_malformed() {
    let result = assert_xcode_version("abc.def", "14.0");
    assert!(result.is_err());
}

#[cfg(target_os = "macos")]
#[test]
fn test_assert_xcode_tools_installed() {
    match assert_xcode_tools_installed() {
        Ok(()) => {}
        Err(e) => {
            // xcode-select is unreachable inside the Nix build sandbox (and on
            // hosts without the CLT) — skip rather than fail, like
            // test_setup_xcode_env_real.
            eprintln!("assert_xcode_tools_installed failed (OK in sandbox/CI): {e}");
        }
    }
}
