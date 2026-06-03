pub fn run() -> anyhow::Result<()> {
    println!("ios2nix sign: not yet implemented — see Phase 3");
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_sign_ipa_with_certificate() {
        todo!("Phase 1: stub — input: .ipa + cert from temp keychain, expect: Ok(.ipa re-signed with cert)")
    }

    #[test]
    fn test_sign_ipa_invalid_cert() {
        todo!("Phase 1: stub — input: .ipa + invalid cert bytes, expect: Err(invalid certificate)")
    }
}
