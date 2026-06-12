#[allow(unused_imports)]
use super::*;

#[test]
fn test_resolve_pod_url_valid() {
    let json = include_str!("../tests/fixtures/cocoapods-specs/flutter.json");
    let result = parse_podspec(json);

    assert!(result.is_ok());
    let podspec = result.unwrap();
    assert_eq!(podspec.name, "Flutter");
    assert_eq!(podspec.version, "1.0.0");
    match &podspec.source {
        PodSourceKind::Http { url } => {
            assert!(url.contains("flutter_infra_release"));
            assert!(url.ends_with(".zip"));
        }
        _ => panic!("expected Http source"),
    }
}

#[test]
fn test_resolve_pod_url_git_source() {
    let json = include_str!("../tests/fixtures/cocoapods-specs/mbprogresshud-git.json");
    let result = parse_podspec(json);

    assert!(result.is_ok());
    let podspec = result.unwrap();
    assert_eq!(podspec.name, "MBProgressHUD");
    assert_eq!(podspec.version, "1.2.0");
    match &podspec.source {
        PodSourceKind::Git { url, rev } => {
            assert_eq!(url, "https://github.com/jdg/MBProgressHUD.git");
            assert_eq!(rev, "1.2.0");
        }
        _ => panic!("expected Git source"),
    }
}

#[test]
fn test_resolve_pod_url_missing_spec() {
    // This test uses mockito to simulate a missing spec
    // For now, we test that an invalid JSON returns an error
    let result = parse_podspec("invalid json");
    assert!(result.is_err());
}
