/// Stub: Assert Xcode version matches expected via DEVELOPER_DIR.
pub fn assert_xcode_version() -> anyhow::Result<()> {
    Ok(())
}

#[cfg(test)]
#[path = "assert_tests.rs"]
mod tests;
