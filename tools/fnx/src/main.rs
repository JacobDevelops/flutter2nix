use clap::{Parser, Subcommand};

mod cmd;
mod nixutil;

#[derive(Parser)]
#[command(
    name = "fnx",
    about = "flutter2nix developer CLI",
    long_about = "Run checks, builds, tests, and lockfile operations for the flutter2nix repo."
)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run nix flake check (or cargo clippy with optional --crate filter)
    Check(cmd::check::CheckArgs),
    /// Build a Nix package: fnx build [target] (default: nix build)
    Build(cmd::build::BuildArgs),
    /// Run cargo tests: fnx test [crate] (default: --workspace)
    Test(cmd::test::TestArgs),
    /// Generate lockfile via flutter2nix lock (Phase 0: stub)
    Lock(cmd::lock::LockArgs),
    /// Format all Rust code with cargo fmt --all
    Fmt,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    match args.command {
        Command::Check(a) => cmd::check::run(a),
        Command::Build(a) => cmd::build::run(a),
        Command::Test(a) => cmd::test::run(a),
        Command::Lock(a) => cmd::lock::run(a),
        Command::Fmt => cmd::fmt::run(),
    }
}
