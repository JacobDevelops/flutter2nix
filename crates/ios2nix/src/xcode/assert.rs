/// Compare two semantic versions (dot-separated numeric components).
/// Missing components are treated as 0.
/// Returns Ok if found >= minimum, Err otherwise.
pub fn assert_xcode_version(found: &str, minimum: &str) -> anyhow::Result<()> {
    let parse_version = |v: &str| -> anyhow::Result<Vec<u32>> {
        v.split('.')
            .map(|part| {
                part.parse::<u32>()
                    .map_err(|_| anyhow::anyhow!("malformed version component: {}", part))
            })
            .collect()
    };

    let found_parts = parse_version(found)?;
    let minimum_parts = parse_version(minimum)?;

    let max_len = found_parts.len().max(minimum_parts.len());
    for i in 0..max_len {
        let found_part = found_parts.get(i).copied().unwrap_or(0);
        let minimum_part = minimum_parts.get(i).copied().unwrap_or(0);

        if found_part > minimum_part {
            return Ok(());
        } else if found_part < minimum_part {
            return Err(anyhow::anyhow!(
                "Xcode version too old: found {}, minimum required {}",
                found,
                minimum
            ));
        }
    }

    Ok(())
}

/// Check if Xcode tools are installed and accessible.
#[cfg(target_os = "macos")]
pub fn assert_xcode_tools_installed() -> anyhow::Result<()> {
    let output = std::process::Command::new("xcode-select")
        .arg("-p")
        .output()
        .map_err(|e| anyhow::anyhow!("failed to run xcode-select: {}", e))?;

    if !output.status.success() {
        anyhow::bail!("xcode-select failed: Xcode tools may not be installed");
    }

    let path_str = String::from_utf8_lossy(&output.stdout);
    let path = path_str.trim();

    if path.is_empty() {
        anyhow::bail!("xcode-select returned empty path");
    }

    let path_obj = std::path::Path::new(path);
    if !path_obj.exists() {
        anyhow::bail!("Xcode path does not exist: {}", path);
    }

    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn assert_xcode_tools_installed() -> anyhow::Result<()> {
    anyhow::bail!("xcode tools check requires macOS")
}

#[cfg(test)]
#[path = "assert_tests.rs"]
mod tests;
