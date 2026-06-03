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
mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[tokio::test]
    async fn test_tapi_invocation_timeout() {
        todo!("Phase 1: stub — config with timeout_secs:1, simulated slow JVM, expect Err containing 'timeout'")
    }

    #[tokio::test]
    async fn test_tapi_invocation_jvm_not_found() {
        todo!("Phase 1: stub — config pointing to non-existent project, expect Err containing 'java' or 'not found'")
    }
}
