use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
pub struct CheckArgs {
    /// iOS project directory
    #[arg(long, default_value = ".")]
    pub ios_dir: PathBuf,

    /// Path to existing lockfile
    #[arg(long)]
    pub lockfile: Option<PathBuf>,

    /// CocoaPods spec repositories (repeatable)
    #[arg(long)]
    pub spec_repo: Option<Vec<String>>,

    /// Cache directory for pod resolutions
    #[arg(long)]
    pub cache_dir: Option<PathBuf>,

    /// Timeout in seconds for HTTP requests
    #[arg(long, default_value = "600")]
    pub timeout_secs: u64,
}

pub struct CheckCommand {
    pub ios_dir: PathBuf,
    pub lockfile: Option<PathBuf>,
    pub spec_repos: Option<Vec<String>>,
    pub cache_dir: Option<PathBuf>,
    pub timeout_secs: u64,
}

pub async fn run(cmd: CheckCommand) -> anyhow::Result<()> {
    let repos = cmd.spec_repos.as_deref().unwrap_or(&[]);
    let fresh = crate::cli::lock::build_dependency_graph(
        &cmd.ios_dir,
        repos,
        cmd.cache_dir.as_deref(),
        cmd.timeout_secs,
    )
    .await?;

    let lockfile_path = cmd
        .lockfile
        .unwrap_or_else(|| cmd.ios_dir.join("ios2nix.lock"));

    if !lockfile_path.exists() {
        anyhow::bail!(
            "lockfile not found at {} — run `ios2nix lock` to create it",
            lockfile_path.display()
        );
    }

    let on_disk = nix_core::lockfile::read_lockfile(&lockfile_path)?;
    let diff = nix_core::lockfile::diff_lockfiles(&on_disk, &fresh);

    if diff.is_empty() {
        Ok(())
    } else {
        eprintln!("Lockfile is stale:\n{diff}");
        anyhow::bail!("lockfile is stale — run `ios2nix lock` to update")
    }
}

#[cfg(test)]
#[path = "check_tests.rs"]
mod tests;
