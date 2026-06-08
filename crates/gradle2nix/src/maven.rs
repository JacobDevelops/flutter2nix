use anyhow::Context;
use std::path::PathBuf;
use std::time::Duration;

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
    /// "group:artifact:version@extension", "group:artifact:version:classifier@extension"
    pub fn parse(s: &str) -> anyhow::Result<Self> {
        let (coord_str, extension) = match s.rfind('@') {
            Some(at_idx) => (&s[..at_idx], s[at_idx + 1..].to_string()),
            None => (s, "jar".to_string()),
        };

        let parts: Vec<&str> = coord_str.split(':').collect();
        anyhow::ensure!(
            parts.len() >= 3 && parts.len() <= 4,
            "invalid Maven coordinate '{}': expected group:artifact:version[:classifier][@extension]",
            s
        );

        Ok(Self {
            group: parts[0].to_string(),
            artifact: parts[1].to_string(),
            version: parts[2].to_string(),
            classifier: parts.get(3).filter(|c| !c.is_empty()).map(|c| c.to_string()),
            extension,
        })
    }

    /// Convert to canonical Maven repository path.
    /// e.g. "com.google.guava:guava:31.1-jre" → "com/google/guava/guava/31.1-jre/guava-31.1-jre.jar"
    pub fn to_artifact_path(&self) -> String {
        let group_path = self.group.replace('.', "/");
        let filename = match &self.classifier {
            Some(c) => format!("{}-{}-{}.{}", self.artifact, self.version, c, self.extension),
            None => format!("{}-{}.{}", self.artifact, self.version, self.extension),
        };
        format!("{}/{}/{}/{}", group_path, self.artifact, self.version, filename)
    }
}

pub struct MavenResolverConfig {
    pub repository_urls: Vec<String>,
    /// Flat stub directory used in tests (files named `{artifact_path}.sha256`).
    pub local_cache_dir: Option<PathBuf>,
    /// Gradle user home for module-cache lookup. `None` → auto-detect from
    /// `GRADLE_USER_HOME` env var, falling back to `~/.gradle`.
    pub gradle_user_home: Option<PathBuf>,
    pub timeout_secs: u64,
    pub max_concurrency: usize,
}

impl Default for MavenResolverConfig {
    fn default() -> Self {
        Self {
            repository_urls: vec![
                "https://repo.maven.apache.org/maven2".to_string(),
                "https://dl.google.com/dl/android/maven2".to_string(),
            ],
            local_cache_dir: None,
            gradle_user_home: None,
            timeout_secs: 60,
            max_concurrency: 10,
        }
    }
}

fn detect_gradle_user_home() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("GRADLE_USER_HOME") {
        return Some(PathBuf::from(path));
    }
    std::env::var("HOME")
        .ok()
        .map(|h| PathBuf::from(h).join(".gradle"))
}

/// Look up a JAR in Gradle's module cache and compute its SHA-256.
///
/// Gradle stores downloaded artifacts at:
///   `{gradle_user_home}/caches/modules-2/files-2.1/{group}/{artifact}/{version}/{sha1}/{filename}`
///
/// The `sha1` directory name is the SHA-1 of the file — we don't need it, we just
/// iterate subdirectories looking for the expected filename.
fn find_sha256_in_gradle_cache(
    coord: &MavenCoordinate,
    gradle_user_home: &std::path::Path,
) -> anyhow::Result<Option<String>> {
    use sha2::Digest;

    let artifact_dir = gradle_user_home
        .join("caches/modules-2/files-2.1")
        .join(&coord.group)
        .join(&coord.artifact)
        .join(&coord.version);

    if !artifact_dir.exists() {
        return Ok(None);
    }

    let filename = match &coord.classifier {
        Some(c) => format!("{}-{}-{}.{}", coord.artifact, coord.version, c, coord.extension),
        None => format!("{}-{}.{}", coord.artifact, coord.version, coord.extension),
    };

    for entry in std::fs::read_dir(&artifact_dir)
        .with_context(|| format!("reading Gradle cache dir '{}'", artifact_dir.display()))?
    {
        let jar_path = entry?.path().join(&filename);
        if jar_path.exists() {
            let bytes = std::fs::read(&jar_path)
                .with_context(|| format!("reading cached JAR '{}'", jar_path.display()))?;
            let hash = sha2::Sha256::digest(&bytes);
            let hex: String = hash.iter().map(|b| format!("{:02x}", b)).collect();
            validate_sha256_hex(&hex, &coord.to_artifact_path())?;
            return Ok(Some(hex));
        }
    }

    Ok(None)
}

