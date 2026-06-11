use crate::dep::{DependencyGraph, LockedDependency};

#[derive(Debug, Clone)]
pub struct NixCocoaPodsCodegenConfig {
    pub indent_width: usize,
    pub sort_deps: bool,
}

fn needs_quoting(name: &str) -> bool {
    name.contains('/') || name.contains('.') || name.contains('-')
}

fn format_pod_name(name: &str) -> String {
    if needs_quoting(name) {
        format!("\"{}\"", name)
    } else {
        name.to_string()
    }
}

fn split_git_url(url: &str) -> anyhow::Result<(String, String)> {
    // url format: git+<url>#<rev>
    if let Some(git_part) = url.strip_prefix("git+") {
        if let Some((repo_url, rev)) = git_part.rsplit_once('#') {
            return Ok((repo_url.to_string(), rev.to_string()));
        }
    }
    anyhow::bail!("Invalid git pod URL format: {}", url)
}

fn format_inline_entry(
    dep: &LockedDependency,
    entry_indent: &str,
    value_indent: &str,
) -> anyhow::Result<String> {
    let name_str = format_pod_name(&dep.name);

    // Check if this is a git pod
    if let Some(ref dep_source) = dep.dep_source {
        if dep_source == "pod-git" {
            let (repo_url, rev) = split_git_url(&dep.url)?;
            return Ok(format!(
                "{entry_indent}{name} = fetchgit {{\n\
                 {value_indent}url = \"{repo_url}\";\n\
                 {value_indent}rev = \"{rev}\";\n\
                 {value_indent}sha256 = \"{sha256}\";\n\
                 {entry_indent}}};",
                name = name_str,
                sha256 = dep.sha256_hex()
            ));
        }
    }

    // Standard fetchurl for http pods
    Ok(format!(
        "{entry_indent}{name} = fetchurl {{\n\
         {value_indent}url = \"{url}\";\n\
         {value_indent}sha256 = \"{sha256}\";\n\
         {entry_indent}}};",
        name = name_str,
        url = dep.url,
        sha256 = dep.sha256_hex()
    ))
}

fn format_modular_entry(
    dep: &LockedDependency,
    entry_indent: &str,
    value_indent: &str,
) -> anyhow::Result<String> {
    let name_str = format_pod_name(&dep.name);

    // Git pods bypass mkPod (it is fetchurl-only) and call fetchgit directly.
    if let Some(ref dep_source) = dep.dep_source {
        if dep_source == "pod-git" {
            let (repo_url, rev) = split_git_url(&dep.url)?;
            return Ok(format!(
                "{entry_indent}{name} = fetchgit {{\n\
                 {value_indent}url = \"{repo_url}\";\n\
                 {value_indent}rev = \"{rev}\";\n\
                 {value_indent}sha256 = \"{sha256}\";\n\
                 {entry_indent}}};",
                name = name_str,
                sha256 = dep.sha256_hex(),
            ));
        }
    }

    // Standard mkPod call for http pods
    Ok(format!(
        "{entry_indent}{name} = mkPod {{\n\
         {value_indent}name = \"{orig_name}\";\n\
         {value_indent}url = \"{url}\";\n\
         {value_indent}sha256 = \"{sha256}\";\n\
         {entry_indent}}};",
        name = name_str,
        orig_name = dep.name,
        url = dep.url,
        sha256 = dep.sha256_hex(),
    ))
}

/// Generate a Nix attribute set of fetchurl calls for CocoaPods (inline format).
/// Output is deterministic (sorted by dep name when sort_deps=true).
pub fn generate_nix_set(
    graph: &DependencyGraph,
    config: &NixCocoaPodsCodegenConfig,
) -> anyhow::Result<String> {
    let mut nodes = graph.nodes.clone();
    if config.sort_deps {
        nodes.sort_by(|a, b| a.name.cmp(&b.name));
    }

    let i1 = " ".repeat(config.indent_width);
    let i2 = " ".repeat(config.indent_width * 2);

    let header = header_for(&nodes);

    let mut out = format!("{header}\n{{\n");
    for dep in &nodes {
        out.push_str(&format_inline_entry(dep, &i1, &i2)?);
        out.push('\n');
    }
    out.push_str("}\n");
    Ok(out)
}

/// Git pods need fetchgit in scope; keep the header minimal otherwise so the
/// committed fixtures (fetchurl-only) stay byte-exact.
fn header_for(nodes: &[LockedDependency]) -> &'static str {
    let has_git_pods = nodes
        .iter()
        .any(|dep| dep.dep_source.as_deref() == Some("pod-git"));
    if has_git_pods {
        "{ lib, fetchurl, fetchgit }:"
    } else {
        "{ lib, fetchurl }:"
    }
}

/// Generate a Nix overlay function wrapping the CocoaPods set with mkPod helper (modular format).
pub fn generate_nix_overlay(
    graph: &DependencyGraph,
    config: &NixCocoaPodsCodegenConfig,
) -> anyhow::Result<String> {
    let mut nodes = graph.nodes.clone();
    if config.sort_deps {
        nodes.sort_by(|a, b| a.name.cmp(&b.name));
    }

    let i1 = " ".repeat(config.indent_width);
    let i2 = " ".repeat(config.indent_width * 2);

    let header = header_for(&nodes);

    let mut out = format!("{header}\nlet\n");
    out.push_str(&format!(
        "{i1}mkPod = {{ name, url, sha256 }}: fetchurl {{ inherit url sha256; }};\n"
    ));
    out.push_str("in\n{\n");

    for dep in &nodes {
        out.push_str(&format_modular_entry(dep, &i1, &i2)?);
        out.push('\n');
    }

    out.push_str("}\n");
    Ok(out)
}

#[cfg(test)]
#[path = "cocoapods_tests.rs"]
mod tests;
