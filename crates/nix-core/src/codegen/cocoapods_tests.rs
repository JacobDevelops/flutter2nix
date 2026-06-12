#[allow(unused_imports)]
use super::*;
use crate::dep::{DependencyGraph, LockedDependency};

fn make_dep(name: &str, version: &str, url: &str, sha256_hex: &str) -> LockedDependency {
    LockedDependency::new(
        name.to_string(),
        version.to_string(),
        url.to_string(),
        sha256_hex.to_string(),
    )
}

fn make_dep_with_source(
    name: &str,
    version: &str,
    url: &str,
    sha256_hex: &str,
    dep_source: &str,
) -> LockedDependency {
    let mut dep = LockedDependency::new(
        name.to_string(),
        version.to_string(),
        url.to_string(),
        sha256_hex.to_string(),
    );
    dep.dep_source = Some(dep_source.to_string());
    dep
}

fn simple_2_pods_graph() -> DependencyGraph {
    DependencyGraph {
        format_version: "1".to_string(),
        nodes: vec![
            make_dep(
                "Flutter",
                "1.0.0",
                "https://storage.googleapis.com/flutter_infra_release/releases/stable/ios/Flutter-1.0.0.zip",
                "deadbeefcafebabe1234567890abcdef1234567890abcdef1234567890abcdef",
            ),
            make_dep(
                "firebase_core",
                "10.0.0",
                "https://github.com/firebase/firebase-ios-sdk/releases/download/10.0.0/firebase_core.zip",
                "cafebabe1234567890abcdefdeadbeefcafebabe1234567890abcdef12345678",
            ),
        ],
    }
}

fn inline_config() -> NixCocoaPodsCodegenConfig {
    NixCocoaPodsCodegenConfig {
        indent_width: 2,
        sort_deps: true,
    }
}

#[test]
fn test_codegen_cocoapods_inline() {
    let graph = simple_2_pods_graph();
    let config = inline_config();
    let output = generate_nix_set(&graph, &config).unwrap();

    let fixture = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../ios2nix/tests/fixtures/nix-outputs/simple-2-pods-inline.nix"
    ))
    .unwrap();
    assert_eq!(output, fixture, "inline output must match fixture exactly");
}

