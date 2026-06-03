use std::path::PathBuf;

#[derive(Clone, Copy, Debug)]
pub enum NixFormat {
    /// A single attribute set of fetchMaven calls
    Inline,
    /// A flake.nix pkgs overlay function
    Flake,
}

pub struct GenerateCommand {
    pub lockfile: Option<PathBuf>,
    pub output: Option<PathBuf>,
    pub format: NixFormat,
}

/// Flow: read lockfile → delegate to nix_core codegen → write Nix output
pub fn run(_cmd: GenerateCommand) -> anyhow::Result<()> {
    todo!("Phase 1: API contract only")
}
