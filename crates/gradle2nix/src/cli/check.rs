use std::path::PathBuf;

pub struct CheckCommand {
    pub gradle_dir: PathBuf,
    pub lockfile: Option<PathBuf>,
    pub repositories: Option<Vec<String>>,
    pub timeout_secs: u64,
}

/// Flow: run lock pipeline in memory → read on-disk lockfile → diff → exit 0 if fresh, 1 if stale
pub async fn run(_cmd: CheckCommand) -> anyhow::Result<()> {
    todo!("Phase 1: API contract only")
}
