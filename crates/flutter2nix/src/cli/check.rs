use std::path::PathBuf;

pub struct CheckCommand {
    pub project_dir: PathBuf,
    pub lockfile: Option<PathBuf>,
    pub repositories: Option<Vec<String>>,
    pub gradle_cache_dir: Option<PathBuf>,
    pub gradle_user_home: Option<PathBuf>,
    pub timeout_secs: u64,
    pub shim_timeout_secs: u64,
}

/// Flow: regenerate the lockfile in memory → read the on-disk flutter2nix.lock →
/// compare → exit 0 if fresh, non-zero if stale. Mirrors `gradle2nix check`.
pub async fn run(cmd: CheckCommand) -> anyhow::Result<()> {
    let fresh = crate::cli::lock::generate_lockfile(
        &cmd.project_dir,
        cmd.repositories.as_deref().unwrap_or(&[]),
        cmd.gradle_cache_dir.as_deref(),
        cmd.gradle_user_home.as_deref(),
        cmd.timeout_secs,
        cmd.shim_timeout_secs,
    )
    .await?;

    let lockfile_path = cmd
        .lockfile
        .unwrap_or_else(|| cmd.project_dir.join("flutter2nix.lock"));
    let on_disk = crate::lockfile::read_lockfile(&lockfile_path)?;

    if on_disk == fresh {
        Ok(())
    } else {
        anyhow::bail!(
            "lockfile '{}' is stale — run `flutter2nix lock` to update",
            lockfile_path.display()
        )
    }
}
