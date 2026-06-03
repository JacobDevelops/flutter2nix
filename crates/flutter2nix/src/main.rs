#![allow(dead_code)]

use clap::{Parser, Subcommand};

mod cli;
mod compose;
mod detect;
mod pub_deps;
mod sdk;

#[derive(Parser)]
#[command(name = "flutter2nix", about = "Flutter integration layer for reproducible Nix builds")]
struct Args {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Generate flutter2nix.nix unified lockfile
    Lock,
    /// Build the Flutter app via Nix
    Build,
    /// Verify flutter2nix.nix is current (exits non-zero if stale)
    Check,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    match args.command {
        Some(Command::Lock) => cli::lock::run(),
        Some(Command::Build) => cli::build::run(),
        Some(Command::Check) => cli::check::run(),
        None => {
            println!("flutter2nix: use --help for available subcommands");
            Ok(())
        }
    }
}
