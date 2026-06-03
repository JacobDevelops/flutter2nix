#[allow(unused_imports)]
use super::*;

#[test]
fn test_maven_coordinate_parse_basic() {
    todo!("Phase 1: stub — 'com.google.guava:guava:31.1-jre' → MavenCoordinate{{group:'com.google.guava', artifact:'guava', version:'31.1-jre', classifier:None, extension:'jar'}}")
}

#[test]
fn test_maven_coordinate_parse_with_classifier() {
    todo!("Phase 1: stub — 'com.google.guava:guava:31.1-jre:sources' → classifier:Some('sources')")
}

#[test]
fn test_maven_coordinate_parse_with_extension() {
    todo!("Phase 1: stub — 'org.apache.maven:maven-core:3.9.0@aar' → extension:'aar'")
}

#[test]
fn test_maven_coordinate_to_artifact_path() {
    todo!("Phase 1: stub — MavenCoordinate{{guava, no classifier}} → 'com/google/guava/guava/31.1-jre/guava-31.1-jre.jar'")
}

#[test]
fn test_maven_coordinate_to_artifact_path_with_classifier() {
    todo!("Phase 1: stub — classifier:'sources' → 'com/google/guava/guava/31.1-jre/guava-31.1-jre-sources.jar'")
}

#[test]
fn test_maven_coordinate_roundtrip_parse_to_path_to_parse() {
    todo!("Phase 1: stub — property-based: parse(coord).to_artifact_path() is deterministic")
}

#[test]
fn test_maven_coordinate_special_chars_in_group() {
    todo!("Phase 1: stub — 'io.netty:netty-codec-http:4.1.100.Final' parses without error; dots in group handled")
}

#[test]
fn test_maven_coordinate_legacy_commons_lang() {
    todo!("Phase 1: stub — 'commons-lang:commons-lang:2.6' (group==artifact) parses and produces correct path")
}

#[tokio::test]
async fn test_resolve_artifact_sha256_from_local_cache() {
    todo!("Phase 1: stub — coord guava:31.1-jre, local_cache_dir=fixtures/maven-repos/maven-central-stub, expect real sha256 hex")
}

#[tokio::test]
async fn test_resolve_artifact_sha256_not_found_404() {
    todo!("Phase 1: stub — coord nonexistent:1.0.0, local_cache_dir=fixtures/maven-repos/missing-artifact, expect Err containing '404' or 'not found'")
}

#[tokio::test]
async fn test_resolve_artifact_sha256_invalid_format_not_hex() {
    todo!("Phase 1: stub — sha256 file contains 'not-a-sha256', expect Err containing 'invalid hex'")
}

#[tokio::test]
async fn test_resolve_artifacts_batch() {
    todo!("Phase 1: stub — [guava:31.1-jre, junit:4.13.2], local cache, expect Vec with 2 entries both populated")
}

#[test]
fn test_error_messages_are_actionable() {
    todo!("Phase 1: stub — various error conditions produce messages with: what went wrong, where, how to fix")
}
