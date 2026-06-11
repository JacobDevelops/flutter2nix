pub fn run() -> anyhow::Result<()> {
    #[cfg(target_os = "macos")]
    {
        anyhow::bail!("ios2nix sign: not yet implemented (Plan 3)")
    }
    #[cfg(not(target_os = "macos"))]
    {
        anyhow::bail!("ios2nix sign requires macOS")
    }
}

#[cfg(test)]
#[path = "sign_tests.rs"]
mod tests;
