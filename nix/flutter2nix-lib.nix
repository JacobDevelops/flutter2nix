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
  # Asset catalogs (app-icon sets) DO compile in a Nix derivation here: actool
  # renders per-device icon variants via CoreSimulatorService, which resolves
  # its device-set path from the build user's passwd home (/var/empty, read-only)
  # — neither HOME nor CORE_SIMULATOR_DEVICE_SET_PATH overrides it, but
  # CoreFoundation's CFFIXED_USER_HOME does (set in the build phase below).
  # Storyboards (ibtool) are not exercised by Flutter apps and remain untested.
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
  #   dartDefines     — list of "KEY=VALUE" --dart-define strings. xcodebuild is
  #                     driven directly (not `flutter build ios`), so these are
  #                     encoded into Generated.xcconfig's DART_DEFINES exactly as
  #                     `flutter build` would; xcode_backend.sh forwards them to
  #                     `flutter assemble`. Release builds reading
  #                     String.fromEnvironment(...) get empty values otherwise.
  #   produceArchive  — when signing is null, emit an unsigned .xcarchive
  #                     ($out/Runner.xcarchive) instead of a bare .app. This is
  #                     the input to an out-of-build signer (sign + export to .ipa
  #                     in a normal process, so signing secrets never enter the
  #                     store) — the iOS analog of an unsigned Android .aab.
  #   scheme          — Xcode scheme to build (default "Runner"). Flutter flavors
  #                     create a scheme per flavor (e.g. "stag"); pass it here.
  #   configuration   — Xcode build configuration (default "Release"). Flavored
  #                     apps use "Release-<flavor>" (e.g. "Release-stag"); this
  #                     drives flavor-keyed build phases (e.g. the per-flavor
  #                     GoogleService-Info copy script).
  buildFlutterIOSApp =
    {
      pkgs,
      src,
      name ? pubLib.pubspecName src,
      lockFile ? src + "/flutter2nix.lock",
      pubspecLockFile ? src + "/pubspec.lock",
      gitHashes ? { },
      flutterSdk ? pkgs.flutter,
      signing ? null,
      exportOptions ? null,
      dartDefines ? [ ],
      produceArchive ? false,
      scheme ? "Runner",
      configuration ? "Release",
      ...
    }:
    let
      packageConfig = pubLib.pubPackageConfig {
        inherit
          pkgs
          name
          src
          pubspecLockFile
          gitHashes
          flutterSdk
          ;
      };
      podsSandbox = iosLib.buildPodsSandbox pkgs (iosLib.readPods lockFile);
    in
    pkgs.stdenv.mkDerivation {
      inherit name src;
      __noChroot = true;
      meta.platforms = lib.platforms.darwin;
      # Surface the network-bound offline layers (the CocoaPods sandbox and the
      # pub package set) so consumers can cache them independently of the app.
      # They are *build* inputs, absent from the .app/.ipa runtime closure, so a
      # plain `nix copy <app>` never carries them and a downstream rebuild
      # re-fetches them. These are the exact derivations the build consumes.
      passthru = {
        inherit packageConfig podsSandbox;
        offlineDeps = pkgs.linkFarm "${name}-offline-deps" [
          {
            name = "pods-sandbox";
            path = podsSandbox;
          }
          {
            name = "package-config";
            path = packageConfig;
          }
        ];
      };
      buildInputs = [
        pkgs.cocoapods
        flutterSdk
      ]
      ++ lib.optionals (signing != null) [ (signing.ios2nix or pkgs.ios2nix) ];
      buildPhase = ''
        runHook preBuild
        export HOME="$NIX_BUILD_TOP"

        # Flutter SDKs packaged from the official tarball keep their .git, and
        # git refuses repos owned by another user ("dubious ownership") — which
        # is every nix store path inside the build. Trust everything within this
        # throwaway HOME; xcodebuild phase scripts inherit it via HOME=$HOME.
        printf '[safe]\n\tdirectory = *\n' > "$HOME/.gitconfig"

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

        # Derive the build name/number from pubspec.yaml exactly as `flutter
        # build` does (`version: <name>+<number>`). The iOS path drives xcodebuild
        # directly rather than `flutter build`, so without this
        # CFBundleShortVersionString/CFBundleVersion fall back to a hardcoded
        # placeholder regardless of pubspec — shipping the wrong version to
        # TestFlight. A bare name with no '+<number>' uses flutter's default of 1.
        flutter_version=$(grep -E '^version:' pubspec.yaml | head -1 \
          | sed 's/^version:[[:space:]]*//; s/[[:space:]]*$//')
        flutter_build_name="''${flutter_version%%+*}"
        flutter_build_number="''${flutter_version##*+}"
        if [ "$flutter_build_number" = "$flutter_version" ] || [ -z "$flutter_build_number" ]; then
          flutter_build_number=1
        fi
        [ -n "$flutter_build_name" ] || flutter_build_name=1.0.0

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
          printf 'FLUTTER_BUILD_NAME=%s\n' "$flutter_build_name"
          printf 'FLUTTER_BUILD_NUMBER=%s\n' "$flutter_build_number"
          printf 'DART_OBFUSCATION=false\n'
          printf 'TRACK_WIDGET_CREATION=true\n'
          printf 'TREE_SHAKE_ICONS=false\n'
          printf 'PACKAGE_CONFIG=.dart_tool/package_config.json\n'
        } > ios/Flutter/Generated.xcconfig

        # Inject --dart-define values the way `flutter build` does: DART_DEFINES
        # is a comma-separated list of base64(KEY=VALUE) that xcode_backend.sh
        # reads from Generated.xcconfig and forwards to `flutter assemble`.
        # Without it, release builds reading String.fromEnvironment(...) get
        # empty values (e.g. API_BASE_URL → blank screen on first build).
        dart_defines=(${lib.concatStringsSep " " (map lib.escapeShellArg dartDefines)})
        if [ "''${#dart_defines[@]}" -gt 0 ]; then
          encoded=""
          for d in "''${dart_defines[@]}"; do
            enc=$(printf '%s' "$d" | base64 | tr -d '\n')
            encoded="''${encoded:+$encoded,}$enc"
          done
          printf 'DART_DEFINES=%s\n' "$encoded" >> ios/Flutter/Generated.xcconfig
        fi

        # Hermetically generate .flutter-plugins-dependencies: flutter_tools
        # writes it during pub get with developer-machine paths and it is
        # gitignored, so a clean checkout ships none — but CocoaPods' podhelper
        # recreates ios/.symlinks/plugins/* from the plugin paths it records.
        # Synthesized from package_config.json (Nix store roots) + each
        # package's pubspec + pubspec.lock; validated byte-equivalent in
        # structure to flutter's own output on a real app.
        rm -f .flutter-plugins-dependencies
        ${pkgs.python3.withPackages (ps: [ ps.pyyaml ])}/bin/python3 \
          ${./generate-flutter-plugins.py} "$(cat ${flutterSdk}/version)"

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

        # CFFIXED_USER_HOME: unblocks CompileAssetCatalog for apps with an
        # app-icon set. actool renders per-device icon variants via
        # CoreSimulatorService, which resolves its SimDeviceSet path from the
        # build user's *passwd* home (/var/empty for nix build users, read-only)
        # — HOME and CORE_SIMULATOR_DEVICE_SET_PATH do NOT override it, but
        # CoreFoundation's CFFIXED_USER_HOME does. Point it at a writable tree
        # with the IB Support dir pre-created; actool then compiles Assets.car
        # (a residual CoreSimulator/Devices ENOMEM line in the log is non-fatal).
        mkdir -p "$NIX_BUILD_TOP/cfhome/Library/Developer/Xcode/UserData/IB Support/Simulator Devices"

        # LANG: CocoaPods (Ruby) needs a UTF-8 locale or unicode_normalize
        # dies on ASCII-8BIT paths.
        sanitized_env=(env -i
          HOME="$HOME"
          CFFIXED_USER_HOME="$NIX_BUILD_TOP/cfhome"
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
        # Asset catalogs with app-icon sets compile in nix-build thanks to the
        # CFFIXED_USER_HOME shim set in sanitized_env above (see that comment);
        # it is also passed as a build setting below so XCBBuildService's
        # CompileAssetCatalog task inherits it.
        xcodebuild_args=(
          -workspace "ios/Runner.xcworkspace"
          -scheme "${scheme}"
          -configuration "${configuration}"
          -destination 'generic/platform=iOS'
          -derivedDataPath "$NIX_BUILD_TOP/DerivedData"
        )
        # NOTE: the sanitized PATH values below must stay in sync with SANITIZED_PATH in crates/ios2nix/src/xcode/env.rs.

        ${
          if signing != null then
            ''
              "''${sanitized_env[@]}" xcodebuild \
                "''${xcodebuild_args[@]}" \
                DEVELOPMENT_TEAM="${signing.teamId}" \
                CODE_SIGN_STYLE=Manual \
                CODE_SIGN_IDENTITY="${signing.identity}" \
                PROVISIONING_PROFILE_SPECIFIER="${signing.profileSpecifier}" \
                OTHER_CODE_SIGN_FLAGS="--keychain $IOS2NIX_KEYCHAIN_PATH" \
                HOME="$HOME" \
                CFFIXED_USER_HOME="$NIX_BUILD_TOP/cfhome" \
                PATH="$NIX_BUILD_TOP/shims:/usr/bin:/bin:/usr/sbin:/sbin" \
                archive -archivePath "$NIX_BUILD_TOP/app.xcarchive"

              # Export the archive to IPA.
              env -i HOME="$TMPDIR" PATH=/usr/bin:/bin:/usr/sbin:/sbin \
                IOS2NIX_KEYCHAIN_PATH="$IOS2NIX_KEYCHAIN_PATH" \
                xcodebuild -exportArchive \
                -archivePath "$NIX_BUILD_TOP/app.xcarchive" \
                -exportOptionsPlist "${exportOptions}" \
                -exportPath "$NIX_BUILD_TOP/export"
            ''
          else if produceArchive then
            ''
              # Unsigned archive: the input to an out-of-build signer. Code
              # signing is disabled here (no secrets in the store); the signer
              # re-signs every bundle during `xcodebuild -exportArchive`.
              "''${sanitized_env[@]}" xcodebuild \
                "''${xcodebuild_args[@]}" \
                CODE_SIGNING_ALLOWED=NO \
                CODE_SIGNING_REQUIRED=NO \
                CODE_SIGN_IDENTITY="" \
                HOME="$HOME" \
                CFFIXED_USER_HOME="$NIX_BUILD_TOP/cfhome" \
                PATH="$NIX_BUILD_TOP/shims:/usr/bin:/bin:/usr/sbin:/sbin" \
                archive -archivePath "$NIX_BUILD_TOP/app.xcarchive"
            ''
          else
            ''
              "''${sanitized_env[@]}" xcodebuild \
                "''${xcodebuild_args[@]}" \
                CODE_SIGNING_ALLOWED=NO \
                HOME="$HOME" \
                CFFIXED_USER_HOME="$NIX_BUILD_TOP/cfhome" \
                PATH="$NIX_BUILD_TOP/shims:/usr/bin:/bin:/usr/sbin:/sbin" \
                build
            ''
        }

        runHook postBuild
      '';
      installPhase = ''
        runHook preInstall
        mkdir -p $out

        ${
          if signing != null then
            ''
              # Copy IPA from export
              for ipa in "$NIX_BUILD_TOP"/export/*.ipa; do
                [ -e "$ipa" ] && cp "$ipa" $out/
              done
            ''
          else if produceArchive then
            ''
              # Copy the unsigned archive (the out-of-build signer's input).
              cp -R "$NIX_BUILD_TOP/app.xcarchive" "$out/Runner.xcarchive"
            ''
          else
            ''
              # Copy unsigned .app
              cp -R "$NIX_BUILD_TOP/DerivedData/Build/Products/${configuration}-iphoneos/"*.app $out/
            ''
        }

        runHook postInstall
      '';
    };

  # Unified entry point for building Flutter apps for one or more platforms.
  # Dispatches to buildFlutterAndroidApp (Android) and buildFlutterIOSApp (iOS)
  # based on the platforms parameter and host platform capabilities.
  #
  # Parameters:
  #   pkgs            — nixpkgs attribute set
  #   src             — Flutter project root
  #   name            — derivation name (default: the pubspec.yaml package name)
  #   lockFile        — flutter2nix lockfile with android.nodes and/or ios.nodes
  #                     (default: src/flutter2nix.lock, where the CLI writes it)
  #   platforms       — list of platforms to build (default: ["android" "ios"])
  #   androidSdk      — Android SDK (required for Android builds, default: null)
  #   gradlePackage   — Gradle for Android builds; must match the wrapper version
  #                     the lockfile was captured with (default: autodetected
  #                     from the app's gradle-wrapper.properties)
  #   signing         — null or signing config for iOS (passed to buildFlutterIOSApp)
  #   exportOptions   — path to ExportOptions.plist (passed to buildFlutterIOSApp)
  #   dartDefines     — list of "KEY=VALUE" --dart-define strings for iOS
  #                     (passed to buildFlutterIOSApp; see there)
  #   produceArchive  — emit an unsigned iOS .xcarchive instead of a .app
  #                     (passed to buildFlutterIOSApp; see there)
  #   scheme          — Xcode scheme for iOS (default "Runner"; e.g. a flavor
  #                     scheme like "stag"). Passed to buildFlutterIOSApp.
  #   configuration   — Xcode build configuration for iOS (default "Release";
  #                     e.g. "Release-stag"). Passed to buildFlutterIOSApp.
  #   ...             — other parameters passed through to the platform builders
  #
  # Returns an attrset with keys for each built platform (e.g., { android = drv; ios = drv; })
  buildFlutterApp =
    {
      pkgs,
      src,
      name ? pubLib.pubspecName src,
      lockFile ? src + "/flutter2nix.lock",
      platforms ? [
        "android"
        "ios"
      ],
      androidSdk ? null,
      signing ? null,
      ...
    }@args:
    let
      lock =
        if builtins.pathExists lockFile then
          builtins.fromJSON (builtins.readFile lockFile)
        else
          throw "buildFlutterApp: lockfile ${toString lockFile} not found — run `flutter2nix lock` in the app root or pass lockFile explicitly";
      wantsAndroid = builtins.elem "android" platforms;
      wantsIos = builtins.elem "ios" platforms;

      # Throw for missing lockfile sections (before host-capability filtering).
      _sectionCheck =
        (
          if wantsAndroid && !(lock ? android) then
            throw "buildFlutterApp: lockfile ${toString lockFile} has no 'android' section"
          else
            [ ]
        )
        ++ (
          if wantsIos && !(lock ? ios) then
            throw "buildFlutterApp: lockfile ${toString lockFile} has no 'ios' section"
          else
            [ ]
        );

      canBuildAndroid = pkgs.stdenv.isLinux && androidSdk != null;
      canBuildIos = pkgs.stdenv.isDarwin;

      passThrough = {
        pubspecLockFile = args.pubspecLockFile or (src + "/pubspec.lock");
        gitHashes = args.gitHashes or { };
        flutterSdk = args.flutterSdk or pkgs.flutter;
      };

      androidDrv = androidLib.buildFlutterAndroidApp (
        passThrough
        // {
          inherit
            pkgs
            name
            src
            lockFile
            androidSdk
            ;
          jdk = args.jdk or pkgs.jdk17;
          flutterBuildArgs = args.flutterBuildArgs or [ ];
        }
        # Only forward an explicit gradlePackage: when absent,
        # buildFlutterAndroidApp autodetects from gradle-wrapper.properties.
        // lib.optionalAttrs (args ? gradlePackage) { inherit (args) gradlePackage; }
      );

      iosDrv = buildFlutterIOSApp (
        passThrough
        // {
          inherit
            pkgs
            name
            src
            lockFile
            signing
            ;
          exportOptions = args.exportOptions or null;
          dartDefines = args.dartDefines or [ ];
          produceArchive = args.produceArchive or false;
          scheme = args.scheme or "Runner";
          configuration = args.configuration or "Release";
        }
      );

      result =
        { }
        // lib.optionalAttrs (wantsAndroid && canBuildAndroid) { android = androidDrv; }
        // lib.optionalAttrs (wantsIos && canBuildIos) { ios = iosDrv; };
    in
    # seq forces _sectionCheck to be evaluated (even though its result is discarded),
    # ensuring missing lockfile sections throw at eval time rather than being lazily ignored.
    builtins.seq _sectionCheck (
      if result == { } then
        throw "buildFlutterApp: no requested platforms (${lib.concatStringsSep ", " platforms}) can be built on ${pkgs.stdenv.hostPlatform.system}"
      else
        result
    );

in
{
  inherit buildFlutterIOSApp buildFlutterApp;
}