#[test]
fn test_codegen_cocoapods_modular() {
    let fixture = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../ios2nix/tests/fixtures/nix-outputs/complex-20-pods-modular.nix"
    ))
    .unwrap();

    // Build the 20-pod graph programmatically from the fixture data
    let mut nodes = vec![
        make_dep(
            "Flutter",
            "1.0.0",
            "https://storage.googleapis.com/flutter_infra_release/releases/stable/ios/Flutter-1.0.0.zip",
            "deadbeefcafebabe1234567890abcdef1234567890abcdef1234567890abcdef",
        ),
        make_dep(
            "camera_avfoundation",
            "0.9.15",
            "https://github.com/flutter/plugins/releases/download/0.9.15/camera_avfoundation.zip",
            "8888888888888888888888888888888888888888888888888888888888888888",
        ),
        make_dep(
            "connectivity_plus",
            "1.2.0",
            "https://github.com/flutter/plugins/releases/download/1.2.0/connectivity_plus.zip",
            "aaaa0000aaaa0000aaaa0000aaaa0000aaaa0000aaaa0000aaaa0000aaaa0000",
        ),
        make_dep(
            "device_info_plus",
            "9.1.0",
            "https://github.com/flutter/plugins/releases/download/9.1.0/device_info_plus.zip",
            "cccc2222cccc2222cccc2222cccc2222cccc2222cccc2222cccc2222cccc2222",
        ),
        make_dep(
            "file_picker",
            "6.1.0",
            "https://github.com/flutter/plugins/releases/download/6.1.0/file_picker.zip",
            "1111777711117777111177771111777711117777111177771111777711117777",
        ),
        make_dep(
            "firebase_auth",
            "10.0.0",
            "https://github.com/firebase/firebase-ios-sdk/releases/download/10.0.0/firebase_auth.zip",
            "1111111111111111111111111111111111111111111111111111111111111111",
        ),
        make_dep(
            "firebase_core",
            "10.0.0",
            "https://github.com/firebase/firebase-ios-sdk/releases/download/10.0.0/firebase_core.zip",
            "cafebabe1234567890abcdefdeadbeefcafebabe1234567890abcdef12345678",
        ),
        make_dep(
            "firebase_firestore",
            "4.0.0",
            "https://github.com/firebase/firebase-ios-sdk/releases/download/4.0.0/firebase_firestore.zip",
            "2222222222222222222222222222222222222222222222222222222222222222",
        ),
        make_dep(
            "firebase_storage",
            "11.0.0",
            "https://github.com/firebase/firebase-ios-sdk/releases/download/11.0.0/firebase_storage.zip",
            "3333333333333333333333333333333333333333333333333333333333333333",
        ),
        make_dep(
            "google_sign_in_ios",
            "6.0.0",
            "https://github.com/flutter/plugins/releases/download/6.0.0/google_sign_in_ios.zip",
            "4444444444444444444444444444444444444444444444444444444444444444",
        ),
        make_dep(
            "image_picker_ios",
            "0.8.9",
            "https://github.com/flutter/plugins/releases/download/0.8.9/image_picker_ios.zip",
            "7777777777777777777777777777777777777777777777777777777777777777",
        ),
        make_dep(
            "in_app_purchase_storekit",
            "0.3.6",
            "https://github.com/flutter/plugins/releases/download/0.3.6/in_app_purchase_storekit.zip",
            "2222888822228888222288882222888822228888222288882222888822228888",
        ),
        make_dep(
            "local_auth_darwin",
            "2.2.0",
            "https://github.com/flutter/plugins/releases/download/2.2.0/local_auth_darwin.zip",
            "ffff5555ffff5555ffff5555ffff5555ffff5555ffff5555ffff5555ffff5555",
        ),
        make_dep(
            "package_info_plus",
            "7.0.0",
            "https://github.com/flutter/plugins/releases/download/7.0.0/package_info_plus.zip",
            "dddd3333dddd3333dddd3333dddd3333dddd3333dddd3333dddd3333dddd3333",
        ),
        make_dep(
            "path_provider_foundation",
            "2.3.0",
            "https://github.com/flutter/plugins/releases/download/2.3.0/path_provider_foundation.zip",
            "5555555555555555555555555555555555555555555555555555555555555555",
        ),
        make_dep(
            "permission_handler_apple",
            "9.2.0",
            "https://github.com/flutter/plugins/releases/download/9.2.0/permission_handler_apple.zip",
            "eeee4444eeee4444eeee4444eeee4444eeee4444eeee4444eeee4444eeee4444",
        ),
        make_dep(
            "shared_preferences_foundation",
            "2.3.0",
            "https://github.com/flutter/plugins/releases/download/2.3.0/shared_preferences_foundation.zip",
            "6666666666666666666666666666666666666666666666666666666666666666",
        ),
        make_dep(
            "sqflite_darwin",
            "2.3.0",
            "https://github.com/flutter/plugins/releases/download/2.3.0/sqflite_darwin.zip",
            "0000666600006666000066660000666600006666000066660000666600006666",
        ),
        make_dep(
            "url_launcher_ios",
            "6.2.0",
            "https://github.com/flutter/plugins/releases/download/6.2.0/url_launcher_ios.zip",
            "bbbb1111bbbb1111bbbb1111bbbb1111bbbb1111bbbb1111bbbb1111bbbb1111",
        ),
        make_dep(
            "video_player_avfoundation",
            "2.3.0",
            "https://github.com/flutter/plugins/releases/download/2.3.0/video_player_avfoundation.zip",
            "9999999999999999999999999999999999999999999999999999999999999999",
        ),
    ];

    nodes.sort_by(|a, b| a.name.cmp(&b.name));
    let graph = DependencyGraph {
        format_version: "1".to_string(),
        nodes,
    };
    let config = inline_config();
    let output = generate_nix_overlay(&graph, &config).unwrap();

    assert_eq!(output, fixture, "modular output must match fixture exactly");
}

