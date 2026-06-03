pub fn run() -> anyhow::Result<()> {
    println!("ios2nix lock: not yet implemented — see Phase 3");
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_lock_parse_podfile() {
        todo!("Phase 1: stub — input: fixtures/podfile-locks/simple-2-pods.lock, expect: Ok(2 pods extracted)")
    }

    #[test]
    fn test_lock_write_pods_nix() {
        todo!("Phase 1: stub — input: 2-pod lockfile, expect: pods.nix written matching fixtures/nix-outputs/simple-2-pods-inline.nix")
    }
}
