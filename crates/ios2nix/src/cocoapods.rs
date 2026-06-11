use serde_yml::Value;
use std::collections::BTreeMap;

/// Represents a single pod in a Podfile.lock
#[derive(Debug, Clone, PartialEq)]
pub struct Pod {
    pub name: String,
    pub version: String,
    pub deps: Vec<String>,
}

/// Represents an external source entry in Podfile.lock
#[derive(Debug, Clone, PartialEq)]
pub struct ExternalSource {
    pub path: Option<String>,
    pub git: Option<String>,
    pub podspec: Option<String>,
}

/// Represents a checkout option entry (for git-source pods)
#[derive(Debug, Clone, PartialEq)]
pub struct CheckoutOptions {
    pub git: String,
    pub tag: Option<String>,
    pub commit: Option<String>,
    pub branch: Option<String>,
}

/// Parsed Podfile.lock with all sections
#[derive(Debug, Clone, PartialEq)]
pub struct PodfileLock {
    pub pods: Vec<Pod>,
    pub spec_checksums: BTreeMap<String, String>,
    pub podfile_checksum: String,
    pub cocoapods_version: String,
    pub external_sources: BTreeMap<String, ExternalSource>,
    pub checkout_options: BTreeMap<String, CheckoutOptions>,
}

/// Source kind for a pod
#[derive(Debug, Clone, PartialEq)]
pub enum PodSourceKind {
    Http { url: String },
    Git { url: String, rev: String },
    Path { path: String },
}

/// Parse a Podfile.lock (YAML format) into a structured PodfileLock.
pub fn parse_podfile_lock(yaml: &str) -> anyhow::Result<PodfileLock> {
    let value: Value = serde_yml::from_str(yaml)
        .map_err(|e| anyhow::anyhow!("invalid YAML in Podfile.lock: {}", e))?;

    let root = value
        .as_mapping()
        .ok_or_else(|| anyhow::anyhow!("Podfile.lock root must be a YAML mapping"))?;

    // Parse PODS section
    let pods = parse_pods_section(root)?;

    // Parse SPEC CHECKSUMS section.
    // Extracted from raw lines, not the YAML tree: digit-only hex checksums
    // (a real possibility for SHA-1 podspec checksums) are typed as YAML
    // numbers and lose their exact digits through f64, so as_str() on the
    // parsed Value silently drops or corrupts them.
    let spec_checksums = parse_spec_checksums_section(yaml);

    // Every pod must have a SPEC CHECKSUMS entry, keyed by root name — subspecs
    // like "Firebase/Auth" are covered by their root pod "Firebase".
    for pod in &pods {
        let root_name = pod.name.split('/').next().unwrap_or(&pod.name);
        anyhow::ensure!(
            spec_checksums.contains_key(root_name),
            "pod '{}' has no SPEC CHECKSUMS entry (missing checksum for '{}')",
            pod.name,
            root_name
        );
    }

    // Parse PODFILE CHECKSUM — raw-line extracted for the same digit-only
    // reason as SPEC CHECKSUMS (an all-digit SHA-1 would be YAML-typed as a
    // number and corrupted).
    let podfile_checksum = yaml
        .lines()
        .find_map(|line| line.strip_prefix("PODFILE CHECKSUM:"))
        .map(|rest| rest.trim().trim_matches('"').to_string())
        .unwrap_or_default();

    // Parse COCOAPODS version
    let cocoapods_version = root
        .get("COCOAPODS")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_default();

    // Parse EXTERNAL SOURCES (optional)
    let external_sources = parse_external_sources_section(root)?;

    // Parse CHECKOUT OPTIONS (optional)
    let checkout_options = parse_checkout_options_section(root)?;

    Ok(PodfileLock {
        pods,
        spec_checksums,
        podfile_checksum,
        cocoapods_version,
        external_sources,
        checkout_options,
    })
}

fn parse_pods_section(root: &serde_yml::Mapping) -> anyhow::Result<Vec<Pod>> {
    let pods_value = root
        .get("PODS")
        .ok_or_else(|| anyhow::anyhow!("PODS section not found in Podfile.lock"))?;

    let pods_seq = pods_value
        .as_sequence()
        .ok_or_else(|| anyhow::anyhow!("PODS must be a sequence"))?;

    let mut pods = Vec::new();

    for entry in pods_seq {
        if let Some(s) = entry.as_str() {
            // Simple entry: "Pod (version)"
            let (name, version) = parse_pod_entry(s)?;
            pods.push(Pod {
                name,
                version,
                deps: Vec::new(),
            });
        } else if let Some(mapping) = entry.as_mapping() {
            // Mapping entry: {"Pod (version)": [deps]}
            for (key, deps_value) in mapping.iter() {
                // In serde_yml, mapping keys that are scalars return &str directly
                let key_str = key.as_str();
                let (name, version) = parse_pod_entry(key_str)?;
                let deps = if let Some(seq) = deps_value.as_sequence() {
                    seq.iter()
                        .filter_map(|v| v.as_str())
                        .map(|s| s.to_string())
                        .collect()
                } else {
                    Vec::new()
                };
                pods.push(Pod {
                    name,
                    version,
                    deps,
                });
            }
        }
    }

    Ok(pods)
}

