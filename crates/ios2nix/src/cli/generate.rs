use anyhow::Context;
use clap::Parser;
use nix_core::codegen::cocoapods::{
    generate_nix_overlay, generate_nix_set, NixCocoaPodsCodegenConfig,
};
use std::path::PathBuf;

#[derive(Parser)]
pub struct GenerateArgs {
    /// Path to lockfile
    #[arg(long)]
    pub lockfile: Option<PathBuf>,

    /// Output path for Nix expressions
    #[arg(long, short)]
    pub output: Option<PathBuf>,

    /// Output format
    #[arg(long, default_value = "inline", value_parser = ["inline", "modular"])]
    pub format: String,
}

pub struct GenerateCommand {
    pub lockfile: Option<PathBuf>,
    pub output: Option<PathBuf>,
    pub format: String,
}

pub fn run(cmd: GenerateCommand) -> anyhow::Result<()> {
    let lockfile_path = cmd
        .lockfile
        .unwrap_or_else(|| PathBuf::from("ios2nix.lock"));
    let graph = nix_core::lockfile::read_lockfile(&lockfile_path)?;

    let config = NixCocoaPodsCodegenConfig {
        indent_width: 2,
        sort_deps: true,
    };

    let nix_content = match cmd.format.as_str() {
        "inline" => generate_nix_set(&graph, &config)?,
        "modular" => generate_nix_overlay(&graph, &config)?,
        _ => anyhow::bail!("invalid format: {}", cmd.format),
    };

    match cmd.output {
        Some(ref path) => std::fs::write(path, &nix_content)
            .with_context(|| format!("writing Nix output to '{}'", path.display()))?,
        None => print!("{nix_content}"),
    }

    Ok(())
}

#[cfg(test)]
#[path = "generate_tests.rs"]
mod tests;
