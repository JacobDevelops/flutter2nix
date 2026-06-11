/// Stub: Configure Xcode build environment (DEVELOPER_DIR, SDKROOT).
pub fn setup_xcode_env() -> anyhow::Result<()> {
    #[cfg(target_os = "macos")]
    {
        anyhow::bail!("ios2nix xcode env: not yet implemented (Plan 2)")
    }
    #[cfg(not(target_os = "macos"))]
    {
        anyhow::bail!("ios2nix xcode env requires macOS")
    }
}

#[cfg(test)]
#[path = "env_tests.rs"]
mod tests;
