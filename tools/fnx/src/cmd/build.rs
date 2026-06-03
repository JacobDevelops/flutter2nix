use std::process::Command;

use clap::Args;

use crate::nixutil;

#[derive(Args)]
pub struct BuildArgs {
    /// Nix package target (e.g. gradle2nix, ios2nix). Defaults to all packages.
    pub target: Option<String>,

    /// Output symlink path (passed to nix build --out-link)
    #[arg(short = 'o', long)]
    pub out_link: Option<String>,
}

pub fn run(args: BuildArgs) -> anyhow::Result<()> {
    let repo_root = nixutil::find_repo_root()?;

    let mut cmd = Command::new("nix");
    cmd.arg("build").arg("--print-build-logs").current_dir(&repo_root);

    if let Some(ref target) = args.target {
        cmd.arg(format!(".#{target}"));
    }

    if let Some(ref out_link) = args.out_link {
        cmd.args(["--out-link", out_link]);
    }

    let status = cmd.status()?;
    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }

    Ok(())
}
