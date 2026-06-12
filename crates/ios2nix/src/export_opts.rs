use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

/// Export method for xcodebuild -exportArchive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportMethod {
    AppStore,
    AdHoc,
    Enterprise,
    Development,
    DeveloperId,
    Validation,
}

impl std::str::FromStr for ExportMethod {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> anyhow::Result<Self> {
        match s.to_lowercase().as_str() {
            "appstore" | "app-store" | "app-store-connect" => Ok(ExportMethod::AppStore),
            "adhoc" | "ad-hoc" | "release-testing" => Ok(ExportMethod::AdHoc),
            "enterprise" => Ok(ExportMethod::Enterprise),
            "development" | "debugging" => Ok(ExportMethod::Development),
            "developerid" | "developer-id" => Ok(ExportMethod::DeveloperId),
            "validation" => Ok(ExportMethod::Validation),
            _ => anyhow::bail!("unrecognized export method: {}", s),
        }
    }
}

/// Signing style for ExportOptions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SigningStyle {
    Manual,
    Automatic,
}

impl SigningStyle {
    /// Convert to plist string value.
    pub fn as_str(&self) -> &'static str {
        match self {
            SigningStyle::Manual => "manual",
            SigningStyle::Automatic => "automatic",
        }
    }
}

/// Export destination.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Destination {
    Export,
    Upload,
}

impl Destination {
    /// Convert to plist string value.
    pub fn as_str(&self) -> &'static str {
        match self {
            Destination::Export => "export",
            Destination::Upload => "upload",
        }
    }
}

/// Method name style for plist emission (Xcode version dependent).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MethodNameStyle {
    Classic,
    Xcode16,
}

impl MethodNameStyle {
    /// Convert method to the plist string for this style.
    pub fn method_name(&self, method: ExportMethod) -> &'static str {
        match (method, self) {
            (ExportMethod::AppStore, MethodNameStyle::Classic) => "app-store",
            (ExportMethod::AppStore, MethodNameStyle::Xcode16) => "app-store-connect",
            (ExportMethod::AdHoc, MethodNameStyle::Classic) => "ad-hoc",
            (ExportMethod::AdHoc, MethodNameStyle::Xcode16) => "release-testing",
            (ExportMethod::Development, MethodNameStyle::Classic) => "development",
            (ExportMethod::Development, MethodNameStyle::Xcode16) => "debugging",
            (ExportMethod::Enterprise, _) => "enterprise",
            (ExportMethod::DeveloperId, _) => "developer-id",
            (ExportMethod::Validation, _) => "validation",
        }
    }
}

/// Signing configuration (used by archive/export).
#[derive(Debug, Clone)]
pub struct SigningConfig {
    pub team_id: String,
    pub identity: String,
    pub profile_specifier: String, // profile NAME for PROVISIONING_PROFILE_SPECIFIER
    pub keychain: std::path::PathBuf,
}

/// Full ExportOptions model for xcodebuild -exportArchive.
#[derive(Debug, Clone)]
pub struct ExportOptions {
    pub method: ExportMethod,
    pub team_id: String,
    pub signing_style: SigningStyle,
    pub signing_certificate: Option<String>,
    pub provisioning_profiles: BTreeMap<String, String>, // bundleID -> profile UUID
    pub destination: Destination,
    pub strip_swift_symbols: bool,
    pub upload_symbols: bool,
    pub compile_bitcode: bool,
    pub manage_app_version_and_build_number: bool,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            method: ExportMethod::AdHoc,
            team_id: String::new(),
            signing_style: SigningStyle::Manual,
            signing_certificate: None,
            provisioning_profiles: BTreeMap::new(),
            destination: Destination::Export,
            strip_swift_symbols: true,
            upload_symbols: false,
            compile_bitcode: false,
            manage_app_version_and_build_number: false,
        }
    }
}

impl ExportOptions {
    /// Create a new ExportOptions with sensible defaults.
    pub fn new(method: ExportMethod, team_id: String) -> Self {
        Self {
            method,
            team_id,
            ..Default::default()
        }
    }

    /// Validate the options.
    fn validate(&self) -> anyhow::Result<()> {
        // team_id required for all non-Development methods
        if self.method != ExportMethod::Development && self.team_id.is_empty() {
            anyhow::bail!("team_id is required for non-development export methods");
        }

        // Manual style requires signing_certificate and at least one provisioning_profile
        if self.signing_style == SigningStyle::Manual {
            if self
                .signing_certificate
                .as_ref()
                .is_none_or(|s| s.is_empty())
            {
                anyhow::bail!("signing_certificate is required when signingStyle is manual");
            }
            if self.provisioning_profiles.is_empty() {
                anyhow::bail!("provisioning_profiles must have at least one entry when signingStyle is manual");
            }
        }

        // Validate provisioning_profiles UUIDs (36 chars, format 8-4-4-4-12)
        for (bundle_id, uuid) in &self.provisioning_profiles {
            if bundle_id.is_empty() {
                anyhow::bail!("provisioning_profiles keys (bundle IDs) must be non-empty");
            }
            if !is_valid_uuid(uuid) {
                anyhow::bail!(
                    "provisioning_profiles value for '{}' is not a valid UUID: '{}' (use the profile UUID, not its name, for deterministic resolution)",
                    bundle_id, uuid
                );
            }
        }

        Ok(())
    }
}

