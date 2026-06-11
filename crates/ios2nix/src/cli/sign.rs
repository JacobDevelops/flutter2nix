use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug, Clone)]
pub struct SignArgs {
    /// Path to .ipa to sign
    #[arg(long)]
    pub ipa_path: PathBuf,
}

pub fn run() -> anyhow::Result<()> {
    #[cfg(target_os = "macos")]
    {
        anyhow::bail!("ios2nix sign: not yet implemented (Plan 3)")
    }
    #[cfg(not(target_os = "macos"))]
    {
        anyhow::bail!("ios2nix sign requires macOS")
    }
}

#[cfg(test)]
#[path = "sign_tests.rs"]
mod tests;