/// Fetch SHA-256 from a Maven repository.
///
/// Tries the `.sha256` sidecar file first (fast, one small HTTP request).
/// If the sidecar returns 404 — which happens for older Google Maven artifacts
/// and Flutter storage artifacts — falls back to downloading the artifact itself
/// and computing SHA-256 from its bytes.
async fn fetch_sha256_http(
    coord: &MavenCoordinate,
    repo_url: &str,
    client: &reqwest::Client,
    timeout_secs: u64,
) -> anyhow::Result<String> {
    use sha2::Digest;

    let artifact_path = coord.to_artifact_path();
    let base = repo_url.trim_end_matches('/');
    let sha256_url = format!("{}/{}.sha256", base, artifact_path);

    let sha256_response = tokio::time::timeout(
        Duration::from_secs(timeout_secs),
        client.get(&sha256_url).send(),
    )
    .await
    .with_context(|| format!("HTTP request timeout after {}s", timeout_secs))?
    .with_context(|| format!("HTTP request failed for {}", sha256_url))?;

    if sha256_response.status() == reqwest::StatusCode::OK {
        let text = sha256_response
            .text()
            .await
            .with_context(|| format!("reading response body from {}", sha256_url))?;
        let hex = text.trim().to_string();
        validate_sha256_hex(&hex, &artifact_path)?;
        return Ok(hex);
    }

    if sha256_response.status() == reqwest::StatusCode::NOT_FOUND {
        // .sha256 sidecar not hosted (common for older Google Maven / Flutter storage artifacts).
        // Download the artifact itself and compute SHA-256 from the bytes.
        let artifact_url = format!("{}/{}", base, artifact_path);
        let art_response = tokio::time::timeout(
            Duration::from_secs(timeout_secs),
            client.get(&artifact_url).send(),
        )
        .await
        .with_context(|| format!("HTTP request timeout downloading {}", artifact_url))?
        .with_context(|| format!("HTTP request failed for {}", artifact_url))?;

        if art_response.status() == reqwest::StatusCode::OK {
            let bytes = art_response
                .bytes()
                .await
                .with_context(|| format!("reading artifact bytes from {}", artifact_url))?;
            let hash = sha2::Sha256::digest(&bytes);
            let hex: String = hash.iter().map(|b| format!("{:02x}", b)).collect();
            validate_sha256_hex(&hex, &artifact_path)?;
            return Ok(hex);
        }

        anyhow::bail!(
            "HTTP {} from {}: {}",
            art_response.status().as_u16(),
            artifact_url,
            art_response.status().canonical_reason().unwrap_or("Unknown")
        );
    }

    anyhow::bail!(
        "HTTP {} from {}: {}",
        sha256_response.status().as_u16(),
        sha256_url,
        sha256_response.status().canonical_reason().unwrap_or("Unknown")
    );
}

pub async fn resolve_artifact_sha256(
    coord: &MavenCoordinate,
    config: &MavenResolverConfig,
) -> anyhow::Result<String> {
    let artifact_path = coord.to_artifact_path();
    let sha256_rel = format!("{}.sha256", artifact_path);

    // Try local cache first
    if let Some(cache_dir) = &config.local_cache_dir {
        let sha256_path = cache_dir.join(&sha256_rel);
        if sha256_path.exists() {
            let content = std::fs::read_to_string(&sha256_path).with_context(|| {
                format!("reading sha256 file '{}'", sha256_path.display())
            })?;
            let hex = content.trim().to_string();
            validate_sha256_hex(&hex, &artifact_path)?;
            return Ok(hex);
        }
    }

    // Try Gradle module cache (compute SHA256 from cached JAR bytes — avoids HTTP .sha256 endpoints
    // that don't exist for artifacts published before ~2016)
    let gradle_home = config.gradle_user_home.clone().or_else(detect_gradle_user_home);
    if let Some(ref guh) = gradle_home {
        if let Some(hex) = find_sha256_in_gradle_cache(coord, guh)? {
            return Ok(hex);
        }
    }

    // Try HTTP from configured repositories
    let client = reqwest::Client::new();
    let mut last_error: Option<anyhow::Error> = None;

    for repo_url in &config.repository_urls {
        match fetch_sha256_http(coord, repo_url, &client, config.timeout_secs).await {
            Ok(hex) => return Ok(hex),
            Err(e) => {
                log::debug!("HTTP fetch failed from {}: {}", repo_url, e);
                last_error = Some(e);
            }
        }
    }

    if let Some(e) = last_error {
        anyhow::bail!(
            "artifact not found: '{}' — all {} repository URLs failed: {}",
            artifact_path,
            config.repository_urls.len(),
            e
        )
    } else {
        anyhow::bail!(
            "artifact not found: '{}' — no repository URLs configured",
            artifact_path
        )
    }
}

fn validate_sha256_hex(hex: &str, artifact_path: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        hex.len() == 64 && hex.chars().all(|c| c.is_ascii_hexdigit()),
        "invalid hex sha256 for '{}': got '{}'",
        artifact_path,
        hex
    );
    Ok(())
}

/// Resolves all coordinates concurrently, collecting ALL failures before returning.
/// Reports every failed artifact in a single error — no whack-a-mole one-at-a-time failures.
/// A partial lockfile is worse than no lockfile, so the batch still fails if any artifact fails.
pub async fn resolve_artifacts_batch(
    coords: &[MavenCoordinate],
    config: &MavenResolverConfig,
) -> anyhow::Result<Vec<(MavenCoordinate, String)>> {
    use futures::stream::{self, StreamExt};

    let results: Vec<(MavenCoordinate, anyhow::Result<String>)> = stream::iter(coords)
        .map(|coord| async move {
            let result = resolve_artifact_sha256(coord, config).await;
            (coord.clone(), result)
        })
        .buffer_unordered(config.max_concurrency)
        .collect()
        .await;

    let mut successes: Vec<(MavenCoordinate, String)> = Vec::new();
    let mut failures: Vec<String> = Vec::new();

    for (coord, result) in results {
        match result {
            Ok(sha256) => successes.push((coord, sha256)),
            Err(e) => failures.push(format!("  - {}: {:#}", coord.to_artifact_path(), e)),
        }
    }

    if failures.is_empty() {
        return Ok(successes);
    }

    anyhow::bail!(
        "{} artifact(s) failed to resolve:\n{}",
        failures.len(),
        failures.join("\n")
    )
}

#[cfg(test)]
#[path = "maven_tests.rs"]
mod tests;
