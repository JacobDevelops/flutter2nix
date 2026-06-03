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
mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn test_parse_tapi_valid_basic() {
        todo!("Phase 1: stub — input: fixtures/tapi-outputs/basic.json, expect TapiOutput{{version:'8.4.0', artifacts:2}}")
    }

    #[test]
    fn test_parse_tapi_missing_required_field_version() {
        todo!("Phase 1: stub — input: malformed-missing-field.json, expect Err containing 'missing field'")
    }

    #[test]
    fn test_parse_tapi_unknown_extra_fields_ignored() {
        todo!("Phase 1: stub — input: malformed-unknown-fields.json with extra buildId field, expect Ok (forward compat)")
    }

    #[test]
    fn test_parse_tapi_version_mismatch_error() {
        todo!("Phase 1: stub — input: version-mismatch.json (version:'99.0.0'), expect Err containing 'unsupported TAPI version'")
    }

    #[test]
    fn test_parse_tapi_with_classifiers() {
        todo!("Phase 1: stub — input: with-classifiers.json, expect classifier:None and classifier:Some('sources') both present")
    }

    #[test]
    fn test_parse_tapi_with_test_scope() {
        todo!("Phase 1: stub — input: with-test-scope.json, expect artifacts with scope:'test' are present")
    }
}
