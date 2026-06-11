pub fn run() -> anyhow::Result<()> {
    #[cfg(target_os = "macos")]
    {
        anyhow::bail!("ios2nix build: not yet implemented (Plan 2)")
    }
    #[cfg(not(target_os = "macos"))]
    {
        anyhow::bail!("ios2nix build requires macOS")
    }
}

#[cfg(test)]
#[path = "build_tests.rs"]
mod tests;
