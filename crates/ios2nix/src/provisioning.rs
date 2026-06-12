use anyhow::Context;
use std::path::PathBuf;

/// Information extracted from a provisioning profile plist.
#[derive(Debug, Clone)]
pub struct ProfileInfo {
    pub uuid: String,
    pub name: String,
    pub bundle_id: String,
    pub team_id: String,
    pub expiration_date: Option<String>,
}

/// An installed provisioning profile.
#[derive(Debug, Clone)]
pub struct InstalledProfile {
    pub uuid: String,
    pub name: String,
    pub bundle_id: String,
    pub team_id: String,
    pub installed_path: PathBuf,
}

/// Parse a provisioning profile plist (decoded) from raw bytes.
/// This is pure Rust and Linux-testable.
pub fn parse_profile_plist(plist_bytes: &[u8]) -> anyhow::Result<ProfileInfo> {
    let plist_value: plist::Value =
        plist::from_bytes(plist_bytes).context("failed to parse provisioning profile plist")?;

    let plist_dict = plist_value
        .as_dictionary()
        .context("provisioning profile plist must be a dictionary")?;

    // Extract UUID
    let uuid = plist_dict
        .get("UUID")
        .and_then(|v| v.as_string())
        .context("missing UUID in provisioning profile")?
        .to_string();

    // Extract Name
    let name = plist_dict
        .get("Name")
        .and_then(|v| v.as_string())
        .context("missing Name in provisioning profile")?
        .to_string();

    // Extract TeamIdentifier (array, take first element)
    let team_id = plist_dict
        .get("TeamIdentifier")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .and_then(|v| v.as_string())
        .context("missing or invalid TeamIdentifier in provisioning profile")?
        .to_string();

    // Extract ExpirationDate (optional)
    let expiration_date = plist_dict
        .get("ExpirationDate")
        .and_then(|v| v.as_string())
        .map(|s| s.to_string());

    // Extract bundle_id from Entitlements.application-identifier (format: TEAMID.bundle.id)
    let app_identifier = plist_dict
        .get("Entitlements")
        .and_then(|v| v.as_dictionary())
        .and_then(|d| d.get("application-identifier"))
        .and_then(|v| v.as_string())
        .context("missing application-identifier in Entitlements")?;

    // Strip team prefix (TEAMID.)
    let bundle_id = app_identifier
        .split_once('.')
        .map(|(_, rest)| rest)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "application-identifier '{}' must contain team prefix (TEAMID.bundle.id)",
                app_identifier
            )
        })?
        .to_string();

    Ok(ProfileInfo {
        uuid,
        name,
        bundle_id,
        team_id,
        expiration_date,
    })
}

/// Decode a CMS-signed provisioning profile to its plist bytes via `security cms -D`.
#[cfg(target_os = "macos")]
pub fn decode_cms_plist(profile: &std::path::Path) -> anyhow::Result<Vec<u8>> {
    let output = std::process::Command::new("security")
        .args(["cms", "-D", "-i"])
        .arg(profile)
        .output()
        .context("failed to run 'security cms -D' to decode provisioning profile")?;

    if !output.status.success() {
        anyhow::bail!(
            "security cms -D failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(output.stdout)
}

/// Install a provisioning profile (macOS only).
/// Decodes the CMS-signed profile, extracts metadata, and copies it to the canonical directories.
#[cfg(target_os = "macos")]
pub fn install_provisioning_profile(profile: &std::path::Path) -> anyhow::Result<InstalledProfile> {
    let decoded = decode_cms_plist(profile)?;
    let profile_info =
        parse_profile_plist(&decoded).context("failed to parse decoded provisioning profile")?;

    let home = std::env::var("HOME").context("HOME environment variable not set")?;

    // Install to the classic location (still honored by Xcode)
    let classic_dir =
        std::path::Path::new(&home).join("Library/MobileDevice/Provisioning Profiles");
    std::fs::create_dir_all(&classic_dir)
        .context("failed to create provisioning profiles directory")?;

    let classic_path = classic_dir.join(format!("{}.mobileprovision", profile_info.uuid));
    std::fs::copy(profile, &classic_path)
        .context("failed to copy provisioning profile to classic location")?;

    // Install to the Xcode 16 location
    let xcode16_dir =
        std::path::Path::new(&home).join("Library/Developer/Xcode/UserData/Provisioning Profiles");
    std::fs::create_dir_all(&xcode16_dir)
        .context("failed to create Xcode 16 provisioning profiles directory")?;

    let xcode16_path = xcode16_dir.join(format!("{}.mobileprovision", profile_info.uuid));
    std::fs::copy(profile, &xcode16_path)
        .context("failed to copy provisioning profile to Xcode 16 location")?;

    Ok(InstalledProfile {
        uuid: profile_info.uuid,
        name: profile_info.name,
        bundle_id: profile_info.bundle_id,
        team_id: profile_info.team_id,
        installed_path: classic_path,
    })
}

/// Remove an installed provisioning profile (macOS only).
#[cfg(target_os = "macos")]
pub fn remove_installed_profile(uuid: &str) -> anyhow::Result<()> {
    let home = std::env::var("HOME").context("HOME environment variable not set")?;

    // Remove from classic location
    let classic_path = std::path::Path::new(&home).join(format!(
        "Library/MobileDevice/Provisioning Profiles/{}.mobileprovision",
        uuid
    ));
    if classic_path.exists() {
        std::fs::remove_file(&classic_path)
            .context("failed to remove provisioning profile from classic location")?;
    }

    // Remove from Xcode 16 location
    let xcode16_path = std::path::Path::new(&home).join(format!(
        "Library/Developer/Xcode/UserData/Provisioning Profiles/{}.mobileprovision",
        uuid
    ));
    if xcode16_path.exists() {
        std::fs::remove_file(&xcode16_path)
            .context("failed to remove provisioning profile from Xcode 16 location")?;
    }

    Ok(())
}

#[cfg(test)]
#[path = "provisioning_tests.rs"]
mod tests;
