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
fnx check        # nix flake check + e2e suite (nix build .#e2e); local-only, not CI
fnx build        # nix build all packages
fnx test         # cargo test --workspace
fnx fmt          # cargo fmt --all
fnx bench        # wall-clock benchmarks (see below)
```

### Benchmarks

`fnx bench` measures the pipeline end to end. Each target runs a **cold** pass
(fresh Gradle user home) immediately followed by a **warm** pass (same home,
build outputs wiped) — warm is the CI-with-cache scenario. All state lives in
temp directories that are deleted when the run finishes, and Gradle daemons are
disabled so nothing outlives the benchmark.

```bash
fnx bench                          # all targets
fnx bench --target lock            # gradle2nix dependency resolution
fnx bench --target gradle-build    # offline gradle assembleRelease (pure Android)
fnx bench --target flutter-build   # offline flutter build appbundle (goal: warm < 60s)
```

Each run appends a row per target to [benchmarks/BENCHMARKS.md](benchmarks/BENCHMARKS.md)
and a JSON line to `benchmarks/history.jsonl` (both committed), so numbers can be
compared across commits. Timings are machine-local; the host is recorded per run
in history.jsonl.

## gradle2nix — any Gradle project (no Flutter required)

gradle2nix locks the Maven dependency graph of a plain Gradle project — Android,
Spring Boot, Kotlin JVM — into `gradle2nix.lock`, which the Nix library functions
turn into a fully offline build.

```bash
# Install
nix profile install github:JacobDevelops/flutter2nix#gradle2nix

# Generate gradle2nix.lock in the project directory (run from the Gradle root,
# i.e. where settings.gradle[.kts] lives)
gradle2nix lock --project-dir .

# Verify the lockfile is current (use in CI)
gradle2nix check --project-dir .
```

`lock` resolves the dependency graph from POM/module metadata only — project
artifacts (JARs/AARs) are never downloaded, the same way bun resolves npm
packages from registry manifests without fetching tarballs. It also keeps a
persistent lookup cache at
`{gradle-user-home}/caches/gradle2nix/resolve-cache.json` (resolved SHA-256s,
POM texts, confirmed 404s — Maven release URLs are immutable). Repeat runs
against a retained Gradle home skip nearly all network traffic; delete the file
to force full re-resolution.

Consume the lockfile in your flake with `buildAndroidApp` (full APK/AAB derivation)
or `buildGradleProject` (composable helpers — offline Maven repo, init script,
Gradle flags):

```nix
{
  inputs.flutter2nix.url = "github:JacobDevelops/flutter2nix";

  outputs = { self, nixpkgs, flutter2nix, ... }: {
    packages.x86_64-linux.myApp =
      let pkgs = import nixpkgs {
        system = "x86_64-linux";
        config = { allowUnfree = true; android_sdk.accept_license = true; };
      };
      in flutter2nix.lib.buildAndroidApp {
        inherit pkgs;
        name = "my-app";
        src = ./.;
        lockFile = ./gradle2nix.lock;
        androidSdk = (pkgs.androidenv.composeAndroidPackages {
          buildToolsVersions = [ "34.0.0" ];
          platformVersions = [ "34" ];
        }).androidsdk;
      };
  };
}
```

See [docs/gradle2nix-standalone.md](docs/gradle2nix-standalone.md) for the full guide —
installation options, CLI reference, Nix flake integration, and troubleshooting.

## flutter2nix — Flutter apps

flutter2nix wraps gradle2nix (and ios2nix) for Flutter projects, producing a
unified cross-platform `flutter2nix.lock`. It always locks the `android/` Gradle
build (driven internally by `flutter build`); when an `ios/Podfile.lock` is
present it also locks the iOS CocoaPods dependencies into an `ios` section of the
same lockfile.

```bash
# Install
nix profile install github:JacobDevelops/flutter2nix#flutter2nix

# Generate flutter2nix.lock — run from the Flutter project root
# (where pubspec.yaml lives); the android/ Gradle build is detected and locked
flutter2nix lock --project-dir .

