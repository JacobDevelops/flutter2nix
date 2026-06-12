use anyhow::Context;
use nix_core::dep::{DependencyGraph, LockedDependency};
use std::path::{Path, PathBuf};

use crate::maven::{
    artifact_repo_url, extract_pom_bom_imports, extract_pom_direct_deps, extract_pom_parent,
    fetch_pom_content, resolve_artifact_sha256, resolve_artifacts_batch, MavenCoordinate,
    MavenResolverConfig,
};
use crate::resolve_cache::ResolveCache;
use crate::tapi::model::parse_tapi_output;
use crate::tapi::shim::{invoke_tapi_shim, TapiShimConfig};

pub struct LockCommand {
    pub gradle_dir: PathBuf,
    pub output: Option<PathBuf>,
    pub repositories: Option<Vec<String>>,
    /// Local Maven repo directory for sha256 lookup (used in tests; None in production).
    pub gradle_cache_dir: Option<PathBuf>,
    /// Explicit Gradle user home for the TAPI shim and cache-discovery phases.
    /// None: the shim uses GRADLE_USER_HOME/~/.gradle; discovery phases detect it
    /// only when no gradle_cache_dir is set (see MavenResolverConfig::discovery_gradle_home).
    pub gradle_user_home: Option<PathBuf>,
    pub timeout_secs: u64,
    pub shim_timeout_secs: u64,
}

fn coord_to_name(coord: &MavenCoordinate) -> String {
    match &coord.classifier {
        Some(c) => format!("{}:{}:{}:{}", coord.group, coord.artifact, coord.version, c),
        None => format!("{}:{}:{}", coord.group, coord.artifact, coord.version),
    }
}

