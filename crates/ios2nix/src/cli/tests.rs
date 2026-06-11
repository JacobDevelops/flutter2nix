use clap::Parser;
use std::path::PathBuf;

use super::{Args, Command};

#[test]
fn test_cli_arg_parsing_lock() {
    let args = Args::try_parse_from(["ios2nix", "lock"]).expect("should parse lock command");
    match args.command {
        Some(Command::Lock(lock_args)) => {
            assert_eq!(lock_args.ios_dir, PathBuf::from("."));
            assert_eq!(lock_args.output, None);
            assert_eq!(lock_args.timeout_secs, 600);
        }
        _ => panic!("expected Command::Lock"),
    }
}

#[test]
fn test_cli_arg_parsing_build() {
    let args = Args::try_parse_from(["ios2nix", "build"]).expect("should parse build command");
    assert!(matches!(args.command, Some(Command::Build)));
}

#[test]
fn test_cli_arg_parsing_lock_with_output() {
    let args = Args::try_parse_from(["ios2nix", "lock", "--output", "/tmp/test.lock"])
        .expect("should parse lock command with output");
    match args.command {
        Some(Command::Lock(lock_args)) => {
            assert_eq!(lock_args.output, Some(PathBuf::from("/tmp/test.lock")));
        }
        _ => panic!("expected Command::Lock"),
    }
}

#[test]
fn test_cli_arg_parsing_check_and_generate() {
    let args = Args::try_parse_from(["ios2nix", "check", "--lockfile", "/tmp/ios2nix.lock"])
        .expect("should parse check command");
    match args.command {
        Some(Command::Check(check_args)) => {
            assert_eq!(
                check_args.lockfile,
                Some(PathBuf::from("/tmp/ios2nix.lock"))
            );
        }
        _ => panic!("expected Command::Check"),
    }

    let args = Args::try_parse_from(["ios2nix", "generate", "--format", "modular"])
        .expect("should parse generate command");
    match args.command {
        Some(Command::Generate(gen_args)) => {
            assert_eq!(gen_args.format, "modular");
        }
        _ => panic!("expected Command::Generate"),
    }

    assert!(Args::try_parse_from(["ios2nix", "generate", "--format", "bogus"]).is_err());
}
