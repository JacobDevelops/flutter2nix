pub fn run() -> anyhow::Result<()> {
    println!("ios2nix export: not yet implemented — see Phase 3");
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_export_xcarchive_to_ipa() {
        todo!("Phase 1: stub — input: .xcarchive + ExportOptions.plist, expect: Ok(.ipa created at output path)")
    }

    #[test]
    fn test_export_missing_archive() {
        todo!("Phase 1: stub — input: non-existent .xcarchive path, expect: Err(archive not found)")
    }
}
