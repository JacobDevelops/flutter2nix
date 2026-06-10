use anyhow::Context;
use nix_core::dep::{DependencyGraph, LockedDependency};
use std::path::{Path, PathBuf};

use crate::maven::{extract_pom_bom_imports, extract_pom_direct_deps, extract_pom_parent, fetch_pom_content, resolve_artifact_sha256, resolve_artifacts_batch, MavenCoordinate, MavenResolverConfig};
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
}

fn coord_to_name(coord: &MavenCoordinate) -> String {
    match &coord.classifier {
        Some(c) => format!("{}:{}:{}:{}", coord.group, coord.artifact, coord.version, c),
        None => format!("{}:{}:{}", coord.group, coord.artifact, coord.version),
    }
}

/// Route artifact to the correct repository base URL for the lockfile.
/// This determines the download URL written into gradle2nix.lock — it must match
/// the repo that actually hosts the artifact.
fn artifact_repo_url(coord: &MavenCoordinate, _configured_repos: &[String]) -> String {
    const FLUTTER_STORAGE: &str = "https://storage.googleapis.com/download.flutter.io";
    const GOOGLE_MAVEN: &str = "https://dl.google.com/dl/android/maven2";
    const MAVEN_CENTRAL: &str = "https://repo.maven.apache.org/maven2";
    const GRADLE_PLUGIN_PORTAL: &str = "https://plugins.gradle.org/m2";

    if coord.group.starts_with("io.flutter") {
        return FLUTTER_STORAGE.to_string();
    }

    let is_group = |g: &str| coord.group == g || coord.group.starts_with(&format!("{}.", g));

    // Gradle's own Kotlin DSL plugins are published to the Gradle Plugin Portal, not Maven Central.
    if is_group("org.gradle.kotlin") {
        return GRADLE_PLUGIN_PORTAL.to_string();
    }

    // Only Google's own namespaces belong on Google Maven. Third-party AARs
    // (e.g. com.getkeepsafe.relinker) are published to Maven Central.
    let is_google = is_group("androidx")
        || is_group("com.android")
        || is_group("com.google.android")
        || is_group("com.google.firebase")
        || is_group("com.google.gms")
        || is_group("com.google.ar");

    if is_google { GOOGLE_MAVEN.to_string() } else { MAVEN_CENTRAL.to_string() }
}