/// Core lock pipeline: TAPI → parse → resolve SHA-256 → DependencyGraph.
/// Shared by `lock::run` and `check::run`.
pub async fn build_dependency_graph(
    gradle_dir: &Path,
    repositories: &[String],
    gradle_cache_dir: Option<&Path>,
    gradle_user_home: Option<&Path>,
    timeout_secs: u64,
    shim_timeout_secs: u64,
) -> anyhow::Result<DependencyGraph> {
    // 1. Use sidecar for test injection if present
    let sidecar = gradle_dir.join(".gradle2nix-tapi-output.json");
    let tapi_json_override = if sidecar.exists() {
        Some(
            std::fs::read_to_string(&sidecar)
                .with_context(|| format!("reading TAPI sidecar '{}'", sidecar.display()))?,
        )
    } else {
        None
    };

    // 2. Invoke TAPI shim (or use override)
    eprintln!("gradle2nix: running TAPI shim (Gradle dependency extraction)...");
    let raw_json = invoke_tapi_shim(TapiShimConfig {
        gradle_project_dir: gradle_dir.to_path_buf(),
        gradle_user_home: gradle_user_home.map(PathBuf::from),
        gradle_cache_dir: gradle_cache_dir.map(PathBuf::from),
        timeout_secs: shim_timeout_secs,
        tapi_json_override,
        test_command: None,
    })
    .await?;

    // 3. Parse TAPI output
    let tapi_output = parse_tapi_output(&raw_json)?;

    // 4. Convert artifacts to MavenCoordinates (preserve extension via @ext suffix)
    let coords: Vec<MavenCoordinate> = tapi_output
        .artifacts
        .iter()
        .map(|art| {
            let s = match &art.classifier {
                Some(c) => format!(
                    "{}:{}:{}:{}@{}",
                    art.group, art.artifact, art.version, c, art.extension
                ),
                None => format!(
                    "{}:{}:{}@{}",
                    art.group, art.artifact, art.version, art.extension
                ),
            };
            MavenCoordinate::parse(&s)
        })
        .collect::<anyhow::Result<_>>()?;
    eprintln!(
        "gradle2nix: TAPI captured {} artifacts, resolving SHA-256s...",
        coords.len()
    );

    // 5. Resolve SHA-256 for each coordinate
    // Three default repos cover the whole Android/Flutter ecosystem:
    //   1. Maven Central  — Kotlin stdlib, JetBrains, Apache, etc.
    //   2. Google Maven   — AndroidX, Firebase, AGP, GMS
    //   3. Flutter storage — io.flutter embedding JARs (version = engine commit hash)
    const MAVEN_CENTRAL: &str = "https://repo.maven.apache.org/maven2";
    const GOOGLE_MAVEN: &str = "https://dl.google.com/dl/android/maven2";
    const GRADLE_PLUGIN_PORTAL: &str = "https://plugins.gradle.org/m2";
    const FLUTTER_STORAGE: &str = "https://storage.googleapis.com/download.flutter.io";
    let repo_urls = if repositories.is_empty() {
        vec![
            MAVEN_CENTRAL.to_string(),
            GOOGLE_MAVEN.to_string(),
            GRADLE_PLUGIN_PORTAL.to_string(),
            FLUTTER_STORAGE.to_string(),
        ]
    } else {
        repositories.to_vec()
    };

    let mut resolver_config = MavenResolverConfig {
        repository_urls: repo_urls.clone(),
        local_cache_dir: gradle_cache_dir.map(PathBuf::from),
        gradle_user_home: gradle_user_home.map(PathBuf::from),
        timeout_secs,
        max_concurrency: 64,
        resolve_cache: None,
    };
    // Persistent lookup cache lives next to Gradle's own caches so the warm-CI
    // scenario (retained Gradle home) skips re-resolving every URL. Hermetic test
    // mode (local_cache_dir, no gradle home) has no home → caching stays off.
    resolver_config.resolve_cache = resolver_config
        .discovery_gradle_home()
        .map(|home| std::sync::Arc::new(ResolveCache::open(&home)));

    // Persist whatever resolved before bubbling an error: a single broken artifact
    // must not throw away hundreds of completed lookups on the next attempt.
    let batch_result = resolve_artifacts_batch(&coords, &resolver_config).await;
    if let Some(cache) = &resolver_config.resolve_cache {
        if let Err(e) = cache.persist() {
            log::warn!("could not persist resolve cache: {:#}", e);
        }
    }
    let mut resolved = batch_result?;
    eprintln!(
        "gradle2nix: resolved {} artifacts, running discovery phases...",
        resolved.len()
    );

    // 6. Discover declared-version POMs first.
    // Gradle's offline resolution reads every POM that a resolved artifact transitively references
    // and needs the exact declared version present — even if conflict resolution selected a higher
    // version. The shim only captures the selected (conflict-resolved) versions, so declared-but-
    // downgraded versions are missing. Walk the <dependency> elements in each captured POM and add
    // the declared-version POM for any artifact that already exists in the local Gradle cache.
    // Must run BEFORE discover_parent_poms so parent/BOM chains of newly-added POMs are followed.
    resolved = discover_declared_dep_poms(resolved, &resolver_config).await?;
    eprintln!(
        "gradle2nix: declared-dep POMs done ({} total), discovering parent POMs...",
        resolved.len()
    );

    // 6b. Discover transitive parent POMs + BOM imports.
    // Now sees the full set of POMs including declared-version additions above, so it can
    // follow parent chains like commons-io:2.13.0 → commons-parent:58 → junit-bom:5.9.3.
    resolved = discover_parent_poms(resolved, &resolver_config).await?;
    eprintln!(
        "gradle2nix: parent POMs done ({} total), scanning Gradle cache for missing versions...",
        resolved.len()
    );

    // 6b2. Discover all cached versions of known artifacts.
    // Some artifacts (e.g. kotlin-stdlib-common) are metadata-only (no JAR) — the TAPI shim
    // never captures them via resolvedArtifacts, but Gradle's BOM alignment may upgrade a
    // declared version to a higher one that only has .pom/.module in the Gradle cache.
    // Add .pom for ALL Gradle-cached versions of every group:artifact already in the lockfile.
    resolved = discover_all_cached_versions(resolved, &resolver_config).await?;
    eprintln!(
        "gradle2nix: cached-version scan done ({} total), resolving KMP base artifacts...",
        resolved.len()
    );

    // 6c. Discover Kotlin Multiplatform base artifacts.
    // Platform-specific JARs (e.g. kotlinx-serialization-json-jvm:1.4.0) are captured by the shim,
    // but Gradle's dependency resolution starts from the root KMP artifact
    // (kotlinx-serialization-json:1.4.0) whose .pom/.module files live in a separate cache dir.
    // Without the root metadata, offline resolution fails with "Could not resolve".
    resolved = discover_kmp_base_artifacts(resolved, &resolver_config).await?;
    eprintln!(
        "gradle2nix: KMP base artifacts done ({} total), resolving AGP aapt2 binary...",
        resolved.len()
    );

    // 6d. Discover AGP's aapt2 native binary.
    // AGP fetches `com.android.tools.build:aapt2:{version}:{os}@jar` at task execution time
    // via a detached configuration — bypassing TAPI's dependency capture entirely.
    // aapt2-proto is always co-released at the same version, so its presence signals which
    // aapt2 binary version to resolve for the linux Nix build host.
    resolved = discover_agp_aapt2_artifacts(resolved, &resolver_config).await?;
    eprintln!(
        "gradle2nix: discovery complete ({} total artifacts), building lockfile...",
        resolved.len()
    );

    if let Some(cache) = &resolver_config.resolve_cache {
        if let Err(e) = cache.persist() {
            log::warn!("could not persist resolve cache: {:#}", e);
        }
    }

    // 7. Build DependencyGraph — route each artifact to the correct repository URL.
    let mut nodes = resolved
        .into_iter()
        .map(|(coord, sha256)| {
            let repo_url = artifact_repo_url(&coord);
            let url = format!(
                "{}/{}",
                repo_url.trim_end_matches('/'),
                coord.to_artifact_path()
            );
            LockedDependency::new(coord_to_name(&coord), coord.version.clone(), url, sha256)
        })
        .collect::<Vec<_>>();

    // Sort nodes deterministically by (name, url) for reproducible lockfiles.
    // This ensures identical content produces identical output regardless of discovery order.
    nodes.sort_by(|a, b| {
        a.name
            .cmp(&b.name)
            .then_with(|| a.url.cmp(&b.url))
    });

    Ok(DependencyGraph {
        format_version: "1".to_string(),
        nodes,
    })
}

