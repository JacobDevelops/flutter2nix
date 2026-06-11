use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug, Clone)]
pub struct SignSetupArgs {
    /// Path to the .p12 certificate + key file (also read from IOS2NIX_P12_PATH env)
    #[arg(long)]
    pub p12: Option<PathBuf>,

    /// Path to the provisioning profile (or directory of .mobileprovision files) (also read from IOS2NIX_PROFILE_PATH env)
    #[arg(long)]
    pub profile: Option<PathBuf>,
}

impl SignSetupArgs {
    /// Resolve p12 and profile from args or environment variables.
    pub fn resolve(mut self) -> Self {
        if self.p12.is_none() {
            if let Ok(p12_path) = std::env::var("IOS2NIX_P12_PATH") {
                self.p12 = Some(PathBuf::from(p12_path));
            }
        }
        if self.profile.is_none() {
            if let Ok(profile_path) = std::env::var("IOS2NIX_PROFILE_PATH") {
                self.profile = Some(PathBuf::from(profile_path));
            }
        }
        self
    }
}

pub struct SignSetupCommand {
    pub p12: PathBuf,
    pub profile: PathBuf,
}

/// Run the sign-setup subcommand.
/// Creates a temporary keychain, imports the P12 identity, installs the provisioning profile,
/// and returns the keychain path (main prints it to stdout for the Nix `$(...)` capture).
/// The keychain persists after the process exits (parent cleanup via trap).
#[cfg(target_os = "macos")]
pub fn run(cmd: SignSetupCommand) -> anyhow::Result<PathBuf> {
    use anyhow::Context;
    use std::fs;

    let p12_password = std::env::var("IOS2NIX_P12_PASSWORD")
        .context("IOS2NIX_P12_PASSWORD environment variable required")?;

    let keychain_password = std::env::var("IOS2NIX_KEYCHAIN_PASSWORD")
        .context("IOS2NIX_KEYCHAIN_PASSWORD environment variable required")?;

    // Create temporary keychain
    let kc = crate::keychain::TempKeychain::create(&keychain_password)
        .context("failed to create temporary keychain")?;

    // Import the P12 identity
    kc.import_identity(&cmd.p12, &p12_password)
        .context("failed to import identity to keychain")?;

    // Add to search list so codesign/xcodebuild find it
    kc.add_to_search_list()
        .context("failed to add keychain to search list")?;

    // Install provisioning profile(s)
    if cmd.profile.is_dir() {
        // Directory: install every .mobileprovision in it
        for entry in fs::read_dir(&cmd.profile).context("failed to read profile directory")? {
            let entry = entry.context("failed to read directory entry")?;
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "mobileprovision") {
                crate::provisioning::install_provisioning_profile(&path).with_context(|| {
                    format!("failed to install provisioning profile {}", path.display())
                })?;
            }
        }
    } else {
        crate::provisioning::install_provisioning_profile(&cmd.profile)
            .context("failed to install provisioning profile")?;
    }

    Ok(kc.persist())
}

#[cfg(not(target_os = "macos"))]
pub fn run(_cmd: SignSetupCommand) -> anyhow::Result<PathBuf> {
    anyhow::bail!("ios2nix sign-setup requires macOS")
}
