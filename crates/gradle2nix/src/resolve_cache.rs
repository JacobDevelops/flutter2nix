use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use anyhow::Context;
use serde::{Deserialize, Serialize};

/// Persistent cache of Maven repository lookups, keyed by absolute artifact URL.
///
/// Maven release coordinates are immutable: once a URL serves bytes, those bytes
/// (and their SHA-256) never change, so positive entries never expire. A `None`
/// value records a confirmed HTTP 404 — discovery phases probe many speculative
/// coordinates that don't exist, and re-probing them dominates warm-run time.
/// Transport errors (timeouts, DNS, 5xx) are never cached, so a flaky network
/// can't poison the cache.
#[derive(Default, Serialize, Deserialize)]
struct CacheData {
    /// artifact URL → SHA-256 hex of the artifact bytes; None = confirmed 404.
    #[serde(default)]
    sha256: HashMap<String, Option<String>>,
    /// POM URL → POM text; None = confirmed 404.
    #[serde(default)]
    pom: HashMap<String, Option<String>>,
    /// absolute file path → (size, mtime_unix_secs, sha256 hex) for Gradle-cache
    /// JARs, so warm runs don't re-hash hundreds of megabytes of artifacts.
    #[serde(default)]
    file_sha256: HashMap<String, (u64, i64, String)>,
}

pub struct ResolveCache {
    path: PathBuf,
    data: Mutex<CacheData>,
    dirty: AtomicBool,
}

impl ResolveCache {
    /// Open (or start empty) the cache under `{gradle_user_home}/caches/gradle2nix/`.
    /// A corrupt or unreadable cache file is discarded, never an error.
    pub fn open(gradle_user_home: &Path) -> Self {
        let path = gradle_user_home.join("caches/gradle2nix/resolve-cache.json");
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

    /// `Some(Some(hex))` = known hash, `Some(None)` = known 404, `None` = never looked up.
    pub fn lookup_sha256(&self, url: &str) -> Option<Option<String>> {
        self.data.lock().unwrap().sha256.get(url).cloned()
    }

    pub fn store_sha256(&self, url: &str, value: Option<String>) {
        self.data
            .lock()
            .unwrap()
            .sha256
            .insert(url.to_string(), value);
        self.dirty.store(true, Ordering::Relaxed);
    }

    pub fn lookup_pom(&self, url: &str) -> Option<Option<String>> {
        self.data.lock().unwrap().pom.get(url).cloned()
    }

    pub fn store_pom(&self, url: &str, value: Option<String>) {
        self.data.lock().unwrap().pom.insert(url.to_string(), value);
        self.dirty.store(true, Ordering::Relaxed);
    }

    /// Cached hash of a local file, validated against current size + mtime.
    pub fn lookup_file_sha256(&self, path: &Path) -> Option<String> {
        let meta = std::fs::metadata(path).ok()?;
        let mtime = file_mtime_secs(&meta)?;
        let data = self.data.lock().unwrap();
        let (size, cached_mtime, hex) = data.file_sha256.get(path.to_str()?)?;
        if *size == meta.len() && *cached_mtime == mtime {
            Some(hex.clone())
        } else {
            None
        }
    }

    pub fn store_file_sha256(&self, path: &Path, hex: &str) {
        let Some(key) = path.to_str() else { return };
        let Ok(meta) = std::fs::metadata(path) else {
            return;
        };
        let Some(mtime) = file_mtime_secs(&meta) else {
            return;
        };
        self.data
            .lock()
            .unwrap()
            .file_sha256
            .insert(key.to_string(), (meta.len(), mtime, hex.to_string()));
        self.dirty.store(true, Ordering::Relaxed);
    }

    /// Write the cache to disk if anything changed since the last persist.
    /// Atomic via temp-file + rename so a crash never leaves a torn file.
    pub fn persist(&self) -> anyhow::Result<()> {
        if !self.dirty.swap(false, Ordering::Relaxed) {
            return Ok(());
        }
        let parent = self
            .path
            .parent()
            .context("resolve cache path has no parent")?;
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating cache dir '{}'", parent.display()))?;
        let bytes = {
            let data = self.data.lock().unwrap();
            serde_json::to_vec(&*data).context("serialising resolve cache")?
        };
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, bytes)
            .with_context(|| format!("writing cache temp file '{}'", tmp.display()))?;
        std::fs::rename(&tmp, &self.path)
            .with_context(|| format!("renaming cache into place at '{}'", self.path.display()))?;
        Ok(())
    }
}

fn file_mtime_secs(meta: &std::fs::Metadata) -> Option<i64> {
    let mtime = meta.modified().ok()?;
    match mtime.duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => Some(d.as_secs() as i64),
        Err(_) => None,
    }
}

#[cfg(test)]
#[path = "resolve_cache_tests.rs"]
mod tests;
