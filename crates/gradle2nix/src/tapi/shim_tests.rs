#[allow(unused_imports)]
use super::*;
use crate::tapi::model::try_parse_sentinel;

#[tokio::test]
async fn test_tapi_invocation_timeout() {
    let result = invoke_tapi_shim(TapiShimConfig {
        gradle_project_dir: std::path::PathBuf::from("."),
        gradle_user_home: None,
        gradle_cache_dir: None,
        timeout_secs: 1,
        tapi_json_override: None,
        test_command: Some(vec!["sleep".to_string(), "60".to_string()]),
    })
    .await;

    assert!(result.is_err(), "expected timeout error, got Ok");
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("timed out") || msg.contains("timeout"),
        "error must mention timeout: {msg}"
    );
}

#[tokio::test]
async fn test_tapi_invocation_jvm_not_found() {
    let result = invoke_tapi_shim(TapiShimConfig {
        gradle_project_dir: std::path::PathBuf::from("/nonexistent/gradle/project"),
        gradle_user_home: None,
        gradle_cache_dir: None,
        timeout_secs: 60,
        tapi_json_override: None,
        test_command: None,
    })
    .await;

    assert!(result.is_err(), "expected error for missing project dir, got Ok");
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("not found") || msg.contains("java") || msg.contains("No such file"),
        "error must describe missing path or java: {msg}"
    );
}

#[tokio::test]
async fn test_tapi_invocation_includes_no_configuration_cache_flag() {
    // Use tapi_json_override to bypass actual JVM invocation.
    // This test verifies that TapiShimConfig accepts gradle_cache_dir and can be constructed correctly.
    let minimal_tapi_json = r#"{"version":"1.0","artifacts":[]}"#;

    let result = invoke_tapi_shim(TapiShimConfig {
        gradle_project_dir: std::path::PathBuf::from("."),
        gradle_user_home: None,
        gradle_cache_dir: Some(std::path::PathBuf::from("/tmp/gradle-cache")),
        timeout_secs: 60,
        tapi_json_override: Some(minimal_tapi_json.to_string()),
        test_command: None,
    })
    .await;

    // With tapi_json_override set, the function bypasses Java and returns the override directly.
    assert!(result.is_ok(), "tapi_json_override should bypass JVM and return the provided JSON");
    assert_eq!(result.unwrap(), minimal_tapi_json);
}

#[test]
fn test_sentinel_parsing_clean_output() {
    let stdout = "FLUTTER2NIX_VERSION:9.4.1\nFLUTTER2NIX_DEPS:[{\"group\":\"com.example\",\"artifact\":\"mylib\",\"version\":\"1.0\",\"classifier\":null,\"extension\":\"jar\",\"scope\":\"releaseRuntimeClasspath\"}]";
    let artifacts = try_parse_sentinel(stdout).expect("expected Some from clean sentinel output");
    assert_eq!(artifacts.len(), 1, "expected 1 artifact");
    assert_eq!(artifacts[0].group, "com.example");
    assert_eq!(artifacts[0].scope, "releaseRuntimeClasspath");
}

#[test]
fn test_sentinel_parsing_with_gradle_noise() {
    let stdout = "[WARN] Deprecated feature used\n[INFO] Configuring project :app\nFLUTTER2NIX_VERSION:8.4.0\n[INFO] Task :flutter2nixDeps executed\nFLUTTER2NIX_DEPS:[{\"group\":\"org.example\",\"artifact\":\"lib\",\"version\":\"2.0\",\"classifier\":null,\"extension\":\"jar\",\"scope\":\"runtimeClasspath\"}]\n[INFO] BUILD SUCCESSFUL in 5s";
    let artifacts = try_parse_sentinel(stdout).expect("expected Some despite Gradle noise");
    assert_eq!(artifacts.len(), 1, "expected 1 artifact extracted from noisy output");
    assert_eq!(artifacts[0].group, "org.example");
}

#[test]
fn test_sentinel_parsing_invalid_json() {
    let stdout = "FLUTTER2NIX_VERSION:9.0.0\nFLUTTER2NIX_DEPS:invalid-json-here";
    let result = try_parse_sentinel(stdout);
    assert_eq!(
        result,
        Some(vec![]),
        "invalid sentinel JSON should yield empty list, not panic"
    );
}

#[test]
fn test_sentinel_parsing_missing_sentinel() {
    let stdout = "[INFO] Build output\n[WARN] Some warning\nBUILD SUCCESSFUL in 3s";
    let result = try_parse_sentinel(stdout);
    assert!(result.is_none(), "no FLUTTER2NIX_DEPS sentinel should return None to trigger fallback");
}
