# gradle2nix Nix library functions.
# buildGradleProject returns helper values for consumers building their own derivations.
# buildAndroidApp wraps those helpers into a full stdenv.mkDerivation for pure Gradle projects.
# buildFlutterAndroidApp runs flutter build appbundle for Flutter apps that target Android.
{ lib }:

let
  knownRepoBases = [
    "https://dl.google.com/dl/android/maven2/"
    "https://repo.maven.apache.org/maven2/"
    "https://storage.googleapis.com/download.flutter.io/"
  ];

  artifactRelPath = url:
    let
      base = lib.findFirst (b: lib.hasPrefix b url) null knownRepoBases;
    in
    if base != null
    then lib.removePrefix base url
    else throw "gradle2nix-lib: unrecognized Maven repository URL: ${url}";

  pomRelPath = relPath:
    let
      pathParts = lib.splitString "/" relPath;
      dir = lib.concatStringsSep "/" (lib.init pathParts);
      basename = lib.last pathParts;
      nameParts = lib.splitString "." basename;
      nameNoExt = lib.concatStringsSep "." (lib.init nameParts);
    in
    "${dir}/${nameNoExt}.pom";

  minimalPom = name: version:
    let
      coords = lib.splitString ":" name;
      group = lib.elemAt coords 0;
      artifact = lib.elemAt coords 1;
    in
    ''
      <?xml version="1.0" encoding="UTF-8"?>
      <project>
        <modelVersion>4.0.0</modelVersion>
        <groupId>${group}</groupId>
        <artifactId>${artifact}</artifactId>
        <version>${version}</version>
      </project>
    '';

  # Reads nodes from a gradle2nix.lock ({ nodes: [...] }) or
  # flutter2nix.lock ({ android: { nodes: [...] } }).
  readNodes = lockFile:
    let
      lock = builtins.fromJSON (builtins.readFile lockFile);
    in
    if lock ? android then lock.android.nodes
    else if lock ? nodes then lock.nodes
    else throw "gradle2nix-lib: unrecognized lockfile format in ${toString lockFile}";

  # Builds a local Maven repository from lockfile nodes.
  # Each artifact is fetched by its locked sha256; a minimal POM is generated
  # alongside it so Gradle can resolve metadata without network access.
  buildMavenRepo = pkgs: nodes:
    let
      entries = map (node:
        let
          rel = artifactRelPath node.url;
          pom = pomRelPath rel;
          fetched = pkgs.fetchurl { url = node.url; sha256 = node.sha256; };
          pomXml = minimalPom node.name node.version;
        in
        { inherit rel pom fetched pomXml; }
      ) nodes;

      installCmds = lib.concatMapStrings (e: ''
        install -Dm644 ${e.fetched} "$out/${e.rel}"
        cat > "$out/${e.pom}" << 'POMEOF'
        ${e.pomXml}
        POMEOF
      '') entries;
    in
    pkgs.runCommand "gradle-maven-repo" { } ''
      ${installCmds}
    '';

  # Writes a Gradle init script that redirects all repository lookups to
  # the local Maven repo and enables metadata resolution from POMs + artifacts.
  makeInitScript = pkgs: mavenRepo:
    pkgs.writeText "gradle2nix-init.gradle" ''
      allprojects {
        repositories {
          maven {
            url "file://${mavenRepo}"
            metadataSources {
              mavenPom()
              artifact()
            }
          }
        }
        configurations.all {
          resolutionStrategy.cacheChangingModulesFor 0, 'seconds'
          resolutionStrategy.cacheDynamicVersionsFor 0, 'seconds'
        }
      }
      gradle.projectsLoaded {
        rootProject.allprojects {
          buildscript {
            repositories {
              maven {
                url "file://${mavenRepo}"
                metadataSources {
                  mavenPom()
                  artifact()
                }
              }
            }
          }
        }
      }
    '';

  # Returns an attrset of build helpers: mavenRepo, initScript, buildInputs,
  # and baseGradleFlags. Compose these into your own stdenv.mkDerivation.
  buildGradleProject =
    { pkgs
    , lockFile
    , jdk ? pkgs.jdk17
    , ...
    }:
    let
      nodes = readNodes lockFile;
      mavenRepo = buildMavenRepo pkgs nodes;
      initScript = makeInitScript pkgs mavenRepo;
    in
    {
      inherit mavenRepo initScript;
      buildInputs = [ jdk pkgs.gradle ];
      baseGradleFlags = [
        "--offline"
        "--no-daemon"
        "--no-configuration-cache"
        "--init-script"
        "${initScript}"
      ];
    };

