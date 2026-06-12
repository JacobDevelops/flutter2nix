use anyhow::Context;
use nix_core::codegen::maven::{generate_nix_overlay, generate_nix_set, NixMavenCodegenConfig};
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
pub fn run(cmd: GenerateCommand) -> anyhow::Result<()> {
    let lockfile_path = cmd
        .lockfile
        .unwrap_or_else(|| PathBuf::from("gradle2nix.lock"));
    let graph = nix_core::lockfile::read_lockfile(&lockfile_path)?;

    let (fetcher, nix_content) = match cmd.format {
        NixFormat::Inline => {
            let config = NixMavenCodegenConfig {
                fetcher: "fetchMaven".to_string(),
                indent_width: 2,
                sort_deps: true,
            };
            ("fetchMaven", generate_nix_set(&graph, &config)?)
        }
        NixFormat::Flake => {
            let config = NixMavenCodegenConfig {
                fetcher: "pkgs.fetchMaven".to_string(),
                indent_width: 2,
                sort_deps: true,
            };
            ("pkgs.fetchMaven", generate_nix_overlay(&graph, &config)?)
        }
    };
    let _ = fetcher;

    match cmd.output {
        Some(ref path) => std::fs::write(path, &nix_content)
            .with_context(|| format!("writing Nix output to '{}'", path.display()))?,
        None => print!("{nix_content}"),
    }

    Ok(())
}
