{
  description = "flutter2nix: reproducible Nix toolchain for Flutter/Android/iOS builds";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
      fenix,
    }:
    # lib is top-level (not per-system) so consumers access flake.lib.buildGradleProject directly.
    {
      lib =
        (import ./nix/gradle2nix-lib.nix { lib = nixpkgs.lib; })
        // (import ./nix/ios2nix-lib.nix { lib = nixpkgs.lib; })
        // (import ./nix/flutter2nix-lib.nix { lib = nixpkgs.lib; });
    }
    // flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;
          config = {
            allowUnfree = true;
            android_sdk.accept_license = true;
          };
        };
        rust = fenix.packages.${system}.stable;
        rustPlatform = pkgs.makeRustPlatform {
          cargo = rust.toolchain;
          rustc = rust.toolchain;
        };

        sharedNativeBuildInputs = [ pkgs.pkg-config ];
        # Empty: reqwest uses rustls, so no openssl/libssl is linked. Don't re-add
        # openssl here — see the TLS-backend change that removed it.
        sharedBuildInputs = [ ];

        fnx = rustPlatform.buildRustPackage {
          pname = "fnx";
          version = "0.1.0";
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;
          cargoBuildFlags = [
            "-p"
            "fnx"
          ];
          cargoTestFlags = [
            "-p"
            "fnx"
          ];
          nativeBuildInputs = sharedNativeBuildInputs;
          buildInputs = sharedBuildInputs;
        };

        # Pre-built tapi-shim JAR copied from source tree and hash-locked for reproducibility.
        # To update: cd crates/gradle2nix/tapi-shim && gradle build && nix hash file crates/gradle2nix/tapi-shim/build/libs/tapi-shim.jar
        tapi-shim-jar =
          pkgs.runCommand "tapi-shim-jar"
            {
              outputHash = "sha256-YmU5pJGhoskAlyEJn/SpFksjP670fCbucdNdynPLAm4=";
              outputHashMode = "flat";
            }
            ''
              cp ${./crates/gradle2nix/tapi-shim/build/libs/tapi-shim.jar} $out
            '';

        gradle2nix = rustPlatform.buildRustPackage {
          pname = "gradle2nix";
          version = "0.1.0";
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;
          cargoBuildFlags = [
            "-p"
            "gradle2nix"
          ];
          # Lib tests only — the integration suite (cli/check-flutter-sdk/relocate-
          # plugins/strip-dev-deps) needs python3 and runs in the cargo-test check.
          cargoTestFlags = [
            "-p"
            "gradle2nix"
            "--lib"
          ];
          nativeBuildInputs = sharedNativeBuildInputs;
          buildInputs = sharedBuildInputs;
          # Place the JAR where include_bytes! expects it before cargo build runs.
          preBuild = ''
            mkdir -p crates/gradle2nix/tapi-shim/build/libs
            cp ${tapi-shim-jar} crates/gradle2nix/tapi-shim/build/libs/tapi-shim.jar
          '';
        };
        flutter2nix-cli = rustPlatform.buildRustPackage {
          pname = "flutter2nix";
          version = "0.1.0";
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;
          cargoBuildFlags = [
            "-p"
            "flutter2nix"
          ];
          # Lib tests only — cli_tests is an integration suite run in the cargo-test check.
          cargoTestFlags = [
            "-p"
            "flutter2nix"
            "--lib"
          ];
          nativeBuildInputs = sharedNativeBuildInputs;
          buildInputs = sharedBuildInputs;
          # flutter2nix links the gradle2nix lib, which embeds the TAPI shim JAR.
          preBuild = ''
            mkdir -p crates/gradle2nix/tapi-shim/build/libs
            cp ${tapi-shim-jar} crates/gradle2nix/tapi-shim/build/libs/tapi-shim.jar
          '';
        };
        ios2nix = rustPlatform.buildRustPackage {
          pname = "ios2nix";
          version = "0.1.0";
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;
          cargoBuildFlags = [
            "-p"
            "ios2nix"
          ];
          # Lib tests only — the cli_tests integration suite needs real
          # xcodebuild/signing material (it is #[ignore]-gated and run by
          # fnx check); keychain tests self-skip when `security` is absent.
          cargoTestFlags = [
            "-p"
            "ios2nix"
            "--lib"
          ];
          nativeBuildInputs = sharedNativeBuildInputs;
          buildInputs = sharedBuildInputs;
          meta.platforms = pkgs.lib.platforms.darwin;
        };
        # Init script over the committed fixture lockfile (its file:// URL pulls in
        # the offline Maven repo). Exposed for `fnx bench`, which drives offline
        # Gradle builds outside the Nix sandbox. Same derivations the e2e checks use.
        benchGradle = self.lib.buildGradleProject {
          inherit pkgs;
          lockFile = ./tests/fixtures/flutter/minimal-app/android/flutter2nix.lock;
        };
        androidSdk =
          (pkgs.androidenv.composeAndroidPackages {
            buildToolsVersions = [ "34.0.0" ];
            platformVersions = [
              "34"
              "36"
            ];
            includeCmake = true;
            cmakeVersions = [ "3.22.1" ];
            includeNDK = true;
            ndkVersions = [ "26.1.10909125" ];
          }).androidsdk;

        # All end-to-end builds in one place. Each entry runs a real gradle/flutter build
        # against the minimal fixture app — building the derivation IS running the test.
        # Add a new e2e test here and it is automatically picked up by both `packages.e2e`
        # (the whole-suite aggregate) and exposed individually under `packages.<name>`.
        # Deliberately kept OUT of flake `checks`: `nix flake check` runs in CI and these
        # realise the full Android SDK + NDK + Flutter SDK, which overflows runner disk.
        # Run them locally with `fnx check` (which builds `.#e2e`) or `nix build .#e2e`.
        # Linux-only (Android SDK) and gated on the fixture lockfile existing.
        e2eTests =
          pkgs.lib.optionalAttrs
            (
              pkgs.stdenv.isLinux
              && builtins.pathExists ./tests/fixtures/flutter/minimal-app/android/flutter2nix.lock
            )
            {
              # Pure Gradle Android build (no Flutter CLI) — isolates Gradle infra from Flutter.
              # Reuses flutter2nix.lock which already contains AGP 8.6.0 + Kotlin 2.1.0 artifacts.
              buildAndroidApp-e2e = self.lib.buildAndroidApp {
                inherit pkgs androidSdk;
                name = "gradle-android-e2e";
                src = ./tests/fixtures/gradle/android-minimal-app;
                lockFile = ./tests/fixtures/flutter/minimal-app/android/flutter2nix.lock;
                gradleTask = "assembleRelease";
              };
              # Full Flutter appbundle build (via buildFlutterApp dispatcher).
              # Uses the unified lockfile (root flutter2nix.lock has both
              # android+ios sections — the CLI's default output location).
              buildFlutterAndroidApp-e2e =
                (self.lib.buildFlutterApp {
                  inherit pkgs androidSdk;
                  name = "flutter-android-e2e";
                  src = ./tests/fixtures/flutter/minimal-app;
                  lockFile = ./tests/fixtures/flutter/minimal-app/flutter2nix.lock;
                }).android;
            };
        # iOS e2e: unsigned `flutter build ios` of the Flutter fixture against
        # the unified flutter2nix.lock (android + ios sections — the iOS half
        # of the composition pipeline). Signed export stays in the cargo-level
        # signing e2e (needs local material). Darwin-only; same local-only
        # tier as the android e2e.
        iosE2eTests =
          pkgs.lib.optionalAttrs
            (pkgs.stdenv.isDarwin && builtins.pathExists ./tests/fixtures/flutter/minimal-app/flutter2nix.lock)
            {
              # Build unsigned iOS app (via buildFlutterApp dispatcher).
              buildFlutterIOSApp-e2e =
                (self.lib.buildFlutterApp {
                  inherit pkgs;
                  name = "flutter-ios-e2e";
                  src = ./tests/fixtures/flutter/minimal-app;
                  lockFile = ./tests/fixtures/flutter/minimal-app/flutter2nix.lock;
                }).ios;
            };
        # Whole-suite aggregate: `nix build .#e2e` realises every e2e entry.
        # Empty no-op derivation when the platform/fixture gates are closed.
        e2eAll = pkgs.linkFarm "e2e-all" (
          pkgs.lib.mapAttrsToList (name: path: { inherit name path; }) (e2eTests // iosE2eTests)
        );
      in
      {
        packages = {
          inherit fnx tapi-shim-jar gradle2nix;
          flutter2nix = flutter2nix-cli;
          bench-init-script = benchGradle.initScript;
          # Whole e2e suite — `nix build .#e2e` (or `fnx check`) runs every e2e test.
          e2e = e2eAll;
          default = self.packages.${system}.flutter2nix;
          # Each e2e test is also exposed individually (e.g. `.#buildFlutterAndroidApp-e2e`).
        }
        // pkgs.lib.optionalAttrs pkgs.stdenv.isDarwin {
          inherit ios2nix;
        }
        // e2eTests
        // iosE2eTests;

        # Inject binaries into pkgs for use in derivations (especially ios2nix for signing workflows).
        pkgs =
          pkgs
          // {
            inherit gradle2nix;
            flutter2nix = flutter2nix-cli;
          }
          // pkgs.lib.optionalAttrs pkgs.stdenv.isDarwin {
            inherit ios2nix;
          };

        # Checks: use buildRustPackage so Cargo.lock deps are vendored (no network in sandbox)
        checks = {
          cargo-check = rustPlatform.buildRustPackage {
            pname = "cargo-check";
            version = "0.1.0";
            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;
            nativeBuildInputs = sharedNativeBuildInputs;
            buildInputs = sharedBuildInputs;
            preBuild = ''
              mkdir -p crates/gradle2nix/tapi-shim/build/libs
              cp ${tapi-shim-jar} crates/gradle2nix/tapi-shim/build/libs/tapi-shim.jar
            '';
            buildPhase = "cargo check --workspace --all-targets";
            installPhase = "mkdir -p $out";
            doCheck = false;
          };
          cargo-clippy = rustPlatform.buildRustPackage {
            pname = "cargo-clippy";
            version = "0.1.0";
            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;
            nativeBuildInputs = sharedNativeBuildInputs;
            buildInputs = sharedBuildInputs;
            preBuild = ''
              mkdir -p crates/gradle2nix/tapi-shim/build/libs
              cp ${tapi-shim-jar} crates/gradle2nix/tapi-shim/build/libs/tapi-shim.jar
            '';
            buildPhase = "cargo clippy --workspace --all-targets -- -D warnings";
            installPhase = "mkdir -p $out";
            doCheck = false;
          };
          cargo-test = rustPlatform.buildRustPackage {
            pname = "cargo-test";
            version = "0.1.0";
            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;
            # python3: gradle2nix's relocate-plugins/check-flutter-sdk/strip-dev-deps
            # integration tests invoke the bundled nix/*.py helpers and hard-fail
            # ("python3 must be available") without an interpreter on PATH. The
            # scripts use only the stdlib, so the bare interpreter is enough.
            nativeBuildInputs = sharedNativeBuildInputs ++ [ pkgs.python3 ];
            buildInputs = sharedBuildInputs;
            preBuild = ''
              mkdir -p crates/gradle2nix/tapi-shim/build/libs
              cp ${tapi-shim-jar} crates/gradle2nix/tapi-shim/build/libs/tapi-shim.jar
            '';
            # Custom buildPhase as the check (doCheck=false bypasses rustPlatform's
            # checkPhase), mirroring the clippy check above. reqwest uses rustls, so
            # no libssl is needed on the loader path.
            buildPhase = "cargo test --workspace";
            installPhase = "mkdir -p $out";
            doCheck = false;
          };
          # Verifies buildGradleProject fetches 3 real artifacts and builds a
          # valid local Maven repo tree from the android-minimal fixture lockfile.
          android-maven-repo-test =
            (self.lib.buildGradleProject {
              pkgs = pkgs;
              lockFile = ./tests/fixtures/gradle/android-minimal.lock;
            }).mavenRepo;
          # Disk: the combined Maven repo must reference fetched artifacts as
          # store symlinks, not copies — copying duplicates every artifact
          # (~GBs for a real app: each file exists once as a fetchurl output
          # and again inside gradle-maven-repo).
          maven-repo-zero-copy =
            let
              repo =
                (self.lib.buildGradleProject {
                  inherit pkgs;
                  lockFile = ./tests/fixtures/gradle/android-minimal.lock;
                }).mavenRepo;
            in
            pkgs.runCommand "maven-repo-zero-copy" { } ''
              # Fetched artifacts (jar/aar/module) must be symlinks into the store.
              dups=$(find ${repo} -type f \( -name '*.jar' -o -name '*.aar' -o -name '*.module' \) -print)
              if [ -n "$dups" ]; then
                echo "FAIL: duplicated regular files in maven repo (expected store symlinks):" >&2
                printf '%s\n' "$dups" >&2
                exit 1
              fi
              # Every symlink must resolve (no dangling links).
              broken=$(find ${repo} -type l ! -exec test -e {} \; -print)
              if [ -n "$broken" ]; then
                echo "FAIL: dangling symlinks in maven repo:" >&2
                printf '%s\n' "$broken" >&2
                exit 1
              fi
              touch $out
            '';
          # Opt-in counterpart to maven-repo-zero-copy: consolidateMavenRepo=true
          # must COPY artifacts in (regular files, zero symlinks) so the repo is one
          # self-contained store path — the cold-CI single-NAR property. Without
          # this, the opt-in branch had no coverage (zero-copy only exercises the
          # default symlink path).
          maven-repo-consolidated =
            let
              repo =
                (self.lib.buildGradleProject {
                  inherit pkgs;
                  lockFile = ./tests/fixtures/gradle/android-minimal.lock;
                  consolidateMavenRepo = true;
                }).mavenRepo;
            in
            pkgs.runCommand "maven-repo-consolidated" { } ''
              # No symlinks anywhere: a symlink would pull a fetchurl output into the
              # closure, defeating the single-store-path goal.
              syms=$(find ${repo} -type l -print)
              if [ -n "$syms" ]; then
                echo "FAIL: symlinks in consolidated maven repo (expected copies):" >&2
                printf '%s\n' "$syms" >&2
                exit 1
              fi
              # Artifacts must be present as real files (guard against an empty repo
              # silently passing the no-symlinks assertion).
              files=$(find ${repo} -type f \( -name '*.jar' -o -name '*.aar' -o -name '*.module' \) -print)
              if [ -z "$files" ]; then
                echo "FAIL: no copied artifacts in consolidated maven repo" >&2
                exit 1
              fi
              touch $out
            '';
          # Verifies flutter2nix-format lockfile (android.nodes wrapper) works and
          # that Flutter Storage CDN artifacts (io.flutter:*) are correctly routed.
          flutter-maven-repo-test =
            (self.lib.buildGradleProject {
              pkgs = pkgs;
              lockFile = ./tests/fixtures/flutter/flutter-minimal.lock;
            }).mavenRepo;
          # Guards every fixture lockfile against a bundletool POM-without-JAR. bundletool is a
          # jar-packaged build-classpath artifact (AGP's FinalizeBundleTask resolves it at task
          # time to sign the AAB). An offline lockfile that records a bundletool `.pom` without
          # its `.jar` breaks `signReleaseBundle` (symptom: FinalizeBundleTask$BundleToolRunnable)
          # whenever that version is selected. Such an entry is the fingerprint of a lockfile
          # generated against a Gradle cache polluted by an unrelated build — regenerate with
          # `gradle2nix lock` from a clean `--gradle-user-home`.
          lockfile-bundletool-complete =
            pkgs.runCommand "lockfile-bundletool-complete"
              {
                nativeBuildInputs = [ pkgs.jq ];
                androidLock = ./tests/fixtures/flutter/minimal-app/android/flutter2nix.lock;
                iosLock = ./tests/fixtures/flutter/minimal-app/flutter2nix.lock;
              }
              ''
                fail=0
                for lock in "$androidLock" "$iosLock"; do
                  urls=$(jq -r '(.nodes // .android.nodes // [])[].url | select(contains("/bundletool/"))' "$lock")
                  for ver in $(printf '%s\n' "$urls" | sed -nE 's#.*/bundletool/([^/]+)/.*#\1#p' | sort -u); do
                    if printf '%s\n' "$urls" | grep -q "bundletool-$ver.pom" \
                      && ! printf '%s\n' "$urls" | grep -q "bundletool-$ver.jar"; then
                      echo "FAIL: $lock records bundletool $ver POM without its JAR" >&2
                      fail=1
                    fi
                  done
                done
                if [ "$fail" -ne 0 ]; then
                  echo "regenerate the lockfile with 'gradle2nix lock' from a clean --gradle-user-home" >&2
                  exit 1
                fi
                echo "ok: every bundletool version in every fixture lockfile has a matching JAR"
                mkdir -p "$out"
              '';
          # Type-only: verifies buildAndroidApp returns a derivation. Does not verify SDK content.
          buildAndroidApp-eval =
            let
              drv = self.lib.buildAndroidApp {
                inherit pkgs;
                name = "eval-test";
                src = ./tests/fixtures/gradle;
                lockFile = ./tests/fixtures/gradle/android-minimal.lock;
                androidSdk = (pkgs.androidenv.composeAndroidPackages { }).androidsdk;
              };
            in
            assert drv ? drvPath;
            pkgs.runCommand "buildAndroidApp-eval" { } "touch $out";
          # Type-only: verifies buildFlutterAndroidApp returns a derivation. Does not build.
          buildFlutterAndroidApp-eval =
            let
              drv = self.lib.buildFlutterAndroidApp {
                inherit pkgs;
                name = "flutter-android-eval-test";
                src = ./tests/fixtures/flutter/minimal-app;
                lockFile = ./tests/fixtures/flutter/flutter-minimal.lock;
                androidSdk = (pkgs.androidenv.composeAndroidPackages { }).androidsdk;
              };
            in
            assert drv ? drvPath;
            pkgs.runCommand "buildFlutterAndroidApp-eval" { } "touch $out";
          # Public API: buildFlutterAndroidApp must surface its offline
          # intermediates via passthru so consumers can cache the network-bound
          # Maven/pub layers (they live in the build closure, not the AAB's
          # runtime closure). mavenRepo must be the *same* derivation the build
          # consumes — verified against buildGradleProject over the same lockfile.
          buildFlutterAndroidApp-exposes-offline-deps =
            let
              lockFile = ./tests/fixtures/flutter/flutter-minimal.lock;
              drv = self.lib.buildFlutterAndroidApp {
                inherit pkgs lockFile;
                name = "flutter-android-passthru-eval";
                src = ./tests/fixtures/flutter/minimal-app;
                androidSdk = (pkgs.androidenv.composeAndroidPackages { }).androidsdk;
              };
              gradle = self.lib.buildGradleProject { inherit pkgs lockFile; };
            in
            assert pkgs.lib.assertMsg (
              drv ? mavenRepo && drv ? packageConfig && drv ? offlineDeps
            ) "buildFlutterAndroidApp must passthru mavenRepo, packageConfig, and offlineDeps";
            assert pkgs.lib.assertMsg (drv.mavenRepo.outPath == gradle.mavenRepo.outPath)
              "passthru mavenRepo must be the exact offline Maven repo the build consumes (byte-identical store path)";
            pkgs.runCommand "buildFlutterAndroidApp-exposes-offline-deps" { } "touch $out";
          # Boilerplate: consumers should not have to hand-pin gradlePackage to
          # the wrapper version ("must match the Gradle wrapper version the
          # lockfile was captured with" is a footgun). The builder must read
          # android/gradle/wrapper/gradle-wrapper.properties from src and pick
          # pkgs.gradle_<major> itself; an explicit gradlePackage still wins.
          gradle-wrapper-autodetect-eval =
            let
              mkDrv =
                extra:
                self.lib.buildFlutterAndroidApp (
                  {
                    inherit pkgs;
                    name = "wrapper-autodetect-eval";
                    src = ./tests/fixtures/flutter/wrapper-gradle9;
                    lockFile = ./tests/fixtures/flutter/flutter-minimal.lock;
                    androidSdk = (pkgs.androidenv.composeAndroidPackages { }).androidsdk;
                  }
                  // extra
                );
              auto = mkDrv { };
              explicit = mkDrv { gradlePackage = pkgs.gradle_8; };
            in
            assert pkgs.lib.assertMsg (builtins.elem pkgs.gradle_9 auto.buildInputs)
              "buildFlutterAndroidApp must autodetect gradle_9 from the app's gradle-wrapper.properties (got the fallback instead)";
            assert pkgs.lib.assertMsg (builtins.elem pkgs.gradle_8 explicit.buildInputs)
              "an explicit gradlePackage must override wrapper autodetection";
            pkgs.runCommand "gradle-wrapper-autodetect-eval" { } "touch $out";
          # Boilerplate: name and lockFile must default sensibly — name from
          # pubspec.yaml's `name:` field, lockFile from src/flutter2nix.lock
          # (where the flutter2nix CLI writes it).
          flutter-app-defaults-eval =
            let
              result = self.lib.buildFlutterApp {
                inherit pkgs;
                src = ./tests/fixtures/flutter/minimal-app;
                androidSdk = (pkgs.androidenv.composeAndroidPackages { }).androidsdk;
              };
              drv = result.android or result.ios;
            in
            assert pkgs.lib.assertMsg (drv.name == "minimal_app")
              "buildFlutterApp must default the derivation name to the pubspec.yaml package name (got ${drv.name})";
            builtins.seq drv.drvPath (pkgs.runCommand "flutter-app-defaults-eval" { } "touch $out");
          # flutter_tools requires `git` on PATH at startup; the nixpkgs Flutter
          # wrapper bundles one, but raw Google-tarball SDKs do not — without
          # the builder providing it, `flutter build` dies with "Unable to find
          # git in your PATH" (jfit baseline failure, 2026-06-13).
          buildFlutterAndroidApp-provides-git =
            let
              drv = self.lib.buildFlutterAndroidApp {
                inherit pkgs;
                name = "flutter-android-git-eval";
                src = ./tests/fixtures/flutter/minimal-app;
                lockFile = ./tests/fixtures/flutter/flutter-minimal.lock;
                androidSdk = (pkgs.androidenv.composeAndroidPackages { }).androidsdk;
              };
            in
            assert pkgs.lib.assertMsg (builtins.elem pkgs.git (drv.buildInputs or [ ]))
              "buildFlutterAndroidApp must put pkgs.git in buildInputs: flutter_tools needs git on PATH and raw-tarball Flutter SDKs do not bundle it";
            pkgs.runCommand "buildFlutterAndroidApp-provides-git" { } "touch $out";
          # Verifies buildFlutterAndroidApp infrastructure without running flutter build:
          # - init script is created and references the Maven repo
          buildFlutterAndroidApp-integration-stub =
            let
              gradle = self.lib.buildGradleProject {
                inherit pkgs;
                lockFile = ./tests/fixtures/flutter/flutter-minimal.lock;
              };
            in
            pkgs.runCommand "buildFlutterAndroidApp-integration-stub" { } ''
              test -f ${gradle.initScript}
              grep -q 'file://${gradle.mavenRepo}' ${gradle.initScript}
              touch $out
            '';
          # Dev-shell offline preference: in a non-sandboxed `flutter run` the
          # project declares its own google()/mavenCentral(), so the offline
          # repo only wins if the init script injects it at the FRONT of each
          # RepositoryHandler. Without this, Gradle resolves locked deps over
          # the network (verified: 16+ POM downloads on jfit). Sandboxed builds
          # have only the one repo, so front-insertion is a harmless no-op there.
          init-script-prefers-offline-repo =
            let
              gradle = self.lib.buildGradleProject {
                inherit pkgs;
                lockFile = ./tests/fixtures/flutter/flutter-minimal.lock;
              };
            in
            pkgs.runCommand "init-script-prefers-offline-repo" { } ''
              if ! grep -q 'add(0,' ${gradle.initScript}; then
                echo "FAIL: init script does not inject the offline repo at the front" >&2
                echo "  (no 'add(0, ...)' front-insertion found — locked deps would" >&2
                echo "   resolve over the network in a dev shell with google()/mavenCentral())" >&2
                exit 1
              fi
              touch $out
            '';
          # Dev-shell wiring: offlineGradleDevHook returns a shell-hook string a
          # consumer drops into devenv enterShell / mkShell shellHook so that
          # `flutter run` resolves locked deps from the offline repo. Must set a
          # GRADLE_USER_HOME and install the init script into its init.d/.
          offline-dev-hook-eval =
            let
              hook = self.lib.offlineGradleDevHook {
                inherit pkgs;
                lockFile = ./tests/fixtures/flutter/flutter-minimal.lock;
                gradleUserHome = "$HOME/.gradle-flutter2nix-test";
              };
            in
            assert pkgs.lib.assertMsg (builtins.isString hook)
              "offlineGradleDevHook must return a shell-hook string";
            assert pkgs.lib.assertMsg (pkgs.lib.hasInfix "GRADLE_USER_HOME" hook)
              "the hook must set GRADLE_USER_HOME";
            assert pkgs.lib.assertMsg (pkgs.lib.hasInfix "init.d" hook)
              "the hook must install the offline init script into GRADLE_USER_HOME/init.d";
            pkgs.runCommand "offline-dev-hook-eval" { } "touch $out";
          # Pre-mortem #5 (Nix half): the git+url#rev packing must round-trip
          # into exact fetchgit args. Pure eval — runs on all systems.
          ios2nix-split-git-url-eval =
            let
              result = self.lib.splitGitUrl "git+https://github.com/jdg/MBProgressHUD.git#bca42b801100b2b3a4eda0ba8dd33d858c780b0d";
            in
            assert result.url == "https://github.com/jdg/MBProgressHUD.git";
            assert result.rev == "bca42b801100b2b3a4eda0ba8dd33d858c780b0d";
            pkgs.runCommand "ios2nix-split-git-url-eval" { } "touch $out";
          # The primary cold-CI closure lever (scopeEngineNodes): scoping the
          # offline repo to a build's engine mode(s) must drop exactly the
          # io.flutter <abi>_<mode> / flutter_embedding_<mode> variants the build
          # never links, keep every non-engine node, and be an identity when null
          # (the all-modes superset default). Pure eval over the committed fixture
          # lock (24 io.flutter engine nodes: 8 debug / 8 profile / 8 release).
          scope-engine-modes-eval =
            let
              lock = builtins.fromJSON (
                builtins.readFile ./tests/fixtures/flutter/minimal-app/flutter2nix.lock
              );
              all = lock.android.nodes;
              isEngine = n: pkgs.lib.hasPrefix "io.flutter:" n.name;
              isRelease = n: pkgs.lib.hasInfix "_release:" n.name;
              len = builtins.length;
              filt = builtins.filter;
              released = self.lib.scopeEngineNodes [ "release" ] all;
              nonEngineAll = filt (n: !isEngine n) all;
              releaseEngineAll = filt (n: isEngine n && isRelease n) all;
            in
            assert pkgs.lib.assertMsg (len (self.lib.scopeEngineNodes null all) == len all)
              "scopeEngineNodes null must keep every node (the all-modes superset default)";
            assert pkgs.lib.assertMsg (!(builtins.tryEval (self.lib.scopeEngineNodes [ ] all)).success)
              "scopeEngineNodes [] must throw (dropping every engine variant is a mistake, not the superset)";
            assert pkgs.lib.assertMsg (len released < len all)
              "scoping a fixture with debug/profile engine variants to [release] must drop nodes";
            assert pkgs.lib.assertMsg (len (filt (n: !isEngine n) released) == len nonEngineAll)
              "scopeEngineNodes must keep every non-engine node";
            assert pkgs.lib.assertMsg (len (filt isEngine released) == len releaseEngineAll)
              "scoping to [release] must keep exactly the release engine variants";
            assert pkgs.lib.assertMsg (builtins.all (n: !isEngine n || isRelease n) released)
              "no debug/profile io.flutter engine variant may survive [release] scoping";
            pkgs.runCommand "scope-engine-modes-eval" { } "touch $out";
          # Verifies buildFlutterApp dispatcher works on both platforms.
          # On Linux: android is present (androidSdk provided, isLinux=true).
          # On Darwin: ios is present (isDarwin=true, android filtered).
          buildFlutterApp-eval =
            let
              result = self.lib.buildFlutterApp {
                inherit pkgs;
                name = "build-flutter-app-eval";
                src = ./tests/fixtures/flutter/minimal-app;
                lockFile = ./tests/fixtures/flutter/minimal-app/flutter2nix.lock;
                androidSdk = (pkgs.androidenv.composeAndroidPackages { }).androidsdk;
              };
              drv = result.android or result.ios;
            in
            builtins.seq drv.drvPath (pkgs.runCommand "buildFlutterApp-eval" { } "touch $out");
          default = pkgs.runCommand "flake-check-ok" { } "echo ok > $out";
          # iOS checks are darwin-gated; ios-pods-sandbox-test realises a real
          # fixed-output git fetch (analogue of android-maven-repo-test).
        }
        // pkgs.lib.optionalAttrs pkgs.stdenv.isDarwin {
          ios-pods-sandbox-test =
            let
              sandbox = self.lib.buildPodsSandbox pkgs (self.lib.readPods ./tests/fixtures/ios/minimal-pods.lock);
            in
            pkgs.runCommand "ios-pods-sandbox-test" { } ''
              test -f ${sandbox}/pods/MBProgressHUD/1.2.0/MBProgressHUD.h
              touch $out
            '';
          # Forces full instantiation (drvPath), not just attribute presence —
          # `drv ? drvPath` is lazy and lets broken buildPhase interpolations
          # (e.g. a nonexistent package reference) slip through evaluation.
          buildIOSApp-eval =
            let
              drv = self.lib.buildIOSApp {
                inherit pkgs;
                name = "eval-test";
                src = ./crates/ios2nix/tests/fixtures/xcode-projects/native-app;
                lockFile = ./tests/fixtures/ios/minimal-pods.lock;
                exportOptions = ./crates/ios2nix/tests/fixtures/xcode-projects/native-app/ExportOptions.plist;
              };
            in
            builtins.seq drv.drvPath (pkgs.runCommand "buildIOSApp-eval" { } "touch $out");
        };
      }
    );
}
