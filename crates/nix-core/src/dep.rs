use serde::{Deserialize, Serialize};

fn default_format_version() -> String {
    "1".to_string()
}

/// A single locked dependency (Maven artifact, CocoaPod, pub package, etc.)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LockedDependency {
    pub name: String,
    pub version: String,
    pub url: String,
    pub sha256: String,
}

/// A resolved dependency graph ready for Nix codegen
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DependencyGraph {
    #[serde(rename = "version", default = "default_format_version")]
    pub format_version: String,
    pub nodes: Vec<LockedDependency>,
}

impl Default for DependencyGraph {
    fn default() -> Self {
        Self {
            format_version: default_format_version(),
            nodes: Vec::new(),
        }
    }
}
