use std::path::PathBuf;

pub struct TapiShimConfig {
    pub gradle_project_dir: PathBuf,
    pub gradle_user_home: Option<PathBuf>,
    pub timeout_secs: u64,
    /// If Some, skip JVM invocation and return this string directly.
    /// Set to fixture JSON in tests; always None in production.
    pub tapi_json_override: Option<String>,
}

pub async fn invoke_tapi_shim(_config: TapiShimConfig) -> anyhow::Result<String> {
    todo!("Phase 1: API contract only")
}

#[cfg(test)]
#[path = "shim_tests.rs"]
mod tests;