in
{
  inherit buildGradleProject;

  # Full derivation that runs a Gradle task offline using the locked Maven repo.
  # Copies *.apk and *.aab from the release output directory to $out.
  #
  # Use this for pure Gradle/Maven Android projects (no Flutter, no Dart).
  # For Flutter apps, use buildFlutterAndroidApp instead.
  #
  # androidSdk: pass androidComposition.androidsdk from androidenv.composeAndroidPackages.
  # Must include buildToolsVersions and platformVersions matching the project's build.gradle.
  # nixpkgs places the SDK at ${androidsdk}/libexec/android-sdk (see nixpkgs/androidenv/build-app.nix:37).
  buildAndroidApp =
    { pkgs
    , name
    , src
    , lockFile
    , gradleTask ? "assembleRelease"
    , gradleFlags ? []
    , jdk ? pkgs.jdk17
    , androidSdk
    , ...
    }:
    let
      nodes = readNodes lockFile;
      mavenRepo = buildMavenRepo pkgs nodes;
      initScript = makeInitScript pkgs mavenRepo;
      allFlags = lib.concatStringsSep " " ([
        "--offline"
        "--no-daemon"
        "--no-configuration-cache"
        "--init-script"
        "${initScript}"
      ] ++ gradleFlags);
    in
    pkgs.stdenv.mkDerivation {
      inherit name src;
      buildInputs = [ jdk pkgs.gradle androidSdk ];
      ANDROID_HOME = "${androidSdk}/libexec/android-sdk";
      ANDROID_SDK_ROOT = "${androidSdk}/libexec/android-sdk";
      JAVA_HOME = "${jdk}";
      buildPhase = ''
        # GRADLE_USER_HOME cannot be a mkDerivation attribute: $TMPDIR is sandbox-provided
        # at build time and is not available as a Nix string at evaluation time.
        export GRADLE_USER_HOME=$TMPDIR/gradle-home
        gradle ${gradleTask} ${allFlags}
      '';
      installPhase = ''
        mkdir -p $out
        find . -name "*.apk" -path "*/release/*" -exec cp {} $out/ \;
        find . -name "*.aab" -path "*/release/*" -exec cp {} $out/ \;
      '';
    };

  # Builds a Flutter Android app (AAB/APK) offline using locked Maven and pub caches.
  #
  # Unlike buildAndroidApp (which runs raw gradle assembleRelease), this function:
  # - Invokes `flutter build appbundle`, which compiles Dart first then drives Gradle internally.
  # - Wires the offline Maven repo into Flutter's internal Gradle via $GRADLE_USER_HOME/init.d/.
  # - Requires pubCacheDir: a pre-built Dart pub cache path (caller's responsibility).
  # - Only works on Linux (Android SDK is Linux-native). Set meta.platforms accordingly.
  #
  # Use buildAndroidApp for pure Gradle/Maven projects (no Flutter, no Dart).
  # Use buildFlutterAndroidApp for Flutter apps (Dart + Gradle + Android SDK).
  #
  # Parameters:
  #   pkgs        — nixpkgs attribute set
  #   name        — derivation name
  #   src         — Flutter app source (must contain pubspec.yaml, android/, lib/)
  #   lockFile    — flutter2nix.lock from `gradle2nix lock` (contains android.nodes)
  #   pubCacheDir — pre-built Dart pub cache store path; caller's responsibility.
  #                 Typically built via buildDartApplication with autoPubspecLock.
  #   flutterSdk  — Flutter SDK package (default: pkgs.flutter)
  #   jdk         — JDK package (default: pkgs.jdk17)
  #   androidSdk  — Android SDK from androidenv.composeAndroidPackages { }.androidsdk
  #   gradleFlags — reserved for future use; extra Gradle flags (not passed to flutter CLI)
  #
  # Example:
  #   let
  #     pubCache = (pkgs.buildDartApplication {
  #       pname = "my-app-pub-cache";
  #       version = "1.0.0";
  #       src = ./.; autoPubspecLock = ./pubspec.lock;
  #     }).passthru.pubcacheDir or pkgs.emptyDirectory;
  #   in
  #   buildFlutterAndroidApp {
  #     inherit pkgs name src;
  #     lockFile = ./flutter2nix.lock;
  #     pubCacheDir = pubCache;
  #     androidSdk = androidComposition.androidsdk;
  #   }
  buildFlutterAndroidApp =
    { pkgs
    , name
    , src
    , lockFile
    , pubCacheDir
    , flutterSdk ? pkgs.flutter
    , jdk ? pkgs.jdk17
    , androidSdk
    , gradleFlags ? []
    , ...
    }:
    let
      gradle = buildGradleProject { inherit pkgs lockFile jdk; };
    in
    pkgs.stdenv.mkDerivation {
      inherit name src;
      buildInputs = gradle.buildInputs ++ [ flutterSdk androidSdk ];
      ANDROID_HOME = "${androidSdk}/libexec/android-sdk";
      ANDROID_SDK_ROOT = "${androidSdk}/libexec/android-sdk";
      JAVA_HOME = "${jdk}";
      meta.platforms = lib.platforms.linux;
      buildPhase = ''
        runHook preBuild
        export GRADLE_USER_HOME=$(mktemp -d)
        mkdir -p "$GRADLE_USER_HOME/init.d"
        cp ${gradle.initScript} "$GRADLE_USER_HOME/init.d/gradle2nix-flutter.gradle"
        export HOME="$NIX_BUILD_TOP"
        export PUB_CACHE=${pubCacheDir}
        for pkg in flutter flutter_test; do
          if [ -z "$(find "$PUB_CACHE/hosted/pub.dev" -maxdepth 1 -name "$pkg-*" -type d 2>/dev/null | head -1)" ]; then
            echo "ERROR: pubCacheDir missing package: $pkg" >&2
            echo "  Expected: $PUB_CACHE/hosted/pub.dev/$pkg-*/" >&2
            echo "  Ensure pubCacheDir is built from the same pubspec.lock." >&2
            exit 1
          fi
        done
        # NOTE: gradle.baseGradleFlags contains Gradle-specific flags (--no-daemon,
        # --no-configuration-cache, --init-script) that flutter build does NOT accept.
        # The init script is auto-loaded from $GRADLE_USER_HOME/init.d/. Only --offline
        # is passed to the flutter CLI.
        flutter build appbundle --offline
        runHook postBuild
      '';
      installPhase = ''
        runHook preInstall
        mkdir -p $out
        find . -name "*.aab" -path "*/release/*" -exec cp {} $out/ \;
        find . -name "*.apk" -path "*/release/*" -exec cp {} $out/ \;
        if [ -z "$(find "$out" -name "*.aab" -o -name "*.apk" 2>/dev/null)" ]; then
          echo "ERROR: No AAB or APK found in build output." >&2
          echo "  Expected artifacts under: build/app/outputs/bundle/release/" >&2
          exit 1
        fi
        runHook postInstall
      '';
    };
}
