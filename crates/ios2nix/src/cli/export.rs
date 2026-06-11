use clap::Parser;
use std::path::{Path, PathBuf};

#[derive(Parser, Debug, Clone)]
pub struct ExportArgs {
    /// Path to .xcarchive
    #[arg(long)]
    pub archive_path: PathBuf,

    /// Path to ExportOptions.plist
    #[arg(long)]
    pub export_opts_plist: PathBuf,

    /// Output directory for .ipa
    #[arg(long)]
    pub output_path: PathBuf,
}

pub struct ExportCommand {
    pub archive_path: PathBuf,
    pub export_opts_plist: PathBuf,
    pub output_path: PathBuf,
}

fn xcodebuild_args(cmd: &ExportCommand) -> Vec<String> {
    vec![
        "-exportArchive".to_string(),
        "-archivePath".to_string(),
        cmd.archive_path.to_string_lossy().to_string(),
        "-exportOptionsPlist".to_string(),
        cmd.export_opts_plist.to_string_lossy().to_string(),
        "-exportPath".to_string(),
        cmd.output_path.to_string_lossy().to_string(),
    ]
}

pub fn run(cmd: ExportCommand) -> anyhow::Result<PathBuf> {
    // Validate archive exists (not cfg-gated — needed on all platforms for tests)
    if !cmd.archive_path.exists() {
        anyhow::bail!("archive not found: {}", cmd.archive_path.display());
    }

    // Validate export options plist exists (not cfg-gated)
    if !cmd.export_opts_plist.exists() {
        anyhow::bail!(
            "export options plist not found: {}",
            cmd.export_opts_plist.display()
        );
    }

    #[cfg(target_os = "macos")]
    {
        let env = crate::xcode::env::setup_xcode_env()?;

        let mut xcode_cmd = std::process::Command::new("xcodebuild");
        env.apply_to(&mut xcode_cmd);
        xcode_cmd.args(xcodebuild_args(&cmd));

        let output = xcode_cmd
            .output()
            .map_err(|e| anyhow::anyhow!("failed to run xcodebuild: {}", e))?;

        if !output.status.success() {
            anyhow::bail!(
                "xcodebuild export failed:\n{}",
                crate::cli::failure_detail(&output)
            );
        }

        // Locate the single .ipa under output_path.
        let ipa = find_ipa_in_dir(&cmd.output_path)?;
        Ok(ipa)
    }

    #[cfg(not(target_os = "macos"))]
    {
        anyhow::bail!("ios2nix export requires macOS")
    }
}

fn find_ipa_in_dir(dir: &Path) -> anyhow::Result<PathBuf> {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "ipa") {
                return Ok(path);
            }
        }
    }
    anyhow::bail!("no .ipa file found in {}", dir.display())
}

#[cfg(test)]
#[path = "export_tests.rs"]
mod tests;
