/// Stub: Configure Xcode build environment (DEVELOPER_DIR, SDKROOT).
pub fn setup_xcode_env() -> anyhow::Result<()> {
    Ok(())
}

#[cfg(test)]
#[path = "env_tests.rs"]
mod tests;
