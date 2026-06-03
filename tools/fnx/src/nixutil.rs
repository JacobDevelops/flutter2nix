use std::path::PathBuf;

use anyhow::{bail, Context};

/// Walk up from cwd to find the repo root.
/// Looks for flake.nix first, then .git as fallback.
pub fn find_repo_root() -> anyhow::Result<PathBuf> {
    let cwd = std::env::current_dir().context("failed to get current directory")?;

    for marker in &["flake.nix", ".git"] {
        let mut dir = cwd.clone();
        loop {
            if dir.join(marker).exists() {
                return Ok(dir);
            }
            match dir.parent() {
                Some(parent) => dir = parent.to_path_buf(),
                None => break,
            }
        }
    }

    bail!("repo root not found: no flake.nix or .git in any ancestor directory")
}
