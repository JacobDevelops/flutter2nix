/// Stub: Generate ExportOptions.plist for xcodebuild -exportArchive.
pub fn generate_export_options() -> anyhow::Result<()> {
    Ok(())
}

#[cfg(test)]
#[path = "export_opts_tests.rs"]
mod tests;
