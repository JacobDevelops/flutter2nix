use std::path::{Path, PathBuf};
use std::process::Command;

/// A temporary keychain for code signing (macOS only).
/// RAII: Drop deletes the keychain file.
///
/// Deliberate deviation from plan §2's "optional but recommended" step: the
/// user's *default* keychain is never switched. Every signing path passes the
/// keychain explicitly (`OTHER_CODE_SIGN_FLAGS=--keychain` on archive,
/// `codesign --keychain` on re-sign) and `add_to_search_list` covers lookup,
/// while a default-keychain switch would leave the host misconfigured if the
/// process died before restoring it.
#[cfg(target_os = "macos")]
pub struct TempKeychain {
    path: PathBuf,
    password: String,
    persist: bool,
}

#[cfg(target_os = "macos")]
impl TempKeychain {
    /// Create a temporary keychain with the given password.
    /// The keychain is created, unlocked, and configured with reasonable timeout settings.
    pub fn create(password: &str) -> anyhow::Result<Self> {
        use anyhow::Context;

        // Create a unique keychain path in a temp directory
        let keychain_path = tempfile::NamedTempFile::new()
            .context("failed to create temp keychain path")?
            .into_temp_path()
            .to_path_buf();

        // security create-keychain -p "$KPW" "$KC"
        let output = Command::new("security")
            .args(["create-keychain", "-p"])
            .arg(password)
            .arg(&keychain_path)
            .output()
            .context("failed to run 'security create-keychain'")?;

        if !output.status.success() {
            anyhow::bail!(
                "security create-keychain failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        // security set-keychain-settings -lut 21600 "$KC"
        let output = Command::new("security")
            .args(["set-keychain-settings", "-lut", "21600"])
            .arg(&keychain_path)
            .output()
            .context("failed to run 'security set-keychain-settings'")?;

        if !output.status.success() {
            anyhow::bail!(
                "security set-keychain-settings failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        // security unlock-keychain -p "$KPW" "$KC"
        let output = Command::new("security")
            .args(["unlock-keychain", "-p"])
            .arg(password)
            .arg(&keychain_path)
            .output()
            .context("failed to run 'security unlock-keychain'")?;

        if !output.status.success() {
            anyhow::bail!(
                "security unlock-keychain failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        Ok(TempKeychain {
            path: keychain_path,
            password: password.to_string(),
            persist: false,
        })
    }

    /// Return the path to the keychain.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Import a PKCS12 identity (certificate + private key) into the keychain.
    /// This also sets up partition-list to allow non-interactive codesign use.
    pub fn import_identity(&self, p12: &Path, p12_password: &str) -> anyhow::Result<()> {
        use anyhow::Context;

        // security import "$P12" -P "$P12PW" -k "$KC" -T /usr/bin/codesign -T /usr/bin/security -f pkcs12
        // (the input file must directly follow `import`)
        let output = Command::new("security")
            .arg("import")
            .arg(p12)
            .args(["-f", "pkcs12", "-P", p12_password, "-k"])
            .arg(&self.path)
            .args(["-T", "/usr/bin/codesign", "-T", "/usr/bin/security"])
            .output()
            .context("failed to run 'security import'")?;

        if !output.status.success() {
            anyhow::bail!(
                "security import failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        // THE CRITICAL STEP: set-key-partition-list
        // security set-key-partition-list -S apple-tool:,apple:,codesign: -s -k "$KPW" "$KC"
        let output = Command::new("security")
            .args([
                "set-key-partition-list",
                "-S",
                "apple-tool:,apple:,codesign:",
                "-s",
                "-k",
            ])
            .arg(&self.password)
            .arg(&self.path)
            .output()
            .context("failed to run 'security set-key-partition-list'")?;

        if !output.status.success() {
            anyhow::bail!(
                "security set-key-partition-list failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        Ok(())
    }

    /// Add this keychain to the search list (prepend, preserving existing entries).
    pub fn add_to_search_list(&self) -> anyhow::Result<()> {
        use anyhow::Context;

        // Get current search list: security list-keychains -d user
        let output = Command::new("security")
            .args(["list-keychains", "-d", "user"])
            .output()
            .context("failed to run 'security list-keychains -d user'")?;

        if !output.status.success() {
            anyhow::bail!(
                "security list-keychains -d user failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        let current_list_str = String::from_utf8_lossy(&output.stdout);
        let mut existing: Vec<String> = current_list_str
            .lines()
            .map(|line| line.trim().trim_matches('"').to_string())
            .filter(|s| !s.is_empty())
            .collect();

        // Prepend our keychain
        let keychain_path_str = self.path.to_string_lossy().to_string();
        existing.insert(0, keychain_path_str);

        // Set the new search list: security list-keychains -d user -s KC <existing...>
        let mut cmd = Command::new("security");
        cmd.args(["list-keychains", "-d", "user", "-s"]);
        for keychain in existing {
            cmd.arg(&keychain);
        }

        let output = cmd
            .output()
            .context("failed to run 'security list-keychains -d user -s'")?;

        if !output.status.success() {
            anyhow::bail!(
                "security list-keychains -d user -s failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        Ok(())
    }

    /// Get the list of signing identities available in the keychain.
    pub fn signing_identities(&self) -> anyhow::Result<Vec<String>> {
        use anyhow::Context;

        // security find-identity -v -p codesigning <KC>
        let output = Command::new("security")
            .args(["find-identity", "-v", "-p", "codesigning"])
            .arg(&self.path)
            .output()
            .context("failed to run 'security find-identity'")?;

        if !output.status.success() {
            anyhow::bail!(
                "security find-identity failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut identities = Vec::new();

        for line in stdout.lines() {
            // Format: "  1) XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX \"Apple Distribution: Example Corp (ABCD123456)\""
            // Extract the quoted identity name
            if let Some(start) = line.find('"') {
                if let Some(end) = line.rfind('"') {
                    if start < end {
                        let identity = line[start + 1..end].to_string();
                        identities.push(identity);
                    }
                }
            }
        }

        Ok(identities)
    }

    /// Prevent Drop from deleting the keychain; return the path.
    /// Used when the keychain must outlive the process (e.g., sign-setup Nix subcommand).
    pub fn persist(mut self) -> PathBuf {
        self.persist = true;
        self.path.clone()
    }
}

#[cfg(target_os = "macos")]
impl Drop for TempKeychain {
    fn drop(&mut self) {
        if self.persist {
            return;
        }

        // Ignore errors — best effort cleanup
        let _ = Command::new("security")
            .args(["delete-keychain"])
            .arg(&self.path)
            .output();
    }
}

#[cfg(test)]
#[path = "keychain_tests.rs"]
mod tests;
