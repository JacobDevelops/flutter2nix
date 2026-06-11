use anyhow::Context;
use clap::Parser;
use futures::stream::{self, StreamExt};
use nix_core::dep::DependencyGraph;
use reqwest::Client;
use std::path::PathBuf;
use std::sync::Arc;

use crate::cocoapods::{parse_podfile_lock, PodSourceKind};
use crate::lockfile::write_lockfile;
use crate::podspec::fetch_podspec;
use crate::resolve_cache::ResolveCache;

#[derive(Parser)]
pub struct LockArgs {
    /// iOS project directory
    #[arg(long, default_value = ".")]
    pub ios_dir: PathBuf,

    /// Output path for lockfile
    #[arg(long, short)]
    pub output: Option<PathBuf>,

    /// CocoaPods spec repositories (repeatable)
    #[arg(long)]
    pub spec_repo: Option<Vec<String>>,

    /// Cache directory for pod resolutions
    #[arg(long)]
    pub cache_dir: Option<PathBuf>,

    /// Timeout in seconds for HTTP requests
    #[arg(long, default_value = "600")]
    pub timeout_secs: u64,
}

pub struct LockCommand {
    pub ios_dir: PathBuf,
    pub output: Option<PathBuf>,
    pub spec_repos: Option<Vec<String>>,
    pub cache_dir: Option<PathBuf>,
    pub timeout_secs: u64,
}

/// Core lock pipeline: resolve iOS dependencies to a DependencyGraph.
/// Shared by `lock::run` and `check::run`.
pub async fn build_dependency_graph(
    ios_dir: &std::path::Path,
    spec_repos: &[String],
    cache_dir: Option<&std::path::Path>,
    timeout_secs: u64,
) -> anyhow::Result<DependencyGraph> {
    // 1. Sidecar short-circuit: if `.ios2nix-podspecs.json` exists, parse it directly
    let sidecar_path = ios_dir.join(".ios2nix-podspecs.json");
    if sidecar_path.exists() {
        let sidecar_json = std::fs::read_to_string(&sidecar_path)
            .with_context(|| format!("reading sidecar '{}'", sidecar_path.display()))?;
        return build_graph_from_sidecar(&sidecar_json);
    }

    // 2. Parse Podfile.lock
    let podfile_lock_path = ios_dir.join("Podfile.lock");
    let podfile_lock_content = std::fs::read_to_string(&podfile_lock_path)
        .with_context(|| format!("reading Podfile.lock '{}'", podfile_lock_path.display()))?;
    let podfile_lock = parse_podfile_lock(&podfile_lock_content)?;

    eprintln!(
        "ios2nix: parsed Podfile.lock with {} pods",
        podfile_lock.pods.len()
    );

    // 3. Classify pods source-driven (never name heuristics): EXTERNAL SOURCES
    // path entries are excluded; CHECKOUT OPTIONS entries already carry the git
    // source (a git pod does not exist in the CDN spec repos, so it must never
    // go through fetch_podspec); everything else resolves via its podspec.
    let mut git_pods: Vec<(crate::cocoapods::Pod, PodSourceKind)> = Vec::new();
    let mut cdn_pods: Vec<crate::cocoapods::Pod> = Vec::new();
    for pod in &podfile_lock.pods {
        let root_name = pod.name.split('/').next().unwrap_or(&pod.name);

        if let Some(ext_src) = podfile_lock.external_sources.get(root_name) {
            if ext_src.path.is_some() {
                // Path pod — excluded from nodes
                continue;
            }
        }

        if let Some(co) = podfile_lock.checkout_options.get(root_name) {
            let rev = co
                .commit
                .clone()
                .or_else(|| co.tag.clone())
                .or_else(|| co.branch.clone())
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "git pod '{}' has CHECKOUT OPTIONS without commit/tag/branch",
                        pod.name
                    )
                })?;
            git_pods.push((
                pod.clone(),
                PodSourceKind::Git {
                    url: co.git.clone(),
                    rev,
                },
            ));
            continue;
        }

        cdn_pods.push(pod.clone());
    }

    let third_party_count = git_pods.len() + cdn_pods.len();
    eprintln!("ios2nix: classified {} third-party pods", third_party_count);

    // 4. Fetch podspecs for CDN pods (bounded concurrency).
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(timeout_secs.clamp(1, 600)))
        .build()
        .context("building HTTP client")?;
    let spec_repos_vec = if spec_repos.is_empty() {
        vec!["https://cdn.cocoapods.org/".to_string()]
    } else {
        spec_repos.to_vec()
    };

    const MAX_CONCURRENCY: usize = 16;
    let resolved_pods: Vec<_> = stream::iter(cdn_pods)
        .map(|pod| {
            let client = &client;
            let spec_repos = &spec_repos_vec;
            async move {
                let root_name = pod.name.split('/').next().unwrap_or(&pod.name).to_string();
                let result = fetch_podspec(&root_name, &pod.version, spec_repos, client).await;
                (pod, result)
            }
        })
        .buffer_unordered(MAX_CONCURRENCY)
        .collect()
        .await;

    // 5. Prefetch content hashes (bounded concurrency, shared cache).
    // No cache_dir → throwaway temp-dir cache so nothing is persisted into cwd.
    let _cache_tmp;
    let cache = match cache_dir {
        Some(dir) => Arc::new(ResolveCache::open(dir)),
        None => {
            let tmp = tempfile::tempdir().context("creating throwaway cache dir")?;
            let cache = Arc::new(ResolveCache::open(tmp.path()));
            _cache_tmp = tmp;
            cache
        }
    };

    let mut to_prefetch: Vec<(String, String, PodSourceKind)> = Vec::new();
    let mut failed_pods: Vec<(String, anyhow::Error)> = Vec::new();

    for (pod, source) in git_pods {
        to_prefetch.push((pod.name, pod.version, source));
    }
    for (pod, podspec_result) in resolved_pods {
        match podspec_result {
            Ok(podspec) => {
                if matches!(podspec.source, PodSourceKind::Path { .. }) {
                    // Path-sourced podspec — excluded like lockfile path pods.
                    continue;
                }
                to_prefetch.push((pod.name, pod.version, podspec.source));
            }
            Err(e) => failed_pods.push((pod.name, e)),
        }
    }

    let prefetch_results: Vec<_> = stream::iter(to_prefetch)
        .map(|(name, version, source)| {
            let client = &client;
            let cache = Arc::clone(&cache);
            async move {
                let result =
                    crate::resolve_cache::prefetch_content_hash(&source, None, client, &cache)
                        .await;
                (name, version, source, result)
            }
        })
        .buffer_unordered(MAX_CONCURRENCY)
        .collect()
        .await;

    let mut nodes = Vec::new();
    for (name, version, source, result) in prefetch_results {
        match result {
            Ok(sha256) => {
                let (url, dep_source) = match &source {
                    PodSourceKind::Http { url } => (url.clone(), "pod-http"),
                    PodSourceKind::Git { url, rev } => (format!("git+{}#{}", url, rev), "pod-git"),
                    PodSourceKind::Path { .. } => unreachable!("path pods filtered above"),
                };
                let mut dep = nix_core::dep::LockedDependency::new(name, version, url, sha256);
                dep.dep_source = Some(dep_source.to_string());
                nodes.push(dep);
            }
            Err(e) => failed_pods.push((name, e)),
        }
    }

    // Persist whatever resolved before bubbling an error (mirror gradle2nix).
    if let Err(e) = cache.flush() {
        eprintln!("warning: could not persist resolve cache: {}", e);
    }

    // 6. Guard: a pod that fails to resolve is a hard error, never a silent
    // drop — an incomplete lockfile means a link failure at offline build time
    // (overview pre-mortem #3).
    if !failed_pods.is_empty() {
        let detail = failed_pods
            .iter()
            .map(|(name, e)| format!("  {}: {:#}", name, e))
            .collect::<Vec<_>>()
            .join("\n");
        anyhow::bail!(
            "failed to resolve {} of {} third-party pods:\n{}",
            failed_pods.len(),
            third_party_count,
            detail
        );
    }

    // 7. Sort for determinism
    nodes.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(DependencyGraph {
        format_version: "1".to_string(),
        nodes,
    })
}

