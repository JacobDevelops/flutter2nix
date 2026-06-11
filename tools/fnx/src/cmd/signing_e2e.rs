use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;
use std::process::Command;

use anyhow::Context;

/// Local, untracked env file gating the iOS signing e2e suite — the same role
/// the fixture-lockfile `pathExists` gates play for the Nix e2e derivations.
/// Holds the IOS2NIX_* signing contract (paths/IDs only — the .p12 password is
/// referenced via IOS2NIX_P12_PASSWORD_FILE so no secret lives in the file).
pub const SIGNING_ENV_FILE: &str = ".ios2nix-signing.env";

const REQUIRED_KEYS: [&str; 5] = [
    "IOS2NIX_P12_PATH",
    "IOS2NIX_P12_PASSWORD",
    "IOS2NIX_PROFILE_PATH",
    "IOS2NIX_TEAM_ID",
    "IOS2NIX_SIGNING_IDENTITY",
];

/// Run the `#[ignore]`-gated iOS signing integration tests when signing
/// material is configured; skip with a note otherwise. CI never reaches this:
/// fnx is local-dev only and CI's `cargo test` does not run ignored tests.
pub fn run_if_configured(repo_root: &Path) -> anyhow::Result<()> {
    let env_file = repo_root.join(SIGNING_ENV_FILE);
    if !env_file.exists() {
        eprintln!("fnx check: iOS signing e2e skipped ({SIGNING_ENV_FILE} not present)");
        return Ok(());
    }
    if !cfg!(target_os = "macos") {
        eprintln!("fnx check: iOS signing e2e skipped (requires macOS)");
        return Ok(());
    }

    let mut vars = parse_env_file(&env_file)?;

    if let Some(pw_file) = vars.remove("IOS2NIX_P12_PASSWORD_FILE") {
        let pw = std::fs::read_to_string(&pw_file)
            .with_context(|| format!("failed to read IOS2NIX_P12_PASSWORD_FILE {pw_file}"))?;
        vars.insert(
            "IOS2NIX_P12_PASSWORD".to_string(),
            pw.trim_end().to_string(),
        );
    }

    validate_required(&vars, &env_file)?;

    // Throwaway password for the temp keychain the tests create (and delete).
    vars.entry("IOS2NIX_KEYCHAIN_PASSWORD".to_string())
        .or_insert(random_password()?);

    eprintln!(
        "fnx check: running iOS signing e2e (cargo test -p ios2nix --test cli_tests -- --ignored)..."
    );
    let mut cmd = Command::new("cargo");
    cmd.args([
        "test",
        "-p",
        "ios2nix",
        "--test",
        "cli_tests",
        "--",
        "--ignored",
        "--test-threads=1",
    ])
    .current_dir(repo_root);
    for (key, value) in &vars {
        cmd.env(key, value);
    }

    let status = cmd.status()?;
    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }
    Ok(())
}

/// Parse a KEY=VALUE env file ('#' comments and blank lines allowed).
fn parse_env_file(path: &Path) -> anyhow::Result<BTreeMap<String, String>> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;

    let mut vars = BTreeMap::new();
    for (lineno, line) in content.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (key, value) = line
            .split_once('=')
            .with_context(|| format!("{}:{}: expected KEY=VALUE", path.display(), lineno + 1))?;
        vars.insert(key.trim().to_string(), value.trim().to_string());
    }
    Ok(vars)
}

fn validate_required(vars: &BTreeMap<String, String>, env_file: &Path) -> anyhow::Result<()> {
    let missing: Vec<&str> = REQUIRED_KEYS
        .iter()
        .filter(|k| !vars.contains_key(**k))
        .copied()
        .collect();
    if !missing.is_empty() {
        anyhow::bail!(
            "{} is missing required keys: {} (IOS2NIX_P12_PASSWORD may be supplied \
             indirectly via IOS2NIX_P12_PASSWORD_FILE)",
            env_file.display(),
            missing.join(", ")
        );
    }
    Ok(())
}

/// 32 hex chars from /dev/urandom — never logged, lives only in the child env.
fn random_password() -> anyhow::Result<String> {
    let mut buf = [0u8; 16];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut buf))
        .context("failed to read /dev/urandom for keychain password")?;
    Ok(buf.iter().map(|b| format!("{b:02x}")).collect())
}

#[cfg(test)]
#[path = "signing_e2e_tests.rs"]
mod tests;
