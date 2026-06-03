/// Stub: Assert Xcode version matches expected via DEVELOPER_DIR.
pub fn assert_xcode_version() -> anyhow::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_assert_xcode_version_valid() {
        todo!("Phase 1: stub — input: Xcode 15.3 >= minimum 14.0, expect: Ok(())")
    }

    #[test]
    fn test_assert_xcode_version_too_old() {
        todo!("Phase 1: stub — input: Xcode 13.0 < minimum 14.0, expect: Err(Xcode version too old)")
    }

    #[test]
    fn test_assert_xcode_tools_installed() {
        todo!("Phase 1: stub — expect: xcode-select -p returns valid path, Ok(())")
    }
}
