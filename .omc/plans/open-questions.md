# Open Questions — flutter2nix Plans

## buildFlutterAndroidApp — 2026-06-08

- [ ] **Android SDK Guard Strategy** — Should we use `assert pkgs.stdenv.isLinux` or `if !pkgs.stdenv.isLinux then throw "..."`? Recommendation: throw with clear message ("buildFlutterAndroidApp only works on Linux; Android SDK is not available on Darwin").
  - *Why it matters:* Determines error UX on Darwin (eval-time fail vs. derivation construction fail).

- [ ] **Test Fixture Size for Integration Tests** — Should we use the full jfit `apps/mobile` app for integration tests, or start with a minimal stub Flutter app?
  - *Recommendation:* Minimal stub (faster CI turnaround). E2E tests with real jfit app are optional in phase 2.
  - *Why it matters:* Balances test realism with CI feedback speed.

- [ ] **Pub Cache Provisioning Method** — Build it as a separate Nix derivation (reproducible, no repo bloat), or commit a pre-built pub cache tarball to the repo?
  - *Recommendation:* Build as a derivation in `tests/nix/flutter-minimal-pub-cache.nix`, called from `flake.nix` checks.
  - *Why it matters:* Keeps the repo lightweight and makes cache updates reproducible (tied to `pubspec.lock` version).

- [ ] **Full APK/AAB Build in CI** — Should the integration test actually run `flutter build appbundle` (slow, requires full Gradle + Android compilation), or just verify that the Maven repo and pub cache are correctly wired?
  - *Recommendation:* Start with Maven + pub verification (fast, deterministic). Add optional E2E test in phase 2.
  - *Why it matters:* CI feedback speed vs. real-world confidence. Maven/pub wiring is the hard part; APK generation is routine once deps are in place.
