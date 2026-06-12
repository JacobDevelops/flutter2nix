//! Podspec fetch/parse and source normalization (Phase 1).

use crate::cocoapods::PodSourceKind;
use anyhow::Context;

const DEFAULT_SPEC_REPO: &str = "https://cdn.cocoapods.org/";
const MAX_HTTP_TIMEOUT_SECS: u64 = 60;

/// Parsed podspec JSON
#[derive(Debug, Clone, PartialEq)]
pub struct Podspec {
    pub name: String,
    pub version: String,
    pub source: PodSourceKind,
    pub subspecs: Vec<String>,
}

/// Parse podspec JSON into a Podspec struct
pub fn parse_podspec(json: &str) -> anyhow::Result<Podspec> {
    let value: serde_json::Value = serde_json::from_str(json)
        .map_err(|e| anyhow::anyhow!("invalid JSON in podspec: {}", e))?;

    let name = value
        .get("name")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow::anyhow!("podspec missing 'name' field"))?;

    let version = value
        .get("version")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow::anyhow!("podspec missing 'version' field"))?;

    let source_value = value
        .get("source")
        .ok_or_else(|| anyhow::anyhow!("podspec missing 'source' field"))?;

    let source = parse_source(source_value)?;

    let subspecs = value
        .get("subspecs")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .map(|s| s.to_string())
                .collect()
        })
        .unwrap_or_default();

    Ok(Podspec {
        name,
        version,
        source,
        subspecs,
    })
}

/// Parse a source object from a podspec
fn parse_source(source: &serde_json::Value) -> anyhow::Result<PodSourceKind> {
    let source_map = source
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("source must be an object"))?;

    // Try http
    if let Some(url) = source_map.get("http").and_then(|v| v.as_str()) {
        return Ok(PodSourceKind::Http {
            url: url.to_string(),
        });
    }

    // Try git
    if let Some(url) = source_map.get("git").and_then(|v| v.as_str()) {
        // Prefer commit, then tag, then branch
        let rev = source_map
            .get("commit")
            .and_then(|v| v.as_str())
            .or_else(|| source_map.get("tag").and_then(|v| v.as_str()))
            .or_else(|| source_map.get("branch").and_then(|v| v.as_str()))
            .ok_or_else(|| anyhow::anyhow!("git source missing commit/tag/branch"))?
            .to_string();
        return Ok(PodSourceKind::Git {
            url: url.to_string(),
            rev,
        });
    }

    // Try path (podspec source)
    if let Some(path) = source_map.get("path").and_then(|v| v.as_str()) {
        return Ok(PodSourceKind::Path {
            path: path.to_string(),
        });
    }

    Err(anyhow::anyhow!(
        "source contains neither 'http', 'git', nor 'path'"
    ))
}

/// Fetch a podspec from one or more spec repositories
pub async fn fetch_podspec(
    name: &str,
    version: &str,
    spec_repos: &[String],
    client: &reqwest::Client,
) -> anyhow::Result<Podspec> {
    let repos = if spec_repos.is_empty() {
        vec![DEFAULT_SPEC_REPO.to_string()]
    } else {
        spec_repos.to_vec()
    };

    // Compute the sharded path for CocoaPods CDN: first 3 chars of md5
    let md5_digest = md5::compute(name.as_bytes());
    let md5_hex = format!("{:x}", md5_digest);
    let mut chars = md5_hex.chars();
    let s1 = chars.next().unwrap();
    let s2 = chars.next().unwrap();
    let s3 = chars.next().unwrap();

    for repo_url in &repos {
        // Try sharded layout: {repo}/Specs/{s1}/{s2}/{s3}/{Name}/{Version}/{Name}.podspec.json
        let sharded_url = format!(
            "{}/Specs/{}/{}/{}/{}/{}/{}.podspec.json",
            repo_url.trim_end_matches('/'),
            s1,
            s2,
            s3,
            name,
            version,
            name
        );

        if let Ok(resp) = http_get(client, &sharded_url, MAX_HTTP_TIMEOUT_SECS).await {
            let body = resp
                .text()
                .await
                .context("failed to read podspec response body")?;
            return parse_podspec(&body);
        }

        // Try unsharded layout: {repo}/{Name}/{Version}/{Name}.podspec.json
        let unsharded_url = format!(
            "{}/{}/{}/{}.podspec.json",
            repo_url.trim_end_matches('/'),
            name,
            version,
            name
        );

        if let Ok(resp) = http_get(client, &unsharded_url, MAX_HTTP_TIMEOUT_SECS).await {
            let body = resp
                .text()
                .await
                .context("failed to read podspec response body")?;
            return parse_podspec(&body);
        }
    }

    anyhow::bail!(
        "spec not found for pod '{}' version '{}' in repos: {}",
        name,
        version,
        repos
            .iter()
            .map(|r| r.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    )
}

/// HTTP GET with one retry on transport errors
async fn http_get(
    client: &reqwest::Client,
    url: &str,
    timeout_secs: u64,
) -> anyhow::Result<reqwest::Response> {
    let per_try = std::time::Duration::from_secs(timeout_secs.min(MAX_HTTP_TIMEOUT_SECS));
    let mut last: Option<anyhow::Error> = None;

    for attempt in 0..2 {
        match tokio::time::timeout(per_try, client.get(url).send()).await {
            Ok(Ok(resp)) => {
                if resp.status() == 200 {
                    return Ok(resp);
                } else {
                    log::debug!(
                        "HTTP attempt {} returned {}: {}",
                        attempt + 1,
                        resp.status(),
                        url
                    );
                    last = Some(anyhow::anyhow!("HTTP {} from {}", resp.status(), url));
                }
            }
            Ok(Err(e)) => {
                log::debug!("HTTP attempt {} failed for {}: {}", attempt + 1, url, e);
                last =
                    Some(anyhow::Error::new(e).context(format!("HTTP request failed for {}", url)));
            }
            Err(_) => {
                log::debug!("HTTP attempt {} timed out for {}", attempt + 1, url);
                last = Some(anyhow::anyhow!(
                    "HTTP request timeout after {}s for {}",
                    per_try.as_secs(),
                    url
                ));
            }
        }
    }
    Err(last.expect("loop ran at least once"))
}

#[cfg(test)]
#[path = "podspec_tests.rs"]
mod tests;
