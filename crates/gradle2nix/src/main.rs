use clap::{Parser, Subcommand};
use gradle2nix::cli;
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "gradle2nix",
    about = "Gradle/Maven dependency materialiser for Nix"
)]
struct Args {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Generate gradle.nix lockfile from a Gradle project
    Lock(LockArgs),
    /// Verify gradle.nix is current (exits non-zero if stale)
    Check(CheckArgs),
    /// Generate Nix expressions from an existing lockfile
    Generate(GenerateArgs),
}

#[derive(Parser)]
struct LockArgs {
    /// Gradle project directory to lock
    #[arg(long, default_value = ".")]
    project_dir: PathBuf,

    /// Output path for lockfile (defaults to gradle.nix in project-dir)
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
    /// Gradle project directory
    #[arg(long, default_value = ".")]
    project_dir: PathBuf,

    /// Path to lockfile to check (defaults to gradle.nix in project-dir)
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

#[derive(Parser)]
struct GenerateArgs {
    /// Path to gradle.nix lockfile
    #[arg(long)]
    lockfile: Option<PathBuf>,

    /// Output path for Nix expressions (prints to stdout if not specified)
    #[arg(long, short)]
    output: Option<PathBuf>,

    /// Output format (inline or flake)
    #[arg(long, default_value = "inline")]
    format: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    match args.command {
        Some(Command::Lock(lock_args)) => {
            let repositories = lock_args
                .repositories
                .map(|repos| repos.split(',').map(|s| s.trim().to_string()).collect());

            cli::lock::run(cli::lock::LockCommand {
                gradle_dir: lock_args.project_dir,
                output: lock_args.output,
                repositories,
                gradle_cache_dir: lock_args.gradle_cache_dir,
                gradle_user_home: lock_args.gradle_user_home,
                timeout_secs: lock_args.timeout_secs,
                shim_timeout_secs: lock_args.shim_timeout_secs,
            })
            .await
        }
        Some(Command::Check(check_args)) => {
            let repositories = check_args
                .repositories
                .map(|repos| repos.split(',').map(|s| s.trim().to_string()).collect());

            cli::check::run(cli::check::CheckCommand {
                gradle_dir: check_args.project_dir,
                lockfile: check_args.lockfile,
                repositories,
                gradle_cache_dir: check_args.gradle_cache_dir,
                gradle_user_home: check_args.gradle_user_home,
                timeout_secs: check_args.timeout_secs,
                shim_timeout_secs: check_args.shim_timeout_secs,
            })
            .await
        }
        Some(Command::Generate(gen_args)) => {
            let format = match gen_args.format.as_str() {
                "flake" => cli::generate::NixFormat::Flake,
                _ => cli::generate::NixFormat::Inline,
            };

            cli::generate::run(cli::generate::GenerateCommand {
                lockfile: gen_args.lockfile,
                output: gen_args.output,
                format,
            })
        }
        None => {
            println!("gradle2nix: use --help for available subcommands");
            Ok(())
        }
    }
}
