# gradle2nix Standalone Usage

gradle2nix is a Gradle/Maven dependency materialiser for Nix. It extracts all transitive
dependencies from a Gradle project using the Gradle Tooling API and writes a reproducible
lockfile (`gradle2nix.lock`). This lockfile can then be used in a Nix build to fetch
and verify all dependencies without network access.

gradle2nix works with any Gradle project — Spring Boot, Android, Kotlin JVM — not just Flutter.

## Installation

### From the flutter2nix flake (recommended)

```bash
# Run without installing
nix run github:JacobDevelops/flutter2nix#gradle2nix -- --help

# Or install to your profile
nix profile install github:JacobDevelops/flutter2nix#gradle2nix
```

### In a devShell

```nix
# flake.nix
{
  inputs.flutter2nix.url = "github:JacobDevelops/flutter2nix";

  outputs = { self, flutter2nix, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system: {
      devShells.default = nixpkgs.legacyPackages.${system}.mkShell {
        packages = [ flutter2nix.packages.${system}.gradle2nix ];
      };
    });
}
```

### Build from source

```bash
git clone https://github.com/JacobDevelops/flutter2nix
cd flutter2nix
# Build the tapi-shim JAR first (required before cargo build)
cd tapi-shim && gradle build && cd ..
cargo build -p gradle2nix --release
./target/release/gradle2nix --help
```

## Usage

### Lock — generate dependency lockfile

Run inside a Gradle project directory to generate `gradle2nix.lock`:

```bash
gradle2nix lock --project-dir ./android
# Output: ./android/gradle2nix.lock
```

The lockfile records each dependency's group, artifact, version, and SHA-256 hash.
Commit this file to your repository alongside your `build.gradle` / `build.gradle.kts`.

### Check — verify lockfile is up to date

```bash
gradle2nix check --project-dir ./android
# Exit 0: lockfile is current
# Exit 1: lockfile is stale (dependencies changed)
```

Run this in CI to catch unlocked dependency changes before they reach a Nix build.

### Generate — emit Nix expressions

```bash
gradle2nix generate --project-dir ./android --out ./nix/gradle-deps.nix
```

Produces a Nix file that fetches and verifies each locked dependency using `fetchurl` with
SHA-256 SRI hashes.

### Options

```
gradle2nix [COMMAND] [OPTIONS]

Commands:
  lock      Generate gradle2nix.lock from a live Gradle project
  check     Verify gradle2nix.lock is current
  generate  Emit a Nix expression from gradle2nix.lock

Options:
  --project-dir <PATH>   Path to Gradle project root [default: .]
  --lock-file <PATH>     Path to lockfile [default: <project-dir>/gradle2nix.lock]
  --out <PATH>           Output path for generate subcommand [default: stdout]
  -v, --verbose          Enable verbose logging
  -h, --help             Print help
```

## Integration

### Nix flake — library functions

The flutter2nix flake exports two library functions for consuming gradle2nix lockfiles in
your own flakes.


```nix
# flake.nix
{
  inputs = {
    flutter2nix.url = "github:JacobDevelops/flutter2nix";
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, flutter2nix, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system: let
      pkgs = nixpkgs.legacyPackages.${system};
    in {
      # Full derivation: runs `gradle assembleRelease` offline and copies the
      # release APK/AAB to $out. pkgs must allow unfree + accept the Android SDK
      # license (android_sdk.accept_license = true).
      packages.myAndroidApp = flutter2nix.lib.buildAndroidApp {
        inherit pkgs;
        name = "my-app";
        src = ./.;
        lockFile = ./android/gradle2nix.lock;
        gradleTask = "assembleRelease";        # default
        androidSdk = (pkgs.androidenv.composeAndroidPackages {
          buildToolsVersions = [ "34.0.0" ];
          platformVersions = [ "34" ];
        }).androidsdk;
      };

      # Build-helper attrset for composing your own derivation:
      # { mavenRepo, initScript, buildInputs, baseGradleFlags }
      myGradleHelpers = flutter2nix.lib.buildGradleProject {
        inherit pkgs;
        lockFile = ./gradle2nix.lock;
      };
    });
}
```

### CI workflow

```yaml
# .github/workflows/lock.yml
- name: Verify gradle2nix lockfile
  run: nix run github:JacobDevelops/flutter2nix#gradle2nix -- check --project-dir ./android
```

## Troubleshooting

### `build.rs` panics: tapi-shim JAR not found

You are building gradle2nix outside of Nix (e.g., bare `cargo build`). The tapi-shim JAR
must be built first:

```bash
cd tapi-shim && gradle build && cd ..
cargo build -p gradle2nix
```

If you are inside a Nix build, the JAR is provided via the `preBuild` hook — ensure you
are using `nix build .#gradle2nix`, not a bare `cargo build`.

### `nix build .#gradle2nix` fails with hash mismatch

The tapi-shim JAR was rebuilt and its hash changed. Update `flake.nix`:

```bash
cd tapi-shim && gradle clean build
nix hash file tapi-shim/build/libs/tapi-shim.jar
# Copy the printed hash and update outputHash in flake.nix
nix build .#gradle2nix
```

### `gradle2nix lock` fails: Gradle Tooling API cannot connect

Ensure the project has a valid `gradlew` wrapper and that `JAVA_HOME` is set:

```bash
export JAVA_HOME=$(nix eval --raw nixpkgs#jdk)
gradle2nix lock --project-dir ./android
```

### Lockfile is stale in CI but current locally

Run `gradle2nix check` locally. If it exits 0, CI may be using a different Gradle version.
Pin the Gradle wrapper version in `gradle/wrapper/gradle-wrapper.properties`.
