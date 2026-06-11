use crate::export_opts::SigningConfig;
use clap::Parser;
use std::path::{Path, PathBuf};

#[derive(Parser, Debug, Clone)]
pub struct ArchiveArgs {
    /// Workspace path
    #[arg(long, default_value = "ios/Runner.xcworkspace")]
    pub workspace: PathBuf,

    /// Scheme to archive (default: "Runner")
    #[arg(long, default_value = "Runner")]
    pub scheme: String,

    /// Configuration to archive (default: "Release")
    #[arg(long, default_value = "Release")]
    pub configuration: String,

    /// Output .xcarchive path
    #[arg(long)]
    pub archive_path: PathBuf,

    /// Apple Developer Team ID (for signed archive, also from IOS2NIX_TEAM_ID env)
    #[arg(long)]
    pub team_id: Option<String>,

    /// Signing identity name (e.g., "Apple Distribution: Example Corp (TEAM123456)")
    #[arg(long)]
    pub signing_identity: Option<String>,

    /// Provisioning profile specifier (profile name or UUID)
    #[arg(long)]
    pub profile_specifier: Option<String>,

    /// Path to keychain for signing
    #[arg(long)]
    pub keychain: Option<PathBuf>,

    /// Override the product bundle identifier (PRODUCT_BUNDLE_IDENTIFIER),
    /// e.g. to match a provisioning profile's exact App ID
    #[arg(long)]
    pub bundle_id: Option<String>,

    /// DerivedData path (xcodebuild -derivedDataPath); reuse across runs for warm builds
    #[arg(long)]
    pub derived_data: Option<PathBuf>,
}

pub struct ArchiveCommand {
    pub workspace: PathBuf,
    pub scheme: String,
    pub configuration: String,
    pub archive_path: PathBuf,
    pub signing: Option<SigningConfig>,
    pub bundle_id: Option<String>,
    pub derived_data: Option<PathBuf>,
}

impl ArchiveCommand {
    /// Create a new ArchiveCommand from ArchiveArgs, validating signing fields.
    pub fn from_args(args: &ArchiveArgs) -> anyhow::Result<Self> {
        // Check if any signing field is present
        let has_team_id = args.team_id.is_some();
        let has_identity = args.signing_identity.is_some();
        let has_profile = args.profile_specifier.is_some();
        let has_keychain = args.keychain.is_some();

        let signing = if has_team_id || has_identity || has_profile || has_keychain {
            // If any is present, all must be present
            if !(has_team_id && has_identity && has_profile && has_keychain) {
                anyhow::bail!("signing requires all of: --team-id, --signing-identity, --profile-specifier, --keychain");
            }
            Some(SigningConfig {
                team_id: args.team_id.clone().unwrap(),
                identity: args.signing_identity.clone().unwrap(),
                profile_specifier: args.profile_specifier.clone().unwrap(),
                keychain: args.keychain.clone().unwrap(),
            })
        } else {
            None
        };

        Ok(ArchiveCommand {
            workspace: args.workspace.clone(),
            scheme: args.scheme.clone(),
            configuration: args.configuration.clone(),
            archive_path: args.archive_path.clone(),
            signing,
            bundle_id: args.bundle_id.clone(),
            derived_data: args.derived_data.clone(),
        })
    }
}

fn xcodebuild_args(cmd: &ArchiveCommand) -> Vec<String> {
    let mut args = vec!["archive".to_string()];
    args.push("-workspace".to_string());
    args.push(cmd.workspace.to_string_lossy().to_string());
    args.push("-scheme".to_string());
    args.push(cmd.scheme.clone());
    args.push("-configuration".to_string());
    args.push(cmd.configuration.clone());
    args.push("-archivePath".to_string());
    args.push(cmd.archive_path.to_string_lossy().to_string());
    args.push("-destination".to_string());
    args.push("generic/platform=iOS".to_string());

    if let Some(derived_data) = &cmd.derived_data {
        args.push("-derivedDataPath".to_string());
        args.push(derived_data.to_string_lossy().to_string());
    }

    if let Some(signing) = &cmd.signing {
        args.push(format!("DEVELOPMENT_TEAM={}", signing.team_id));
        args.push("CODE_SIGN_STYLE=Manual".to_string());
        args.push(format!("CODE_SIGN_IDENTITY={}", signing.identity));
        args.push(format!(
            "PROVISIONING_PROFILE_SPECIFIER={}",
            signing.profile_specifier
        ));
        args.push(format!(
            "OTHER_CODE_SIGN_FLAGS=--keychain {}",
            signing.keychain.to_string_lossy()
        ));
    } else {
        args.push("CODE_SIGNING_ALLOWED=NO".to_string());
    }

    if let Some(bundle_id) = &cmd.bundle_id {
        args.push(format!("PRODUCT_BUNDLE_IDENTIFIER={}", bundle_id));
    }

    args
}

/// Verify .xcarchive structure: must contain Products/Applications/<exactly-one>.app/Info.plist.
/// Returns Ok(path to .app) on success.
pub fn verify_archive_structure(archive: &Path) -> anyhow::Result<PathBuf> {
    let apps_dir = archive.join("Products/Applications");
    if !apps_dir.exists() {
        anyhow::bail!("archive missing Products/Applications directory");
    }

    let mut app_dirs = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&apps_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && path.extension().is_some_and(|ext| ext == "app") {
                app_dirs.push(path);
            }
        }
    }

    if app_dirs.len() != 1 {
        anyhow::bail!(
            "archive must contain exactly one .app directory, found {}",
            app_dirs.len()
        );
    }

    let app_path = &app_dirs[0];
    let info_plist = app_path.join("Info.plist");
    if !info_plist.exists() {
        anyhow::bail!("archive .app missing Info.plist");
    }

    Ok(app_path.clone())
}

pub fn run(cmd: ArchiveCommand) -> anyhow::Result<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        let env = crate::xcode::env::setup_xcode_env()?;

        let mut xcode_cmd = std::process::Command::new("xcodebuild");
        env.apply_to(&mut xcode_cmd);
        xcode_cmd.args(xcodebuild_args(&cmd));

        let output = xcode_cmd
            .output()
            .map_err(|e| anyhow::anyhow!("failed to run xcodebuild: {}", e))?;

        if !output.status.success() {
            anyhow::bail!(
                "xcodebuild archive failed:\n{}",
                crate::cli::failure_detail(&output)
            );
        }

        verify_archive_structure(&cmd.archive_path)?;
        Ok(cmd.archive_path)
    }

    #[cfg(not(target_os = "macos"))]
    {
        anyhow::bail!("ios2nix archive requires macOS")
    }
}

#[cfg(test)]
#[path = "archive_tests.rs"]
mod tests;
