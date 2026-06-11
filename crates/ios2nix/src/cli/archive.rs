use crate::export_opts::SigningConfig;
use clap::Parser;
use std::path::{Path, PathBuf};

#[derive(Parser, Debug, Clone)]
pub struct ArchiveArgs {
    /// Workspace path
    #[arg(long, default_value = "ios/Runner.xcworkspace")]
    pub workspace: PathBuf,

    /// Scheme to archive (default: "Runner")
    #[arg(long, default_value = "Runner")]
    pub scheme: String,

    /// Configuration to archive (default: "Release")
    #[arg(long, default_value = "Release")]
    pub configuration: String,

    /// Output .xcarchive path
    #[arg(long)]
    pub archive_path: PathBuf,
}

pub struct ArchiveCommand {
    pub workspace: PathBuf,
    pub scheme: String,
    pub configuration: String,
    pub archive_path: PathBuf,
    pub signing: Option<SigningConfig>,
}

fn xcodebuild_args(cmd: &ArchiveCommand) -> Vec<String> {
    let mut args = vec!["archive".to_string()];
    args.push("-workspace".to_string());
    args.push(cmd.workspace.to_string_lossy().to_string());
    args.push("-scheme".to_string());
    args.push(cmd.scheme.clone());
    args.push("-configuration".to_string());
    args.push(cmd.configuration.clone());
    args.push("-archivePath".to_string());
    args.push(cmd.archive_path.to_string_lossy().to_string());
    args.push("-destination".to_string());
    args.push("generic/platform=iOS".to_string());

    if cmd.signing.is_none() {
        args.push("CODE_SIGNING_ALLOWED=NO".to_string());
    }
    // Plan 3 §5a: signing flags when Some(_s) — unimplemented

    args
}

/// Verify .xcarchive structure: must contain Products/Applications/<exactly-one>.app/Info.plist.
/// Returns Ok(path to .app) on success.
pub fn verify_archive_structure(archive: &Path) -> anyhow::Result<PathBuf> {
    let apps_dir = archive.join("Products/Applications");
    if !apps_dir.exists() {
        anyhow::bail!("archive missing Products/Applications directory");
    }

    let mut app_dirs = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&apps_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && path.extension().is_some_and(|ext| ext == "app") {
                app_dirs.push(path);
            }
        }
    }

    if app_dirs.len() != 1 {
        anyhow::bail!(
            "archive must contain exactly one .app directory, found {}",
            app_dirs.len()
        );
    }

    let app_path = &app_dirs[0];
    let info_plist = app_path.join("Info.plist");
    if !info_plist.exists() {
        anyhow::bail!("archive .app missing Info.plist");
    }

    Ok(app_path.clone())
}

pub fn run(cmd: ArchiveCommand) -> anyhow::Result<PathBuf> {
    // Plan 3 §5a owns the manual-signing flag branch; archiving with signing
    // silently un-applied would produce a wrongly-signed archive.
    if cmd.signing.is_some() {
        anyhow::bail!("manual-signing archive flags are Plan 3 — unimplemented");
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
                "xcodebuild archive failed:\n{}",
                crate::cli::failure_detail(&output)
            );
        }

        verify_archive_structure(&cmd.archive_path)?;
        Ok(cmd.archive_path)
    }

    #[cfg(not(target_os = "macos"))]
    {
        anyhow::bail!("ios2nix archive requires macOS")
    }
}

#[cfg(test)]
#[path = "archive_tests.rs"]
mod tests;
