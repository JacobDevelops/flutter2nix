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
mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn test_nix_codegen_simple_2_deps_inline() {
        todo!("Phase 1: stub — DependencyGraph with guava+junit, inline format, output matches fixtures/nix-outputs/simple-2-deps-inline.nix")
    }

    #[test]
    fn test_nix_codegen_flake_format() {
        todo!("Phase 1: stub — same deps, flake format, output is valid Nix overlay function")
    }

    #[test]
    fn test_nix_codegen_special_chars_in_group() {
        todo!("Phase 1: stub — dep with group 'io.netty', Nix output properly quotes the key")
    }

    #[test]
    fn test_nix_codegen_deterministic_output() {
        todo!("Phase 1: stub — same graph called twice, output is bitwise identical")
    }

    #[test]
    fn test_nix_codegen_large_graph_20_deps() {
        todo!("Phase 1: stub — graph with 20 deps from complex-20-deps.json, output is valid Nix parseable by nix eval")
    }
}
