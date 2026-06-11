# flutter2nix Nix library: Flutter-specific builders composing the platform
# libs (pub-lib for Dart packages, ios2nix-lib for the pod sandbox).
{ lib }:

let
  pubLib = import ./pub-lib.nix { inherit lib; };
  iosLib = import ./ios2nix-lib.nix { inherit lib; };
in
{
  # Builds the unsigned iOS .app for a Flutter project.
  #
  # The Dart side is hermetic: a Nix-generated package_config.json replaces
  # `pub get`, and the pod sandbox is content-addressed from the lockfile's
  # ios section. The Xcode side is impure (__noChroot): /usr/bin/xcodebuild
  # dispatches through xcode-select, and the .app is not bit-reproducible.
  # Signing/.ipa export is the ios2nix archive/export pipeline's job — this
  # builder stops at an unsigned device build.
  #
  # KNOWN LIMITATION: asset catalogs and storyboards cannot compile in a Nix
  # derivation — actool/ibtool spawn XPC helpers that resolve the build user's
  # passwd home (/var/empty) and need a CoreSimulator user context. Apps that
  # ship them must build through the impure ios2nix CLI pipeline (which runs
  # as the real user); the e2e fixture is storyboard- and catalog-free.
  #
  # Parameters:
  #   pkgs            — nixpkgs attribute set
  #   name            — derivation name
  #   src             — Flutter project root (pubspec.yaml + ios/)
  #   lockFile        — flutter2nix lockfile (provides the ios pod section)
  #   pubspecLockFile — pubspec.lock (default: src + /pubspec.lock)
  #   gitHashes       — pub git dependency hashes (pub2nix)
  #   flutterSdk      — Flutter SDK (default: pkgs.flutter)
  buildFlutterIOSApp =
    { pkgs
    , name
    , src
    , lockFile
    , pubspecLockFile ? src + "/pubspec.lock"
    , gitHashes ? { }
    , flutterSdk ? pkgs.flutter
    , ...
    }:
    let
      packageConfig = pubLib.pubPackageConfig {
        inherit pkgs name src pubspecLockFile gitHashes flutterSdk;
      };
      podsSandbox = iosLib.buildPodsSandbox pkgs (iosLib.readPods lockFile);
    in
    pkgs.stdenv.mkDerivation {
      inherit name src;
      __noChroot = true;
      meta.platforms = lib.platforms.darwin;
      buildInputs = [ pkgs.cocoapods flutterSdk ];
      buildPhase = ''
        runHook preBuild
        export HOME="$NIX_BUILD_TOP"

        # Install the Nix-generated package config so `flutter build --no-pub`
        # resolves all Dart packages from the store without running pub. The
        # copied pubspec.lock keeps flutter_tools' freshness check consistent.
        mkdir -p .dart_tool
        cp ${packageConfig} .dart_tool/package_config.json
        chmod u+w .dart_tool/package_config.json
        install -m644 ${pubspecLockFile} pubspec.lock
        ${pkgs.python3.withPackages (ps: [ ps.pyyaml ])}/bin/python3 \
          ${pkgs.path}/pkgs/build-support/dart/pub2nix/package-graph.py \
          > .dart_tool/package_graph.json

        # Flutter's CocoaPods integration reads FLUTTER_ROOT from this
        # generated file; it is machine-specific and gitignored, so synthesize
        # it for the sandbox copy.
        mkdir -p ios/Flutter
        {
          printf 'FLUTTER_ROOT=%s\n' '${flutterSdk}'
          printf 'FLUTTER_APPLICATION_PATH=%s\n' "$PWD"
          printf 'COCOAPODS_PARALLEL_CODE_SIGN=true\n'
          printf 'FLUTTER_TARGET=lib/main.dart\n'
          printf 'FLUTTER_BUILD_DIR=build\n'
          printf 'FLUTTER_BUILD_NAME=1.0.0\n'
          printf 'FLUTTER_BUILD_NUMBER=1\n'
          printf 'DART_OBFUSCATION=false\n'
          printf 'TRACK_WIDGET_CREATION=true\n'
          printf 'TREE_SHAKE_ICONS=false\n'
          printf 'PACKAGE_CONFIG=.dart_tool/package_config.json\n'
        } > ios/Flutter/Generated.xcconfig

        # Make the hash-fetched pod sources visible to pod install (no-op for
        # pod-less apps; the sandbox tree is empty then).
        mkdir -p ios/Pods
        ln -s ${podsSandbox}/pods/* ios/Pods/ 2>/dev/null || true

        # All Flutter work for iOS happens inside the Xcode build phases
        # (xcode_backend.sh reads Generated.xcconfig and runs flutter
        # assemble), so drive xcodebuild directly rather than via
        # `flutter build ios`: xcodebuild resolves DerivedData through the
        # build user's passwd entry (/var/empty in the sandbox), and only the
        # -derivedDataPath flag — which flutter cannot forward — relocates it.
        #
        # xcodebuild must never see the Nix toolchain env (CC/LD/NIX_* mangle
        # the link step — spike Finding 4): run everything under env -i with
        # the system PATH plus the Flutter SDK and CocoaPods.
        # flutter assemble copies Flutter.framework out of the read-only Nix
        # store and re-signs it in place; the copy keeps mode 444, so codesign
        # fails with EACCES. Shim codesign to make its target writable first.
        mkdir -p "$NIX_BUILD_TOP/shims"
        cat > "$NIX_BUILD_TOP/shims/codesign" <<'SHIM'
        #!/bin/sh
        # codesign rewrites the binary via a temp file in its parent directory,
        # so the whole enclosing framework tree must be writable.
        for arg do target="$arg"; done
        if [ -e "$target" ]; then
          chmod -R u+w "$(dirname "$target")" 2>/dev/null || true
        fi
        exec /usr/bin/codesign "$@"
        SHIM
        chmod +x "$NIX_BUILD_TOP/shims/codesign"

        # LANG: CocoaPods (Ruby) needs a UTF-8 locale or unicode_normalize
        # dies on ASCII-8BIT paths.
        sanitized_env=(env -i
          HOME="$HOME"
          LANG=en_US.UTF-8
          LC_ALL=en_US.UTF-8
          PATH="$NIX_BUILD_TOP/shims:${flutterSdk}/bin:${pkgs.cocoapods}/bin:/usr/bin:/bin:/usr/sbin:/sbin")

        "''${sanitized_env[@]}" sh -c 'cd ios && pod install --no-repo-update'

        # HOME/PATH as explicit build settings: xcodebuild rebuilds the env for
        # script phases, dropping both. xcode_backend.sh's flutter invocation
        # needs a writable HOME (it falls back to the build user's /var/empty)
        # and resolves codesign via PATH (which must hit the shim above).
        "''${sanitized_env[@]}" xcodebuild \
          -workspace ios/Runner.xcworkspace \
          -scheme Runner \
          -configuration Release \
          -destination 'generic/platform=iOS' \
          -derivedDataPath "$NIX_BUILD_TOP/DerivedData" \
          CODE_SIGNING_ALLOWED=NO \
          HOME="$HOME" \
          PATH="$NIX_BUILD_TOP/shims:/usr/bin:/bin:/usr/sbin:/sbin" \
          build

        runHook postBuild
      '';
      installPhase = ''
        runHook preInstall
        mkdir -p $out
        cp -R "$NIX_BUILD_TOP/DerivedData/Build/Products/Release-iphoneos/"*.app $out/
        runHook postInstall
      '';
    };

  # Unified Android + iOS entry point — pending; use buildFlutterAndroidApp /
  # buildFlutterIOSApp directly.
  buildFlutterApp = _: throw "buildFlutterApp: unified entry point not implemented — use buildFlutterAndroidApp / buildFlutterIOSApp";
}