# Verify the lockfile is current (use in CI)
flutter2nix check
```

Consume it with `buildFlutterAndroidApp`, which runs `flutter build appbundle`
fully offline — Dart packages resolved from `pubspec.lock` (via pub2nix), Maven
artifacts from `flutter2nix.lock`:

```nix
flutter2nix.lib.buildFlutterAndroidApp {
  inherit pkgs;
  name = "my-flutter-app";
  src = ./.;                              # Flutter project root
  lockFile = ./flutter2nix.lock;
  # pubspecLockFile defaults to src + "/pubspec.lock" — must come from a real
  # `flutter pub get` run so hosted packages carry sha256 hashes.
  androidSdk = (pkgs.androidenv.composeAndroidPackages {
    buildToolsVersions = [ "34.0.0" ];
    platformVersions = [ "34" "36" ];
    includeCmake = true;
    cmakeVersions = [ "3.22.1" ];
    includeNDK = true;
    ndkVersions = [ "26.1.10909125" ];
  }).androidsdk;
}
```

For iOS, `buildFlutterIOSApp` runs `xcodebuild archive` hermetically on macOS
(CocoaPods resolved from the lockfile's `ios` section), and `buildFlutterApp`
dispatches to the Android and/or iOS builders based on host platform and the
requested `platforms`. See [docs/ios-testing.md](docs/ios-testing.md) for the
iOS build and signing guide.

### `consolidateMavenRepo` (cold-CI transfer opt-in)

By default the offline Maven repo **symlinks** each fetched artifact, so its Nix
closure is ~2900 content-addressed store paths. That dedupes across lockfile
versions and projects and lets a warm builder transfer only the paths that
actually changed on a dep bump — the right default for persistent builders.

On an **ephemeral CI runner** (cold store every run) that same shape costs ~2900
individual object GETs from the binary cache — minutes of per-object latency even
though every path is a cache hit. Set `consolidateMavenRepo = true;` to copy every
artifact into a **single** store path, so the whole repo substitutes as one NAR in
one request:

```nix
flutter2nix.lib.buildFlutterApp {
  inherit pkgs src androidSdk;
  consolidateMavenRepo = true;   # cold-runner CI; default false
}
```

Tradeoff: a single NAR shares nothing, so each lockfile version is a full (~5.6GB)
NAR in the store/cache and every dep bump re-pushes/re-pulls the whole closure.
Only flip it on when your runners are ephemeral. The flag is accepted by
`buildFlutterApp`, `buildFlutterAndroidApp`, `buildAndroidApp`, and
`buildGradleProject`.

**When you set this, push your cache with zstd** (`compression=zstd` in the
`nix copy --to` URL) — the one big NAR decompresses single-threaded on the cold
restore path, and Nix's xz default is slow to decompress. Measure the gap on your
repo with `benchmarks/measure-closure.sh --compression`.

### Fast cold-CI restore (drop-in action)

On a fresh runner, restoring the dependency closure from the cache — not the
compile — dominates the build (on one consumer: ~11 min of a ~14.5 min step). Drop
in the reusable composite action to realise the heavy closure first, with the
substitution parallelism a cold runner needs:

```yaml
- uses: <your-org>/flutter2nix/.github/actions/prefetch-nix-closure@<rev>
  with:
    installables: ".#my-app-offline-deps"   # whatever surfaces your offline deps
- run: nix build .#my-app
```

It encodes nothing project-specific — you name the installable. See
[`.github/actions/prefetch-nix-closure`](.github/actions/prefetch-nix-closure/action.yml).

See the doc comments in [nix/gradle2nix-lib.nix](nix/gradle2nix-lib.nix) for all
parameters, and the `buildFlutterAndroidApp-e2e` check in [flake.nix](flake.nix)
for a complete working example.

For CI setup and cache tuning (which layer to cache, compression, and the
`consolidateMavenRepo` cold-vs-warm tradeoff), see
[docs/ci-cache-strategy.md](docs/ci-cache-strategy.md).

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup and [docs/](docs/) for detailed guides.
