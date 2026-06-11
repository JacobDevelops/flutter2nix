use clap::Parser;
use std::path::{Path, PathBuf};

#[derive(Parser, Debug, Clone)]
pub struct SignArgs {
    /// Path to .ipa to sign
    #[arg(long)]
    pub ipa_path: PathBuf,

    /// Signing identity name (e.g., "Apple Distribution: Example Corp (TEAM123456)" or "-" for ad-hoc)
    #[arg(long)]
    pub signing_identity: String,

    /// Optional path to keychain for signing
    #[arg(long)]
    pub keychain: Option<PathBuf>,

    /// Output .ipa path (defaults to input path with -signed suffix)
    #[arg(long)]
    pub output: Option<PathBuf>,
}

pub struct SignCommand {
    pub ipa_path: PathBuf,
    pub identity: String,
    pub keychain: Option<PathBuf>,
    pub output: PathBuf,
}

impl SignCommand {
    pub fn from_args(args: &SignArgs) -> anyhow::Result<Self> {
        let output = args.output.clone().unwrap_or_else(|| {
            let path = args
                .ipa_path
                .with_extension("")
                .to_string_lossy()
                .to_string();
            PathBuf::from(format!("{}-signed.ipa", path))
        });

        Ok(SignCommand {
            ipa_path: args.ipa_path.clone(),
            identity: args.signing_identity.clone(),
            keychain: args.keychain.clone(),
            output,
        })
    }
}

/// An app extension (.appex) bundle with its embedded frameworks.
pub struct ExtensionBundle {
    pub path: PathBuf,
    pub frameworks: Vec<PathBuf>,
}

/// The inside-out signing order for an unpacked .app: a parent signature is
/// invalidated if a child is re-signed afterward, so frameworks come first,
/// then each extension (its own frameworks before the extension), the app last.
pub struct SigningOrder {
    pub frameworks: Vec<PathBuf>,
    pub extensions: Vec<ExtensionBundle>,
    pub app: PathBuf,
}

fn compute_signing_order(app_path: &Path) -> anyhow::Result<SigningOrder> {
    use anyhow::Context;

    let mut frameworks = Vec::new();
    let frameworks_dir = app_path.join("Frameworks");
    if frameworks_dir.exists() {
        for entry in std::fs::read_dir(&frameworks_dir).context("failed to read Frameworks")? {
            let entry = entry.context("failed to read directory entry")?;
            frameworks.push(entry.path());
        }
    }

    let mut extensions = Vec::new();
    let plugins_dir = app_path.join("PlugIns");
    if plugins_dir.exists() {
        for entry in std::fs::read_dir(&plugins_dir).context("failed to read PlugIns")? {
            let entry = entry.context("failed to read directory entry")?;
            let ext_path = entry.path();
            if ext_path.extension().is_some_and(|ext| ext == "appex") {
                let mut ext_frameworks = Vec::new();
                let ext_fw_dir = ext_path.join("Frameworks");
                if ext_fw_dir.exists() {
                    for fw_entry in std::fs::read_dir(&ext_fw_dir)
                        .context("failed to read extension Frameworks")?
                    {
                        let fw_entry = fw_entry.context("failed to read directory entry")?;
                        ext_frameworks.push(fw_entry.path());
                    }
                }
                extensions.push(ExtensionBundle {
                    path: ext_path,
                    frameworks: ext_frameworks,
                });
            }
        }
    }

    Ok(SigningOrder {
        frameworks,
        extensions,
        app: app_path.to_path_buf(),
    })
}

/// Decode the provisioning profile to extract entitlements.
/// Returns the path to a temporary XML plist file containing the entitlements.
#[cfg(target_os = "macos")]
fn extract_entitlements(profile_path: &Path) -> anyhow::Result<PathBuf> {
    use anyhow::Context;

    let decoded = crate::provisioning::decode_cms_plist(profile_path)?;

    let plist_value: plist::Value =
        plist::from_bytes(&decoded).context("failed to parse provisioning profile plist")?;

    let plist_dict = plist_value
        .as_dictionary()
        .context("provisioning profile plist must be a dictionary")?;

    let entitlements_dict = plist_dict
        .get("Entitlements")
        .and_then(|v| v.as_dictionary())
        .context("provisioning profile missing Entitlements dict")?;

    let entitlements_plist = plist::Value::Dictionary(entitlements_dict.clone());

    let temp_file =
        tempfile::NamedTempFile::new().context("failed to create temp entitlements file")?;
    let temp_path = temp_file.into_temp_path().to_path_buf();

    entitlements_plist
        .to_file_xml(&temp_path)
        .context("failed to serialize entitlements to XML")?;

    Ok(temp_path)
}

