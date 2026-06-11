use std::path::PathBuf;

pub struct SigningConfig {
    pub team_id: String,
    pub identity: String,
    pub profile_uuid: String,
    pub keychain: PathBuf,
}

/// Stub: Generate ExportOptions.plist for xcodebuild -exportArchive.
pub fn generate_export_options() -> anyhow::Result<()> {
    #[cfg(target_os = "macos")]
    {
        anyhow::bail!("ios2nix export_opts: not yet implemented (Plan 3)")
    }
    #[cfg(not(target_os = "macos"))]
    {
        anyhow::bail!("ios2nix export_opts requires macOS")
    }
}

#[cfg(test)]
#[path = "export_opts_tests.rs"]
mod tests;
