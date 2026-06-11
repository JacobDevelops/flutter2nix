use crate::export_opts::SigningConfig;
use std::path::PathBuf;

pub struct ArchiveCommand {
    pub workspace: PathBuf,
    pub scheme: String,
    pub configuration: String,
    pub archive_path: PathBuf,
    pub signing: Option<SigningConfig>,
}

pub fn run() -> anyhow::Result<()> {
    #[cfg(target_os = "macos")]
    {
        anyhow::bail!("ios2nix archive: not yet implemented (Plan 2)")
    }
    #[cfg(not(target_os = "macos"))]
    {
        anyhow::bail!("ios2nix archive requires macOS")
    }
}

#[cfg(test)]
#[path = "archive_tests.rs"]
mod tests;
