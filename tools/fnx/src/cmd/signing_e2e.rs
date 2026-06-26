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

/// Load the IOS2NIX_* signing vars from the local env file, ready to inject
/// into a child process: password file resolved, required keys validated, and
/// a fresh throwaway keychain password generated when not supplied. Returns
/// `None` (no error) when **real** material is not fully usable: the host is
/// not macOS, the file is absent, or any path it references (p12, password
/// file, profile) is missing/unreadable. A dangling env file is treated as
/// "no real material" rather than an error so the signing e2e can fall back to
/// simulate mode (and the build benchmark, which needs real material, skips).
pub fn load_signing_vars(repo_root: &Path) -> anyhow::Result<Option<BTreeMap<String, String>>> {
    let env_file = repo_root.join(SIGNING_ENV_FILE);
    if !env_file.exists() || !cfg!(target_os = "macos") {
        return Ok(None);
    }

    let mut vars = parse_env_file(&env_file)?;

    // Resolve the indirect password file. A missing/unreadable password file
    // means the real material is gone — fall back rather than erroring.
    if let Some(pw_file) = vars.remove("IOS2NIX_P12_PASSWORD_FILE") {
        let Ok(pw) = std::fs::read_to_string(&pw_file) else {
            return Ok(None);
        };
        vars.insert(
            "IOS2NIX_P12_PASSWORD".to_string(),
            pw.trim_end().to_string(),
        );
    }

    validate_required(&vars, &env_file)?;

    // The p12 and profile the contract points at must actually exist on disk;
    // otherwise sign-setup would fail. Treat dangling refs as "no real material".
    let referenced_paths_exist = ["IOS2NIX_P12_PATH", "IOS2NIX_PROFILE_PATH"]
        .iter()
        .all(|key| vars.get(*key).is_some_and(|p| Path::new(p).exists()));
    if !referenced_paths_exist {
        return Ok(None);
    }

    // Throwaway password for the temp keychain the consumer creates (and deletes).
    vars.entry("IOS2NIX_KEYCHAIN_PASSWORD".to_string())
        .or_insert(random_password()?);

    Ok(Some(vars))
}

/// Run the `#[ignore]`-gated iOS signing integration tests against the best
/// available signing material:
///
/// * **Real material present and fully readable** → run the whole ignored
///   `cli_tests` suite against it (the original e2e path, unchanged).
/// * **Otherwise, on macOS** → mint throwaway self-signed material and run only
///   the hermetic `simulate` test against it (it drives the real ios2nix
///   sign-setup keychain import, provisioning parse, and re-sign + verify
///   paths — the parts a self-signed cert can satisfy without Apple trust).
/// * **Non-macOS** → skip with a note.
///
/// CI never reaches this: fnx is local-dev only and CI's `cargo test` does not
/// run ignored tests.
pub fn run_if_configured(repo_root: &Path) -> anyhow::Result<()> {
    if let Some(vars) = load_signing_vars(repo_root)? {
        eprintln!(
            "fnx check: running iOS signing e2e (real material; cargo test -p ios2nix --test cli_tests -- --ignored)..."
        );
        return run_cli_tests(repo_root, &vars, None);
    }

    if !cfg!(target_os = "macos") {
        eprintln!("fnx check: iOS signing e2e skipped (not macOS)");
        return Ok(());
    }

    eprintln!(
        "fnx check: no usable real signing material ({SIGNING_ENV_FILE} absent or its referenced \
         p12/password/profile missing) — simulating with throwaway self-signed material..."
    );
    let material =
        super::signing_sim::mint().context("failed to mint simulated signing material")?;
    eprintln!(
        "fnx check: minted self-signed signing material; running iOS signing e2e (simulate; \
         cargo test -p ios2nix --test cli_tests -- --ignored simulate)..."
    );
    run_cli_tests(repo_root, material.vars(), Some("simulate"))
}

/// Invoke the ignored `cli_tests` suite with the given IOS2NIX_* vars in the
/// child env. An optional `name_filter` restricts the run to matching tests
/// (used to select only the hermetic `simulate` test).
fn run_cli_tests(
    repo_root: &Path,
    vars: &BTreeMap<String, String>,
    name_filter: Option<&str>,
) -> anyhow::Result<()> {
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
    ]);
    if let Some(filter) = name_filter {
        cmd.arg(filter);
    }
    cmd.current_dir(repo_root);
    for (key, value) in vars {
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
