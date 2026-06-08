use anyhow::Context;
use std::path::PathBuf;

pub struct LockCommand {
    pub project_dir: PathBuf,
    pub output: Option<PathBuf>,
    pub repositories: Option<Vec<String>>,
    pub gradle_cache_dir: Option<PathBuf>,
    pub timeout_secs: u64,
}

pub async fn run(cmd: LockCommand) -> anyhow::Result<()> {
    let project_dir = &cmd.project_dir;

    anyhow::ensure!(
        crate::detect::detect_flutter_project(project_dir),
        "not a Flutter project: pubspec.yaml not found in '{}'",
        project_dir.display()
    );

    let repos = cmd.repositories.as_deref().unwrap_or(&[]);

    let android_dir = project_dir.join("android");
    let android_section = if crate::detect::detect_android(project_dir) {
        let graph = gradle2nix::cli::lock::build_dependency_graph(
            &android_dir,
            repos,
            cmd.gradle_cache_dir.as_deref(),
            cmd.timeout_secs,
        )
        .await
        .with_context(|| {
            format!("resolving Android dependencies in '{}'", android_dir.display())
        })?;
        Some(crate::lockfile::AndroidSection { nodes: graph.nodes })
    } else {
        None
    };

    let lock = crate::lockfile::FlutterLockfile { android: android_section };

    let output_path = cmd
        .output
        .unwrap_or_else(|| project_dir.join("flutter2nix.lock"));
    crate::lockfile::write_lockfile(&output_path, &lock)
        .with_context(|| format!("writing flutter2nix lockfile to '{}'", output_path.display()))?;

    println!("Wrote flutter2nix lockfile: {}", output_path.display());
    Ok(())
}