#[test]
fn test_codegen_cocoapods_subspec_quoting() {
    let graph = DependencyGraph {
        format_version: "1".to_string(),
        nodes: vec![
            make_dep(
                "Firebase/CoreOnly",
                "10.0.0",
                "https://github.com/firebase/firebase-ios-sdk/releases/download/10.0.0/Firebase-CoreOnly.zip",
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            ),
            make_dep(
                "GTMSessionFetcher/Core",
                "3.1.0",
                "https://github.com/google/gtm-session-fetcher/releases/download/3.1.0/GTMSessionFetcher-Core.zip",
                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            ),
            make_dep(
                "nanopb-2.30908.0",
                "2.30908.0",
                "https://github.com/nanopb/nanopb/releases/download/2.30908.0/nanopb.zip",
                "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
            ),
            make_dep(
                "PINCache",
                "3.0.3",
                "https://github.com/pinterest/PINCache/releases/download/3.0.3/PINCache.zip",
                "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
            ),
        ],
    };

    let config = inline_config();
    let output = generate_nix_set(&graph, &config).unwrap();

    let fixture = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../ios2nix/tests/fixtures/nix-outputs/subspec-quoting-inline.nix"
    ))
    .unwrap();
    assert_eq!(
        output, fixture,
        "subspec quoting output must match fixture exactly"
    );
}

#[test]
fn test_codegen_cocoapods_git_pod() {
    let graph = DependencyGraph {
        format_version: "1".to_string(),
        nodes: vec![make_dep_with_source(
            "MBProgressHUD",
            "1.2.0",
            "git+https://github.com/jdg/MBProgressHUD.git#1.2.0-rev-sha",
            "1111111111111111111111111111111111111111111111111111111111111111",
            "pod-git",
        )],
    };

    let config = inline_config();
    let output = generate_nix_set(&graph, &config).unwrap();

    assert!(
        output.contains("{ lib, fetchurl, fetchgit }:"),
        "header must include fetchgit when git pods present"
    );
    assert!(
        output.contains("fetchgit {"),
        "must emit fetchgit call for git pods"
    );
    assert!(
        output.contains("url = \"https://github.com/jdg/MBProgressHUD.git\";"),
        "must extract repo URL from git pod URL"
    );
    assert!(
        output.contains("rev = \"1.2.0-rev-sha\";"),
        "must extract rev from git pod URL"
    );
}

#[test]
fn test_codegen_cocoapods_deterministic_output() {
    let graph = simple_2_pods_graph();
    let config = inline_config();
    let out1 = generate_nix_set(&graph, &config).unwrap();
    let out2 = generate_nix_set(&graph, &config).unwrap();
    assert_eq!(out1, out2, "codegen must be deterministic");
}

#[test]
fn test_codegen_cocoapods_no_sort() {
    let graph = DependencyGraph {
        format_version: "1".to_string(),
        nodes: vec![
            make_dep(
                "Zebra",
                "1.0.0",
                "https://example.com/zebra.zip",
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            ),
            make_dep(
                "Apple",
                "1.0.0",
                "https://example.com/apple.zip",
                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            ),
        ],
    };

    let config_no_sort = NixCocoaPodsCodegenConfig {
        indent_width: 2,
        sort_deps: false,
    };
    let output = generate_nix_set(&graph, &config_no_sort).unwrap();

    // Without sorting, Zebra should appear before Apple (insertion order)
    let zebra_pos = output.find("Zebra").expect("Zebra should be in output");
    let apple_pos = output.find("Apple").expect("Apple should be in output");
    assert!(
        zebra_pos < apple_pos,
        "without sort_deps, pods should maintain insertion order"
    );
}
