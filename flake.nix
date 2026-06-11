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

  outputs = { self, nixpkgs, flake-utils, fenix }:
    # lib is top-level (not per-system) so consumers access flake.lib.buildGradleProject directly.
    {
      lib = (import ./nix/gradle2nix-lib.nix { lib = nixpkgs.lib; })
        // (import ./nix/ios2nix-lib.nix { lib = nixpkgs.lib; })
        // (import ./nix/flutter2nix-lib.nix { lib = nixpkgs.lib; });
    } // flake-utils.lib.eachDefaultSystem (system:
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
        sharedBuildInputs = [ pkgs.openssl ];

        fnx = rustPlatform.buildRustPackage {
          pname = "fnx";
          version = "0.1.0";
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;
          cargoBuildFlags = [ "-p" "fnx" ];
          cargoTestFlags = [ "-p" "fnx" ];
          nativeBuildInputs = sharedNativeBuildInputs;
          buildInputs = sharedBuildInputs;
        };

        # Pre-built tapi-shim JAR copied from source tree and hash-locked for reproducibility.
        # To update: cd tapi-shim && gradle build && nix hash file tapi-shim/build/libs/tapi-shim.jar
        tapi-shim-jar = pkgs.runCommand "tapi-shim-jar" {
          outputHash = "sha256-j1nGEED92U0QjDK+/YzqGm0gsyhxVEfIf2L4/eNYPLA=";
          outputHashMode = "flat";
        } ''
          cp ${./tapi-shim/build/libs/tapi-shim.jar} $out
        '';

        gradle2nix = rustPlatform.buildRustPackage {
          pname = "gradle2nix";
          version = "0.1.0";
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;
          cargoBuildFlags = [ "-p" "gradle2nix" ];
          cargoTestFlags = [ "-p" "gradle2nix" ];
          nativeBuildInputs = sharedNativeBuildInputs;
          buildInputs = sharedBuildInputs;
          # Place the JAR where include_bytes! expects it before cargo build runs.
          preBuild = ''
            mkdir -p tapi-shim/build/libs
            cp ${tapi-shim-jar} tapi-shim/build/libs/tapi-shim.jar
          '';
        };
        flutter2nix-cli = rustPlatform.buildRustPackage {
          pname = "flutter2nix";
          version = "0.1.0";
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;
          cargoBuildFlags = [ "-p" "flutter2nix" ];
          cargoTestFlags = [ "-p" "flutter2nix" ];
          nativeBuildInputs = sharedNativeBuildInputs;
          buildInputs = sharedBuildInputs;
          # flutter2nix links the gradle2nix lib, which embeds the TAPI shim JAR.
          preBuild = ''
            mkdir -p tapi-shim/build/libs
            cp ${tapi-shim-jar} tapi-shim/build/libs/tapi-shim.jar
          '';
        };
        ios2nix = rustPlatform.buildRustPackage {
          pname = "ios2nix";
          version = "0.1.0";
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;
          cargoBuildFlags = [ "-p" "ios2nix" ];
          # Lib tests only — the cli_tests integration suite needs real
          # xcodebuild/signing material (it is #[ignore]-gated and run by
          # fnx check); keychain tests self-skip when `security` is absent.
          cargoTestFlags = [ "-p" "ios2nix" "--lib" ];
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
        androidSdk = (pkgs.androidenv.composeAndroidPackages {
          buildToolsVersions = [ "34.0.0" ];
          platformVersions = [ "34" "36" ];
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
        e2eTests = pkgs.lib.optionalAttrs (
          pkgs.stdenv.isLinux
          && builtins.pathExists ./tests/fixtures/flutter/minimal-app/android/flutter2nix.lock
        ) {
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
          # Uses the unified lockfile (ios/flutter2nix.lock has both android+ios sections).
          buildFlutterAndroidApp-e2e = (self.lib.buildFlutterApp {
            inherit pkgs androidSdk;
            name = "flutter-android-e2e";
            src = ./tests/fixtures/flutter/minimal-app;
            lockFile = ./tests/fixtures/flutter/minimal-app/ios/flutter2nix.lock;
          }).android;
        };
        # iOS e2e: unsigned `flutter build ios` of the Flutter fixture against
        # the unified flutter2nix.lock (android + ios sections — the iOS half
        # of the composition pipeline). Signed export stays in the cargo-level
        # signing e2e (needs local material). Darwin-only; same local-only
        # tier as the android e2e.
        iosE2eTests = pkgs.lib.optionalAttrs (
          pkgs.stdenv.isDarwin
          && builtins.pathExists ./tests/fixtures/flutter/minimal-app/ios/flutter2nix.lock
        ) {
          # Build unsigned iOS app (via buildFlutterApp dispatcher).
          buildFlutterIOSApp-e2e = (self.lib.buildFlutterApp {
            inherit pkgs;
            name = "flutter-ios-e2e";
            src = ./tests/fixtures/flutter/minimal-app;
            lockFile = ./tests/fixtures/flutter/minimal-app/ios/flutter2nix.lock;
          }).ios;
        };
        # Whole-suite aggregate: `nix build .#e2e` realises every e2e entry.
        # Empty no-op derivation when the platform/fixture gates are closed.
        e2eAll = pkgs.linkFarm "e2e-all"
          (pkgs.lib.mapAttrsToList (name: path: { inherit name path; })
            (e2eTests // iosE2eTests));
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
        } // pkgs.lib.optionalAttrs pkgs.stdenv.isDarwin {
          inherit ios2nix;
        } // e2eTests // iosE2eTests;

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
              mkdir -p tapi-shim/build/libs
              cp ${tapi-shim-jar} tapi-shim/build/libs/tapi-shim.jar
            '';
            buildPhase = "cargo check --workspace";
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
              mkdir -p tapi-shim/build/libs
              cp ${tapi-shim-jar} tapi-shim/build/libs/tapi-shim.jar
            '';
            buildPhase = "cargo clippy --workspace -- -D warnings";
            installPhase = "mkdir -p $out";
            doCheck = false;
          };
          # Verifies buildGradleProject fetches 3 real artifacts and builds a
          # valid local Maven repo tree from the android-minimal fixture lockfile.
          android-maven-repo-test = (self.lib.buildGradleProject {
            pkgs = pkgs;
            lockFile = ./tests/fixtures/gradle/android-minimal.lock;
          }).mavenRepo;
          # Verifies flutter2nix-format lockfile (android.nodes wrapper) works and
          # that Flutter Storage CDN artifacts (io.flutter:*) are correctly routed.
          flutter-maven-repo-test = (self.lib.buildGradleProject {
            pkgs = pkgs;
            lockFile = ./tests/fixtures/flutter/flutter-minimal.lock;
          }).mavenRepo;
          # Type-only: verifies buildAndroidApp returns a derivation. Does not verify SDK content.
          buildAndroidApp-eval = let
            drv = self.lib.buildAndroidApp {
              inherit pkgs;
              name = "eval-test";
              src = ./tests/fixtures/gradle;
              lockFile = ./tests/fixtures/gradle/android-minimal.lock;
              androidSdk = (pkgs.androidenv.composeAndroidPackages { }).androidsdk;
            };
          in assert drv ? drvPath;
             pkgs.runCommand "buildAndroidApp-eval" { } "touch $out";
          # Type-only: verifies buildFlutterAndroidApp returns a derivation. Does not build.
          buildFlutterAndroidApp-eval = let
            drv = self.lib.buildFlutterAndroidApp {
              inherit pkgs;
              name = "flutter-android-eval-test";
              src = ./tests/fixtures/flutter/minimal-app;
              lockFile = ./tests/fixtures/flutter/flutter-minimal.lock;
              androidSdk = (pkgs.androidenv.composeAndroidPackages { }).androidsdk;
            };
          in assert drv ? drvPath;
             pkgs.runCommand "buildFlutterAndroidApp-eval" { } "touch $out";
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
          # Pre-mortem #5 (Nix half): the git+url#rev packing must round-trip
          # into exact fetchgit args. Pure eval — runs on all systems.
          ios2nix-split-git-url-eval = let
            result = self.lib.splitGitUrl
              "git+https://github.com/jdg/MBProgressHUD.git#bca42b801100b2b3a4eda0ba8dd33d858c780b0d";
          in
          assert result.url == "https://github.com/jdg/MBProgressHUD.git";
          assert result.rev == "bca42b801100b2b3a4eda0ba8dd33d858c780b0d";
          pkgs.runCommand "ios2nix-split-git-url-eval" { } "touch $out";
          # Verifies buildFlutterApp dispatcher works on both platforms.
          # On Linux: android is present (androidSdk provided, isLinux=true).
          # On Darwin: ios is present (isDarwin=true, android filtered).
          buildFlutterApp-eval = let
            result = self.lib.buildFlutterApp {
              inherit pkgs;
              name = "build-flutter-app-eval";
              src = ./tests/fixtures/flutter/minimal-app;
              lockFile = ./tests/fixtures/flutter/minimal-app/ios/flutter2nix.lock;
              androidSdk = (pkgs.androidenv.composeAndroidPackages { }).androidsdk;
            };
            drv = result.android or result.ios;
          in builtins.seq drv.drvPath
             (pkgs.runCommand "buildFlutterApp-eval" { } "touch $out");
          default = pkgs.runCommand "flake-check-ok" { } "echo ok > $out";
        # iOS checks are darwin-gated; ios-pods-sandbox-test realises a real
        # fixed-output git fetch (analogue of android-maven-repo-test).
        } // pkgs.lib.optionalAttrs pkgs.stdenv.isDarwin {
          ios-pods-sandbox-test = let
            sandbox = self.lib.buildPodsSandbox pkgs (self.lib.readPods ./tests/fixtures/ios/minimal-pods.lock);
          in
          pkgs.runCommand "ios-pods-sandbox-test" { } ''
            test -f ${sandbox}/pods/MBProgressHUD/1.2.0/MBProgressHUD.h
            touch $out
          '';
          # Forces full instantiation (drvPath), not just attribute presence —
          # `drv ? drvPath` is lazy and lets broken buildPhase interpolations
          # (e.g. a nonexistent package reference) slip through evaluation.
          buildIOSApp-eval = let
            drv = self.lib.buildIOSApp {
              inherit pkgs;
              name = "eval-test";
              src = ./crates/ios2nix/tests/fixtures/xcode-projects/native-app;
              lockFile = ./tests/fixtures/ios/minimal-pods.lock;
              exportOptions = ./crates/ios2nix/tests/fixtures/xcode-projects/native-app/ExportOptions.plist;
            };
          in builtins.seq drv.drvPath
             (pkgs.runCommand "buildIOSApp-eval" { } "touch $out");
        };
      }
    );
}
