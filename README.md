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

## gradle2nix

Generate a reproducible lockfile for any Gradle project and consume it in Nix:

```bash
# Install
nix profile install github:JacobDevelops/flutter2nix#gradle2nix

# Generate lockfile for your Android/Gradle project
gradle2nix lock --project-dir ./android

# Verify lockfile is current (use in CI)
gradle2nix check --project-dir ./android

# Build gradle2nix itself via Nix
nix build .#gradle2nix
```

See [docs/gradle2nix-standalone.md](docs/gradle2nix-standalone.md) for the full guide —
installation options, CLI reference, Nix flake integration, and troubleshooting.

### Use in your flake

```nix
{
  inputs.flutter2nix.url = "github:JacobDevelops/flutter2nix";

  outputs = { self, flutter2nix, ... }: {
    packages.x86_64-linux.myApp = flutter2nix.lib.buildAndroidApp {
      name = "my-app";
      projectDir = ./android;
      lockFile = ./android/gradle2nix.lock;
    };
  };
}
```

> **Note:** Library functions (`buildGradleProject`, `buildAndroidApp`) are Phase 2 stubs
> that return attribute sets for integration now. Full build orchestration ships in Phase 5.

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup and [docs/](docs/) for detailed guides.
