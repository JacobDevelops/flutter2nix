use std::path::PathBuf;

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct MavenCoordinate {
    pub group: String,
    pub artifact: String,
    pub version: String,
    pub classifier: Option<String>,
    pub extension: String,
}

impl MavenCoordinate {
    /// Parse a Maven coordinate string.
    /// Formats: "group:artifact:version", "group:artifact:version:classifier",
    /// "group:artifact:version@extension"
    pub fn parse(_s: &str) -> anyhow::Result<Self> {
        todo!("Phase 1: API contract only")
    }

    /// Convert to canonical Maven repository path.
    /// e.g. "com.google.guava:guava:31.1-jre" → "com/google/guava/guava/31.1-jre/guava-31.1-jre.jar"
    pub fn to_artifact_path(&self) -> String {
        todo!("Phase 1: API contract only")
    }
}

pub struct MavenResolverConfig {
    pub repository_urls: Vec<String>,
    pub local_cache_dir: Option<PathBuf>,
    pub timeout_secs: u64,
}

pub async fn resolve_artifact_sha256(
    _coord: &MavenCoordinate,
    _config: &MavenResolverConfig,
) -> anyhow::Result<String> {
    todo!("Phase 1: API contract only")
}

/// Phase 1 semantics: ALL-OR-NOTHING (fail-fast).
/// If ANY coordinate fails, the entire batch fails. No partial results.
/// Rationale: a partial lockfile is worse than no lockfile.
pub async fn resolve_artifacts_batch(
    _coords: &[MavenCoordinate],
    _config: &MavenResolverConfig,
) -> anyhow::Result<Vec<(MavenCoordinate, String)>> {
    todo!("Phase 1: API contract only")
}

#[cfg(test)]
#[path = "maven_tests.rs"]
mod tests;
