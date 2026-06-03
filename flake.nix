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
    # lib is top-level (not per-system) so consumers access flake.lib.buildAndroidApp directly
    {
      lib = {
        buildAndroidApp = _: throw "buildAndroidApp: not implemented — see Phase 1";
        buildIOSApp = _: throw "buildIOSApp: not implemented — see Phase 1";
        buildFlutterApp = _: throw "buildFlutterApp: not implemented — see Phase 1";
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

        # Phase 0 stubs — replaced with real Rust binaries in Phase 1
        packages = {
          inherit fnx;
          gradle2nix = pkgs.emptyDirectory;
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
            buildPhase = "cargo check --workspace";
            installPhase = "mkdir -p $out";
            doCheck = false;
          };
          cargo-clippy = rustPlatform.buildRustPackage {
            pname = "cargo-clippy";
            version = "0.1.0";
            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;
            buildPhase = "cargo clippy --workspace -- -D warnings";
            installPhase = "mkdir -p $out";
            doCheck = false;
          };
          default = pkgs.runCommand "flake-check-ok" { } "echo ok > $out";
        };
      }
    );
}
