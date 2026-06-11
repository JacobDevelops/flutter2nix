use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct XcodeEnv {
    pub developer_dir: PathBuf,
    pub sdkroot: PathBuf,
}

/// Resolve DEVELOPER_DIR from environment or fallback.
/// - If DEVELOPER_DIR is set and starts with /nix/store, ignore it (toolchain pollution per spike Finding 4).
/// - If set otherwise, validate that <dir>/usr/bin/xcodebuild exists; error if not.
/// - If unset/ignored, use fallback() to resolve (e.g., xcode-select -p).
pub fn resolve_developer_dir(
    get: impl Fn(&str) -> Option<String>,
    fallback: impl Fn() -> anyhow::Result<PathBuf>,
) -> anyhow::Result<PathBuf> {
    if let Some(env_val) = get("DEVELOPER_DIR") {
        // Ignore Nix store pollution (spike Finding 4).
        if env_val.starts_with("/nix/store") {
            return fallback();
        }

        // Validate the user-provided path.
        let dir = PathBuf::from(&env_val);
        let xcodebuild_path = dir.join("usr/bin/xcodebuild");
        if xcodebuild_path.exists() {
            return Ok(dir);
        } else {
            anyhow::bail!(
                "invalid Xcode path: {} (xcodebuild not found at {})",
                env_val,
                xcodebuild_path.display()
            );
        }
    }

    fallback()
}

/// The PATH forced onto xcodebuild by `sanitized_env` — system dirs only.
/// Callers that need extra tools visible to script phases (e.g. a codesign
/// shim) prepend to this rather than relying on the ambient PATH.
pub const SANITIZED_PATH: &str = "/usr/bin:/bin:/usr/sbin:/sbin";

/// Drop environment variables that must never reach xcodebuild.
/// Strips NIX_*, CC, CXX, LD, SDKROOT, DEVELOPER_DIR, CPATH, LIBRARY_PATH, MACOSX_DEPLOYMENT_TARGET.
/// Forces PATH to `SANITIZED_PATH`.
/// Spike Finding 4: a Nix dev shell exports these vars; any reaching xcodebuild breaks device builds.
pub fn sanitized_env(vars: impl IntoIterator<Item = (String, String)>) -> Vec<(String, String)> {
    let blocked_prefixes = ["NIX_"];
    let blocked_exact = [
        "CC",
        "CXX",
        "LD",
        "SDKROOT",
        "DEVELOPER_DIR",
        "CPATH",
        "LIBRARY_PATH",
        "MACOSX_DEPLOYMENT_TARGET",
    ];

    vars.into_iter()
        .filter(|(k, _)| {
            k != "PATH"
                && !blocked_prefixes.iter().any(|prefix| k.starts_with(prefix))
                && !blocked_exact.contains(&k.as_str())
        })
        .chain(std::iter::once((
            "PATH".to_string(),
            SANITIZED_PATH.to_string(),
        )))
        .collect()
}

impl XcodeEnv {
    /// Apply this Xcode environment to a Command.
    /// Clears the environment, applies sanitized vars, and injects DEVELOPER_DIR and SDKROOT.
    pub fn apply_to(&self, cmd: &mut std::process::Command) {
        cmd.env_clear();

        let current_env: Vec<(String, String)> = std::env::vars().collect();
        let sanitized = sanitized_env(current_env);

        for (k, v) in sanitized {
            cmd.env(&k, &v);
        }

        cmd.env("DEVELOPER_DIR", &self.developer_dir);
        cmd.env("SDKROOT", &self.sdkroot);
    }
}

/// Setup Xcode environment: resolve DEVELOPER_DIR and compute SDKROOT.
/// macOS arm: resolve_developer_dir via xcode-select -p, compute sdkroot as Platforms/iPhoneOS.platform/Developer/SDKs/iPhoneOS.sdk, error if sdkroot missing.
/// Linux arm: bail with message "xcode env requires macOS".
#[cfg(target_os = "macos")]
pub fn setup_xcode_env() -> anyhow::Result<XcodeEnv> {
    let developer_dir = resolve_developer_dir(
        |key| std::env::var(key).ok(),
        || {
            // xcode-select -p echoes $DEVELOPER_DIR when set — strip it, or a
            // polluted Nix dev shell feeds the apple-sdk store path back to us.
            let output = std::process::Command::new("xcode-select")
                .env_remove("DEVELOPER_DIR")
                .arg("-p")
                .output()
                .map_err(|e| anyhow::anyhow!("failed to run xcode-select: {}", e))?;

            if !output.status.success() {
                anyhow::bail!("xcode-select failed");
            }

            let path_str = String::from_utf8_lossy(&output.stdout);
            let path = path_str.trim().to_string();
            if path.is_empty() {
                anyhow::bail!("xcode-select returned empty path");
            }

            Ok(PathBuf::from(path))
        },
    )?;

    let sdkroot = developer_dir.join("Platforms/iPhoneOS.platform/Developer/SDKs/iPhoneOS.sdk");
    if !sdkroot.exists() {
        anyhow::bail!("iPhoneOS SDK not found at {}", sdkroot.display());
    }

    Ok(XcodeEnv {
        developer_dir,
        sdkroot,
    })
}

#[cfg(not(target_os = "macos"))]
pub fn setup_xcode_env() -> anyhow::Result<XcodeEnv> {
    anyhow::bail!("xcode env requires macOS")
}

#[cfg(test)]
#[path = "env_tests.rs"]
mod tests;
