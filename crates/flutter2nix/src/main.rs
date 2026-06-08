use clap::{Parser, Subcommand};
use flutter2nix::cli;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "flutter2nix", about = "Flutter integration layer for reproducible Nix builds")]
struct Args {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Generate flutter2nix.lock unified lockfile
    Lock(LockArgs),
    /// Build the Flutter app via Nix
    Build,
    /// Verify flutter2nix.lock is current (exits non-zero if stale)
    Check,
}

#[derive(Parser)]
struct LockArgs {
    /// Flutter project directory
    #[arg(long, default_value = ".")]
    project_dir: PathBuf,

    /// Output path for lockfile (defaults to flutter2nix.lock in project-dir)
    #[arg(long, short)]
    output: Option<PathBuf>,

    /// Additional Maven repository URLs (comma-separated)
    #[arg(long)]
    repositories: Option<String>,

    /// Gradle cache directory for local artifact lookups (used in tests)
    #[arg(long)]
    gradle_cache_dir: Option<PathBuf>,

    /// Timeout in seconds for network requests
    #[arg(long, default_value = "60")]
    timeout_secs: u64,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    match args.command {
        Some(Command::Lock(lock_args)) => {
            let repositories = lock_args.repositories.map(|repos| {
                repos.split(',').map(|s| s.trim().to_string()).collect()
            });
            cli::lock::run(cli::lock::LockCommand {
                project_dir: lock_args.project_dir,
                output: lock_args.output,
                repositories,
                gradle_cache_dir: lock_args.gradle_cache_dir,
                timeout_secs: lock_args.timeout_secs,
            })
            .await
        }
        Some(Command::Build) => cli::build::run(),
        Some(Command::Check) => cli::check::run(),
        None => {
            println!("flutter2nix: use --help for available subcommands");
            Ok(())
        }
    }
}
