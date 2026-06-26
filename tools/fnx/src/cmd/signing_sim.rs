//! Self-signed "simulate" mode for the iOS signing e2e suite.
//!
//! When real Apple distribution material is unavailable, this module mints
//! throwaway self-signed signing material at runtime so the ios2nix signing
//! pipeline can be exercised hermetically — no real secrets required:
//!
//!   1. A self-signed code-signing certificate + RSA key (openssl), carrying
//!      the codeSigning EKU so `security import` accepts it as an identity.
//!   2. A `.p12` bundling that key + cert, protected by a generated password.
//!   3. A CMS/PKCS#7-signed `.mobileprovision` (signed with the self-signed
//!      cert via `openssl smime`) carrying the fields ios2nix's provisioning
//!      parser reads (UUID, Name, TeamIdentifier, Entitlements, ExpirationDate).
//!
//! Everything lives under a temp dir owned by [`SimMaterial`]; its `Drop`
//! removes the tree. Secrets are passed only through the child process env and
//! are never logged or written into the repo.
//!
//! ## Why a self-signed cert can't drive `codesign -s <identity>`
//!
//! macOS refuses to code-sign with an untrusted certificate
//! (`CSSMERR_TP_NOT_TRUSTED`), and the only non-interactive way to establish
//! trust requires admin/sudo or a blocking GUI prompt — neither acceptable in
//! an unattended `fnx check`. The simulate integration test therefore drives
//! the real `ios2nix::cli::sign` re-sign path with an ad-hoc signature (which
//! the `sign` command explicitly supports and which yields a
//! `codesign --verify`-valid bundle), while the self-signed cert is still
//! exercised through the keychain import (`sign-setup`) and the CMS-signed
//! provisioning profile parse/install paths. The heavy `xcodebuild` device
//! archive tests fundamentally need an Apple-chained cert and are not run in
//! simulate mode.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

use anyhow::{bail, Context};
use tempfile::TempDir;

/// Identity common name baked into the generated cert and surfaced as
/// `IOS2NIX_SIGNING_IDENTITY`.
const SIM_IDENTITY: &str = "ios2nix Simulated Signing";
/// Team identifier baked into the synthesized profile and surfaced as
/// `IOS2NIX_TEAM_ID`. Ten chars, matching Apple's team-id shape.
const SIM_TEAM_ID: &str = "SIMTEAM123";
/// Bundle id baked into the synthesized profile's application-identifier.
const SIM_BUNDLE_ID: &str = "com.ios2nix.simulated";

/// Minted self-signed signing material. Owns the temp dir backing every path
/// referenced by the returned env vars; dropping it removes the tree.
pub struct SimMaterial {
    _dir: TempDir,
    vars: BTreeMap<String, String>,
}

impl SimMaterial {
    /// The `IOS2NIX_*` vars to inject into the signing-e2e child process —
    /// same shape `load_signing_vars` returns, plus `IOS2NIX_SIMULATE=1`.
    pub fn vars(&self) -> &BTreeMap<String, String> {
        &self.vars
    }
}

/// Mint self-signed signing material into a fresh temp dir.
pub fn mint() -> anyhow::Result<SimMaterial> {
    let dir = tempfile::TempDir::new().context("failed to create simulate material temp dir")?;
    let root = dir.path();

    let key_path = root.join("leaf.key");
    let cert_path = root.join("leaf.pem");
    let p12_path = root.join("identity.p12");
    let profile_path = root.join("simulated.mobileprovision");

    let p12_password = random_password()?;

    generate_self_signed_cert(&key_path, &cert_path)?;
    export_p12(&key_path, &cert_path, &p12_path, &p12_password)?;
    synthesize_profile(&key_path, &cert_path, &profile_path)?;

    let mut vars = BTreeMap::new();
    vars.insert(
        "IOS2NIX_P12_PATH".to_string(),
        p12_path.to_string_lossy().into_owned(),
    );
    vars.insert("IOS2NIX_P12_PASSWORD".to_string(), p12_password);
    vars.insert(
        "IOS2NIX_PROFILE_PATH".to_string(),
        profile_path.to_string_lossy().into_owned(),
    );
    vars.insert("IOS2NIX_TEAM_ID".to_string(), SIM_TEAM_ID.to_string());
    vars.insert(
        "IOS2NIX_SIGNING_IDENTITY".to_string(),
        SIM_IDENTITY.to_string(),
    );
    // Throwaway password for the temp keychain the consumer creates (and deletes).
    vars.insert("IOS2NIX_KEYCHAIN_PASSWORD".to_string(), random_password()?);
    // Marker the simulate integration test gates on.
    vars.insert("IOS2NIX_SIMULATE".to_string(), "1".to_string());

    Ok(SimMaterial { _dir: dir, vars })
}

