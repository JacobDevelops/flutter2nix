#[allow(unused_imports)]
use super::*;
use crate::dep::{DependencyGraph, LockedDependency};
use base64::Engine;

fn make_dep(name: &str, version: &str, url: &str, sha256_hex: &str) -> LockedDependency {
    LockedDependency::new(
        name.to_string(),
        version.to_string(),
        url.to_string(),
        sha256_hex.to_string(),
    )
}

fn simple_graph() -> DependencyGraph {
    DependencyGraph {
        format_version: "1".to_string(),
        nodes: vec![
            make_dep(
                "com.google.guava:guava:31.1-jre",
                "31.1-jre",
                "https://repo.maven.apache.org/maven2/com/google/guava/guava/31.1-jre/guava-31.1-jre.jar",
                "c4b87ce48bf565fce5dfc5db0ce5f13f69e58e6b0f7b960e42ab2e42e7cef83c",
            ),
            make_dep(
                "junit:junit:4.13.2",
                "4.13.2",
                "https://repo.maven.apache.org/maven2/junit/junit/4.13.2/junit-4.13.2.jar",
                "8e495b634469d64fb8acfa3495a065cdc1e19432e3508bfc5cc1e73eaebc19b0",
            ),
        ],
    }
}

fn inline_config() -> NixMavenCodegenConfig {
    NixMavenCodegenConfig {
        fetcher: "fetchMaven".to_string(),
        indent_width: 2,
        sort_deps: true,
    }
}

#[test]
fn test_nix_codegen_simple_2_deps_inline() {
    let graph = simple_graph();
    let config = inline_config();
    let output = generate_nix_set(&graph, &config).unwrap();

    let fixture = std::fs::read_to_string(
        "../../crates/gradle2nix/tests/fixtures/nix-outputs/simple-2-deps-inline.nix",
    )
    .unwrap();
    assert_eq!(output, fixture, "inline output must match fixture exactly");
}

#[test]
fn test_nix_codegen_flake_format() {
    let graph = simple_graph();
    let config = NixMavenCodegenConfig {
        fetcher: "pkgs.fetchMaven".to_string(),
        indent_width: 2,
        sort_deps: true,
    };
    let output = generate_nix_overlay(&graph, &config).unwrap();

    let fixture = std::fs::read_to_string(
        "../../crates/gradle2nix/tests/fixtures/nix-outputs/simple-2-deps-flake.nix",
    )
    .unwrap();
    assert_eq!(output, fixture, "flake output must match fixture exactly");
}

