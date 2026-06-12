use clap::Parser;
use std::path::PathBuf;

use crate::xcode::build_output::XcodeBuildOutput;

#[derive(Parser, Debug, Clone)]
pub struct BuildArgs {
    /// Project directory (default: ".")
    #[arg(long, default_value = ".")]
    pub project_dir: PathBuf,

    /// Workspace path (default: <project_dir>/Runner.xcworkspace)
    #[arg(long)]
    pub workspace: Option<PathBuf>,

    /// Scheme to build (default: "Runner")
    #[arg(long, default_value = "Runner")]
    pub scheme: String,

    /// Configuration to build (default: "Release")
    #[arg(long, default_value = "Release")]
    pub configuration: String,

    /// Derived data path
    #[arg(long)]
    pub derived_data: Option<PathBuf>,
}

pub struct BuildCommand {
    pub project_dir: PathBuf,
    pub workspace: PathBuf,
    pub scheme: String,
    pub configuration: String,
    pub derived_data: Option<PathBuf>,
}

fn xcodebuild_args(cmd: &BuildCommand) -> Vec<String> {
    let mut args = vec!["build".to_string()];
    args.push("-workspace".to_string());
    args.push(cmd.workspace.to_string_lossy().to_string());
    args.push("-scheme".to_string());
    args.push(cmd.scheme.clone());
    args.push("-configuration".to_string());
    args.push(cmd.configuration.clone());
    args.push("-destination".to_string());
    args.push("generic/platform=iOS".to_string());
    if let Some(dd) = &cmd.derived_data {
        args.push("-derivedDataPath".to_string());
        args.push(dd.to_string_lossy().to_string());
    }
    args.push("CODE_SIGNING_ALLOWED=NO".to_string());
    args
}

pub fn run(cmd: BuildCommand) -> anyhow::Result<XcodeBuildOutput> {
    // Sidecar short-circuit: if <project_dir>/.ios2nix-xcode-output.json exists, read and parse it.
    let sidecar_path = cmd.project_dir.join(".ios2nix-xcode-output.json");
    if sidecar_path.exists() {
        let json = std::fs::read_to_string(&sidecar_path).map_err(|e| {
            anyhow::anyhow!("failed to read sidecar {}: {}", sidecar_path.display(), e)
        })?;
        return crate::xcode::build_output::parse_xcode_build_output(&json);
    }

    // macOS arm: setup env, pod install (if needed), xcodebuild build.
    #[cfg(target_os = "macos")]
    {
        let env = crate::xcode::env::setup_xcode_env()?;

        // Run pod install if Podfile exists and Pods doesn't.
        if cmd.project_dir.join("Podfile").exists() && !cmd.project_dir.join("Pods").exists() {
            let mut pod_cmd = std::process::Command::new("pod");
            pod_cmd.current_dir(&cmd.project_dir);
            pod_cmd.arg("install");
            pod_cmd.arg("--no-repo-update");

            let output = pod_cmd
                .output()
                .map_err(|e| anyhow::anyhow!("failed to run pod install: {}", e))?;

            if !output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                anyhow::bail!(
                    "pod install failed:\nstdout:\n{}\nstderr:\n{}",
                    stdout,
                    stderr
                );
            }
        }

        let mut xcode_cmd = std::process::Command::new("xcodebuild");
        env.apply_to(&mut xcode_cmd);
        xcode_cmd.args(xcodebuild_args(&cmd));

        let output = xcode_cmd
            .output()
            .map_err(|e| anyhow::anyhow!("failed to run xcodebuild: {}", e))?;

        if !output.status.success() {
            anyhow::bail!(
                "xcodebuild build failed:\n{}",
                crate::cli::failure_detail(&output)
            );
        }

        // Get Xcode version and synthesize output.
        let mut version_cmd = std::process::Command::new("xcodebuild");
        env.apply_to(&mut version_cmd);
        version_cmd.arg("-version");

        let version_output = version_cmd
            .output()
            .map_err(|e| anyhow::anyhow!("failed to get xcodebuild version: {}", e))?;

        let version_str = String::from_utf8_lossy(&version_output.stdout);
        let version = version_str
            .lines()
            .next()
            .and_then(|line| line.strip_prefix("Xcode "))
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("could not parse Xcode version"))?;

        crate::xcode::assert::assert_xcode_version(&version, "15.0")?;

        Ok(XcodeBuildOutput {
            version,
            architectures: vec!["arm64".to_string()], // generic/platform=iOS device builds are arm64-only
            frameworks: vec![],
            codesign_identity: None,
        })
    }

    #[cfg(not(target_os = "macos"))]
    {
        anyhow::bail!("ios2nix build requires macOS (or a .ios2nix-xcode-output.json sidecar)")
    }
}

#[cfg(test)]
#[path = "build_tests.rs"]
mod tests;
