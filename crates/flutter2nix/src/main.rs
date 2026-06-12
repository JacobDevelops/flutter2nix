use clap::{Parser, Subcommand};
use flutter2nix::cli;
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "flutter2nix",
    about = "Flutter integration layer for reproducible Nix builds"
)]
struct Args {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Generate flutter2nix.lock unified lockfile
    Lock(LockArgs),
    /// Verify flutter2nix.lock is current (exits non-zero if stale)
    Check(CheckArgs),
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

    /// Explicit Gradle user home for the TAPI shim and cache-discovery phases
    /// (defaults to GRADLE_USER_HOME / ~/.gradle)
    #[arg(long)]
    gradle_user_home: Option<PathBuf>,

    /// Timeout in seconds for per-HTTP-request operations
    #[arg(long, default_value = "60")]
    timeout_secs: u64,

    /// Timeout in seconds for entire TAPI shim extraction run
    #[arg(long, default_value = "1800")]
    shim_timeout_secs: u64,
}

#[derive(Parser)]
struct CheckArgs {
    /// Flutter project directory
    #[arg(long, default_value = ".")]
    project_dir: PathBuf,

    /// Lockfile to verify (defaults to flutter2nix.lock in project-dir)
    #[arg(long)]
    lockfile: Option<PathBuf>,

    /// Additional Maven repository URLs (comma-separated)
    #[arg(long)]
    repositories: Option<String>,

    /// Gradle cache directory for local artifact lookups (used in tests)
    #[arg(long)]
    gradle_cache_dir: Option<PathBuf>,

    /// Explicit Gradle user home for the TAPI shim and cache-discovery phases
    /// (defaults to GRADLE_USER_HOME / ~/.gradle)
    #[arg(long)]
    gradle_user_home: Option<PathBuf>,

    /// Timeout in seconds for per-HTTP-request operations
    #[arg(long, default_value = "60")]
    timeout_secs: u64,

    /// Timeout in seconds for entire TAPI shim extraction run
    #[arg(long, default_value = "1800")]
    shim_timeout_secs: u64,
}

fn parse_repositories(repositories: Option<String>) -> Option<Vec<String>> {
    repositories.map(|repos| repos.split(',').map(|s| s.trim().to_string()).collect())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    match args.command {
        Some(Command::Lock(lock_args)) => {
            cli::lock::run(cli::lock::LockCommand {
                project_dir: lock_args.project_dir,
                output: lock_args.output,
                repositories: parse_repositories(lock_args.repositories),
                gradle_cache_dir: lock_args.gradle_cache_dir,
                gradle_user_home: lock_args.gradle_user_home,
                timeout_secs: lock_args.timeout_secs,
                shim_timeout_secs: lock_args.shim_timeout_secs,
            })
            .await
        }
        Some(Command::Check(check_args)) => {
            cli::check::run(cli::check::CheckCommand {
                project_dir: check_args.project_dir,
                lockfile: check_args.lockfile,
                repositories: parse_repositories(check_args.repositories),
                gradle_cache_dir: check_args.gradle_cache_dir,
                gradle_user_home: check_args.gradle_user_home,
                timeout_secs: check_args.timeout_secs,
                shim_timeout_secs: check_args.shim_timeout_secs,
            })
            .await
        }
        None => {
            println!("flutter2nix: use --help for available subcommands");
            Ok(())
        }
    }
}
