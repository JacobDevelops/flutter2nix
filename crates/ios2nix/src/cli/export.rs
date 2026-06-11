use std::path::PathBuf;

pub struct ExportCommand {
    pub archive_path: PathBuf,
    pub export_opts_plist: PathBuf,
    pub output_path: PathBuf,
}

pub fn run() -> anyhow::Result<()> {
    #[cfg(target_os = "macos")]
    {
        anyhow::bail!("ios2nix export: not yet implemented (Plan 2)")
    }
    #[cfg(not(target_os = "macos"))]
    {
        anyhow::bail!("ios2nix export requires macOS")
    }
}

#[cfg(test)]
#[path = "export_tests.rs"]
mod tests;
