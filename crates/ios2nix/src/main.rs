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
        Some(Command::Build(build_args)) => {
            let workspace = build_args
                .workspace
                .unwrap_or_else(|| build_args.project_dir.join("Runner.xcworkspace"));
            let cmd = cli::build::BuildCommand {
                project_dir: build_args.project_dir,
                workspace,
                scheme: build_args.scheme,
                configuration: build_args.configuration,
                derived_data: build_args.derived_data,
            };
            match cli::build::run(cmd) {
                Ok(output) => {
                    println!(
                        "version={} architectures={:?}",
                        output.version, output.architectures
                    );
                    Ok(())
                }
                Err(e) => Err(e),
            }
        }
        Some(Command::Archive(archive_args)) => {
            let cmd = cli::archive::ArchiveCommand {
                workspace: archive_args.workspace,
                scheme: archive_args.scheme,
                configuration: archive_args.configuration,
                archive_path: archive_args.archive_path,
                signing: None,
            };
            match cli::archive::run(cmd) {
                Ok(archive_path) => {
                    println!("{}", archive_path.display());
                    Ok(())
                }
                Err(e) => Err(e),
            }
        }
        Some(Command::Export(export_args)) => {
            let cmd = cli::export::ExportCommand {
                archive_path: export_args.archive_path,
                export_opts_plist: export_args.export_opts_plist,
                output_path: export_args.output_path,
            };
            match cli::export::run(cmd) {
                Ok(ipa_path) => {
                    println!("{}", ipa_path.display());
                    Ok(())
                }
                Err(e) => Err(e),
            }
        }
        Some(Command::Sign(_)) => cli::sign::run(),
        None => {
            println!("ios2nix: use --help for available subcommands");
            Ok(())
        }
    }
}

#[cfg(test)]
#[path = "main_tests.rs"]
mod tests;
