pub mod archive;
pub mod build;
pub mod export;
pub mod lock;
pub mod sign;

#[cfg(test)]
mod tests {
    #[test]
    fn test_cli_arg_parsing_lock() {
        todo!("Phase 1: stub — input: [\"ios2nix\", \"lock\"], expect: parses to Command::Lock with no extra args")
    }

    #[test]
    fn test_cli_arg_parsing_build() {
        todo!("Phase 1: stub — input: [\"ios2nix\", \"build\"], expect: parses to Command::Build with no extra args")
    }
}
