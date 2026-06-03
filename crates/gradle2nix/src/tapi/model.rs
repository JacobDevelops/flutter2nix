use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TapiArtifact {
    pub group: String,
    pub artifact: String,
    pub version: String,
    pub classifier: Option<String>,
    pub extension: String,
    pub scope: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TapiOutput {
    pub version: String,
    pub artifacts: Vec<TapiArtifact>,
    pub configurations: Vec<String>,
}

pub fn parse_tapi_output(_raw_json: &str) -> anyhow::Result<TapiOutput> {
    todo!("Phase 1: API contract only")
}

pub fn validate_tapi_output(_output: &TapiOutput) -> anyhow::Result<()> {
    todo!("Phase 1: API contract only")
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;