/// Generate a self-signed RSA code-signing certificate + key via openssl.
/// The cert carries `extendedKeyUsage = codeSigning` and `digitalSignature`
/// key usage so `security import` registers it as a usable identity.
fn generate_self_signed_cert(key_path: &Path, cert_path: &Path) -> anyhow::Result<()> {
    let config = format!(
        "[req]\n\
         distinguished_name = dn\n\
         x509_extensions = v3\n\
         prompt = no\n\
         [dn]\n\
         CN = {SIM_IDENTITY}\n\
         O = ios2nix\n\
         C = US\n\
         [v3]\n\
         keyUsage = critical, digitalSignature\n\
         extendedKeyUsage = critical, codeSigning\n\
         basicConstraints = critical, CA:false\n"
    );
    let config_path = key_path.with_extension("cnf");
    std::fs::write(&config_path, config).context("failed to write openssl config")?;

    run_quiet(
        Command::new("openssl").args([
            "req",
            "-x509",
            "-newkey",
            "rsa:2048",
            "-keyout",
            &key_path.to_string_lossy(),
            "-out",
            &cert_path.to_string_lossy(),
            "-days",
            "30",
            "-nodes",
            "-config",
            &config_path.to_string_lossy(),
        ]),
        "openssl req (self-signed cert)",
    )
}

/// Export the key + cert into a password-protected `.p12`, labelled with the
/// identity common name so the keychain surfaces it under that name.
fn export_p12(
    key_path: &Path,
    cert_path: &Path,
    p12_path: &Path,
    password: &str,
) -> anyhow::Result<()> {
    run_quiet(
        Command::new("openssl").args([
            "pkcs12",
            "-export",
            "-inkey",
            &key_path.to_string_lossy(),
            "-in",
            &cert_path.to_string_lossy(),
            "-out",
            &p12_path.to_string_lossy(),
            "-passout",
            &format!("pass:{password}"),
            "-name",
            SIM_IDENTITY,
        ]),
        "openssl pkcs12 (p12 export)",
    )
}

/// Synthesize a CMS/PKCS#7-signed `.mobileprovision` from a plist carrying the
/// fields ios2nix's provisioning parser reads, signed with the self-signed
/// cert. `openssl smime -sign ... -outform DER -nodetach` produces a CMS blob
/// `security cms -D` (the ios2nix decode path) accepts — unlike `security cms
/// -S`, which refuses an untrusted signing identity.
fn synthesize_profile(
    key_path: &Path,
    cert_path: &Path,
    profile_path: &Path,
) -> anyhow::Result<()> {
    let uuid = sim_uuid()?;
    let plist = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
         <plist version=\"1.0\"><dict>\n\
         \x20 <key>AppIDName</key><string>ios2nix Simulated</string>\n\
         \x20 <key>ApplicationIdentifierPrefix</key><array><string>{SIM_TEAM_ID}</string></array>\n\
         \x20 <key>CreationDate</key><date>2026-01-01T00:00:00Z</date>\n\
         \x20 <key>Platform</key><array><string>iOS</string></array>\n\
         \x20 <key>IsXcodeManaged</key><false/>\n\
         \x20 <key>Entitlements</key><dict>\n\
         \x20   <key>application-identifier</key><string>{SIM_TEAM_ID}.{SIM_BUNDLE_ID}</string>\n\
         \x20   <key>com.apple.developer.team-identifier</key><string>{SIM_TEAM_ID}</string>\n\
         \x20   <key>get-task-allow</key><false/>\n\
         \x20   <key>keychain-access-groups</key><array><string>{SIM_TEAM_ID}.*</string></array>\n\
         \x20 </dict>\n\
         \x20 <key>ExpirationDate</key><date>2099-01-01T00:00:00Z</date>\n\
         \x20 <key>Name</key><string>ios2nix Simulated Distribution</string>\n\
         \x20 <key>TeamIdentifier</key><array><string>{SIM_TEAM_ID}</string></array>\n\
         \x20 <key>TeamName</key><string>ios2nix Simulated</string>\n\
         \x20 <key>TimeToLive</key><integer>365</integer>\n\
         \x20 <key>UUID</key><string>{uuid}</string>\n\
         \x20 <key>Version</key><integer>1</integer>\n\
         </dict></plist>\n"
    );
    let plist_path = profile_path.with_extension("plist");
    std::fs::write(&plist_path, plist).context("failed to write profile plist")?;

    run_quiet(
        Command::new("openssl").args([
            "smime",
            "-sign",
            "-in",
            &plist_path.to_string_lossy(),
            "-out",
            &profile_path.to_string_lossy(),
            "-signer",
            &cert_path.to_string_lossy(),
            "-inkey",
            &key_path.to_string_lossy(),
            "-outform",
            "DER",
            "-nodetach",
        ]),
        "openssl smime (CMS-sign profile)",
    )
}

/// Run a command, discarding stdout/stderr unless it fails (then surface them).
fn run_quiet(cmd: &mut Command, label: &str) -> anyhow::Result<()> {
    let output = cmd
        .output()
        .with_context(|| format!("failed to run {label}"))?;
    if !output.status.success() {
        bail!(
            "{label} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

/// A lowercase UUID in 8-4-4-4-12 form, sourced from `uuidgen`.
fn sim_uuid() -> anyhow::Result<String> {
    let output = Command::new("uuidgen")
        .output()
        .context("failed to run uuidgen")?;
    if !output.status.success() {
        bail!("uuidgen failed");
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .trim()
        .to_lowercase())
}

/// 32 hex chars from `/dev/urandom` — never logged, lives only in the child env.
fn random_password() -> anyhow::Result<String> {
    use std::io::Read;
    let mut buf = [0u8; 16];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut buf))
        .context("failed to read /dev/urandom for password")?;
    Ok(buf.iter().map(|b| format!("{b:02x}")).collect())
}
