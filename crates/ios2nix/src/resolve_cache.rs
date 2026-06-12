//! Content-hash prefetch cache (Phase 1).

use crate::cocoapods::PodSourceKind;
use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

/// Persistent cache of pod source hashes, keyed by source URL
#[derive(Default, Serialize, Deserialize)]
struct CacheData {
    /// source key (http URL or git+url#rev) → SHA-256 hex; None = confirmed 404
    #[serde(default)]
    sha256: HashMap<String, Option<String>>,
}

/// Manages pod resolution cache with atomic persistence
pub struct ResolveCache {
    path: PathBuf,
    data: Mutex<CacheData>,
    dirty: AtomicBool,
}

impl ResolveCache {
    /// Open (or start empty) the cache at the given directory
    pub fn open(cache_dir: &Path) -> Self {
        let path = cache_dir.join("resolve-cache.json");
        let data = std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        Self {
            path,
            data: Mutex::new(data),
            dirty: AtomicBool::new(false),
        }
    }

    /// Lookup a cached SHA-256 hash
    /// Returns:
    /// - Some(Some(hex)) = known hash
    /// - Some(None) = known 404
    /// - None = never looked up
    pub fn lookup_sha256(&self, source_key: &str) -> Option<Option<String>> {
        self.data.lock().unwrap().sha256.get(source_key).cloned()
    }

    /// Store a SHA-256 hash (or None for 404)
    pub fn store_sha256(&self, source_key: &str, value: Option<String>) {
        self.data
            .lock()
            .unwrap()
            .sha256
            .insert(source_key.to_string(), value);
        self.dirty.store(true, Ordering::Relaxed);
    }

    /// Flush dirty cache to disk (atomic: write-to-temp then rename)
    pub fn flush(&self) -> anyhow::Result<()> {
        if !self.dirty.load(Ordering::Relaxed) {
            return Ok(());
        }

        let data = self.data.lock().unwrap();
        let json = serde_json::to_string_pretty(&*data)?;

        // Ensure parent directory exists
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).ok();
        }

        // Write to temp file then atomically rename
        let temp_path = self.path.with_extension("json.tmp");
        std::fs::write(&temp_path, json)?;
        std::fs::rename(&temp_path, &self.path)?;

        self.dirty.store(false, Ordering::Relaxed);
        Ok(())
    }
}

impl Drop for ResolveCache {
    fn drop(&mut self) {
        let _ = self.flush();
    }
}

/// Prefetch and hash a pod source, caching the result
pub async fn prefetch_content_hash(
    src: &PodSourceKind,
    expected_sha256: Option<&str>,
    client: &reqwest::Client,
    cache: &ResolveCache,
) -> anyhow::Result<String> {
    let source_key = src.source_key()?;

    match src {
        PodSourceKind::Http { url } => {
            // Cache lookup
            if let Some(Some(hex)) = cache.lookup_sha256(&source_key) {
                // Verify hash matches expected (if provided)
                if let Some(expected) = expected_sha256 {
                    if hex != expected {
                        anyhow::bail!(
                            "hash mismatch for {}: cached {} != expected {}",
                            url,
                            hex,
                            expected
                        );
                    }
                }
                return Ok(hex);
            }

            // Fetch and hash
            let bytes = download_with_retry(client, url).await?;
            let hash_hex = sha256_hex(&bytes);

            // Verify hash matches expected (if provided)
            if let Some(expected) = expected_sha256 {
                if hash_hex != expected {
                    anyhow::bail!(
                        "hash mismatch for {}: computed {} != expected {}",
                        url,
                        hash_hex,
                        expected
                    );
                }
            }

            cache.store_sha256(&source_key, Some(hash_hex.clone()));
            Ok(hash_hex)
        }
        PodSourceKind::Git { url, rev } => {
            // Cache lookup
            if let Some(Some(hex)) = cache.lookup_sha256(&source_key) {
                return Ok(hex);
            }

            // Shell out to nix-prefetch-git
            let output = tokio::process::Command::new("nix-prefetch-git")
                .arg("--url")
                .arg(url)
                .arg("--rev")
                .arg(rev)
                .arg("--quiet")
                .output()
                .await
                .context("failed to run nix-prefetch-git")?;

            if !output.status.success() {
                anyhow::bail!(
                    "nix-prefetch-git failed for {} at {}: {}",
                    url,
                    rev,
                    String::from_utf8_lossy(&output.stderr)
                );
            }

            let stdout = String::from_utf8(output.stdout)?;
            let nix_output: serde_json::Value = serde_json::from_str(stdout.trim())?;
            let hash_hex = nix_output
                .get("sha256")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("nix-prefetch-git output missing sha256 field"))?
                .to_string();

            cache.store_sha256(&source_key, Some(hash_hex.clone()));
            Ok(hash_hex)
        }
        PodSourceKind::Path { path: _ } => {
            anyhow::bail!("path pods have no content hash")
        }
    }
}

/// Download with one retry on transport errors
async fn download_with_retry(client: &reqwest::Client, url: &str) -> anyhow::Result<Vec<u8>> {
    let timeout = std::time::Duration::from_secs(60);
    let mut last_error = None;

    for attempt in 0..2 {
        match tokio::time::timeout(timeout, client.get(url).send()).await {
            Ok(Ok(resp)) => {
                if resp.status() == 200 {
                    return resp
                        .bytes()
                        .await
                        .map(|b| b.to_vec())
                        .context(format!("failed to read response body from {}", url));
                } else {
                    last_error = Some(anyhow::anyhow!(
                        "HTTP {} from {} (attempt {})",
                        resp.status(),
                        url,
                        attempt + 1
                    ));
                }
            }
            Ok(Err(e)) => {
                log::debug!("HTTP attempt {} failed for {}: {}", attempt + 1, url, e);
                last_error = Some(anyhow::Error::new(e));
            }
            Err(_) => {
                log::debug!("HTTP attempt {} timed out for {}", attempt + 1, url);
                last_error = Some(anyhow::anyhow!(
                    "HTTP timeout after {}s for {} (attempt {})",
                    timeout.as_secs(),
                    url,
                    attempt + 1
                ));
            }
        }
    }

    Err(last_error.unwrap())
}

/// Compute SHA-256 hash of bytes and return as hex string
fn sha256_hex(data: &[u8]) -> String {
    use sha2::Digest;
    let mut hasher = sha2::Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
#[path = "resolve_cache_tests.rs"]
mod tests;
