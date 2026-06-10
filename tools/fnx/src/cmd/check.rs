use std::process::Command;

use clap::Args;

use crate::nixutil;

#[derive(Args)]
pub struct CheckArgs {
    /// Run cargo clippy instead of nix flake check
    #[arg(long)]
    pub cargo: bool,

    /// Filter to a specific crate (only with --cargo)
    #[arg(long)]
    pub crate_name: Option<String>,
}

pub fn run(args: CheckArgs) -> anyhow::Result<()> {
    let repo_root = nixutil::find_repo_root()?;

    if args.cargo {
        let mut cmd = Command::new("cargo");
        cmd.arg("clippy").current_dir(&repo_root);

        if let Some(ref name) = args.crate_name {
            cmd.args(["-p", name]);
        } else {
            cmd.arg("--workspace");
        }

        cmd.args(["--", "-D", "warnings"]);

        let status = cmd.status()?;
        if !status.success() {
            std::process::exit(status.code().unwrap_or(1));
        }
    } else {
        let status = Command::new("nix")
            .args(["flake", "check", "--print-build-logs"])
            .current_dir(&repo_root)
            .status()?;

        if !status.success() {
            std::process::exit(status.code().unwrap_or(1));
        }

        // fnx is a local-dev tool and never runs in CI, so it also runs the e2e suite
        // that is deliberately kept out of `nix flake check` (the e2e builds realise the
        // full Android SDK + NDK + Flutter SDK, which overflows GitHub runner disk).
        // `.#e2e` aggregates every e2e test; it is an empty no-op on non-Linux or when
        // the fixture lockfile is absent. `--no-link` avoids clobbering ./result.
        eprintln!("fnx check: running e2e suite (nix build .#e2e)...");
        let status = Command::new("nix")
            .args(["build", ".#e2e", "--no-link", "--print-build-logs"])
            .current_dir(&repo_root)
            .status()?;

        if !status.success() {
            std::process::exit(status.code().unwrap_or(1));
        }
    }

    Ok(())
}
