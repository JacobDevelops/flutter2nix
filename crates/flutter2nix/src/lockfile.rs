use anyhow::Context;
use nix_core::dep::LockedDependency;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct FlutterLockfile {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub android: Option<AndroidSection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ios: Option<IosSection>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct AndroidSection {
    pub nodes: Vec<LockedDependency>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct IosSection {
    pub nodes: Vec<LockedDependency>,
}

pub fn write_lockfile(path: &Path, lock: &FlutterLockfile) -> anyhow::Result<()> {
    let json = serde_json::to_string_pretty(lock).context("serializing flutter2nix lockfile")?;
    std::fs::write(path, json)
        .with_context(|| format!("writing flutter2nix lockfile to '{}'", path.display()))
}

pub fn read_lockfile(path: &Path) -> anyhow::Result<FlutterLockfile> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read lockfile '{}'", path.display()))?;
    serde_json::from_str(&content)
        .with_context(|| format!("failed to parse lockfile '{}'", path.display()))
}

#[cfg(test)]
#[path = "lockfile_tests.rs"]
mod tests;