/// Check if a string is a valid UUID (36 chars, format 8-4-4-4-12 hex).
fn is_valid_uuid(s: &str) -> bool {
    if s.len() != 36 {
        return false;
    }
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 5 {
        return false;
    }
    if parts[0].len() != 8
        || parts[1].len() != 4
        || parts[2].len() != 4
        || parts[3].len() != 4
        || parts[4].len() != 12
    {
        return false;
    }
    // All parts must be valid hex
    for part in parts {
        if u64::from_str_radix(part, 16).is_err() {
            return false;
        }
    }
    true
}

/// Resolve the Xcode version and return the appropriate MethodNameStyle.
pub fn resolve_method_name_style() -> MethodNameStyle {
    // First check env override
    if let Ok(override_val) = std::env::var("IOS2NIX_XCODE_METHOD_NAMES") {
        match override_val.to_lowercase().as_str() {
            "classic" => return MethodNameStyle::Classic,
            "xcode16" => return MethodNameStyle::Xcode16,
            _ => {}
        }
    }

    // On macOS, try to detect Xcode version
    #[cfg(target_os = "macos")]
    {
        if let Ok(output) = Command::new("xcodebuild").args(["-version"]).output() {
            if let Ok(stdout) = String::from_utf8(output.stdout) {
                // First line is usually "Xcode X.Y"
                if let Some(first_line) = stdout.lines().next() {
                    // Try to extract major version number
                    for word in first_line.split_whitespace() {
                        if let Ok(major_version) = word.parse::<u32>() {
                            if major_version >= 16 {
                                return MethodNameStyle::Xcode16;
                            }
                        }
                    }
                }
            }
        }
    }

    // Default to Classic
    MethodNameStyle::Classic
}

/// Escape XML special characters in a string.
fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Generate ExportOptions.plist as XML string.
pub fn generate_export_options_plist(
    opts: &ExportOptions,
    style: MethodNameStyle,
) -> anyhow::Result<String> {
    opts.validate()?;

    let method_name = style.method_name(opts.method);
    let signing_style = opts.signing_style.as_str();
    let destination = opts.destination.as_str();

    let mut plist = String::new();
    plist.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    plist.push_str("<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n");
    plist.push_str("<plist version=\"1.0\"><dict>\n");

    // method
    plist.push_str("  <key>method</key><string>");
    plist.push_str(&escape_xml(method_name));
    plist.push_str("</string>\n");

    // teamID
    plist.push_str("  <key>teamID</key><string>");
    plist.push_str(&escape_xml(&opts.team_id));
    plist.push_str("</string>\n");

    // signingStyle
    plist.push_str("  <key>signingStyle</key><string>");
    plist.push_str(signing_style);
    plist.push_str("</string>\n");

    // signingCertificate (optional)
    if let Some(cert) = &opts.signing_certificate {
        plist.push_str("  <key>signingCertificate</key><string>");
        plist.push_str(&escape_xml(cert));
        plist.push_str("</string>\n");
    }

    // provisioningProfiles (if manual and not empty)
    if !opts.provisioning_profiles.is_empty() {
        plist.push_str("  <key>provisioningProfiles</key><dict>\n");
        for (bundle_id, uuid) in &opts.provisioning_profiles {
            plist.push_str("    <key>");
            plist.push_str(&escape_xml(bundle_id));
            plist.push_str("</key><string>");
            plist.push_str(&escape_xml(uuid));
            plist.push_str("</string>\n");
        }
        plist.push_str("  </dict>\n");
    }

    // destination
    plist.push_str("  <key>destination</key><string>");
    plist.push_str(destination);
    plist.push_str("</string>\n");

    // stripSwiftSymbols
    plist.push_str("  <key>stripSwiftSymbols</key><");
    plist.push_str(if opts.strip_swift_symbols {
        "true"
    } else {
        "false"
    });
    plist.push_str("/>\n");

    // uploadSymbols (only for App Store)
    if opts.method == ExportMethod::AppStore {
        plist.push_str("  <key>uploadSymbols</key><");
        plist.push_str(if opts.upload_symbols { "true" } else { "false" });
        plist.push_str("/>\n");
    }

    // compileBitcode
    plist.push_str("  <key>compileBitcode</key><");
    plist.push_str(if opts.compile_bitcode {
        "true"
    } else {
        "false"
    });
    plist.push_str("/>\n");

    // manageAppVersionAndBuildNumber
    plist.push_str("  <key>manageAppVersionAndBuildNumber</key><");
    plist.push_str(if opts.manage_app_version_and_build_number {
        "true"
    } else {
        "false"
    });
    plist.push_str("/>\n");

    plist.push_str("</dict></plist>\n");

    Ok(plist)
}

/// Write ExportOptions to a plist file.
pub fn write_export_options(
    opts: &ExportOptions,
    style: MethodNameStyle,
    path: &Path,
) -> anyhow::Result<()> {
    let plist_content = generate_export_options_plist(opts, style)?;
    std::fs::write(path, plist_content).map_err(|e| {
        anyhow::anyhow!(
            "failed to write export options to {}: {}",
            path.display(),
            e
        )
    })?;
    Ok(())
}

#[cfg(test)]
#[path = "export_opts_tests.rs"]
mod tests;
