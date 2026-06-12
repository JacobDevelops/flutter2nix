use anyhow::Context;
use std::path::{Path, PathBuf};

pub struct LockCommand {
    pub project_dir: PathBuf,
    pub output: Option<PathBuf>,
    pub repositories: Option<Vec<String>>,
    pub gradle_cache_dir: Option<PathBuf>,
    pub gradle_user_home: Option<PathBuf>,
    pub timeout_secs: u64,
    pub shim_timeout_secs: u64,
}

/// Resolve the project's dependency graph into a FlutterLockfile (in memory).
/// Shared by `lock::run` (which writes it) and `check::run` (which diffs it).
pub async fn generate_lockfile(
    project_dir: &Path,
    repositories: &[String],
    gradle_cache_dir: Option<&Path>,
    gradle_user_home: Option<&Path>,
    timeout_secs: u64,
    shim_timeout_secs: u64,
) -> anyhow::Result<crate::lockfile::FlutterLockfile> {
    anyhow::ensure!(
        crate::detect::detect_flutter_project(project_dir),
        "not a Flutter project: pubspec.yaml not found in '{}'",
        project_dir.display()
    );

    let android_dir = project_dir.join("android");
    let android_section = if crate::detect::detect_android(project_dir) {
        let graph = gradle2nix::cli::lock::build_dependency_graph(
            &android_dir,
            repositories,
            gradle_cache_dir,
            gradle_user_home,
            timeout_secs,
            shim_timeout_secs,
        )
        .await
        .with_context(|| {
            format!(
                "resolving Android dependencies in '{}'",
                android_dir.display()
            )
        })?;
        Some(crate::lockfile::AndroidSection { nodes: graph.nodes })
    } else {
        None
    };

    let ios_dir = project_dir.join("ios");
    let ios_section = if crate::detect::detect_ios(project_dir) {
        // CocoaPods resolution must not use the Gradle cache.
        // TODO: future --ios-cache-dir flag for CocoaPods artifact caching
        let graph = ios2nix::cli::lock::build_dependency_graph(
            &ios_dir,
            &[],
            None,
            timeout_secs,
        )
        .await
        .with_context(|| format!("resolving iOS dependencies in '{}'", ios_dir.display()))?;
        Some(crate::lockfile::IosSection { nodes: graph.nodes })
    } else {
        None
    };

    Ok(crate::lockfile::FlutterLockfile {
        android: android_section,
        ios: ios_section,
    })
}

pub async fn run(cmd: LockCommand) -> anyhow::Result<()> {
    let lock = generate_lockfile(
        &cmd.project_dir,
        cmd.repositories.as_deref().unwrap_or(&[]),
        cmd.gradle_cache_dir.as_deref(),
        cmd.gradle_user_home.as_deref(),
        cmd.timeout_secs,
        cmd.shim_timeout_secs,
    )
    .await?;

    let output_path = cmd
        .output
        .unwrap_or_else(|| cmd.project_dir.join("flutter2nix.lock"));
    crate::lockfile::write_lockfile(&output_path, &lock).with_context(|| {
        format!(
            "writing flutter2nix lockfile to '{}'",
            output_path.display()
        )
    })?;

    println!("Wrote flutter2nix lockfile: {}", output_path.display());
    Ok(())
}
