# flutter2nix

A modular Nix toolchain for building Flutter apps reproducibly on Android and iOS.

## Tools

- **[gradle2nix](crates/gradle2nix/)** — Gradle/Maven dependency materialiser for Nix via Gradle Tooling API. Useful for any Gradle project (Spring Boot, Android, Kotlin JVM).
- **[ios2nix](crates/ios2nix/)** — End-to-end iOS/Xcode orchestration for Nix. Archive, export, and sign real-device IPAs reproducibly.
- **[flutter2nix](crates/flutter2nix/)** — Flutter integration layer composing gradle2nix + ios2nix. Unified cross-platform lockfile.
- **[nix-core](crates/nix-core/)** — Shared Rust library (published to crates.io) for Nix expression codegen and locked dependency models.

## Quick Start

```bash
# Clone and enter dev shell
git clone https://github.com/JacobDevelops/flutter2nix
cd flutter2nix
nix develop

# Verify compilation
cargo check

# Use the fnx dev CLI
fnx check        # nix flake check
fnx build        # nix build all packages
fnx test         # cargo test --workspace
fnx fmt          # cargo fmt --all
```

## Usage

```bash
# Generate lockfile for a Flutter Android project
gradle2nix lock --project-dir ./android

# Build in Nix sandbox
nix build .#gradle2nix
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup and [docs/](docs/) for detailed guides.
