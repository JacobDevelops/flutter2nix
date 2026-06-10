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
    # buildIOSApp and buildFlutterApp remain unimplemented until Phase 3/4.
    {
      lib = (import ./nix/gradle2nix-lib.nix { lib = nixpkgs.lib; }) // {
        buildIOSApp = _: throw "buildIOSApp: not implemented — see Phase 3";
        buildFlutterApp = _: throw "buildFlutterApp: not implemented — see Phase 4";
      };
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
          nativeBuildInputs = sharedNativeBuildInputs;
          buildInputs = sharedBuildInputs;
          # flutter2nix links the gradle2nix lib, which embeds the TAPI shim JAR.
          preBuild = ''
            mkdir -p tapi-shim/build/libs
            cp ${tapi-shim-jar} tapi-shim/build/libs/tapi-shim.jar
          '';
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
      in
      {
        devShells.default = pkgs.mkShell {
          nativeBuildInputs = sharedNativeBuildInputs;
          buildInputs = sharedBuildInputs ++ [
            rust.toolchain
            pkgs.nixpkgs-fmt
            # fnx as a cargo-run wrapper: always built from the current worktree.
            # The nix-built fnx package went stale whenever its source changed
            # after shell entry (until the next direnv reload).
            (pkgs.writeShellScriptBin "fnx" ''exec cargo run -q -p fnx -- "$@"'')
            pkgs.flutter
            pkgs.jdk17
            pkgs.gradle_8
            androidSdk
          ];
          shellHook = ''
            export LD_LIBRARY_PATH="${pkgs.openssl.out}/lib:$LD_LIBRARY_PATH"
            # The LD_LIBRARY_PATH above leaks Nix's OpenSSL (and its glibc 2.42)
            # into every child process. When git/jj shell out to the system
            # /usr/bin/ssh for pushes, ssh picks up the Nix libs and dies with
            # `GLIBC_ABI_DT_X86_64_PLT not found`. Run ssh with a clean env so it
            # uses the system glibc.
            export GIT_SSH_COMMAND='env -u LD_LIBRARY_PATH ssh'
            export ANDROID_HOME="${androidSdk}/libexec/android-sdk"
            # Auto-write local.properties so Gradle can find the Flutter SDK and Android SDK.
            # This file is gitignored and must point at the SDKs on the current machine.
            local_props="tests/fixtures/flutter/minimal-app/android/local.properties"
            mkdir -p "$(dirname "$local_props")"
            printf "flutter.sdk=${pkgs.flutter}\nsdk.dir=${androidSdk}/libexec/android-sdk\n" > "$local_props"
          '';
        };

        packages = {
          inherit fnx tapi-shim-jar gradle2nix;
          flutter2nix = flutter2nix-cli;
          bench-init-script = benchGradle.initScript;
          # iOS orchestration is macOS-only and unreleased (Phase 3).
          ios2nix = pkgs.emptyDirectory;
          default = self.packages.${system}.flutter2nix;
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
              # Init script over the committed fixture lockfile (its file:// URL pulls in
        # the offline Maven repo). Exposed for `fnx bench`, which drives offline
        # Gradle builds outside the Nix sandbox. Same derivations the e2e checks use.
        benchGradle = self.lib.buildGradleProject {
          inherit pkgs;
          lockFile = ./tests/fixtures/flutter/minimal-app/android/flutter2nix.lock;
        };
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
              # Init script over the committed fixture lockfile (its file:// URL pulls in
        # the offline Maven repo). Exposed for `fnx bench`, which drives offline
        # Gradle builds outside the Nix sandbox. Same derivations the e2e checks use.
        benchGradle = self.lib.buildGradleProject {
          inherit pkgs;
          lockFile = ./tests/fixtures/flutter/minimal-app/android/flutter2nix.lock;
        };
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
          default = pkgs.runCommand "flake-check-ok" { } "echo ok > $out";
        }
        # E2E check: runs flutter build appbundle against a minimal fixture app.
        # Linux-only (Android SDK) and activated only when the fixture lockfile exists.
        # To generate the lockfile (local.properties is written automatically by nix develop):
        #   cargo run -p gradle2nix -- lock \
        #     --project-dir tests/fixtures/flutter/minimal-app/android \
        #     --output tests/fixtures/flutter/minimal-app/android/flutter2nix.lock
        // pkgs.lib.optionalAttrs (
          pkgs.stdenv.isLinux
          && builtins.pathExists ./tests/fixtures/flutter/minimal-app/android/flutter2nix.lock
        ) {
          # Pure Gradle Android build (no Flutter CLI) — isolates Gradle infrastructure from Flutter.
          # Reuses the flutter2nix.lock which already contains AGP 8.6.0 + Kotlin 2.1.0 artifacts.
          buildAndroidApp-e2e = self.lib.buildAndroidApp {
            inherit pkgs;
            name = "gradle-android-e2e";
            src = ./tests/fixtures/gradle/android-minimal-app;
            lockFile = ./tests/fixtures/flutter/minimal-app/android/flutter2nix.lock;
            gradleTask = "assembleRelease";
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
          };
          buildFlutterAndroidApp-e2e = self.lib.buildFlutterAndroidApp {
            inherit pkgs;
            name = "flutter-android-e2e";
            src = ./tests/fixtures/flutter/minimal-app;
            lockFile = ./tests/fixtures/flutter/minimal-app/android/flutter2nix.lock;
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
          };
        };
      }
    );
}
