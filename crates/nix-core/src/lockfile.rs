//! Lockfile read/write/diff operations for dependency graphs.

use crate::dep::{DependencyGraph, LockedDependency};
use std::collections::HashMap;
use std::fmt;

/// Write a dependency graph to a lockfile (JSON format)
pub fn write_lockfile(path: &std::path::Path, graph: &DependencyGraph) -> anyhow::Result<()> {
    let json = serde_json::to_string_pretty(graph)?;
    std::fs::write(path, json)?;
    Ok(())
}

/// Read a dependency graph from a lockfile (JSON format)
pub fn read_lockfile(path: &std::path::Path) -> anyhow::Result<DependencyGraph> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("failed to read lockfile '{}': {}", path.display(), e))?;
    let graph = serde_json::from_str::<DependencyGraph>(&content).map_err(|e| {
        if e.is_syntax() || e.is_eof() {
            anyhow::anyhow!("invalid JSON in lockfile '{}': {}", path.display(), e)
        } else {
            anyhow::anyhow!("failed to parse lockfile '{}': {}", path.display(), e)
        }
    })?;
    Ok(graph)
}

/// Represents changes between two lockfiles.
/// Identity key: LockedDependency::name (includes classifier when present).
/// Format: "group:artifact:version" or "group:artifact:version:classifier"
/// Two entries with same name but any field difference → counted as "modified".
#[derive(Debug, Clone, PartialEq)]
pub struct LockfileDiff {
    pub added: Vec<LockedDependency>,
    pub removed: Vec<LockedDependency>,
    pub modified: Vec<(LockedDependency, LockedDependency)>,
}

impl LockfileDiff {
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty() && self.modified.is_empty()
    }
}

impl fmt::Display for LockfileDiff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for dep in &self.added {
            writeln!(f, "+{} ({})", dep.name, dep.version)?;
        }
        for dep in &self.removed {
            writeln!(f, "-{} ({})", dep.name, dep.version)?;
        }
        for (old, new) in &self.modified {
            writeln!(
                f,
                "~{}: sha256 {} → {}",
                old.name,
                old.sha256_hex(),
                new.sha256_hex()
            )?;
        }
        Ok(())
    }
}

/// Compute the diff between two dependency graphs.
/// Returns empty diff if graphs are identical; non-empty if stale.
/// The `check` command calls this and exits non-zero on any non-empty diff.
pub fn diff_lockfiles(old: &DependencyGraph, new: &DependencyGraph) -> LockfileDiff {
    let old_map: HashMap<&str, &LockedDependency> =
        old.nodes.iter().map(|d| (d.name.as_str(), d)).collect();
    let new_map: HashMap<&str, &LockedDependency> =
        new.nodes.iter().map(|d| (d.name.as_str(), d)).collect();

    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut modified = Vec::new();

    for (name, new_dep) in &new_map {
        match old_map.get(name) {
            Some(old_dep) if old_dep != new_dep => {
                modified.push(((*old_dep).clone(), (*new_dep).clone()));
            }
            None => added.push((*new_dep).clone()),
            _ => {}
        }
    }

    for (name, old_dep) in &old_map {
        if !new_map.contains_key(name) {
            removed.push((*old_dep).clone());
        }
    }

    LockfileDiff {
        added,
        removed,
        modified,
    }
}

#[cfg(test)]
#[path = "lockfile_tests.rs"]
mod tests;
