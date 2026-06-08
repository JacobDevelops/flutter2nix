use crate::dep::{DependencyGraph, LockedDependency};

#[derive(Debug, Clone)]
pub struct NixMavenCodegenConfig {
    pub fetcher: String,
    pub indent_width: usize,
    pub sort_deps: bool,
}

fn extract_repo_and_artifact(dep: &LockedDependency) -> (String, String) {
    // Split "group:artifact:version[:classifier]" to get the group path
    let name_parts: Vec<&str> = dep.name.splitn(4, ':').collect();
    if name_parts.len() >= 3 {
        let group_path = name_parts[0].replace('.', "/");
        let needle = format!("/{}/", group_path);
        if let Some(pos) = dep.url.find(&needle) {
            let repo = dep.url[..=pos].to_string();
            let artifact = dep.url[pos + 1..].to_string();
            return (repo, artifact);
        }
    }
    (String::new(), dep.url.clone())
}

fn format_dep_entry(
    dep: &LockedDependency,
    fetcher: &str,
    entry_indent: &str,
    value_indent: &str,
) -> anyhow::Result<String> {
    let (repo, artifact) = extract_repo_and_artifact(dep);
    let sha256_sri = dep.sha256_as_sri()?;
    Ok(format!(
        "{entry_indent}\"{name}\" = {fetcher} {{\n\
         {value_indent}repo = \"{repo}\";\n\
         {value_indent}artifact = \"{artifact}\";\n\
         {value_indent}sha256 = \"{sha256}\";\n\
         {entry_indent}}};",
        name = dep.name,
        sha256 = sha256_sri,
    ))
}

/// Generate a Nix attribute set of fetchMaven calls.
/// Output is deterministic (sorted by dep name when sort_deps=true).
pub fn generate_nix_set(
    graph: &DependencyGraph,
    config: &NixMavenCodegenConfig,
) -> anyhow::Result<String> {
    let mut nodes = graph.nodes.clone();
    if config.sort_deps {
        nodes.sort_by(|a, b| a.name.cmp(&b.name));
    }

    let i1 = " ".repeat(config.indent_width);
    let i2 = " ".repeat(config.indent_width * 2);

    let mut out = String::from("{\n");
    for (idx, dep) in nodes.iter().enumerate() {
        out.push_str(&format_dep_entry(dep, &config.fetcher, &i1, &i2)?);
        out.push('\n');
        if idx < nodes.len() - 1 {
            out.push('\n');
        }
    }
    out.push_str("}\n");
    Ok(out)
}

/// Generate a Nix pkgs overlay function wrapping the fetchMaven set.
pub fn generate_nix_overlay(
    graph: &DependencyGraph,
    config: &NixMavenCodegenConfig,
) -> anyhow::Result<String> {
    let mut nodes = graph.nodes.clone();
    if config.sort_deps {
        nodes.sort_by(|a, b| a.name.cmp(&b.name));
    }

    let i1 = " ".repeat(config.indent_width);
    let i2 = " ".repeat(config.indent_width * 2);
    let i3 = " ".repeat(config.indent_width * 3);

    let mut out = String::from("{ pkgs, ... }:\n\n{\n");
    out.push_str(&format!("{i1}maven-deps = {{\n"));

    for (idx, dep) in nodes.iter().enumerate() {
        out.push_str(&format_dep_entry(dep, &config.fetcher, &i2, &i3)?);
        out.push('\n');
        if idx < nodes.len() - 1 {
            out.push('\n');
        }
    }

    out.push_str(&format!("{i1}}};\n}}\n"));
    Ok(out)
}

#[cfg(test)]
#[path = "maven_tests.rs"]
mod tests;
