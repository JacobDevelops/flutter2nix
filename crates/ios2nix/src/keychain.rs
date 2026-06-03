/// Manage a temporary keychain for code signing.
pub fn create_temp_keychain() -> anyhow::Result<()> {
    Ok(())
}

#[cfg(test)]
#[path = "keychain_tests.rs"]
mod tests;
