use clap::Args;

#[derive(Args)]
pub struct LockArgs {
    /// Target platform: android, ios, or all
    #[arg(long, default_value = "android")]
    pub target: String,
}

pub fn run(args: LockArgs) -> anyhow::Result<()> {
    println!(
        "fnx lock --target {}: not yet implemented — use flutter2nix lock directly in Phase 2",
        args.target
    );
    Ok(())
}
