pub fn run() -> anyhow::Result<()> {
    println!("ios2nix archive: not yet implemented — see Phase 3");
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_archive_create_xcarchive() {
        todo!("Phase 1: stub — input: successful build output, expect: Ok(.xcarchive created at output path)")
    }

    #[test]
    fn test_archive_verify_structure() {
        todo!("Phase 1: stub — input: .xcarchive path, expect: contains Products/Applications/<app>.app and Info.plist")
    }
}
