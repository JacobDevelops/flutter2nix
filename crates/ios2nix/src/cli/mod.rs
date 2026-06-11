pub mod archive;
pub mod build;
pub mod check;
pub mod export;
pub mod generate;
pub mod lock;
pub mod sign;
pub mod sign_setup;

use clap::{Parser, Subcommand};

/// Failure detail for a spawned xcodebuild: last stdout lines + stderr —
/// xcodebuild reports most build errors on stdout, not stderr.
pub(crate) fn failure_detail(output: &std::process::Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout_tail: Vec<&str> = {
        let lines: Vec<&str> = stdout.lines().collect();
        lines[lines.len().saturating_sub(40)..].to_vec()
    };
    format!("{}\n{}", stdout_tail.join("\n"), stderr.trim())
}

#[derive(Parser)]
#[command(
    name = "ios2nix",
    about = "iOS/Xcode orchestration for reproducible Nix builds"
)]
pub struct Args {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand)]
pub enum Command {
    /// Generate pods.nix from Podfile.lock
    Lock(lock::LockArgs),
    /// Check if lockfile is current
    Check(check::CheckArgs),
    /// Generate Nix expressions from lockfile
    Generate(generate::GenerateArgs),
    /// Build the iOS project
    Build(build::BuildArgs),
    /// Create an .xcarchive
    Archive(archive::ArchiveArgs),
    /// Export an .ipa from an .xcarchive (signing is Plan 3)
    Export(export::ExportArgs),
    /// Set up signing: create temp keychain, import identity, install profile
    #[command(name = "sign-setup")]
    SignSetup(sign_setup::SignSetupArgs),
    /// Sign an existing .ipa
    Sign(sign::SignArgs),
}

#[cfg(test)]
mod tests;