/// Build DependencyGraph from a sidecar `.ios2nix-podspecs.json`
fn build_graph_from_sidecar(json: &str) -> anyhow::Result<DependencyGraph> {
    #[derive(serde::Deserialize)]
    struct SidecarPod {
        name: String,
        version: String,
        source: SidecarSource,
    }

    #[derive(serde::Deserialize)]
    #[serde(tag = "type")]
    enum SidecarSource {
        #[serde(rename = "http")]
        Http { url: String, sha256: String },
        #[serde(rename = "git")]
        Git {
            url: String,
            rev: String,
            sha256: String,
        },
        #[serde(rename = "path")]
        Path { path: String },
    }

    #[derive(serde::Deserialize)]
    struct Sidecar {
        pods: Vec<SidecarPod>,
    }

    let sidecar: Sidecar = serde_json::from_str(json)?;

    let mut nodes = Vec::new();
    for pod in sidecar.pods {
        match pod.source {
            SidecarSource::Path { .. } => {
                // Path pods are excluded
                continue;
            }
            SidecarSource::Http { url, sha256 } => {
                let mut dep =
                    nix_core::dep::LockedDependency::new(pod.name, pod.version, url, sha256);
                dep.dep_source = Some("pod-http".to_string());
                nodes.push(dep);
            }
            SidecarSource::Git { url, rev, sha256 } => {
                let mut dep = nix_core::dep::LockedDependency::new(
                    pod.name,
                    pod.version,
                    format!("git+{}#{}", url, rev),
                    sha256,
                );
                dep.dep_source = Some("pod-git".to_string());
                nodes.push(dep);
            }
        }
    }

    nodes.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(DependencyGraph {
        format_version: "1".to_string(),
        nodes,
    })
}

pub async fn run(cmd: LockCommand) -> anyhow::Result<()> {
    let graph = build_dependency_graph(
        &cmd.ios_dir,
        cmd.spec_repos.as_deref().unwrap_or(&[]),
        cmd.cache_dir.as_deref(),
        cmd.timeout_secs,
    )
    .await?;

    let output_path = cmd
        .output
        .unwrap_or_else(|| cmd.ios_dir.join("ios2nix.lock"));
    write_lockfile(&output_path, &graph)?;

    println!("Wrote lockfile: {}", output_path.display());
    Ok(())
}

#[cfg(test)]
#[path = "lock_tests.rs"]
mod tests;
