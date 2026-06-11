use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct XcodeBuildOutput {
    pub version: String,
    pub architectures: Vec<String>,
    #[serde(default)]
    pub frameworks: Vec<String>,
    pub codesign_identity: Option<String>,
}

/// Parse and validate Xcode build output JSON.
/// Supports schema versions 5–30 (Xcode 26 is current).
pub fn parse_xcode_build_output(json: &str) -> anyhow::Result<XcodeBuildOutput> {
    let output: XcodeBuildOutput = serde_json::from_str(json)?;

    // Extract major version from "X.Y.Z" format.
    let major_version = output
        .version
        .split('.')
        .next()
        .and_then(|s| s.parse::<u32>().ok())
        .ok_or_else(|| anyhow::anyhow!("malformed version: {}", output.version))?;

    // Supported range: 5–30
    if !(5..=30).contains(&major_version) {
        anyhow::bail!(
            "unsupported schema version {}: supported range is 5–30",
            major_version
        );
    }

    Ok(output)
}

#[cfg(test)]
#[path = "build_output_tests.rs"]
mod tests;
