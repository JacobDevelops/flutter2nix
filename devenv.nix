{
  pkgs,
  lib,
  config,
  inputs,
  ...
}:
let
  system = pkgs.stdenv.hostPlatform.system;
  rust = inputs.fenix.packages.${system}.stable;
  androidPkgs = import inputs.nixpkgs {
    inherit system;
    config = {
      allowUnfree = true;
      android_sdk.accept_license = true;
    };
  };
  androidSdk = (androidPkgs.androidenv.composeAndroidPackages {
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
in
{

  packages = with pkgs; [
    rust.toolchain
    pkg-config
    openssl
    nixpkgs-fmt
    # fnx as a cargo-run wrapper: always built from the current worktree so it
    # reflects any in-progress changes without a shell reload.
    (pkgs.writeShellScriptBin "fnx" ''exec cargo run -q -p fnx -- "$@"'')
    flutter
    jdk17
    gradle_8
    androidSdk
    jujutsu
    # ios2nix lock hashes git-source pods via nix-prefetch-git (works on Linux).
    nix-prefetch-git
  ]
  # cocoapods drives pod install for the iOS fixtures/benchmarks, but it is a
  # Darwin-only package (no Linux build) — guard it so the dev shell still
  # evaluates on Linux, where iOS work is impossible anyway.
  ++ lib.optionals pkgs.stdenv.isDarwin [ pkgs.cocoapods ];

  env = {
    ANDROID_HOME = "${androidSdk}/libexec/android-sdk";
  };

  enterShell = ''
    export LD_LIBRARY_PATH="${pkgs.openssl.out}/lib:$LD_LIBRARY_PATH"
    # Prevent Nix's OpenSSL/glibc from leaking into git/jj SSH subprocesses —
    # system ssh picks up Nix libs and dies with a glibc symbol-version mismatch.
    export GIT_SSH_COMMAND='env -u LD_LIBRARY_PATH ssh'
    # Auto-write local.properties so Gradle can find the Flutter SDK and Android SDK.
    # This file is gitignored and must point at the SDKs on the current machine.
    local_props="$DEVENV_ROOT/tests/fixtures/flutter/minimal-app/android/local.properties"
    mkdir -p "$(dirname "$local_props")"
    printf "flutter.sdk=${pkgs.flutter}\nsdk.dir=${androidSdk}/libexec/android-sdk\n" > "$local_props"
  '';
}
