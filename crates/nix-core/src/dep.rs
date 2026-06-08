use base64::Engine;
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
    #[serde(rename = "sha256")]
    pub(crate) sha256_hex: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dep_source: Option<String>,
}

impl LockedDependency {
    pub fn new(name: String, version: String, url: String, sha256_hex: String) -> Self {
        Self {
            name,
            version,
            url,
            sha256_hex,
            dep_source: None,
        }
    }

    pub fn sha256_as_sri(&self) -> anyhow::Result<String> {
        hex_to_sri(&self.sha256_hex)
    }

    pub fn sha256_hex(&self) -> &str {
        &self.sha256_hex
    }
}

pub fn hex_to_sri(hex: &str) -> anyhow::Result<String> {
    let bytes = hex::decode(hex)?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Ok(format!("sha256-{}", b64))
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
