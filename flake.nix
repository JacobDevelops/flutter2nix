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
    # buildGradleProject and buildAndroidApp are Phase 2 stubs (passthrough attrs, _phase5Placeholder = true).
    # buildIOSApp and buildFlutterApp remain unimplemented until Phase 3/4.
    {
      lib = (import ./nix/gradle2nix-lib.nix { lib = nixpkgs.lib; }) // {
        buildIOSApp = _: throw "buildIOSApp: not implemented — see Phase 3";
        buildFlutterApp = _: throw "buildFlutterApp: not implemented — see Phase 4";
      };
    } // flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        rust = fenix.packages.${system}.stable;
        rustPlatform = pkgs.makeRustPlatform {
          cargo = rust.toolchain;
          rustc = rust.toolchain;
        };

        fnx = rustPlatform.buildRustPackage {
          pname = "fnx";
          version = "0.1.0";
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;
          cargoBuildFlags = [ "-p" "fnx" ];
          cargoTestFlags = [ "-p" "fnx" ];
        };

        # Pre-built tapi-shim JAR copied from source tree and hash-locked for reproducibility.
        # To update: cd tapi-shim && gradle build && nix hash file tapi-shim/build/libs/tapi-shim.jar
        tapi-shim-jar = pkgs.runCommand "tapi-shim-jar" {
          outputHash = "sha256-6/Qk7GA0Z1urrYC3RWPSBSit/OoB9It+xDYR2FniKMs=";
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
          # Place the JAR where include_bytes! expects it before cargo build runs.
          preBuild = ''
            mkdir -p tapi-shim/build/libs
            cp ${tapi-shim-jar} tapi-shim/build/libs/tapi-shim.jar
          '';
        };
      in
      {
        devShells.default = pkgs.mkShell {
          buildInputs = [
            rust.toolchain
            pkgs.nixpkgs-fmt
            fnx
          ];
        };

        packages = {
          inherit fnx tapi-shim-jar gradle2nix;
          ios2nix = pkgs.emptyDirectory;
          flutter2nix = pkgs.emptyDirectory;
          default = self.packages.${system}.flutter2nix;
        };

        # Checks: use buildRustPackage so Cargo.lock deps are vendored (no network in sandbox)
        checks = {
          cargo-check = rustPlatform.buildRustPackage {
            pname = "cargo-check";
            version = "0.1.0";
            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;
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
              src = ./tests/fixtures/flutter;
              lockFile = ./tests/fixtures/flutter/flutter-minimal.lock;
              pubCacheDir = pkgs.emptyDirectory;
              androidSdk = (pkgs.androidenv.composeAndroidPackages { }).androidsdk;
            };
          in assert drv ? drvPath;
             pkgs.runCommand "buildFlutterAndroidApp-eval" { } "touch $out";
          # Verifies buildFlutterAndroidApp infrastructure without running flutter build:
          # - init script is created and references the Maven repo
          # - pub cache validation shell loop finds expected packages in a stub cache
          buildFlutterAndroidApp-integration-stub =
            let
              gradle = self.lib.buildGradleProject {
                inherit pkgs;
                lockFile = ./tests/fixtures/flutter/flutter-minimal.lock;
              };
              stubPubCache = pkgs.runCommand "stub-pub-cache" { } ''
                mkdir -p $out/hosted/pub.dev/flutter-3.24.0
                mkdir -p $out/hosted/pub.dev/flutter_test-3.24.0
              '';
            in
            pkgs.runCommand "buildFlutterAndroidApp-integration-stub" { } ''
              test -f ${gradle.initScript}
              grep -q 'file://${gradle.mavenRepo}' ${gradle.initScript}
              export PUB_CACHE=${stubPubCache}
              for pkg in flutter flutter_test; do
                result=$(find "$PUB_CACHE/hosted/pub.dev" -maxdepth 1 -name "$pkg-*" -type d 2>/dev/null | head -1)
                if [ -z "$result" ]; then
                  echo "ERROR: stub pubCacheDir missing package: $pkg"
                  exit 1
                fi
              done
              touch $out
            '';
          default = pkgs.runCommand "flake-check-ok" { } "echo ok > $out";
        }
        # E2E check: runs flutter build appbundle against a minimal fixture app.
        # Linux-only (Android SDK) and activated only when the fixture lockfile exists.
        # Generate tests/fixtures/flutter/minimal-app/android/flutter2nix.lock by running
        # `gradle2nix lock --project-dir tests/fixtures/flutter/minimal-app/android` locally.
        // pkgs.lib.optionalAttrs (
          pkgs.stdenv.isLinux
          && builtins.pathExists ./tests/fixtures/flutter/minimal-app/android/flutter2nix.lock
        ) {
          buildFlutterAndroidApp-e2e =
            let
              # Build a minimal pub cache from the Flutter SDK's bundled packages.
              # The pub cache validation loop in buildFlutterAndroidApp requires
              # flutter-* and flutter_test-* dirs under hosted/pub.dev/.
              minimalPubCache = pkgs.runCommand "flutter-minimal-pub-cache" { } ''
                mkdir -p "$out/hosted/pub.dev"
                for pkg in flutter flutter_test; do
                  for base in "${pkgs.flutter}/packages/$pkg" "${pkgs.flutter}/bin/cache/pkg/$pkg"; do
                    if [ -d "$base" ]; then
                      ver=$(grep -m1 '^version:' "$base/pubspec.yaml" 2>/dev/null \
                        | sed 's/version:[[:space:]]*//' | tr -d '[:space:]"' \
                        || echo "0.0.0")
                      ln -s "$base" "$out/hosted/pub.dev/$pkg-$ver"
                      break
                    fi
                  done
                done
              '';
            in
            self.lib.buildFlutterAndroidApp {
              inherit pkgs;
              name = "flutter-android-e2e";
              src = ./tests/fixtures/flutter/minimal-app;
              lockFile = ./tests/fixtures/flutter/minimal-app/android/flutter2nix.lock;
              pubCacheDir = minimalPubCache;
              androidSdk = (pkgs.androidenv.composeAndroidPackages {
                buildToolsVersions = [ "34.0.0" ];
                platformVersions = [ "34" ];
              }).androidsdk;
            };
        };
      }
    );
}
