use std::path::PathBuf;
use std::process::Command;

use clap::Args;

use crate::nixutil;

#[derive(Args)]
pub struct LockArgs {
    /// Gradle project directory to lock
    #[arg(long)]
    pub project_dir: Option<PathBuf>,

    /// Output path for the lockfile (defaults to flutter2nix.lock in project-dir)
    #[arg(long, short = 'o')]
    pub output: Option<PathBuf>,
}

pub fn run(args: LockArgs) -> anyhow::Result<()> {
    let repo_root = nixutil::find_repo_root()?;

    let project_dir = args.project_dir.unwrap_or_else(|| {
        repo_root.join("tests/fixtures/flutter/minimal-app/android")
    });

    let output = args.output.unwrap_or_else(|| {
        project_dir.join("flutter2nix.lock")
    });

    let status = Command::new("cargo")
        .args([
            "run", "-p", "gradle2nix", "--",
            "lock",
            "--project-dir", project_dir.to_str().unwrap(),
            "--output", output.to_str().unwrap(),
        ])
        .current_dir(&repo_root)
        .status()?;

    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }

    Ok(())
}
