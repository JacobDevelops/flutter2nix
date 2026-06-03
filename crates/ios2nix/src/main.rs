#![allow(dead_code)]

use clap::{Parser, Subcommand};

mod cli;
mod cocoapods;
mod export_opts;
mod keychain;
mod xcode;

#[derive(Parser)]
#[command(name = "ios2nix", about = "iOS/Xcode orchestration for reproducible Nix builds")]
struct Args {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Generate pods.nix from Podfile.lock
    Lock,
    /// Build the iOS project
    Build,
    /// Create an .xcarchive
    Archive,
    /// Export a signed .ipa from an .xcarchive
    Export,
    /// Sign an existing .ipa
    Sign,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    match args.command {
        Some(Command::Lock) => cli::lock::run(),
        Some(Command::Build) => cli::build::run(),
        Some(Command::Archive) => cli::archive::run(),
        Some(Command::Export) => cli::export::run(),
        Some(Command::Sign) => cli::sign::run(),
        None => {
            println!("ios2nix: use --help for available subcommands");
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_main_entrypoint_ok() {
        todo!("Phase 1: stub — expect: main() returns Ok(()) when called with valid subcommand args")
    }
}