/// Core lock pipeline: TAPI → parse → resolve SHA-256 → DependencyGraph.
/// Shared by `lock::run` and `check::run`.
pub async fn build_dependency_graph(
    gradle_dir: &Path,
    repositories: &[String],
    gradle_cache_dir: Option<&Path>,
    gradle_user_home: Option<&Path>,
    timeout_secs: u64,
) -> anyhow::Result<DependencyGraph> {
    // 1. Use sidecar for test injection if present
    let sidecar = gradle_dir.join(".gradle2nix-tapi-output.json");
    let tapi_json_override = if sidecar.exists() {
        Some(std::fs::read_to_string(&sidecar).with_context(|| {
            format!("reading TAPI sidecar '{}'", sidecar.display())
        })?)
    } else {
        None
    };

    // 2. Invoke TAPI shim (or use override)
    eprintln!("gradle2nix: running TAPI shim (Gradle dependency extraction)...");
    let raw_json = invoke_tapi_shim(TapiShimConfig {
        gradle_project_dir: gradle_dir.to_path_buf(),
        gradle_user_home: gradle_user_home.map(PathBuf::from),
        gradle_cache_dir: gradle_cache_dir.map(PathBuf::from),
        timeout_secs,
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
                Some(c) => format!("{}:{}:{}:{}@{}", art.group, art.artifact, art.version, c, art.extension),
                None => format!("{}:{}:{}@{}", art.group, art.artifact, art.version, art.extension),
            };
            MavenCoordinate::parse(&s)
        })
        .collect::<anyhow::Result<_>>()?;
    eprintln!("gradle2nix: TAPI captured {} artifacts, resolving SHA-256s...", coords.len());

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

    let resolver_config = MavenResolverConfig {
        repository_urls: repo_urls.clone(),
        local_cache_dir: gradle_cache_dir.map(PathBuf::from),
        gradle_user_home: gradle_user_home.map(PathBuf::from),
        timeout_secs,
        max_concurrency: 10,
    };
    let mut resolved = resolve_artifacts_batch(&coords, &resolver_config).await?;
    eprintln!("gradle2nix: resolved {} artifacts, running discovery phases...", resolved.len());

    // 6. Discover declared-version POMs first.
    // Gradle's offline resolution reads every POM that a resolved artifact transitively references
    // and needs the exact declared version present — even if conflict resolution selected a higher
    // version. The shim only captures the selected (conflict-resolved) versions, so declared-but-
    // downgraded versions are missing. Walk the <dependency> elements in each captured POM and add
    // the declared-version POM for any artifact that already exists in the local Gradle cache.
    // Must run BEFORE discover_parent_poms so parent/BOM chains of newly-added POMs are followed.
    resolved = discover_declared_dep_poms(resolved, &resolver_config).await?;
    eprintln!("gradle2nix: declared-dep POMs done ({} total), discovering parent POMs...", resolved.len());

    // 6b. Discover transitive parent POMs + BOM imports.
    // Now sees the full set of POMs including declared-version additions above, so it can
    // follow parent chains like commons-io:2.13.0 → commons-parent:58 → junit-bom:5.9.3.
    resolved = discover_parent_poms(resolved, &resolver_config, &repo_urls).await?;
    eprintln!("gradle2nix: parent POMs done ({} total), scanning Gradle cache for missing versions...", resolved.len());

    // 6b2. Discover all cached versions of known artifacts.
    // Some artifacts (e.g. kotlin-stdlib-common) are metadata-only (no JAR) — the TAPI shim
    // never captures them via resolvedArtifacts, but Gradle's BOM alignment may upgrade a
    // declared version to a higher one that only has .pom/.module in the Gradle cache.
    // Add .pom for ALL Gradle-cached versions of every group:artifact already in the lockfile.
    resolved = discover_all_cached_versions(resolved, &resolver_config).await?;
    eprintln!("gradle2nix: cached-version scan done ({} total), resolving KMP base artifacts...", resolved.len());

    // 6c. Discover Kotlin Multiplatform base artifacts.
    // Platform-specific JARs (e.g. kotlinx-serialization-json-jvm:1.4.0) are captured by the shim,
    // but Gradle's dependency resolution starts from the root KMP artifact
    // (kotlinx-serialization-json:1.4.0) whose .pom/.module files live in a separate cache dir.
    // Without the root metadata, offline resolution fails with "Could not resolve".
    resolved = discover_kmp_base_artifacts(resolved, &resolver_config).await?;
    eprintln!("gradle2nix: KMP base artifacts done ({} total), resolving AGP aapt2 binary...", resolved.len());

    // 6d. Discover AGP's aapt2 native binary.
    // AGP fetches `com.android.tools.build:aapt2:{version}:{os}@jar` at task execution time
    // via a detached configuration — bypassing TAPI's dependency capture entirely.
    // aapt2-proto is always co-released at the same version, so its presence signals which
    // aapt2 binary version to resolve for the linux Nix build host.
    resolved = discover_agp_aapt2_artifacts(resolved, &resolver_config).await?;
    eprintln!("gradle2nix: discovery complete ({} total artifacts), building lockfile...", resolved.len());

    // 7. Build DependencyGraph — route each artifact to the correct repository URL.
    let nodes = resolved
        .into_iter()
        .map(|(coord, sha256)| {
            let repo_url = artifact_repo_url(&coord, &repo_urls);
            let url = format!("{}/{}", repo_url.trim_end_matches('/'), coord.to_artifact_path());
            LockedDependency::new(
                coord_to_name(&coord),
                coord.version.clone(),
                url,
                sha256,
            )
        })
        .collect();

    Ok(DependencyGraph { format_version: "1".to_string(), nodes })
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

    crate::lockfile::write_lockfile(&output_path, &graph)
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
            if seen.contains(&key) { None } else { Some(coord) }
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
    let gradle_home = config.discovery_gradle_home();
    let gradle_home = match gradle_home {
        Some(h) => h,
        None => return Ok(resolved),
    };
    let modules_dir = gradle_home.join("caches/modules-2/files-2.1");

    let client = reqwest::Client::new();
    let mut seen: std::collections::HashSet<String> = resolved
        .iter()
        .map(|(c, _)| format!("{}:{}:{}", c.group, c.artifact, c.version))
        .collect();

    let mut to_process: Vec<MavenCoordinate> = resolved
        .iter()
        .filter(|(c, _)| c.extension == "pom")
        .map(|(c, _)| c.clone())
        .collect();

    while !to_process.is_empty() {
        let mut next_wave: Vec<MavenCoordinate> = Vec::new();

        for coord in &to_process {
            let repo_url = artifact_repo_url(coord, &config.repository_urls);
            let single_repo = vec![repo_url.clone()];
            let pom_text = match fetch_pom_content(coord, &single_repo, &client, config.timeout_secs).await {
                Some(t) => t,
                None => continue,
            };

            for (dep_g, dep_a, dep_v) in extract_pom_direct_deps(&pom_text) {
                let key = format!("{}:{}:{}", dep_g, dep_a, dep_v);
                if !seen.insert(key) {
                    continue;
                }
                // Only include if this exact version is already in the local Gradle cache.
                let cache_ver_dir = modules_dir.join(&dep_g).join(&dep_a).join(&dep_v);
                if !cache_ver_dir.exists() {
                    continue;
                }
                let dep_coord = MavenCoordinate {
                    group: dep_g,
                    artifact: dep_a,
                    version: dep_v,
                    classifier: None,
                    extension: "pom".to_string(),
                };
                match resolve_artifact_sha256(&dep_coord, config).await {
                    Ok(sha256) => {
                        next_wave.push(dep_coord.clone());
                        resolved.push((dep_coord, sha256));
                    }
                    Err(e) => {
                        log::debug!("declared dep POM not resolvable {}: {:#}", dep_coord.to_artifact_path(), e);
                    }
                }
            }
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
        "-jvm", "-js", "-android", "-native",
        "-linuxX64", "-linuxArm64",
        "-macosX64", "-macosArm64",
        "-iosArm64", "-iosX64", "-iosSimulatorArm64",
        "-watchosX64", "-watchosArm32", "-watchosArm64",
        "-tvosX64", "-tvosArm64",
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
                        candidates.push((coord.group.clone(), base_name.to_string(), coord.version.clone(), ext.to_string()));
                    }
                }
            }
        }
    }

    // Resolve candidates concurrently, ignoring failures (not all stripped names are real artifacts).
    let mut set = tokio::task::JoinSet::new();
    for (group, artifact, version, ext) in candidates {
        let coord = MavenCoordinate { group, artifact, version, classifier: None, extension: ext };
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
    repo_urls: &[String],
) -> anyhow::Result<Vec<(MavenCoordinate, String)>> {
    let client = reqwest::Client::new();
    let mut seen: std::collections::HashSet<String> = resolved
        .iter()
        .filter(|(c, _)| c.extension == "pom")
        .map(|(c, _)| format!("{}:{}:{}", c.group, c.artifact, c.version))
        .collect();

    let mut to_fetch: Vec<(MavenCoordinate, String)> = resolved
        .iter()
        .filter(|(c, _)| c.extension == "pom")
        .cloned()
        .collect();

    while !to_fetch.is_empty() {
        let mut next_wave: Vec<(MavenCoordinate, String)> = Vec::new();

        for (coord, _) in &to_fetch {
            let repo_url = artifact_repo_url(coord, repo_urls);
            let single_repo = vec![repo_url.clone()];
            let content = fetch_pom_content(coord, &single_repo, &client, config.timeout_secs).await;
            let pom_text = match content {
                Some(t) => t,
                None => continue,
            };

            // Collect both <parent> and <dependencyManagement> BOM imports.
            let mut candidates: Vec<(String, String, String)> = Vec::new();
            if let Some(parent) = extract_pom_parent(&pom_text) {
                candidates.push(parent);
            }
            candidates.extend(extract_pom_bom_imports(&pom_text));

            for (pg, pa, pv) in candidates {
                let key = format!("{}:{}:{}", pg, pa, pv);
                if !seen.insert(key) {
                    continue;
                }
                let dep_coord = MavenCoordinate {
                    group: pg,
                    artifact: pa,
                    version: pv,
                    classifier: None,
                    extension: "pom".to_string(),
                };
                match resolve_artifact_sha256(&dep_coord, config).await {
                    Ok(sha256) => {
                        next_wave.push((dep_coord.clone(), sha256.clone()));
                        resolved.push((dep_coord, sha256));
                    }
                    Err(e) => {
                        log::warn!("Could not resolve POM dep {}: {:#}", dep_coord.to_artifact_path(), e);
                    }
                }
            }
        }

        to_fetch = next_wave;
    }

    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn coord(group: &str, artifact: &str, version: &str, ext: &str) -> MavenCoordinate {
        MavenCoordinate {
            group: group.to_string(),
            artifact: artifact.to_string(),
            version: version.to_string(),
            classifier: None,
            extension: ext.to_string(),
        }
    }

    #[test]
    fn firebase_exact_group_routes_to_google_maven() {
        // Regression: "com.google.firebase" (no subpackage) was routed to Maven Central
        // because starts_with("com.google.firebase.") failed on the exact group name.
        let c = coord("com.google.firebase", "firebase-annotations", "16.2.0", "jar");
        assert_eq!(
            artifact_repo_url(&c, &[]),
            "https://dl.google.com/dl/android/maven2"
        );
    }

    #[test]
    fn firebase_subpackage_routes_to_google_maven() {
        let c = coord("com.google.firebase.encoders", "firebase-encoders-json", "18.0.0", "jar");
        assert_eq!(
            artifact_repo_url(&c, &[]),
            "https://dl.google.com/dl/android/maven2"
        );
    }

    #[test]
    fn androidx_subpackage_routes_to_google_maven() {
        let c = coord("androidx.core", "core-ktx", "1.12.0", "aar");
        assert_eq!(
            artifact_repo_url(&c, &[]),
            "https://dl.google.com/dl/android/maven2"
        );
    }

    #[test]
    fn third_party_aar_routes_to_maven_central() {
        // AARs from third-party groups are on Maven Central, not Google Maven.
        let c = coord("com.getkeepsafe.relinker", "relinker", "1.4.5", "aar");
        assert_eq!(
            artifact_repo_url(&c, &[]),
            "https://repo.maven.apache.org/maven2"
        );
    }

    #[test]
    fn kotlin_stdlib_routes_to_maven_central() {
        let c = coord("org.jetbrains.kotlin", "kotlin-stdlib", "1.9.0", "jar");
        assert_eq!(
            artifact_repo_url(&c, &[]),
            "https://repo.maven.apache.org/maven2"
        );
    }

    #[test]
    fn flutter_io_routes_to_flutter_storage() {
        let c = coord("io.flutter", "flutter_embedding_debug", "1.0.0-abc123", "jar");
        assert_eq!(
            artifact_repo_url(&c, &[]),
            "https://storage.googleapis.com/download.flutter.io"
        );
    }

    #[test]
    fn gradle_kotlin_dsl_routes_to_plugin_portal() {
        // Gradle's own Kotlin DSL plugins are only published to the Gradle Plugin Portal;
        // routing them to Maven Central would produce 404 URLs in the lockfile.
        let c = coord("org.gradle.kotlin", "gradle-kotlin-dsl-plugins", "5.2.0", "jar");
        assert_eq!(
            artifact_repo_url(&c, &[]),
            "https://plugins.gradle.org/m2"
        );
    }
}
