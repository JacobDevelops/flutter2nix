/// Stub: Assert Xcode version matches expected via DEVELOPER_DIR.
pub fn assert_xcode_version() -> anyhow::Result<()> {
    #[cfg(target_os = "macos")]
    {
        anyhow::bail!("ios2nix xcode assert: not yet implemented (Plan 2)")
    }
    #[cfg(not(target_os = "macos"))]
    {
        anyhow::bail!("ios2nix xcode assert requires macOS")
    }
}

#[cfg(test)]
#[path = "assert_tests.rs"]
mod tests;