/// Run `codesign -f -s <identity>` on one bundle, with optional keychain and entitlements.
#[cfg(target_os = "macos")]
fn codesign_bundle(
    cmd: &SignCommand,
    target: &Path,
    entitlements: Option<&Path>,
) -> anyhow::Result<()> {
    use anyhow::Context;

    let mut sign = std::process::Command::new("codesign");
    sign.args(["-f", "-s", &cmd.identity, "--timestamp=none"]);
    if let Some(kc) = &cmd.keychain {
        sign.arg("--keychain").arg(kc);
    }
    if let Some(ent) = entitlements {
        sign.arg("--entitlements").arg(ent);
    }
    sign.arg(target);

    let output = sign.output().context("failed to run codesign")?;
    if !output.status.success() {
        anyhow::bail!(
            "codesign failed on {}: {}",
            target.display(),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}

/// Sign a bundle, applying entitlements from its embedded.mobileprovision when present.
#[cfg(target_os = "macos")]
fn codesign_with_embedded_entitlements(cmd: &SignCommand, bundle: &Path) -> anyhow::Result<()> {
    use anyhow::Context;

    let embedded_profile = bundle.join("embedded.mobileprovision");
    let entitlements =
        if embedded_profile.exists() {
            Some(extract_entitlements(&embedded_profile).with_context(|| {
                format!("failed to extract entitlements for {}", bundle.display())
            })?)
        } else {
            None
        };
    codesign_bundle(cmd, bundle, entitlements.as_deref())
}

/// Re-sign an IPA file with the given identity and optional keychain.
/// Returns the path to the signed IPA.
#[cfg(target_os = "macos")]
pub fn run(cmd: SignCommand) -> anyhow::Result<PathBuf> {
    use anyhow::Context;
    use std::process::Command;

    if !cmd.ipa_path.exists() {
        anyhow::bail!("IPA file not found: {}", cmd.ipa_path.display());
    }

    let work_dir = tempfile::TempDir::new().context("failed to create working directory")?;

    let unzip_output = Command::new("unzip")
        .arg("-q")
        .arg(&cmd.ipa_path)
        .arg("-d")
        .arg(work_dir.path())
        .output()
        .context("failed to run unzip")?;

    if !unzip_output.status.success() {
        anyhow::bail!(
            "unzip failed: {}",
            String::from_utf8_lossy(&unzip_output.stderr)
        );
    }

    // Find the .app directory
    let payload_dir = work_dir.path().join("Payload");
    let mut app_path = None;

    for entry in std::fs::read_dir(&payload_dir).context("failed to read Payload")? {
        let entry = entry.context("failed to read directory entry")?;
        let path = entry.path();
        if path.is_dir() && path.extension().is_some_and(|ext| ext == "app") {
            app_path = Some(path);
            break;
        }
    }

    let app_path = app_path.context("no .app found in Payload")?;
    let order = compute_signing_order(&app_path)?;

    // 1. Main-app frameworks
    for fw in &order.frameworks {
        codesign_bundle(&cmd, fw, None)?;
    }

    // 2. Extensions: each extension's frameworks first, then the extension itself
    for ext in &order.extensions {
        for fw in &ext.frameworks {
            codesign_bundle(&cmd, fw, None)?;
        }
        codesign_with_embedded_entitlements(&cmd, &ext.path)?;
    }

    // 3. Main app last
    codesign_with_embedded_entitlements(&cmd, &order.app)?;

    // 4. Verify — fails loudly if any nested code is unsigned
    let verify_output = Command::new("codesign")
        .args(["--verify", "--deep", "--strict"])
        .arg(&order.app)
        .output()
        .context("failed to run codesign --verify")?;

    if !verify_output.status.success() {
        anyhow::bail!(
            "codesign --verify failed: {}",
            String::from_utf8_lossy(&verify_output.stderr)
        );
    }

    // 5. Repackage the IPA
    let zip_output = Command::new("zip")
        .arg("-qry")
        .arg(&cmd.output)
        .arg("Payload")
        .current_dir(work_dir.path())
        .output()
        .context("failed to run zip")?;

    if !zip_output.status.success() {
        anyhow::bail!(
            "zip repackaging failed: {}",
            String::from_utf8_lossy(&zip_output.stderr)
        );
    }

    Ok(cmd.output)
}

#[cfg(not(target_os = "macos"))]
pub fn run(_cmd: SignCommand) -> anyhow::Result<PathBuf> {
    anyhow::bail!("ios2nix sign requires macOS")
}

#[cfg(test)]
#[path = "sign_tests.rs"]
mod tests;
