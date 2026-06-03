#![allow(dead_code)]

use clap::{Parser, Subcommand};
use std::path::PathBuf;

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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    match args.command {
        Some(Command::Lock) => {
            cli::lock::run(cli::lock::LockCommand {
                gradle_dir: PathBuf::from("."),
                output: None,
                repositories: None,
                timeout_secs: 60,
            })
            .await
        }
        Some(Command::Check) => {
            cli::check::run(cli::check::CheckCommand {
                gradle_dir: PathBuf::from("."),
                lockfile: None,
                repositories: None,
                timeout_secs: 60,
            })
            .await
        }
        Some(Command::Generate) => cli::generate::run(cli::generate::GenerateCommand {
            lockfile: None,
            output: None,
            format: cli::generate::NixFormat::Inline,
        }),
        None => {
            println!("gradle2nix: use --help for available subcommands");
            Ok(())
        }
    }
}
