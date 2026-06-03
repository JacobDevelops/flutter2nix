use std::path::PathBuf;

pub struct LockCommand {
    pub gradle_dir: PathBuf,
    pub output: Option<PathBuf>,
    pub repositories: Option<Vec<String>>,
    pub timeout_secs: u64,
}

/// Flow: TAPI shim → parse JSON → convert to MavenCoordinates → resolve SHA-256 → build DependencyGraph → write lockfile
pub async fn run(_cmd: LockCommand) -> anyhow::Result<()> {
    todo!("Phase 1: API contract only")
}
