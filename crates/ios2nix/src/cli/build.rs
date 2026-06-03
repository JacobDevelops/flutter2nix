pub fn run() -> anyhow::Result<()> {
    println!("ios2nix build: not yet implemented — see Phase 3");
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_build_invoke_xcodebuild() {
        todo!("Phase 1: stub — input: fixtures/xcode-projects/simple-app with .ios2nix-xcode-output.json sidecar, expect: sidecar read instead of real xcodebuild")
    }

    #[test]
    fn test_build_capture_output() {
        todo!("Phase 1: stub — input: fixtures/xcode-outputs/basic.json, expect: Ok(XcodeBuildOutput parsed from output)")
    }
}
