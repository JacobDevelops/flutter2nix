use crate::maven::MavenCoordinate;
use regex::Regex;
use serde::{Deserialize, Serialize};

const SUPPORTED_GRADLE_MAJOR_VERSIONS: &[u64] = &[7, 8, 9];

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

pub fn parse_tapi_output(raw: &str) -> anyhow::Result<TapiOutput> {
    if raw.contains("FLUTTER2NIX_DEPS:") {
        let version = parse_sentinel_version(raw);
        let artifacts = try_parse_sentinel(raw).unwrap_or_default();
        let mut configurations: Vec<String> = artifacts
            .iter()
            .map(|a| a.scope.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        configurations.sort();
        let output = TapiOutput {
            version,
            artifacts,
            configurations,
        };
        validate_tapi_output(&output)?;
        return Ok(output);
    }
    // Legacy JSON format — used by existing fixture-based model tests
    let output: TapiOutput = serde_json::from_str(raw)
        .map_err(|e| anyhow::anyhow!("failed to parse TAPI output: {}", e))?;
    validate_tapi_output(&output)?;
    Ok(output)
}

pub fn validate_tapi_output(output: &TapiOutput) -> anyhow::Result<()> {
    let major: u64 = output
        .version
        .split('.')
        .next()
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| anyhow::anyhow!("unsupported TAPI version: {}", output.version))?;

    if !SUPPORTED_GRADLE_MAJOR_VERSIONS.contains(&major) {
        anyhow::bail!(
            "unsupported TAPI version: {} (supported Gradle major versions: 7, 8, 9)",
            output.version
        );
    }
    Ok(())
}

/// Fallback parser for buildscript dependencies from build.gradle(.kts) content.
/// Extracts classpath dependencies using regex when TAPI output doesn't expose buildscript block.
///
/// # Behavior:
/// - No buildscript block → Ok(vec![]) (silent)
/// - Buildscript block + ≥1 classpath coords → Ok(coords) with warning log
/// - Buildscript block + 0 coords → Err with descriptive message
pub fn parse_buildscript_deps(build_gradle_content: &str) -> anyhow::Result<Vec<MavenCoordinate>> {
    // Check if buildscript block exists at all
    let buildscript_pattern = Regex::new(r"buildscript\s*\{")
        .map_err(|e| anyhow::anyhow!("failed to compile buildscript regex: {}", e))?;

    if !buildscript_pattern.is_match(build_gradle_content) {
        // No buildscript block at all — return empty silently
        return Ok(vec![]);
    }

    // Buildscript block exists; now extract classpath dependencies
    // Pattern: classpath("group:artifact:version") or classpath('group:artifact:version')
    let classpath_pattern = Regex::new(r#"classpath\s*\(\s*["']([^"']+)["']\s*\)"#)
        .map_err(|e| anyhow::anyhow!("failed to compile classpath regex: {}", e))?;

    let coords: Vec<MavenCoordinate> = classpath_pattern
        .captures_iter(build_gradle_content)
        .filter_map(|cap| {
            cap.get(1).and_then(|m| {
                let coord_str = m.as_str();
                MavenCoordinate::parse(coord_str).ok()
            })
        })
        .collect();

    if coords.is_empty() {
        // Buildscript block detected but no valid classpath coords found
        anyhow::bail!(
            "Buildscript block detected but no classpath dependencies found — \
             check build.gradle(.kts) syntax or file an issue"
        );
    }

    // Log warning for production visibility
    log::warn!(
        "parse_buildscript_deps: extracted {} buildscript coords from build.gradle",
        coords.len()
    );

    Ok(coords)
}

/// Returns `Some(artifacts)` if a `FLUTTER2NIX_DEPS:` sentinel line is found in `stdout`,
/// `None` if no sentinel is present (caller should trigger IdeaProject fallback).
/// Invalid JSON in the sentinel yields `Some(vec![])` rather than panicking.
pub fn try_parse_sentinel(stdout: &str) -> Option<Vec<TapiArtifact>> {
    let re = Regex::new(r"(?m)^FLUTTER2NIX_DEPS:(.*)$").unwrap();
    let m = re.captures(stdout)?;
    let json_str = m.get(1).map(|m| m.as_str().trim()).unwrap_or("[]");
    match serde_json::from_str::<Vec<TapiArtifact>>(json_str) {
        Ok(artifacts) => Some(artifacts),
        Err(_) => Some(vec![]),
    }
}

pub fn parse_sentinel_version(stdout: &str) -> String {
    let re = Regex::new(r"(?m)^FLUTTER2NIX_VERSION:(.+)$").unwrap();
    re.captures(stdout)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().trim().to_string())
        .unwrap_or_default()
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;
