/// Stub: Parse Podfile.lock and resolve pod URLs/hashes.
pub fn parse_podfile_lock() -> anyhow::Result<()> {
    Ok(())
}

#[cfg(test)]
#[path = "cocoapods_tests.rs"]
mod tests;
