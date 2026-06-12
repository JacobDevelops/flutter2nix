use super::*;
use nix_core::dep::LockedDependency;

fn make_node(name: &str) -> LockedDependency {
    LockedDependency::new(
        name.to_string(),
        "1.0".to_string(),
        format!("https://repo.maven.apache.org/maven2/{name}/1.0/{name}-1.0.jar"),
        "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=".to_string(),
    )
}

#[test]
fn test_android_only_roundtrip() {
    let lock = FlutterLockfile {
        android: Some(AndroidSection {
            nodes: vec![make_node("junit:junit:4.13.2")],
        }),
        ios: None,
    };
    let json = serde_json::to_string(&lock).unwrap();
    let decoded: FlutterLockfile = serde_json::from_str(&json).unwrap();
    assert_eq!(lock, decoded);
}

#[test]
fn test_empty_lockfile_roundtrip() {
    let lock = FlutterLockfile {
        android: None,
        ios: None,
    };
    let json = serde_json::to_string(&lock).unwrap();
    let decoded: FlutterLockfile = serde_json::from_str(&json).unwrap();
    assert_eq!(lock, decoded);
}

#[test]
fn test_android_absent_means_no_field_in_json() {
    let lock = FlutterLockfile {
        android: None,
        ios: None,
    };
    let json = serde_json::to_string(&lock).unwrap();
    assert!(
        !json.contains("android"),
        "android field must be absent when None, got: {json}"
    );
    assert!(
        !json.contains("ios"),
        "ios field must be absent when None, got: {json}"
    );
}

#[test]
fn test_android_present_serializes_nodes() {
    let lock = FlutterLockfile {
        android: Some(AndroidSection {
            nodes: vec![make_node("com.example:lib:1.0")],
        }),
        ios: None,
    };
    let json = serde_json::to_string(&lock).unwrap();
    assert!(json.contains("android"), "android field must be present");
    assert!(
        json.contains("com.example:lib:1.0"),
        "node name must be present"
    );
    assert!(!json.contains("\"ios\""), "ios field must not appear");
}

#[test]
fn test_write_read_roundtrip() {
    let lock = FlutterLockfile {
        android: Some(AndroidSection {
            nodes: vec![
                make_node("junit:junit:4.13.2"),
                make_node("com.google.guava:guava:31.1-jre"),
            ],
        }),
        ios: None,
    };
    let tmp = tempfile::NamedTempFile::new().unwrap();
    write_lockfile(tmp.path(), &lock).unwrap();
    let read_back = read_lockfile(tmp.path()).unwrap();
    assert_eq!(lock, read_back);
}

#[test]
fn test_read_missing_file_errors() {
    let result = read_lockfile(std::path::Path::new("/nonexistent/flutter2nix.lock"));
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("flutter2nix.lock"),
        "error must mention file path: {msg}"
    );
}

#[test]
fn test_ios_only_roundtrip() {
    let lock = FlutterLockfile {
        android: None,
        ios: Some(IosSection {
            nodes: vec![LockedDependency::new(
                "Firebase".to_string(),
                "10.0.0".to_string(),
                "https://github.com/firebase/firebase-ios-sdk/releases/download/10.0.0/firebase_core.zip".to_string(),
                "cafebabe1234567890abcdefdeadbeefcafebabe1234567890abcdef12345678".to_string(),
            )],
        }),
    };
    let json = serde_json::to_string(&lock).unwrap();
    let decoded: FlutterLockfile = serde_json::from_str(&json).unwrap();
    assert_eq!(lock, decoded);
}

#[test]
fn test_ios_absent_means_no_field_in_json() {
    let lock = FlutterLockfile {
        android: None,
        ios: None,
    };
    let json = serde_json::to_string(&lock).unwrap();
    assert!(
        !json.contains("\"ios\""),
        "ios field must be absent when None, got: {json}"
    );
}

#[test]
fn test_ios_present_serializes_nodes() {
    let lock = FlutterLockfile {
        android: None,
        ios: Some(IosSection {
            nodes: vec![LockedDependency::new(
                "firebase_core".to_string(),
                "10.0.0".to_string(),
                "https://example.com/firebase_core.zip".to_string(),
                "cafebabe1234567890abcdefdeadbeefcafebabe1234567890abcdef12345678".to_string(),
            )],
        }),
    };
    let json = serde_json::to_string(&lock).unwrap();
    assert!(json.contains("\"ios\""), "ios field must be present");
    assert!(json.contains("firebase_core"), "pod name must be present");
    assert!(
        !json.contains("\"android\""),
        "android field must not appear"
    );
}

#[test]
fn test_android_and_ios_roundtrip() {
    let lock = FlutterLockfile {
        android: Some(AndroidSection {
            nodes: vec![make_node("junit:junit:4.13.2")],
        }),
        ios: Some(IosSection {
            nodes: vec![LockedDependency::new(
                "Firebase".to_string(),
                "10.0.0".to_string(),
                "https://github.com/firebase/firebase-ios-sdk/releases/download/10.0.0/firebase.zip".to_string(),
                "cafebabe1234567890abcdefdeadbeefcafebabe1234567890abcdef12345678".to_string(),
            )],
        }),
    };
    let json = serde_json::to_string(&lock).unwrap();
    let decoded: FlutterLockfile = serde_json::from_str(&json).unwrap();
    assert_eq!(lock, decoded);
    assert!(json.contains("\"android\""));
    assert!(json.contains("\"ios\""));
}
