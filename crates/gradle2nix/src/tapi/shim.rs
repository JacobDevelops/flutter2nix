use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;

pub struct TapiShimConfig {
    pub gradle_project_dir: PathBuf,
    pub gradle_user_home: Option<PathBuf>,
    pub gradle_cache_dir: Option<PathBuf>,
    pub timeout_secs: u64,
    /// If Some, skip JVM invocation and return this string directly.
    /// Set to fixture JSON in tests; always None in production.
    pub tapi_json_override: Option<String>,
    /// If Some, run this command instead of `java -jar <jar>`.
    /// Used in tests to inject a controllable slow process (e.g. `sleep 60`).
    /// Always None in production.
    pub test_command: Option<Vec<String>>,
}

pub async fn invoke_tapi_shim(config: TapiShimConfig) -> anyhow::Result<String> {
    // Test injection: bypass real JVM invocation
    if let Some(json) = config.tapi_json_override {
        return Ok(json);
    }

    // Keeps the extracted JAR alive for the lifetime of the java process;
    // dropping it (normal return, error, or timeout) deletes the extraction.
    let mut _jar_guard: Option<super::jar_source::ShimJar> = None;

    // Build command: test injection or real JVM
    let mut cmd = if let Some(argv) = config.test_command {
        let mut c = Command::new(&argv[0]);
        c.args(&argv[1..]);
        c
    } else {
        // Resolve JAR
        let source = super::jar_source::select_tapi_shim_source()?;
        let jar = super::jar_source::extract_jar_to_temp(source)?;

        let project_dir = config.gradle_project_dir.canonicalize().map_err(|e| {
            anyhow::anyhow!("Gradle project directory not found: {}", e)
        })?;

        let mut c = Command::new("java");
        // TapiShim.kt passes --no-configuration-cache internally via .withArguments()
        c.arg("-jar").arg(&jar.path).arg(project_dir);
        if let Some(gradle_home) = config.gradle_user_home {
            c.env("GRADLE_USER_HOME", gradle_home);
        }
        _jar_guard = Some(jar);
        c
    };
    cmd.stdout(Stdio::piped()).stderr(Stdio::inherit());
    // On timeout the future is dropped before the child exits — without this the
    // orphaned JVM keeps running and its own temp Gradle home is never cleaned.
    cmd.kill_on_drop(true);

    let duration = Duration::from_secs(config.timeout_secs);
    let child = cmd.spawn().map_err(|e| {
        anyhow::anyhow!("failed to spawn java: {} — is JVM installed?", e)
    })?;

    let output = timeout(duration, child.wait_with_output())
        .await
        .map_err(|_| anyhow::anyhow!("TAPI shim timed out after {}s", config.timeout_secs))?
        .map_err(|e| anyhow::anyhow!("TAPI shim process error: {}", e))?;

    if !output.status.success() {
        anyhow::bail!(
            "TAPI shim exited with status {}: check stderr for details",
            output.status
        );
    }

    let json = String::from_utf8(output.stdout)
        .map_err(|e| anyhow::anyhow!("TAPI shim output is not valid UTF-8: {}", e))?;

    Ok(json)
}

#[cfg(test)]
#[path = "shim_tests.rs"]
mod tests;
