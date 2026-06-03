#![allow(dead_code)]

use clap::{Parser, Subcommand};

mod cli;
mod lockfile;
mod maven;
mod tapi;

#[derive(Parser)]
#[command(name = "gradle2nix", about = "Gradle/Maven dependency materialiser for Nix")]
struct Args {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Generate gradle.nix lockfile from a Gradle project
    Lock,
    /// Verify gradle.nix is current (exits non-zero if stale)
    Check,
    /// Generate Nix expressions from an existing lockfile
    Generate,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    match args.command {
        Some(Command::Lock) => cli::lock::run(),
        Some(Command::Check) => cli::check::run(),
        Some(Command::Generate) => cli::generate::run(),
        None => {
            println!("gradle2nix: use --help for available subcommands");
            Ok(())
        }
    }
}
