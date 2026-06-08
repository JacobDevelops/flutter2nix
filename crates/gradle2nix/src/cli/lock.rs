use anyhow::Context;
use nix_core::dep::{DependencyGraph, LockedDependency};
use std::path::{Path, PathBuf};

use crate::maven::{resolve_artifacts_batch, MavenCoordinate, MavenResolverConfig};
use crate::tapi::model::parse_tapi_output;
use crate::tapi::shim::{invoke_tapi_shim, TapiShimConfig};

pub struct LockCommand {
    pub gradle_dir: PathBuf,
    pub output: Option<PathBuf>,
    pub repositories: Option<Vec<String>>,
    /// Local Maven repo directory for sha256 lookup (used in tests; None in production).
    pub gradle_cache_dir: Option<PathBuf>,
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

    if coord.group.starts_with("io.flutter") {
        return FLUTTER_STORAGE.to_string();
    }

    // Match exact group names AND their sub-namespaces (e.g. "com.google.firebase" and
    // "com.google.firebase.encoders" are both Google Maven artifacts, not Maven Central).
    let is_google_group = |g: &str| coord.group == g || coord.group.starts_with(&format!("{}.", g));
    let is_google = coord.extension == "aar"
        || is_google_group("androidx")
        || is_google_group("com.google.android")
        || is_google_group("com.google.firebase")
        || coord.group == "com.android.tools.build"
        || coord.group == "com.android.tools";

    if is_google { GOOGLE_MAVEN.to_string() } else { MAVEN_CENTRAL.to_string() }
}

/// Core lock pipeline: TAPI → parse → resolve SHA-256 → DependencyGraph.
/// Shared by `lock::run` and `check::run`.
pub async fn build_dependency_graph(
    gradle_dir: &Path,
    repositories: &[String],
    gradle_cache_dir: Option<&Path>,
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
    let raw_json = invoke_tapi_shim(TapiShimConfig {
        gradle_project_dir: gradle_dir.to_path_buf(),
        gradle_user_home: None,
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

    // 5. Resolve SHA-256 for each coordinate
    // Three default repos cover the whole Android/Flutter ecosystem:
    //   1. Maven Central  — Kotlin stdlib, JetBrains, Apache, etc.
    //   2. Google Maven   — AndroidX, Firebase, AGP, GMS
    //   3. Flutter storage — io.flutter embedding JARs (version = engine commit hash)
    const MAVEN_CENTRAL: &str = "https://repo.maven.apache.org/maven2";
    const GOOGLE_MAVEN: &str = "https://dl.google.com/dl/android/maven2";
    const FLUTTER_STORAGE: &str = "https://storage.googleapis.com/download.flutter.io";
    let repo_urls = if repositories.is_empty() {
        vec![MAVEN_CENTRAL.to_string(), GOOGLE_MAVEN.to_string(), FLUTTER_STORAGE.to_string()]
    } else {
        repositories.to_vec()
    };

    let resolver_config = MavenResolverConfig {
        repository_urls: repo_urls.clone(),
        local_cache_dir: gradle_cache_dir.map(PathBuf::from),
        gradle_user_home: None,
        timeout_secs,
        max_concurrency: 10,
    };
    let resolved = resolve_artifacts_batch(&coords, &resolver_config).await?;

    // 6. Build DependencyGraph — route each artifact to the correct repository URL.
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
        cmd.timeout_secs,
    )
    .await?;

    let output_path = cmd
        .output
        .unwrap_or_else(|| cmd.gradle_dir.join("gradle2nix.lock"));

    crate::lockfile::write_lockfile(&output_path, &graph)
        .with_context(|| format!("writing lockfile to '{}'", output_path.display()))?;

    println!("Wrote lockfile: {}", output_path.display());
    Ok(())
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
    fn aar_extension_always_routes_to_google_maven() {
        let c = coord("org.example", "some-android-lib", "1.0.0", "aar");
        assert_eq!(
            artifact_repo_url(&c, &[]),
            "https://dl.google.com/dl/android/maven2"
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
}
