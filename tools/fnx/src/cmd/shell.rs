use std::process::Command;

use crate::nixutil;

pub fn run() -> anyhow::Result<()> {
    let repo_root = nixutil::find_repo_root()?;
    let status = Command::new("devenv")
        .arg("shell")
        .current_dir(&repo_root)
        .status()?;
    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }
    Ok(())
}