/// Flow: TAPI shim → parse JSON → convert to MavenCoordinates → resolve SHA-256 → build DependencyGraph → write lockfile
pub async fn run(cmd: LockCommand) -> anyhow::Result<()> {
    let repos = cmd.repositories.as_deref().unwrap_or(&[]);
    let graph = build_dependency_graph(
        &cmd.gradle_dir,
        repos,
        cmd.gradle_cache_dir.as_deref(),
        cmd.gradle_user_home.as_deref(),
        cmd.timeout_secs,
        cmd.shim_timeout_secs,
    )
    .await?;

    if graph.nodes.is_empty() {
        anyhow::bail!(
            "TAPI shim returned 0 artifacts — Gradle likely failed to resolve the project. \
             Check Gradle output above for errors before overwriting the lockfile."
        );
    }

    let output_path = cmd
        .output
        .unwrap_or_else(|| cmd.gradle_dir.join("gradle2nix.lock"));

    nix_core::lockfile::write_lockfile(&output_path, &graph)
        .with_context(|| format!("writing lockfile to '{}'", output_path.display()))?;

    println!("Wrote lockfile: {}", output_path.display());
    Ok(())
}

/// Discover AGP's aapt2 native binary artifact.
///
/// AGP fetches `com.android.tools.build:aapt2:{version}:linux@jar` at task execution time via a
/// detached configuration, bypassing TAPI's dependency capture. aapt2-proto is always co-released
/// at the same version, so its presence identifies which aapt2 binary version the offline Nix
/// build will need. Resolves the linux-classified JAR from Google Maven.
async fn discover_agp_aapt2_artifacts(
    mut resolved: Vec<(MavenCoordinate, String)>,
    config: &MavenResolverConfig,
) -> anyhow::Result<Vec<(MavenCoordinate, String)>> {
    let seen: std::collections::HashSet<String> = resolved
        .iter()
        .map(|(c, _)| {
            format!(
                "{}:{}:{}:{}:{}",
                c.group,
                c.artifact,
                c.version,
                c.classifier.as_deref().unwrap_or(""),
                c.extension
            )
        })
        .collect();

    let aapt2_versions: std::collections::HashSet<String> = resolved
        .iter()
        .filter(|(c, _)| c.group == "com.android.tools.build" && c.artifact == "aapt2-proto")
        .map(|(c, _)| c.version.clone())
        .collect();

    let candidates: Vec<MavenCoordinate> = aapt2_versions
        .into_iter()
        .filter_map(|version| {
            let coord = MavenCoordinate {
                group: "com.android.tools.build".to_string(),
                artifact: "aapt2".to_string(),
                version: version.clone(),
                classifier: Some("linux".to_string()),
                extension: "jar".to_string(),
            };
            let key = format!("{}:{}:{}:linux:jar", coord.group, coord.artifact, version);
            if seen.contains(&key) {
                None
            } else {
                Some(coord)
            }
        })
        .collect();

    let mut set = tokio::task::JoinSet::new();
    for coord in candidates {
        let config = config.clone();
        set.spawn(async move {
            match resolve_artifact_sha256(&coord, &config).await {
                Ok(sha256) => Some((coord, sha256)),
                Err(_) => None,
            }
        });
    }
    while let Some(result) = set.join_next().await {
        if let Ok(Some(entry)) = result {
            resolved.push(entry);
        }
    }

    Ok(resolved)
}

