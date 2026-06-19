# Changelog

All notable changes to flutter2nix are recorded here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

This project is **not published** to any registry and has no versioned releases,
so there is a single rolling `Unreleased` section rather than version tags. Group
entries by area (gradle2nix / ios2nix / flutter2nix / nix-lib / dev-shell / CI)
and kind (Added / Changed / Fixed / Performance).

## [Unreleased]

### Added
- **nix-lib:** `consolidateMavenRepo` opt-in — copies every fetched artifact into
  a single store path so the whole offline Maven repo substitutes as one NAR in
  one request, for ephemeral cold-store CI runners. Default `false` (symlink
  shape stays optimal for persistent builders). Accepted by `buildFlutterApp`,
  `buildFlutterAndroidApp`, `buildAndroidApp`, and `buildGradleProject`.
- **ios2nix:** Hermetic Flutter iOS archive support — `xcodebuild archive` driven
  reproducibly under Nix, including hermetic `pod install` inputs and store-SDK
  git trust.
- **nix-lib:** `signAndroidAab` — out-of-build helper to sign an AAB with a
  keystore outside the hermetic build derivation.
- **nix-lib:** Offline build intermediates exposed via `passthru` for inspection
  and downstream reuse.
- **dev-shell:** `offlineGradleDevHook` — generic dev-shell hook wiring the
  offline Maven repo into a project's Gradle so `make run-android`-style local
  flows work without network access.
- **flutter2nix:** `buildFlutterApp` dispatcher plus Flutter signing support
  (per-target pbxproj signing, keychain/extra-path shim injection).
- **nix-lib:** `flutterBuildArgs` for flavored Android builds.
- **gradle2nix:** Persistent resolve cache at
  `{gradle-user-home}/caches/gradle2nix/resolve-cache.json` (resolved SHA-256s,
  POM texts, confirmed 404s) so warm runs skip nearly all network traffic.

### Changed
- **ios2nix:** Build name/number derived from `pubspec.yaml` rather than passed
  separately, keeping the archive in sync with the Flutter project.
- **gradle2nix:** Lockfile URL assignment is verification-driven — each candidate
  URL is confirmed before being written, improving lock determinism.
- **gradle2nix:** Build-time Gradle is matched to the lock-time wrapper, and
  Gradle-9 composite-build dependencies are captured.
- **nix-lib:** `name`, `lockFile`, and `gradlePackage` now default from the app
  itself, removing required consumer boilerplate.
- **gradle2nix-init:** Offline Maven repo is injected at the front of each
  `RepositoryHandler` so it always wins over network repositories.
- **flutter2nix:** Reading a missing lockfile (e.g. via `flutter2nix check`) now
  reports `lockfile '…' not found — run \`flutter2nix lock\` to generate it`
  instead of a raw `No such file or directory` IO error.
- **nix-core (gradle2nix / ios2nix):** The shared lockfile reader now reports a
  missing lockfile as `lockfile '…' not found — run the \`lock\` command to
  generate it` instead of a raw IO error, so `gradle2nix check`/`generate` and
  `ios2nix check`/`generate` all give an actionable hint.
- **flutter2nix:** `flutter2nix lock` now warns when an `ios/` directory exists
  but has no `Podfile.lock`, explaining that iOS was skipped and how to fix it
  (`pod install` / `flutter build ios --config-only`) — previously the iOS
  section was silently omitted and only surfaced later as a confusing
  "lockfile has no 'ios' section" at build time.

### Fixed
- **dev-shell:** CocoaPods guarded behind `isDarwin` so the dev shell evaluates
  on Linux.
- **nix-lib:** Android plugin packages relocated to writable copies so Gradle 9
  accepts them (read-only store paths were rejected as plugin `projectDir`s).
- **nix-lib:** `git` provided to `buildFlutterAndroidApp` for raw-tarball Flutter
  SDKs.
- **nix-lib:** Fail fast with a clear message when a Flutter SDK lacks
  pre-resolved `flutter_tools` dependencies.
- **nix-lib:** Dev-dep plugins stripped from `.flutter-plugins-dependencies` to
  prevent pub.dev network access inside the sandbox;
  `.flutter-plugins-dependencies` is generated hermetically.
- **gradle2nix:** Heredoc-in-Nix-string syntax error and Flutter version lookup
  fixed on Linux.
- **gradle2nix:** Determinism and robustness fixes surfaced by first real-app
  validation (jfit).

### Performance
- **gradle2nix:** Warm `lock` reduced from ~225s to ~11s — pooled HTTP client,
  parallel discovery, and the persistent resolve cache; URL-verification results
  are now persisted to the resolve cache (a further warm-lock reduction,
  ~23s → ~14s on jfit).
- **nix-lib:** Offline Maven repo symlinks artifacts into `gradle-maven-repo`
  instead of copying them, cutting on-disk size from ~2.2GB to ~19MB and letting
  warm builders transfer only changed paths on a dep bump.
- **nix-lib:** Upload-artifact step optimized; `mapping.txt` (R8/ProGuard) is
  emitted for release builds.

### Documentation
- **README:** Corrected the stale claim that iOS support was "planned but not
  yet implemented" — `flutter2nix lock` already locks an `ios` CocoaPods section
  when `ios/Podfile.lock` is present, and `buildFlutterIOSApp` / the
  `buildFlutterApp` dispatcher build iOS archives on macOS. Documented the iOS
  consumption path and linked `docs/ios-testing.md`.
- **docs:** Added `docs/ci-cache-strategy.md` — why the Nix binary cache is the
  right caching layer (vs. custom `actions/cache` archives of build inputs),
  cache classes by stability tier, xz-vs-zstd compression guidance, the
  `consolidateMavenRepo` cold-vs-warm decision table, and reproducible commands
  for measuring closure size / path count / substitution time. Linked from the
  README.

### CI
- **ci:** Replaced the hand-rolled `actions/cache` cargo cache with
  `Swatinem/rust-cache@v2`, which keys on the toolchain + `Cargo.lock`, restores
  with prefix fallback (partial reuse when the lockfile changes), and prunes and
  re-saves `target/` on every successful run. The previous cache keyed `target/`
  solely on the `Cargo.lock` hash with no `restore-keys`, so a lockfile change
  meant a fully cold build and an unchanged lockfile never refreshed `target/`.
- **ci:** Added a `cargo fmt --all -- --check` gate to the required structural
  job (and pinned the `rustfmt`/`clippy` toolchain components) so formatting
  drift fails CI instead of accumulating. Existing drift across the three crates
  was normalized in the same pass.
- **bench:** Added `benchmarks/measure-closure.sh` — a read-only, CI-runnable
  check reporting the Nix closure size and store-path count of a build result,
  to make the `consolidateMavenRepo` symlink-vs-consolidated tradeoff
  data-driven. Referenced from `docs/ci-cache-strategy.md`.

[Unreleased]: https://github.com/JacobDevelops/flutter2nix/commits/main
