# flutter2nix Nix library: Flutter-specific builders composing the platform
# libs (pub-lib for Dart packages, ios2nix-lib for the pod sandbox).
{ lib }:

let
  pubLib = import ./pub-lib.nix { inherit lib; };
  iosLib = import ./ios2nix-lib.nix { inherit lib; };
  androidLib = import ./gradle2nix-lib.nix { inherit lib; };

  # Builds the unsigned iOS .app for a Flutter project, or a signed .ipa when signing is provided.
  #
  # The Dart side is hermetic: a Nix-generated package_config.json replaces
  # `pub get`, and the pod sandbox is content-addressed from the lockfile's
  # ios section. The Xcode side is impure (__noChroot): /usr/bin/xcodebuild
  # dispatches through xcode-select, and the .app is not bit-reproducible.
  #
  # When signing is null: builds an unsigned device .app (CODE_SIGNING_ALLOWED=NO, build).
  # When signing is provided: builds a signed archive and exports to .ipa using
  # exportOptions. The signing attrset has the shape { teamId, identity, profileSpecifier, ios2nix? }.
  #
  # KNOWN LIMITATION: asset catalogs and storyboards cannot compile in a Nix
  # derivation — actool/ibtool spawn XPC helpers that resolve the build user's
  # passwd home (/var/empty) and need a CoreSimulator user context. Only viable
  # for storyboard/catalog-free apps; apps with asset catalogs or real CocoaPods
  # must use the ios2nix CLI pipeline (see plan 4 §2a).
  #
  # Parameters:
  #   pkgs            — nixpkgs attribute set
  #   name            — derivation name
  #   src             — Flutter project root (pubspec.yaml + ios/)
  #   lockFile        — flutter2nix lockfile (provides the ios pod section)
  #   pubspecLockFile — pubspec.lock (default: src + /pubspec.lock)
  #   gitHashes       — pub git dependency hashes (pub2nix)
  #   flutterSdk      — Flutter SDK (default: pkgs.flutter)
  #   signing         — null (unsigned) or { teamId, identity, profileSpecifier, ios2nix? }
  #   exportOptions   — path to ExportOptions.plist (required if signing != null)
  buildFlutterIOSApp =
    { pkgs
    , name
    , src
    , lockFile
    , pubspecLockFile ? src + "/pubspec.lock"
    , gitHashes ? { }
    , flutterSdk ? pkgs.flutter
    , signing ? null
    , exportOptions ? null
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
      buildInputs = [ pkgs.cocoapods flutterSdk ]
        ++ lib.optionals (signing != null) [ (signing.ios2nix or pkgs.ios2nix) ];
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

        # If signing is requested, set up the temporary keychain and partition list.
        ${lib.optionalString (signing != null) ''
          IOS2NIX_KEYCHAIN_PATH=$(ios2nix sign-setup \
            --p12 "$IOS2NIX_P12_PATH" \
            --profile "$IOS2NIX_PROFILE_PATH")
          export IOS2NIX_KEYCHAIN_PATH
          trap '[ -n "''${IOS2NIX_KEYCHAIN_PATH}" ] && security delete-keychain "''${IOS2NIX_KEYCHAIN_PATH}" 2>/dev/null || true' EXIT
        ''}

        # Build: either unsigned (build) or signed (archive).
        # Common xcodebuild args are built once to avoid duplication; each branch appends
        # signing-specific flags and the final action arg.
        xcodebuild_args=(
          -workspace "ios/Runner.xcworkspace"
          -scheme "Runner"
          -configuration "Release"
          -destination 'generic/platform=iOS'
          -derivedDataPath "$NIX_BUILD_TOP/DerivedData"
        )
        # NOTE: the sanitized PATH values below must stay in sync with SANITIZED_PATH in crates/ios2nix/src/xcode/env.rs.

        ${if signing != null then ''
          "''${sanitized_env[@]}" xcodebuild \
            "''${xcodebuild_args[@]}" \
            DEVELOPMENT_TEAM="${signing.teamId}" \
            CODE_SIGN_STYLE=Manual \
            CODE_SIGN_IDENTITY="${signing.identity}" \
            PROVISIONING_PROFILE_SPECIFIER="${signing.profileSpecifier}" \
            OTHER_CODE_SIGN_FLAGS="--keychain $IOS2NIX_KEYCHAIN_PATH" \
            HOME="$HOME" \
            PATH="$NIX_BUILD_TOP/shims:/usr/bin:/bin:/usr/sbin:/sbin" \
            archive -archivePath "$NIX_BUILD_TOP/app.xcarchive"

          # Export the archive to IPA.
          env -i HOME="$TMPDIR" PATH=/usr/bin:/bin:/usr/sbin:/sbin \
            IOS2NIX_KEYCHAIN_PATH="$IOS2NIX_KEYCHAIN_PATH" \
            xcodebuild -exportArchive \
            -archivePath "$NIX_BUILD_TOP/app.xcarchive" \
            -exportOptionsPlist "${exportOptions}" \
            -exportPath "$NIX_BUILD_TOP/export"
        '' else ''
          "''${sanitized_env[@]}" xcodebuild \
            "''${xcodebuild_args[@]}" \
            CODE_SIGNING_ALLOWED=NO \
            HOME="$HOME" \
            PATH="$NIX_BUILD_TOP/shims:/usr/bin:/bin:/usr/sbin:/sbin" \
            build
        ''}

        runHook postBuild
      '';
      installPhase = ''
        runHook preInstall
        mkdir -p $out

        ${if signing != null then ''
          # Copy IPA from export
          for ipa in "$NIX_BUILD_TOP"/export/*.ipa; do
            [ -e "$ipa" ] && cp "$ipa" $out/
          done
        '' else ''
          # Copy unsigned .app
          cp -R "$NIX_BUILD_TOP/DerivedData/Build/Products/Release-iphoneos/"*.app $out/
        ''}

        runHook postInstall
      '';
    };

  # Unified entry point for building Flutter apps for one or more platforms.
  # Dispatches to buildFlutterAndroidApp (Android) and buildFlutterIOSApp (iOS)
  # based on the platforms parameter and host platform capabilities.
  #
  # Parameters:
  #   pkgs            — nixpkgs attribute set
  #   name            — derivation name
  #   src             — Flutter project root
  #   lockFile        — flutter2nix lockfile (must have android.nodes and/or ios.nodes)
  #   platforms       — list of platforms to build (default: ["android" "ios"])
  #   androidSdk      — Android SDK (required for Android builds, default: null)
  #   gradlePackage   — Gradle for Android builds; must match the wrapper version
  #                     the lockfile was captured with (default: pkgs.gradle)
  #   signing         — null or signing config for iOS (passed to buildFlutterIOSApp)
  #   exportOptions   — path to ExportOptions.plist (passed to buildFlutterIOSApp)
  #   ...             — other parameters passed through to the platform builders
  #
  # Returns an attrset with keys for each built platform (e.g., { android = drv; ios = drv; })
  buildFlutterApp =
    { pkgs
    , name
    , src
    , lockFile
    , platforms ? [ "android" "ios" ]
    , androidSdk ? null
    , signing ? null
    , ...
    }@args:
    let
      lock = builtins.fromJSON (builtins.readFile lockFile);
      wantsAndroid = builtins.elem "android" platforms;
      wantsIos = builtins.elem "ios" platforms;

      # Throw for missing lockfile sections (before host-capability filtering).
      _sectionCheck =
        (if wantsAndroid && !(lock ? android)
         then throw "buildFlutterApp: lockfile ${toString lockFile} has no 'android' section"
         else [ ])
        ++ (if wantsIos && !(lock ? ios)
            then throw "buildFlutterApp: lockfile ${toString lockFile} has no 'ios' section"
            else [ ]);

      canBuildAndroid = pkgs.stdenv.isLinux && androidSdk != null;
      canBuildIos = pkgs.stdenv.isDarwin;

      passThrough = {
        pubspecLockFile = args.pubspecLockFile or (src + "/pubspec.lock");
        gitHashes = args.gitHashes or { };
        flutterSdk = args.flutterSdk or pkgs.flutter;
      };

      androidDrv = androidLib.buildFlutterAndroidApp (passThrough // {
        inherit pkgs name src lockFile androidSdk;
        jdk = args.jdk or pkgs.jdk17;
        gradlePackage = args.gradlePackage or pkgs.gradle;
        gradleFlags = args.gradleFlags or [];
      });

      iosDrv = buildFlutterIOSApp (passThrough // {
        inherit pkgs name src lockFile signing;
        exportOptions = args.exportOptions or null;
      });

      result = { }
        // lib.optionalAttrs (wantsAndroid && canBuildAndroid) { android = androidDrv; }
        // lib.optionalAttrs (wantsIos && canBuildIos) { ios = iosDrv; };
    in
    # seq forces _sectionCheck to be evaluated (even though its result is discarded),
    # ensuring missing lockfile sections throw at eval time rather than being lazily ignored.
    builtins.seq _sectionCheck (
      if result == { }
      then throw "buildFlutterApp: no requested platforms (${lib.concatStringsSep ", " platforms}) can be built on ${pkgs.stdenv.hostPlatform.system}"
      else result
    );

in
{
  inherit buildFlutterIOSApp buildFlutterApp;
}