#[test]
fn test_nix_codegen_special_chars_in_group() {
    let graph = DependencyGraph {
        format_version: "1".to_string(),
        nodes: vec![make_dep(
            "io.netty:netty-codec-http:4.1.100.Final",
            "4.1.100.Final",
            "https://repo.maven.apache.org/maven2/io/netty/netty-codec-http/4.1.100.Final/netty-codec-http-4.1.100.Final.jar",
            "7890123456789abc7890123456789abc7890123456789abc7890123456789abc",
        )],
    };
    let output = generate_nix_set(&graph, &inline_config()).unwrap();
    assert!(
        output.contains(r#""io.netty:netty-codec-http:4.1.100.Final""#),
        "Nix output must quote the key: {output}"
    );
    assert!(
        output.contains("io/netty/netty-codec-http"),
        "artifact path must use slash-separated group: {output}"
    );
}

#[test]
fn test_nix_codegen_deterministic_output() {
    let graph = simple_graph();
    let config = inline_config();
    let out1 = generate_nix_set(&graph, &config).unwrap();
    let out2 = generate_nix_set(&graph, &config).unwrap();
    assert_eq!(out1, out2, "codegen must be deterministic");
}

#[test]
fn test_nix_codegen_large_graph_20_deps() {
    let fixture_json = std::fs::read_to_string(
        "../../crates/gradle2nix/tests/fixtures/lockfiles/complex-20-deps.lock",
    )
    .unwrap();
    let graph: DependencyGraph = serde_json::from_str(&fixture_json).unwrap();
    let output = generate_nix_set(&graph, &inline_config()).unwrap();
    assert!(
        output.starts_with("{\n"),
        "output must start with opening brace"
    );
    assert!(
        output.ends_with("}\n"),
        "output must end with closing brace and newline"
    );
    assert_eq!(
        output.matches("fetchMaven").count(),
        20,
        "must have 20 fetchMaven entries"
    );
}

#[test]
fn test_locked_dependency_sha256_as_sri_conversion() {
    // Test basic conversion: d7a8fbb307d7809469ca9abcb0082e4f8d5651e46d3cdb762d02d0bf37c9e592
    // should convert to "sha256-16j7swfXgJRpypq8sAguT41WUeRtPNt2LQLQvzfJ5ZI="
    let dep = LockedDependency::new(
        "test:pkg:1.0".to_string(),
        "1.0".to_string(),
        "https://example.com/test.jar".to_string(),
        "d7a8fbb307d7809469ca9abcb0082e4f8d5651e46d3cdb762d02d0bf37c9e592".to_string(),
    );

    let result = dep.sha256_as_sri().unwrap();
    assert_eq!(
        result, "sha256-16j7swfXgJRpypq8sAguT41WUeRtPNt2LQLQvzfJ5ZI=",
        "sha256_as_sri must produce correct SRI format"
    );
}

#[test]
fn test_hex_to_sri_standard_iana_vector() {
    // IANA test vector: SHA-256 of "The quick brown fox jumps over the lazy dog"
    // hex: d7a8fbb307d7809469ca9abcb0082e4f8d5651e46d3cdb762d02d0bf37c9e592
    // base64: 16j7swfXgJRpypq8sAguT41WUeRtPNt2LQLQvzfJ5ZI=
    // SRI: sha256-16j7swfXgJRpypq8sAguT41WUeRtPNt2LQLQvzfJ5ZI=
    use crate::dep::hex_to_sri;

    let result = hex_to_sri("d7a8fbb307d7809469ca9abcb0082e4f8d5651e46d3cdb762d02d0bf37c9e592")
        .expect("should decode valid hex");
    assert_eq!(
        result, "sha256-16j7swfXgJRpypq8sAguT41WUeRtPNt2LQLQvzfJ5ZI=",
        "standard IANA vector must convert correctly"
    );
}

#[test]
fn test_hex_to_sri_roundtrip_decode() {
    use crate::dep::hex_to_sri;

    let original_hex = "d7a8fbb307d7809469ca9abcb0082e4f8d5651e46d3cdb762d02d0bf37c9e592";
    let sri = hex_to_sri(original_hex).unwrap();

    // Extract base64 from "sha256-<base64>"
    let b64_part = sri.strip_prefix("sha256-").unwrap();

    // Decode base64 back to bytes
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(b64_part)
        .expect("should decode valid base64");

    // Encode bytes back to hex
    let roundtrip_hex = hex::encode(&bytes);

    assert_eq!(
        roundtrip_hex, original_hex,
        "roundtrip hex->base64->hex must preserve original"
    );
}

#[test]
fn test_hex_to_sri_invalid_hex_returns_error() {
    use crate::dep::hex_to_sri;

    let result = hex_to_sri("notvalidhex");
    assert!(
        result.is_err(),
        "invalid hex string should return error, got: {:?}",
        result
    );
}

#[test]
fn test_locked_dependency_sha256_serde_key() {
    // Verify that serialization uses "sha256" key (not "sha256_hex") for backward compatibility
    let dep = LockedDependency::new(
        "test:pkg:1.0".to_string(),
        "1.0".to_string(),
        "https://example.com/test.jar".to_string(),
        "d7a8fbb307d7809469ca9abcb0082e4f8d5651e46d3cdb762d02d0bf37c9e592".to_string(),
    );

    let json = serde_json::to_string(&dep).unwrap();

    // Must contain "sha256" key, not "sha256_hex"
    assert!(
        json.contains("\"sha256\""),
        "serialized JSON must use 'sha256' key: {}",
        json
    );
    assert!(
        !json.contains("\"sha256_hex\""),
        "serialized JSON must NOT use 'sha256_hex' key: {}",
        json
    );

    // Verify deserialization round-trip
    let deserialized: LockedDependency = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.sha256_hex(), dep.sha256_hex());
}

#[test]
fn test_generate_nix_uses_sri_format() {
    // Verify that generated Nix output uses SRI format (sha256-<base64>) not raw hex
    let graph = simple_graph();
    let config = inline_config();
    let output = generate_nix_set(&graph, &config).unwrap();

    // Must contain SRI format: sha256-<base64>=
    assert!(
        output.contains("sha256-"),
        "generated Nix must use SRI format (sha256-...), got: {}",
        output
    );

    // Verify specific SRI hashes from the simple_graph
    // guava SHA256 hex: c4b87ce48bf565fce5dfc5db0ce5f13f69e58e6b0f7b960e42ab2e42e7cef83c
    // converts to: sha256-xLh85Iv1Zfzl38XbDOXxP2nljmsPe5YOQqsuQufO+Dw=
    assert!(
        output.contains("sha256-xLh85Iv1Zfzl38XbDOXxP2nljmsPe5YOQqsuQufO+Dw="),
        "guava SHA256 must be in SRI format: {}",
        output
    );

    // junit SHA256 hex: 8e495b634469d64fb8acfa3495a065cdc1e19432e3508bfc5cc1e73eaebc19b0
    // converts to: sha256-jklbY0Rp1k+4rPo0laBlzcHhlDLjUIv8XMHnPq68GbA=
    assert!(
        output.contains("sha256-jklbY0Rp1k+4rPo0laBlzcHhlDLjUIv8XMHnPq68GbA="),
        "junit SHA256 must be in SRI format: {}",
        output
    );

    // Must NOT contain raw hex values
    assert!(
        !output.contains("c4b87ce48bf565fce5dfc5db0ce5f13f69e58e6b0f7b960e42ab2e42e7cef83c"),
        "output must not contain raw hex (guava), got: {}",
        output
    );
}
