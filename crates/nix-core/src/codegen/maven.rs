use crate::dep::DependencyGraph;

#[derive(Debug, Clone)]
pub struct NixMavenCodegenConfig {
    pub fetcher: String,
    pub indent_width: usize,
    pub sort_deps: bool,
}

/// Generate a Nix attribute set of fetchMaven calls.
/// Output is deterministic (sorted by dep name when sort_deps=true).
pub fn generate_nix_set(
    _graph: &DependencyGraph,
    _config: &NixMavenCodegenConfig,
) -> anyhow::Result<String> {
    todo!("Phase 1: API contract only")
}

/// Generate a Nix pkgs overlay function wrapping the fetchMaven set.
pub fn generate_nix_overlay(
    _graph: &DependencyGraph,
    _config: &NixMavenCodegenConfig,
) -> anyhow::Result<String> {
    todo!("Phase 1: API contract only")
}

#[cfg(test)]
#[path = "maven_tests.rs"]
mod tests;
