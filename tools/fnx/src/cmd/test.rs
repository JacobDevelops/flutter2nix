use std::process::Command;

use clap::Args;

use crate::nixutil;

#[derive(Args)]
pub struct TestArgs {
    /// Filter to a specific crate (e.g. nix-core, gradle2nix)
    pub crate_name: Option<String>,
}

pub fn run(args: TestArgs) -> anyhow::Result<()> {
    let repo_root = nixutil::find_repo_root()?;

    let mut cmd = Command::new("cargo");
    cmd.arg("test").current_dir(&repo_root);

    if let Some(ref name) = args.crate_name {
        cmd.args(["-p", name]);
    } else {
        cmd.arg("--workspace");
    }

    let status = cmd.status()?;
    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }

    Ok(())
}
