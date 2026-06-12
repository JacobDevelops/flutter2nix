# gradle2nix Nix library functions.
# buildGradleProject returns helper values for consumers building their own derivations.
# buildAndroidApp wraps those helpers into a full stdenv.mkDerivation for pure Gradle projects.
# buildFlutterAndroidApp runs flutter build appbundle for Flutter apps that target Android.
{ lib }:

let
  pubLib = import ./pub-lib.nix { inherit lib; };

  knownRepoBases = [
    "https://dl.google.com/dl/android/maven2/"
    "https://repo.maven.apache.org/maven2/"
    "https://storage.googleapis.com/download.flutter.io/"
    "https://plugins.gradle.org/m2/"
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

  # Overrides for artifacts whose POM is absent from Maven Central.
  # gradle-kotlin-dsl-plugins:5.2.0 exists on Maven Central as a JAR but its
  # POM is only at plugins.gradle.org/m2/ (unreachable offline). The real POM
  # declares Kotlin 2.0.21 deps, but the lockfile has 1.9.20 — we inject only
  # the two compiler-plugin deps not bundled in Gradle's distribution.
  # On kotlin-dsl version bumps: re-fetch the POM from:
  #   https://plugins.gradle.org/m2/org/gradle/kotlin/gradle-kotlin-dsl-plugins/<ver>/gradle-kotlin-dsl-plugins-<ver>.pom
  # and update this entry and the lockfile accordingly.
  knownPomOverrides = {
    # kotlin-compiler-runner's real POM (Maven Central) also declares kotlin-build-common
    # and kotlinx-coroutines-core-jvm:1.5.0, but neither is in the lockfile at compatible
    # versions. Only kotlin-daemon-client is declared here — it carries the
    # MultiModuleICSettings class that KotlinCompile task decoration requires.
    "org.jetbrains.kotlin:kotlin-compiler-runner:1.9.20" = ''
      <?xml version="1.0" encoding="UTF-8"?>
      <project>
        <modelVersion>4.0.0</modelVersion>
        <groupId>org.jetbrains.kotlin</groupId>
        <artifactId>kotlin-compiler-runner</artifactId>
        <version>1.9.20</version>
        <dependencies>
          <dependency>
            <groupId>org.jetbrains.kotlin</groupId>
            <artifactId>kotlin-daemon-client</artifactId>
            <version>1.9.20</version>
            <scope>compile</scope>
          </dependency>
        </dependencies>
      </project>
    '';

    "org.gradle.kotlin:gradle-kotlin-dsl-plugins:5.2.0" = ''
      <?xml version="1.0" encoding="UTF-8"?>
      <project>
        <modelVersion>4.0.0</modelVersion>
        <groupId>org.gradle.kotlin</groupId>
        <artifactId>gradle-kotlin-dsl-plugins</artifactId>
        <version>5.2.0</version>
        <dependencies>
          <dependency>
            <groupId>org.jetbrains.kotlin</groupId>
            <artifactId>kotlin-sam-with-receiver</artifactId>
            <version>1.9.20</version>
            <scope>runtime</scope>
          </dependency>
          <dependency>
            <groupId>org.jetbrains.kotlin</groupId>
            <artifactId>kotlin-assignment</artifactId>
            <version>1.9.20</version>
            <scope>runtime</scope>
          </dependency>
        </dependencies>
      </project>
    '';
  };

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
  # If a node's URL already ends in .pom (e.g. plugin marker POMs), the fetched
  # file IS the POM — no synthetic POM is generated for those entries.
  # If a node's URL ends in .module (Gradle Module Metadata), no synthetic POM
  # is generated either — Gradle reads the .module file directly via gradleMetadata().
  buildMavenRepo = pkgs: nodes:
    let
      entries = map (node:
        let
          rel = artifactRelPath node.url;
          isMavenPom = lib.hasSuffix ".pom" rel;
          isMavenModule = lib.hasSuffix ".module" rel;
          noSyntheticPom = isMavenPom || isMavenModule;
          pom = if noSyntheticPom then null else pomRelPath rel;
          fetched = pkgs.fetchurl { url = node.url; sha256 = node.sha256; };
          pomXml = if noSyntheticPom then null
                   else
                     let
                       # node.name can be "group:artifact" or "group:artifact:version";
                       # normalise to "group:artifact:version" for map lookup.
                       coords = lib.splitString ":" node.name;
                       groupArtifact = lib.concatStringsSep ":" (lib.sublist 0 2 coords);
                       key = "${groupArtifact}:${node.version}";
                     in knownPomOverrides.${key} or (minimalPom node.name node.version);
        in
        { inherit rel pom fetched pomXml isMavenPom; }
      ) nodes;

      # Process non-POM artifacts first (which also writes synthetic POMs alongside them),
      # then real .pom entries last so they overwrite any synthetic stubs.
      # Without this ordering, a JAR processed after its real POM would re-stamp a
      # synthetic (dep-free) stub on top, breaking transitive resolution.
      orderedEntries =
        (lib.filter (e: !e.isMavenPom) entries) ++
        (lib.filter (e: e.isMavenPom) entries);

      installCmds = lib.concatMapStrings (e: ''
        install -Dm644 ${e.fetched} "$out/${e.rel}"
        ${lib.optionalString (e.pom != null) ''
          cat > "$out/${e.pom}" << 'POMEOF'
          ${e.pomXml}
          POMEOF
        ''}
      '') orderedEntries;
    in
    pkgs.runCommand "gradle-maven-repo" { } ''
      ${installCmds}
    '';

  # Instantiates the Gradle init script template (gradle2nix-init.gradle) with the
  # offline Maven repo path. The template lives in a separate file so it gets Groovy
  # syntax highlighting; replaceVars fails the build if any @var@ is left unsubstituted.
  makeInitScript = pkgs: mavenRepo:
    pkgs.replaceVars ./gradle2nix-init.gradle {
      inherit mavenRepo;
    };

  # Returns an attrset of build helpers: mavenRepo, initScript, buildInputs,
  # and baseGradleFlags. Compose these into your own stdenv.mkDerivation.
  # gradlePackage must match the Gradle wrapper version the lockfile was captured
  # with: Gradle-version-coupled artifacts (e.g. the kotlin-dsl plugin requested by
  # flutter_tools' helper build) are locked at the version the lock-time Gradle
  # implies, and a different build-time Gradle requests versions the offline repo
  # doesn't have.
  buildGradleProject =
    { pkgs
    , lockFile
    , jdk ? pkgs.jdk17
    , gradlePackage ? pkgs.gradle
    , ...
    }:
    let
      nodes = readNodes lockFile;
      mavenRepo = buildMavenRepo pkgs nodes;
      initScript = makeInitScript pkgs mavenRepo;
    in
    {
      inherit mavenRepo initScript;
      buildInputs = [ jdk gradlePackage ];
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
    , gradlePackage ? pkgs.gradle
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
      buildInputs = [ jdk gradlePackage androidSdk ];
      ANDROID_HOME = "${androidSdk}/libexec/android-sdk";
      ANDROID_SDK_ROOT = "${androidSdk}/libexec/android-sdk";
      JAVA_HOME = "${jdk}";
      buildPhase = ''
        # GRADLE_USER_HOME cannot be a mkDerivation attribute: $TMPDIR is sandbox-provided
        # at build time and is not available as a Nix string at evaluation time.
        export GRADLE_USER_HOME=$TMPDIR/gradle-home
        mkdir -p "$GRADLE_USER_HOME"
        # AGP's Maven-fetched aapt2 is a prebuilt dynamically-linked binary that cannot
        # exec inside the Nix sandbox (no /lib64 loader). Point AGP at the patched aapt2
        # from the SDK build-tools instead — same approach as nixpkgs androidenv.
        # $GRADLE_USER_HOME/gradle.properties is the highest-precedence project property
        # source and applies to every Gradle invocation.
        # -L: the composed androidsdk is a symlink farm (build-tools/<ver> links to
        # another store path), so find must follow symlinks to descend into it.
        aapt2="$(find -L "$ANDROID_SDK_ROOT/build-tools" -name aapt2 -type f | head -n1)"
        if [ -z "$aapt2" ]; then
          echo "ERROR: no aapt2 found under $ANDROID_SDK_ROOT/build-tools" >&2
          exit 1
        fi
        echo "android.aapt2FromMavenOverride=$aapt2" >> "$GRADLE_USER_HOME/gradle.properties"
        # The Kotlin compile daemon dies on startup inside the Nix sandbox ("terminated
        # unexpectedly on startup attempt #1 with error code: 0"). Run the compiler in
        # the Gradle JVM instead. Placed here (not in the project's gradle.properties)
        # so it reaches every build in the composite, including flutter_tools.
        echo "kotlin.compiler.execution.strategy=in-process" >> "$GRADLE_USER_HOME/gradle.properties"
        gradle ${gradleTask} ${allFlags}
      '';
      installPhase = ''
        mkdir -p $out
        find . -name "*.apk" -path "*/release/*" -exec cp {} $out/ \;
        find . -name "*.aab" -path "*/release/*" -exec cp {} $out/ \;
      '';
    };

  # Builds a Flutter Android app (AAB/APK) offline using locked Maven and pub dependencies.
  #
  # Unlike buildAndroidApp (which runs raw gradle assembleRelease), this function:
  # - Invokes `flutter build appbundle`, which compiles Dart first then drives Gradle internally.
  # - Wires the offline Maven repo into Flutter's internal Gradle via $GRADLE_USER_HOME/init.d/.
  # - Generates .dart_tool/package_config.json from pubspec.lock via pkgs.pub2nix, so
  #   `flutter build --no-pub` resolves Dart packages entirely from the Nix store.
  #   Hosted packages are fetched by the sha256 recorded in pubspec.lock; sdk packages
  #   (flutter, flutter_test, sky_engine) resolve from the Flutter SDK itself.
  # - Only works on Linux (Android SDK is Linux-native). Set meta.platforms accordingly.
  #
  # Use buildAndroidApp for pure Gradle/Maven projects (no Flutter, no Dart).
  # Use buildFlutterAndroidApp for Flutter apps (Dart + Gradle + Android SDK).
  #
  # Parameters:
  #   pkgs            — nixpkgs attribute set
  #   name            — derivation name
  #   src             — Flutter app source (must contain pubspec.yaml, android/, lib/)
  #   lockFile        — flutter2nix.lock from `gradle2nix lock` (contains android.nodes)
  #   pubspecLockFile — pubspec.lock from a real `flutter pub get` run (must record
  #                     hosted-package sha256 hashes). Default: src + "/pubspec.lock".
  #                     NOTE: converted to JSON at evaluation time (import-from-derivation).
  #   gitHashes       — hashes for git-sourced pub dependencies (pub does not record them)
  #   flutterSdk      — Flutter SDK package (default: pkgs.flutter)
  #   jdk             — JDK package (default: pkgs.jdk17)
  #   gradlePackage   — Gradle package; MUST match the wrapper version the lockfile
  #                     was captured with (default: pkgs.gradle)
  #   androidSdk      — Android SDK from androidenv.composeAndroidPackages { }.androidsdk
  #   flutterBuildArgs — extra args for `flutter build appbundle` (e.g. ["--flavor" "stag"])
  buildFlutterAndroidApp =
    { pkgs
    , name
    , src
    , lockFile
    , pubspecLockFile ? src + "/pubspec.lock"
    , gitHashes ? { }
    , flutterSdk ? pkgs.flutter
    , jdk ? pkgs.jdk17
    , gradlePackage ? pkgs.gradle
    , androidSdk
    , flutterBuildArgs ? []
    , ...
    }:
    let
      gradle = buildGradleProject { inherit pkgs lockFile jdk gradlePackage; };

      packageConfig = pubLib.pubPackageConfig {
        inherit pkgs name src pubspecLockFile gitHashes flutterSdk;
      };
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
        # Bypass the Gradle wrapper download by replacing gradlew with a direct invocation
        # of the Nix-provided Gradle. The wrapper would otherwise try to download its
        # distribution from services.gradle.org, which is blocked in the Nix sandbox.
        # rm -f: works whether or not the app ships a gradlew (read-only from the store).
        rm -f android/gradlew
        cat > android/gradlew << 'GRADLEW_EOF'
#!/bin/sh
exec ${gradlePackage}/bin/gradle --offline "$@"
GRADLEW_EOF
        chmod +x android/gradlew
        # Write correct local.properties so settings.gradle.kts can find flutter.sdk.
        # The file in the source tree has developer-machine paths that don't exist here.
        printf 'flutter.sdk=%s\nsdk.dir=%s\n' \
          "${flutterSdk}" "${androidSdk}/libexec/android-sdk" \
          > android/local.properties
        export HOME="$NIX_BUILD_TOP"
        # Install the Nix-generated package config so `flutter build --no-pub` resolves
        # all Dart packages from the store without running pub. The copied pubspec.lock
        # keeps flutter_tools' freshness check consistent with the config.
        mkdir -p .dart_tool
        cp ${packageConfig} .dart_tool/package_config.json
        chmod u+w .dart_tool/package_config.json
        install -m644 ${pubspecLockFile} pubspec.lock
        # flutter_tools also requires .dart_tool/package_graph.json (pub >= 3.5 writes it
        # during pub get). Generate it with the same script nixpkgs' dartConfigHook uses.
        ${pkgs.python3.withPackages (ps: [ ps.pyyaml ])}/bin/python3 \
          ${pkgs.path}/pkgs/build-support/dart/pub2nix/package-graph.py \
          > .dart_tool/package_graph.json
        # AGP's Maven-fetched aapt2 is a prebuilt dynamically-linked binary that cannot
        # exec inside the Nix sandbox (no /lib64 loader). Point AGP at the patched aapt2
        # from the SDK build-tools instead — same approach as nixpkgs androidenv.
        # $GRADLE_USER_HOME/gradle.properties is the highest-precedence project property
        # source and applies to the Gradle build that flutter drives via gradlew.
        # -L: the composed androidsdk is a symlink farm (build-tools/<ver> links to
        # another store path), so find must follow symlinks to descend into it.
        aapt2="$(find -L "$ANDROID_SDK_ROOT/build-tools" -name aapt2 -type f | head -n1)"
        if [ -z "$aapt2" ]; then
          echo "ERROR: no aapt2 found under $ANDROID_SDK_ROOT/build-tools" >&2
          exit 1
        fi
        echo "android.aapt2FromMavenOverride=$aapt2" >> "$GRADLE_USER_HOME/gradle.properties"
        # The Kotlin compile daemon dies on startup inside the Nix sandbox ("terminated
        # unexpectedly on startup attempt #1 with error code: 0"). Run the compiler in
        # the Gradle JVM instead. Placed here (not in the project's gradle.properties)
        # so it reaches every build in the composite, including flutter_tools.
        echo "kotlin.compiler.execution.strategy=in-process" >> "$GRADLE_USER_HOME/gradle.properties"
        # Hermetically generate .flutter-plugins-dependencies: flutter_tools
        # writes it during pub get with developer-machine paths and it is
        # gitignored, so a clean checkout ships none — but the Flutter Gradle
        # plugin loader includeBuild()s every android plugin from the paths it
        # records. Synthesized from package_config.json (Nix store roots) +
        # each package's pubspec + pubspec.lock.
        rm -f .flutter-plugins-dependencies
        ${pkgs.python3.withPackages (ps: [ ps.pyyaml ])}/bin/python3 \
          ${./generate-flutter-plugins.py} "${flutterSdk.version}"
        # flutter build --no-pub never regenerates GeneratedPluginRegistrant.java for
        # release mode, so a debug-style registrant still references dev-dependency
        # plugins (e.g. integration_test) that the Flutter Gradle plugin excludes from
        # release variants — javac then fails with "package does not exist". Strip
        # dev-dependency registrations, mirroring flutter's own release-mode regen.
        ${pkgs.python3}/bin/python3 ${./strip-dev-deps.py}
        # Preflight: flutter_tools' PubDependencies artifact check runs an ONLINE
        # pub get for the tool itself (even under --no-pub) when the SDK ships no
        # resolved packages/flutter_tools/.dart_tool/package_config.json — raw
        # Google-tarball SDKs don't. Fail fast with an actionable message instead
        # of an opaque "Got socket error trying to find package test" later.
        ${pkgs.python3}/bin/python3 ${./check-flutter-sdk.py} "${flutterSdk}"
        # NOTE: gradle.baseGradleFlags contains Gradle-specific flags (--no-daemon,
        # --no-configuration-cache, --init-script) that flutter build does NOT accept.
        # The init script is auto-loaded from $GRADLE_USER_HOME/init.d/. --no-pub skips
        # pub get since PUB_CACHE is already populated.
        flutter build appbundle --no-pub ${lib.escapeShellArgs flutterBuildArgs}
        runHook postBuild
      '';
      installPhase = ''
        runHook preInstall
        mkdir -p $out
        # -iname *release*: flavored builds land in bundle/<flavor>Release/
        # (e.g. stagRelease), not bundle/release/.
        find . -name "*.aab" -ipath "*release*" -exec cp {} $out/ \;
        find . -name "*.apk" -ipath "*release*" -exec cp {} $out/ \;
        if [ -z "$(find "$out" -name "*.aab" -o -name "*.apk" 2>/dev/null)" ]; then
          echo "ERROR: No AAB or APK found in build output." >&2
          echo "  Expected artifacts under: build/app/outputs/bundle/<variant>Release/" >&2
          exit 1
        fi
        runHook postInstall
      '';
    };
}
