/// Manage a temporary keychain for code signing.
pub fn create_temp_keychain() -> anyhow::Result<()> {
    #[cfg(target_os = "macos")]
    {
        anyhow::bail!("ios2nix keychain: not yet implemented (Plan 3)")
    }
    #[cfg(not(target_os = "macos"))]
    {
        anyhow::bail!("ios2nix keychain requires macOS")
    }
}

#[cfg(test)]
#[path = "keychain_tests.rs"]
mod tests;