fn parse_pod_entry(s: &str) -> anyhow::Result<(String, String)> {
    let s = s.trim();
    let last_space = s
        .rfind(' ')
        .ok_or_else(|| anyhow::anyhow!("invalid pod entry format: {}", s))?;

    let name = s[..last_space].trim();
    let version_part = s[last_space + 1..].trim();

    // Remove parentheses
    let version = version_part
        .strip_prefix('(')
        .and_then(|v| v.strip_suffix(')'))
        .ok_or_else(|| anyhow::anyhow!("invalid pod version format: {}", version_part))?
        .to_string();

    Ok((name.to_string(), version))
}

/// Extract the flat `SPEC CHECKSUMS:` section from raw lines. See the call
/// site for why this bypasses the YAML tree (digit-only hex corruption).
fn parse_spec_checksums_section(yaml: &str) -> BTreeMap<String, String> {
    let mut result = BTreeMap::new();
    let mut in_section = false;

    for line in yaml.lines() {
        if line.starts_with("SPEC CHECKSUMS:") {
            in_section = true;
            continue;
        }
        if !in_section {
            continue;
        }
        if line.trim().is_empty() {
            continue;
        }
        // Section ends at the next top-level (non-indented) key.
        if !line.starts_with(' ') {
            break;
        }
        if let Some((name, hash)) = line.split_once(':') {
            let name = name.trim().trim_matches('"');
            let hash = hash.trim().trim_matches('"');
            if !name.is_empty() && !hash.is_empty() {
                result.insert(name.to_string(), hash.to_string());
            }
        }
    }

    result
}

fn parse_external_sources_section(
    root: &serde_yml::Mapping,
) -> anyhow::Result<BTreeMap<String, ExternalSource>> {
    let external_sources_map = match root.get("EXTERNAL SOURCES") {
        Some(v) => v
            .as_mapping()
            .ok_or_else(|| anyhow::anyhow!("EXTERNAL SOURCES must be a mapping"))?,
        None => return Ok(BTreeMap::new()),
    };

    let mut result = BTreeMap::new();
    for (key, value) in external_sources_map.iter() {
        // In serde_yml, keys return &str directly, values return Option<&str>
        let pod_name = key.as_str();
        if let Some(source_mapping) = value.as_mapping() {
            let mut path = None;
            let mut git = None;
            let mut podspec = None;

            for (k, v) in source_mapping.iter() {
                let ks = k.as_str();
                if let Some(vs) = v.as_str() {
                    match ks {
                        ":path" => path = Some(vs.to_string()),
                        ":git" => git = Some(vs.to_string()),
                        ":podspec" => podspec = Some(vs.to_string()),
                        _ => {}
                    }
                }
            }

            result.insert(pod_name.to_string(), ExternalSource { path, git, podspec });
        }
    }

    Ok(result)
}

fn parse_checkout_options_section(
    root: &serde_yml::Mapping,
) -> anyhow::Result<BTreeMap<String, CheckoutOptions>> {
    let checkout_map = match root.get("CHECKOUT OPTIONS") {
        Some(v) => v
            .as_mapping()
            .ok_or_else(|| anyhow::anyhow!("CHECKOUT OPTIONS must be a mapping"))?,
        None => return Ok(BTreeMap::new()),
    };

    let mut result = BTreeMap::new();
    for (key, value) in checkout_map.iter() {
        // In serde_yml, keys return &str directly, values return Option<&str>
        let pod_name = key.as_str();
        if let Some(opts_mapping) = value.as_mapping() {
            let mut git = None;
            let mut tag = None;
            let mut commit = None;
            let mut branch = None;

            for (k, v) in opts_mapping.iter() {
                let ks = k.as_str();
                if let Some(vs) = v.as_str() {
                    match ks {
                        ":git" => git = Some(vs.to_string()),
                        ":tag" => tag = Some(vs.to_string()),
                        ":commit" => commit = Some(vs.to_string()),
                        ":branch" => branch = Some(vs.to_string()),
                        _ => {}
                    }
                }
            }

            if let Some(git_url) = git {
                result.insert(
                    pod_name.to_string(),
                    CheckoutOptions {
                        git: git_url,
                        tag,
                        commit,
                        branch,
                    },
                );
            }
        }
    }

    Ok(result)
}

#[cfg(test)]
#[path = "cocoapods_tests.rs"]
mod tests;
