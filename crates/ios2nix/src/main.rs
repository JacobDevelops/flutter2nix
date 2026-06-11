use clap::Parser;
use ios2nix::cli::{self, Args, Command};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    match args.command {
        Some(Command::Lock(lock_args)) => {
            let cmd = cli::lock::LockCommand {
                ios_dir: lock_args.ios_dir,
                output: lock_args.output,
                spec_repos: lock_args.spec_repo,
                cache_dir: lock_args.cache_dir,
                timeout_secs: lock_args.timeout_secs,
            };
            cli::lock::run(cmd).await
        }
        Some(Command::Check(check_args)) => {
            let cmd = cli::check::CheckCommand {
                ios_dir: check_args.ios_dir,
                lockfile: check_args.lockfile,
                spec_repos: check_args.spec_repo,
                cache_dir: check_args.cache_dir,
                timeout_secs: check_args.timeout_secs,
            };
            cli::check::run(cmd).await
        }
        Some(Command::Generate(gen_args)) => {
            let cmd = cli::generate::GenerateCommand {
                lockfile: gen_args.lockfile,
                output: gen_args.output,
                format: gen_args.format,
            };
            cli::generate::run(cmd)
        }
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
#[path = "main_tests.rs"]
mod tests;
