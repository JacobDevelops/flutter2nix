use std::path::Path;

pub fn detect_flutter_project(project_dir: &Path) -> bool {
    project_dir.join("pubspec.yaml").exists()
}

pub fn detect_android(project_dir: &Path) -> bool {
    project_dir.join("android").is_dir()
}

pub fn detect_ios(project_dir: &Path) -> bool {
    project_dir.join("ios").is_dir() && project_dir.join("ios/Podfile.lock").exists()
}
