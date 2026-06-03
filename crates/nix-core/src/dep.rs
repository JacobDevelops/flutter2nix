use serde::{Deserialize, Serialize};

/// A single locked dependency (Maven artifact, CocoaPod, pub package, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockedDependency {
    pub name: String,
    pub version: String,
    pub url: String,
    pub sha256: String,
}

/// A resolved dependency graph ready for Nix codegen
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DependencyGraph {
    pub nodes: Vec<LockedDependency>,
}