/// For every group:artifact in `resolved`, add `.pom` entries for ALL other versions of that
/// artifact present in the local Gradle cache. This catches metadata-only artifacts (e.g.
/// `kotlin-stdlib-common:2.0.21` which has only `.pom`/`.module`, no JAR) that are never
/// surfaced by `resolvedArtifacts` but may be selected by BOM version alignment at offline
/// build time.
async fn discover_all_cached_versions(
    mut resolved: Vec<(MavenCoordinate, String)>,
    config: &MavenResolverConfig,
) -> anyhow::Result<Vec<(MavenCoordinate, String)>> {
    let gradle_home = config.discovery_gradle_home();
    let gradle_home = match gradle_home {
        Some(h) => h,
        None => return Ok(resolved),
    };
    let modules_dir = gradle_home.join("caches/modules-2/files-2.1");

    // Deduplicate group:artifact pairs from the resolved set.
    let pairs: Vec<(String, String)> = resolved
        .iter()
        .map(|(c, _)| (c.group.clone(), c.artifact.clone()))
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    // Track by group:artifact:version:extension so we can add both .pom and .module per version.
    let mut seen_coords: std::collections::HashSet<String> = resolved
        .iter()
        .map(|(c, _)| format!("{}:{}:{}:{}", c.group, c.artifact, c.version, c.extension))
        .collect();

    // Phase 1: fast filesystem scan — collect candidates without doing any network I/O.
    // Try both .pom and .module: metadata-only artifacts (e.g. kotlin-stdlib-common post-1.8)
    // have no JAR; Gradle needs the .module file to avoid falling back to a POM that implies one.
    let mut candidates: Vec<MavenCoordinate> = Vec::new();
    for (group, artifact) in &pairs {
        let art_dir = modules_dir.join(group).join(artifact);
        let version_entries = match std::fs::read_dir(&art_dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in version_entries.flatten() {
            let version = match entry.file_name().into_string() {
                Ok(v) => v,
                Err(_) => continue,
            };
            for ext in &["pom", "module"] {
                let key = format!("{}:{}:{}:{}", group, artifact, version, ext);
                if !seen_coords.insert(key) {
                    continue;
                }
                candidates.push(MavenCoordinate {
                    group: group.clone(),
                    artifact: artifact.clone(),
                    version: version.clone(),
                    classifier: None,
                    extension: ext.to_string(),
                });
            }
        }
    }

    let total = candidates.len();
    eprintln!(
        "gradle2nix: cached-version scan — {} entries across {} group:artifact pairs",
        total,
        pairs.len()
    );

    // Phase 2: concurrent resolution with bounded parallelism.
    let sem = std::sync::Arc::new(tokio::sync::Semaphore::new(config.max_concurrency));
    let mut set = tokio::task::JoinSet::new();
    for coord in candidates {
        let config = config.clone();
        let sem = sem.clone();
        set.spawn(async move {
            let _permit = sem.acquire().await.ok()?;
            match resolve_artifact_sha256(&coord, &config).await {
                Ok(sha256) => Some((coord, sha256)),
                Err(_) => None,
            }
        });
    }

    let mut completed = 0usize;
    while let Some(result) = set.join_next().await {
        if let Ok(Some(entry)) = result {
            resolved.push(entry);
        }
        completed += 1;
        if total > 0 {
            eprint!("\rgradle2nix: cached-version scan {}/{}", completed, total);
        }
    }
    if total > 0 {
        eprintln!(); // move past the progress line
    }

    Ok(resolved)
}

/// For every POM in `resolved`, walk its `<dependency>` elements and ensure each declared
/// exact version is present in the offline Maven repo — even when conflict resolution selected
/// a higher version. Restricted to versions already in the local Gradle cache so we never
/// fetch things Gradle didn't actually download during the online resolution run.
async fn discover_declared_dep_poms(
    mut resolved: Vec<(MavenCoordinate, String)>,
    config: &MavenResolverConfig,
) -> anyhow::Result<Vec<(MavenCoordinate, String)>> {
    use futures::stream::{self, StreamExt};

    let gradle_home = config.discovery_gradle_home();
    let gradle_home = match gradle_home {
        Some(h) => h,
        None => return Ok(resolved),
    };
    let modules_dir = gradle_home.join("caches/modules-2/files-2.1");

    let mut seen: std::collections::HashSet<String> = resolved
        .iter()
        .map(|(c, _)| format!("{}:{}:{}", c.group, c.artifact, c.version))
        .collect();

    let mut to_process: Vec<MavenCoordinate> = resolved
        .iter()
        .filter(|(c, _)| c.extension == "pom")
        .map(|(c, _)| c.clone())
        .collect();

    // Wave-based BFS: fetch every POM of the wave concurrently, extract candidates,
    // then resolve all new candidates concurrently. Each wave is one network
    // round-trip deep instead of one round-trip per POM.
    while !to_process.is_empty() {
        let pom_texts: Vec<Option<String>> = stream::iter(to_process)
            .map(|coord| async move {
                let single_repo = vec![artifact_repo_url(&coord)];
                fetch_pom_content(&coord, &single_repo, config).await
            })
            .buffer_unordered(config.max_concurrency)
            .collect()
            .await;

        let mut candidates: Vec<MavenCoordinate> = Vec::new();
        for pom_text in pom_texts.into_iter().flatten() {
            for (dep_g, dep_a, dep_v) in extract_pom_direct_deps(&pom_text) {
                let key = format!("{}:{}:{}", dep_g, dep_a, dep_v);
                if !seen.insert(key) {
                    continue;
                }
                // Only include if this exact version is already in the local Gradle cache.
                if !modules_dir.join(&dep_g).join(&dep_a).join(&dep_v).exists() {
                    continue;
                }
                candidates.push(MavenCoordinate {
                    group: dep_g,
                    artifact: dep_a,
                    version: dep_v,
                    classifier: None,
                    extension: "pom".to_string(),
                });
            }
        }

        let wave: Vec<Option<(MavenCoordinate, String)>> = stream::iter(candidates)
            .map(|coord| async move {
                match resolve_artifact_sha256(&coord, config).await {
                    Ok(sha256) => Some((coord, sha256)),
                    Err(e) => {
                        log::debug!(
                            "declared dep POM not resolvable {}: {:#}",
                            coord.to_artifact_path(),
                            e
                        );
                        None
                    }
                }
            })
            .buffer_unordered(config.max_concurrency)
            .collect()
            .await;

        let mut next_wave: Vec<MavenCoordinate> = Vec::new();
        for (coord, sha256) in wave.into_iter().flatten() {
            next_wave.push(coord.clone());
            resolved.push((coord, sha256));
        }
        to_process = next_wave;
    }

    Ok(resolved)
}

/// For every platform-specific KMP artifact (e.g. `foo-jvm:1.0`), resolve the root KMP
/// metadata artifact (`foo:1.0`) — both `.pom` and `.module`. Gradle's offline resolution
/// starts from the root artifact and uses the `.module` file to select the JVM variant;
/// without it, resolution fails even when the `-jvm` JAR is present.
async fn discover_kmp_base_artifacts(
    mut resolved: Vec<(MavenCoordinate, String)>,
    config: &MavenResolverConfig,
) -> anyhow::Result<Vec<(MavenCoordinate, String)>> {
    const KMP_SUFFIXES: &[&str] = &[
        "-jvm",
        "-js",
        "-android",
        "-native",
        "-linuxX64",
        "-linuxArm64",
        "-macosX64",
        "-macosArm64",
        "-iosArm64",
        "-iosX64",
        "-iosSimulatorArm64",
        "-watchosX64",
        "-watchosArm32",
        "-watchosArm64",
        "-tvosX64",
        "-tvosArm64",
        "-mingwX64",
    ];

    let mut seen: std::collections::HashSet<String> = resolved
        .iter()
        .map(|(c, _)| format!("{}:{}:{}:{}", c.group, c.artifact, c.version, c.extension))
        .collect();

    let mut candidates: Vec<(String, String, String, String)> = Vec::new();
    for (coord, _) in &resolved {
        for suffix in KMP_SUFFIXES {
            if let Some(base_name) = coord.artifact.strip_suffix(suffix) {
                for ext in &["pom", "module"] {
                    let key = format!("{}:{}:{}:{}", coord.group, base_name, coord.version, ext);
                    if seen.insert(key) {
                        candidates.push((
                            coord.group.clone(),
                            base_name.to_string(),
                            coord.version.clone(),
                            ext.to_string(),
                        ));
                    }
                }
            }
        }
    }

    // Resolve candidates concurrently, ignoring failures (not all stripped names are real artifacts).
    let mut set = tokio::task::JoinSet::new();
    for (group, artifact, version, ext) in candidates {
        let coord = MavenCoordinate {
            group,
            artifact,
            version,
            classifier: None,
            extension: ext,
        };
        let config = config.clone();
        set.spawn(async move {
            match resolve_artifact_sha256(&coord, &config).await {
                Ok(sha256) => Some((coord, sha256)),
                Err(_) => None,
            }
        });
    }
    while let Some(result) = set.join_next().await {
        if let Ok(Some(entry)) = result {
            resolved.push(entry);
        }
    }

    Ok(resolved)
}

/// Transitively discover and resolve parent POMs for all POM artifacts in `resolved`.
/// Gradle resolves transitive POMs internally during dependency graph parsing but never
/// surfaces them in `resolvedArtifacts`. Without them in the offline repo, Gradle fails
/// with "Could not parse POM" when reading any artifact whose POM has a `<parent>`.
async fn discover_parent_poms(
    mut resolved: Vec<(MavenCoordinate, String)>,
    config: &MavenResolverConfig,
) -> anyhow::Result<Vec<(MavenCoordinate, String)>> {
    use futures::stream::{self, StreamExt};

    let mut seen: std::collections::HashSet<String> = resolved
        .iter()
        .filter(|(c, _)| c.extension == "pom")
        .map(|(c, _)| format!("{}:{}:{}", c.group, c.artifact, c.version))
        .collect();

    let mut to_fetch: Vec<MavenCoordinate> = resolved
        .iter()
        .filter(|(c, _)| c.extension == "pom")
        .map(|(c, _)| c.clone())
        .collect();

    // Wave-based BFS, same shape as discover_declared_dep_poms: parent chains are
    // shallow (3-4 levels), so the whole walk costs a handful of concurrent waves.
    while !to_fetch.is_empty() {
        let pom_texts: Vec<Option<String>> = stream::iter(to_fetch)
            .map(|coord| async move {
                let single_repo = vec![artifact_repo_url(&coord)];
                fetch_pom_content(&coord, &single_repo, config).await
            })
            .buffer_unordered(config.max_concurrency)
            .collect()
            .await;

        // Collect both <parent> and <dependencyManagement> BOM imports.
        let mut candidates: Vec<MavenCoordinate> = Vec::new();
        for pom_text in pom_texts.into_iter().flatten() {
            let mut found: Vec<(String, String, String)> = Vec::new();
            if let Some(parent) = extract_pom_parent(&pom_text) {
                found.push(parent);
            }
            found.extend(extract_pom_bom_imports(&pom_text));

            for (pg, pa, pv) in found {
                let key = format!("{}:{}:{}", pg, pa, pv);
                if !seen.insert(key) {
                    continue;
                }
                candidates.push(MavenCoordinate {
                    group: pg,
                    artifact: pa,
                    version: pv,
                    classifier: None,
                    extension: "pom".to_string(),
                });
            }
        }

        let wave: Vec<Option<(MavenCoordinate, String)>> = stream::iter(candidates)
            .map(|coord| async move {
                match resolve_artifact_sha256(&coord, config).await {
                    Ok(sha256) => Some((coord, sha256)),
                    Err(e) => {
                        log::warn!(
                            "Could not resolve POM dep {}: {:#}",
                            coord.to_artifact_path(),
                            e
                        );
                        None
                    }
                }
            })
            .buffer_unordered(config.max_concurrency)
            .collect()
            .await;

        let mut next_wave: Vec<MavenCoordinate> = Vec::new();
        for (coord, sha256) in wave.into_iter().flatten() {
            next_wave.push(coord.clone());
            resolved.push((coord, sha256));
        }
        to_fetch = next_wave;
    }

    Ok(resolved)
}

#[cfg(test)]
mod lock_tests {
    use super::*;
    use nix_core::dep::DependencyGraph;

    /// Helper to create a LockedDependency
    fn make_node(name: &str, version: &str, url: &str, sha256: &str) -> LockedDependency {
        LockedDependency::new(name.to_string(), version.to_string(), url.to_string(), sha256.to_string())
    }

    #[test]
    fn test_build_dependency_graph_nodes_sorted_by_name_then_url() {
        // Create a DependencyGraph with unsorted nodes (intentionally out of order)
        let graph = DependencyGraph {
            format_version: "1".to_string(),
            nodes: vec![
                // Deliberately unsorted: z first, then a, then middle
                make_node(
                    "org.slf4j:slf4j-api:2.0.9",
                    "2.0.9",
                    "https://repo.maven.apache.org/maven2/org/slf4j/slf4j-api/2.0.9/slf4j-api-2.0.9.jar",
                    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                ),
                make_node(
                    "com.google.guava:guava:31.1-jre",
                    "31.1-jre",
                    "https://repo.maven.apache.org/maven2/com/google/guava/guava/31.1-jre/guava-31.1-jre.jar",
                    "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                ),
                make_node(
                    "junit:junit:4.13.2",
                    "4.13.2",
                    "https://repo.maven.apache.org/maven2/junit/junit/4.13.2/junit-4.13.2.jar",
                    "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
                ),
            ],
        };

        // Verify they are indeed unsorted in the input
        assert_eq!(graph.nodes[0].name, "org.slf4j:slf4j-api:2.0.9");
        assert_eq!(graph.nodes[1].name, "com.google.guava:guava:31.1-jre");
        assert_eq!(graph.nodes[2].name, "junit:junit:4.13.2");

        // Now manually test the sorting logic that build_dependency_graph uses
        let mut sorted = graph.nodes.clone();
        sorted.sort_by(|a, b| {
            a.name
                .cmp(&b.name)
                .then_with(|| a.url.cmp(&b.url))
        });

        // Verify they are sorted by name (and url as tiebreaker)
        assert_eq!(sorted[0].name, "com.google.guava:guava:31.1-jre");
        assert_eq!(sorted[1].name, "junit:junit:4.13.2");
        assert_eq!(sorted[2].name, "org.slf4j:slf4j-api:2.0.9");
    }

    #[test]
    fn test_node_ordering_with_same_name_different_urls() {
        // Test that when names are identical (e.g. same coordinate but .jar and .pom),
        // the secondary sort by URL puts them in a consistent order
        let graph = DependencyGraph {
            format_version: "1".to_string(),
            nodes: vec![
                make_node(
                    "com.example:lib:1.0",
                    "1.0",
                    "https://example.com/lib/1.0/lib-1.0.pom",
                    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                ),
                make_node(
                    "com.example:lib:1.0",
                    "1.0",
                    "https://example.com/lib/1.0/lib-1.0.jar",
                    "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                ),
            ],
        };

        let mut sorted = graph.nodes.clone();
        sorted.sort_by(|a, b| {
            a.name
                .cmp(&b.name)
                .then_with(|| a.url.cmp(&b.url))
        });

        // URL-based sort should put .jar before .pom (lexicographic)
        assert_eq!(sorted[0].url, "https://example.com/lib/1.0/lib-1.0.jar");
        assert_eq!(sorted[1].url, "https://example.com/lib/1.0/lib-1.0.pom");
    }
}
