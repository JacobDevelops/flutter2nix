use nix_core::dep::{DependencyGraph, LockedDependency};
use serde::{Deserialize, Serialize};

pub fn write_lockfile(_path: &std::path::Path, _graph: &DependencyGraph) -> anyhow::Result<()> {
    todo!("Phase 1: API contract only")
}

pub fn read_lockfile(_path: &std::path::Path) -> anyhow::Result<DependencyGraph> {
    todo!("Phase 1: API contract only")
}

/// Identity key: LockedDependency::name (includes classifier when present).
/// Format: "group:artifact:version" or "group:artifact:version:classifier"
/// Two entries with same name but any field difference → counted as "modified".
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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

/// Returns empty diff if graphs are identical; non-empty if stale.
/// The `check` command calls this and exits non-zero on any non-empty diff.
pub fn diff_lockfiles(_old: &DependencyGraph, _new: &DependencyGraph) -> LockfileDiff {
    todo!("Phase 1: API contract only")
}

#[cfg(test)]
#[path = "lockfile_tests.rs"]
mod tests;
