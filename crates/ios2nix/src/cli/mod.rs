pub mod archive;
pub mod build;
pub mod check;
pub mod export;
pub mod generate;
pub mod lock;
pub mod sign;

use clap::{Parser, Subcommand};

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
    Build,
    /// Create an .xcarchive
    Archive,
    /// Export a signed .ipa from an .xcarchive
    Export,
    /// Sign an existing .ipa
    Sign,
}

#[cfg(test)]
mod tests;
