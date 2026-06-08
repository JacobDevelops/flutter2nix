#[allow(unused_imports)]
use super::*;

#[test]
fn test_parse_tapi_valid_basic() {
    let json = std::fs::read_to_string("tests/fixtures/tapi-outputs/basic.json").unwrap();
    let output = parse_tapi_output(&json).unwrap();
    assert_eq!(output.version, "8.4.0");
    assert_eq!(output.artifacts.len(), 2);
}

#[test]
fn test_parse_tapi_missing_required_field_version() {
    let json = std::fs::read_to_string("tests/fixtures/tapi-outputs/malformed-missing-field.json").unwrap();
    let err = parse_tapi_output(&json).unwrap_err();
    assert!(
        err.to_string().contains("missing field"),
        "expected 'missing field' in error, got: {err}"
    );
}

#[test]
fn test_parse_tapi_unknown_extra_fields_ignored() {
    let json = std::fs::read_to_string("tests/fixtures/tapi-outputs/malformed-unknown-fields.json").unwrap();
    let result = parse_tapi_output(&json);
    assert!(result.is_ok(), "expected Ok, got: {:?}", result.unwrap_err());
}

#[test]
fn test_parse_tapi_version_mismatch_error() {
    let json = std::fs::read_to_string("tests/fixtures/tapi-outputs/version-mismatch.json").unwrap();
    let err = parse_tapi_output(&json).unwrap_err();
    assert!(
        err.to_string().contains("unsupported TAPI version"),
        "expected 'unsupported TAPI version' in error, got: {err}"
    );
}

#[test]
fn test_parse_tapi_with_classifiers() {
    let json = std::fs::read_to_string("tests/fixtures/tapi-outputs/with-classifiers.json").unwrap();
    let output = parse_tapi_output(&json).unwrap();
    let has_no_classifier = output.artifacts.iter().any(|a| a.classifier.is_none());
    let has_sources = output
        .artifacts
        .iter()
        .any(|a| a.classifier.as_deref() == Some("sources"));
    assert!(has_no_classifier, "expected at least one artifact with no classifier");
    assert!(has_sources, "expected at least one artifact with classifier 'sources'");
}

#[test]
fn test_parse_tapi_with_test_scope() {
    let json = std::fs::read_to_string("tests/fixtures/tapi-outputs/with-test-scope.json").unwrap();
    let output = parse_tapi_output(&json).unwrap();
    let has_test_scope = output.artifacts.iter().any(|a| a.scope == "test");
    assert!(has_test_scope, "expected at least one artifact with scope 'test'");
}

#[test]
fn test_buildscript_fallback_regex_parsing() {
    let content = r#"
buildscript {
    dependencies {
        classpath("com.android.tools.build:gradle:7.4.0")
        classpath("org.jetbrains.kotlin:kotlin-gradle-plugin:1.8.0")
    }
}
"#;
    let coords = parse_buildscript_deps(content).unwrap();
    assert_eq!(coords.len(), 2, "expected 2 buildscript coords, got {}", coords.len());

    // Verify AGP is in the results
    let has_agp = coords.iter().any(|c| {
        c.group == "com.android.tools.build" && c.artifact == "gradle" && c.version == "7.4.0"
    });
    assert!(has_agp, "expected AGP coordinate in results");

    // Verify Kotlin plugin is in the results
    let has_kotlin = coords.iter().any(|c| {
        c.group == "org.jetbrains.kotlin" && c.artifact == "kotlin-gradle-plugin" && c.version == "1.8.0"
    });
    assert!(has_kotlin, "expected Kotlin plugin coordinate in results");
}

#[test]
fn test_buildscript_fallback_zero_coords_errors() {
    let content = r#"
buildscript {
    // No classpath dependencies
}
"#;
    let result = parse_buildscript_deps(content);
    assert!(result.is_err(), "expected Err when buildscript block exists but has no classpath deps");
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("Buildscript block detected but no classpath dependencies found"),
        "expected specific error message, got: {msg}"
    );
}

#[test]
fn test_no_buildscript_block_succeeds() {
    let content = r#"
plugins {
    kotlin("jvm") version "1.8.0"
}

dependencies {
    implementation("com.google.guava:guava:31.1-jre")
}
"#;
    let coords = parse_buildscript_deps(content).unwrap();
    assert_eq!(coords.len(), 0, "expected empty vec when no buildscript block, got: {:?}", coords);
}
