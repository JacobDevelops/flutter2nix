use std::path::PathBuf;

pub struct CheckCommand {
    pub gradle_dir: PathBuf,
    pub lockfile: Option<PathBuf>,
    pub repositories: Option<Vec<String>>,
    /// Local Maven repo directory for sha256 lookup (used in tests; None in production).
    pub gradle_cache_dir: Option<PathBuf>,
    pub gradle_user_home: Option<PathBuf>,
    pub timeout_secs: u64,
}

/// Flow: run lock pipeline in memory → read on-disk lockfile → diff → exit 0 if fresh, 1 if stale
pub async fn run(cmd: CheckCommand) -> anyhow::Result<()> {
    let repos = cmd.repositories.as_deref().unwrap_or(&[]);
    let fresh = crate::cli::lock::build_dependency_graph(
        &cmd.gradle_dir,
        repos,
        cmd.gradle_cache_dir.as_deref(),
        cmd.gradle_user_home.as_deref(),
        cmd.timeout_secs,
    )
    .await?;

    let lockfile_path = cmd
        .lockfile
        .unwrap_or_else(|| cmd.gradle_dir.join("gradle2nix.lock"));

    if lockfile_path.extension().and_then(|e| e.to_str()) == Some("json") {
        eprintln!("warning: .json lockfile extension is deprecated; rename to .lock");
    }

    let on_disk = nix_core::lockfile::read_lockfile(&lockfile_path)?;
    let diff = nix_core::lockfile::diff_lockfiles(&on_disk, &fresh);

    if diff.is_empty() {
        Ok(())
    } else {
        eprintln!("Lockfile is stale:\n{diff}");
        anyhow::bail!("lockfile is stale — run `gradle2nix lock` to update")
    }
}
