# Shared Dart/Flutter pub machinery: converts a pubspec.lock into a
# .dart_tool/package_config.json built entirely from Nix store paths, so
# `flutter build --no-pub` resolves every Dart package without network or a
# pub get. Used by buildFlutterAndroidApp (gradle2nix-lib) and
# buildFlutterIOSApp (flutter2nix-lib).
{ lib }:

{
  # Returns the package_config.json derivation for a Flutter app source tree.
  pubPackageConfig =
    { pkgs
    , name
    , src
    , pubspecLockFile
    , gitHashes ? { }
    , flutterSdk
    }:
    let
      # pubspec.lock is YAML; pub2nix wants it as a Nix attrset. Same conversion
      # nixpkgs buildDartApplication uses for autoPubspecLock (IFD).
      pubspecLock = lib.importJSON (
        pkgs.runCommand "${name}-pubspec-lock-json" {
          nativeBuildInputs = [ pkgs.yq ];
        } ''yq . '${pubspecLockFile}' > "$out"''
      );

      pubspecLockData = pkgs.pub2nix.readPubspecLock {
        inherit src pubspecLock gitHashes;
        packageRoot = ".";
        # Resolves `source: sdk` packages (flutter, flutter_test, sky_engine) from
        # the Flutter SDK — same lookup paths the pub client uses.
        # https://github.com/dart-lang/pub/blob/master/lib/src/sdk/flutter.dart
        sdkSourceBuilders = {
          "flutter" = pkgName:
            pkgs.runCommand "flutter-sdk-${pkgName}" { passthru.packageRoot = "."; } ''
              for path in '${flutterSdk}/packages/${pkgName}' '${flutterSdk}/bin/cache/pkg/${pkgName}'; do
                if [ -d "$path" ]; then
                  ln -s "$path" "$out"
                  break
                fi
              done
              if [ ! -e "$out" ]; then
                echo 1>&2 'The Flutter SDK does not contain the requested package: ${pkgName}!'
                exit 1
              fi
            '';
        };
      };

      depPackageConfig = pkgs.pub2nix.generatePackageConfig {
        pname = name;
        dependencies = builtins.concatLists (builtins.attrValues pubspecLockData.dependencies);
        inherit (pubspecLockData) dependencySources;
      };

      # Language version for the root package entry, derived from the lock's Dart SDK
      # constraint — mirrors nixpkgs' linkPackageConfig.
      languageVersion =
        let
          m = builtins.match "^[[:space:]]*(\\^|>=|>)?[[:space:]]*([0-9]+\\.[0-9]+)\\.[0-9]+.*$" pubspecLock.sdks.dart;
        in
        if m != null then builtins.elemAt m 1
        else if pubspecLock.sdks.dart == "any" then "null"
        else "2.7";
    in
    # Append the root package itself; rootUri "../" resolves from .dart_tool/.
    pkgs.runCommand "${name}-package-config.json" {
      nativeBuildInputs = [ pkgs.jq pkgs.yq ];
    } ''
      packageName="$(yq --raw-output .name '${src}/pubspec.yaml')"
      jq --arg name "$packageName" --arg languageVersion ${languageVersion} \
        '.packages |= . + [{ name: $name, rootUri: "../", packageUri: "lib/", languageVersion: (if $languageVersion == "null" then null else $languageVersion end) }]' \
        '${depPackageConfig}' > "$out"
    '';
}
