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

  artifactRelPath =
    url:
    let
      base = lib.findFirst (b: lib.hasPrefix b url) null knownRepoBases;
    in
    if base != null then
      lib.removePrefix base url
    else
      throw "gradle2nix-lib: unrecognized Maven repository URL: ${url}";

  pomRelPath =
    relPath:
    let
      pathParts = lib.splitString "/" relPath;
      dir = lib.concatStringsSep "/" (lib.init pathParts);
      basename = lib.last pathParts;
      nameParts = lib.splitString "." basename;
      nameNoExt = lib.concatStringsSep "." (lib.init nameParts);
    in
    "${dir}/${nameNoExt}.pom";

  minimalPom =
    name: version:
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
  readNodes =
    lockFile:
    let
      lock = builtins.fromJSON (builtins.readFile lockFile);
    in
    if !builtins.pathExists lockFile then
      throw "gradle2nix-lib: lockfile ${toString lockFile} not found — run `flutter2nix lock` in the app root or pass lockFile explicitly"
    else if lock ? android then
      lock.android.nodes
    else if lock ? nodes then
      lock.nodes
    else
      throw "gradle2nix-lib: unrecognized lockfile format in ${toString lockFile}";

  # Parses the Gradle major version pinned by the project's committed wrapper
  # properties (android/gradle/wrapper for Flutter apps, gradle/wrapper for
  # pure Gradle projects). Returns null when no wrapper file exists.
  wrapperGradleMajor =
    src:
    let
      propsFile = lib.findFirst builtins.pathExists null [
        (src + "/android/gradle/wrapper/gradle-wrapper.properties")
        (src + "/gradle/wrapper/gradle-wrapper.properties")
      ];
      distLine =
        if propsFile == null then
          null
        else
          lib.findFirst (l: lib.hasPrefix "distributionUrl=" l) null (
            lib.splitString "\n" (builtins.readFile propsFile)
          );
      m =
        if distLine == null then
          null
        else
          builtins.match ".*gradle-([0-9]+)\\.[^/]*\\.zip[[:space:]]*" distLine;
    in
    if m == null then null else lib.head m;

  # Default Gradle for a project: the major version its wrapper pins — which
  # is the version the lockfile was captured with, since gradle2nix locks
  # through the wrapper — falling back to pkgs.gradle when no wrapper exists.
  # Hand-pinning gradlePackage to match the wrapper was the #1 consumer
  # footgun (a mismatched Gradle requests kotlin-dsl versions the offline
  # repo doesn't have).
  defaultGradlePackage =
    pkgs: src:
    let
      major = wrapperGradleMajor src;
    in
    if major == null then
      pkgs.gradle
    else
      pkgs."gradle_${major}"
        or (throw "gradle2nix-lib: project pins Gradle ${major}.x in gradle-wrapper.properties but nixpkgs has no gradle_${major}");

  # Flutter publishes its engine native libraries as per-build-mode × per-ABI
  # Maven artifacts under group `io.flutter`: `<abi>_<mode>` (e.g.
  # arm64_v8a_release) for the per-ABI libflutter.so, and
  # `flutter_embedding_<mode>` for the Java embedder, with mode ∈
  # {debug, profile, release}. Returns a node's engine build mode, or null when
  # the node is not an io.flutter engine variant (so non-engine nodes are never
  # scoped out). builtins.match is whole-string anchored, so `arm64_v8a_release`
  # yields mode "release" and `flutter_embedding_debug` yields "debug".
  engineModeOf =
    node:
    let
      coords = lib.splitString ":" node.name;
      group = lib.head coords;
      artifact = if lib.length coords > 1 then lib.elemAt coords 1 else "";
      m = builtins.match "(.*)_(debug|profile|release)" artifact;
    in
    if group == "io.flutter" && m != null then lib.elemAt m 1 else null;

  # Scopes lockfile nodes to the Flutter engine build mode(s) a build actually
  # links. keepModes = null (default) keeps every node — the safe all-modes
  # superset, so one offline repo serves debug `flutter run` AND release CI. A
  # non-null list (e.g. [ "release" ]) drops io.flutter engine variants whose mode
  # is not in the list; every non-engine node is always kept. Pure selection over
  # the lockfile — the lockfile keeps the superset and artifact bytes are
  # untouched, so there is no hash or hermeticity impact; it only narrows which
  # engine .so get materialized into the offline repo (a release AAB links only
  # *_release, so debug+profile engine .so — the bulk of the closure — are dead
  # weight a cold runner would otherwise restore).
  scopeEngineNodes =
    keepModes: nodes:
    if keepModes == null then
      nodes
    # [] would drop every engine variant and leave a Flutter build with no engine
    # to resolve — an opaque Gradle failure deep in the build. Fail fast: null
    # keeps the superset, a non-empty list names the mode(s) the build links.
    else if keepModes == [ ] then
      throw "gradle2nix-lib: engineModes = [] drops every Flutter engine variant — use null to keep the all-modes superset, or list the mode(s) the build links (e.g. [ \"release\" ])"
    else
      lib.filter (
        node:
        let
          mode = engineModeOf node;
        in
        mode == null || lib.elem mode keepModes
      ) nodes;

  # Builds a local Maven repository from lockfile nodes.
  # Each artifact is fetched by its locked sha256; a minimal POM is generated
  # alongside it so Gradle can resolve metadata without network access.
  # If a node's URL already ends in .pom (e.g. plugin marker POMs), the fetched
  # file IS the POM — no synthetic POM is generated for those entries.
  # If a node's URL ends in .module (Gradle Module Metadata), no synthetic POM
  # is generated either — Gradle reads the .module file directly via gradleMetadata().
  #
  # consolidate (default false): when false, fetched artifacts are SYMLINKED into
  # the repo so the closure stays content-addressed and deduped (the default —
  # see installArtifact below). When true, artifacts are COPIED in so the whole
  # repo collapses into a single self-contained store path; opt in via
  # consolidateMavenRepo for cold-runner CI where ~2900 per-object cache GETs
  # dominate. Tradeoffs are documented on the consolidateMavenRepo flag.
  buildMavenRepo =
    pkgs: nodes: consolidate:
    let
      entries = map (
        node:
        let
          rel = artifactRelPath node.url;
          isMavenPom = lib.hasSuffix ".pom" rel;
          isMavenModule = lib.hasSuffix ".module" rel;
          noSyntheticPom = isMavenPom || isMavenModule;
          pom = if noSyntheticPom then null else pomRelPath rel;
          fetched = pkgs.fetchurl {
            url = node.url;
            sha256 = node.sha256;
          };
          pomXml =
            if noSyntheticPom then
              null
            else
              let
                # node.name can be "group:artifact" or "group:artifact:version";
                # normalise to "group:artifact:version" for map lookup.
                coords = lib.splitString ":" node.name;
                groupArtifact = lib.concatStringsSep ":" (lib.sublist 0 2 coords);
                key = "${groupArtifact}:${node.version}";
              in
              knownPomOverrides.${key} or (minimalPom node.name node.version);
        in
        {
          inherit
            rel
            pom
            fetched
            pomXml
            isMavenPom
            ;
        }
      ) nodes;

      # Process non-POM artifacts first (which also writes synthetic POMs alongside them),
      # then real .pom entries last so they overwrite any synthetic stubs.
      # Without this ordering, a JAR processed after its real POM would re-stamp a
      # synthetic (dep-free) stub on top, breaking transitive resolution.
      orderedEntries = (lib.filter (e: !e.isMavenPom) entries) ++ (lib.filter (e: e.isMavenPom) entries);

      # Places a fetched artifact into the repo. Default (consolidate=false):
      # SYMLINK the fetchurl output — the artifact already lives in the store, so
      # symlinking keeps the repo content-addressed and deduped (one store path
      # per artifact, shared across lockfile versions and projects) and keeps
      # on-disk footprint flat. -f: a real .pom (processed last, see
      # orderedEntries) replaces the synthetic stub written alongside its
      # artifact. consolidate=true: COPY the artifact in so the whole repo is one
      # self-contained store path that a binary cache streams in a single NAR
      # request — at the cost of dedup and incremental transfer (each lockfile
      # version is now a full ~5.6GB NAR). -f + chmod u+w: the dest may already
      # exist (the synthetic stub) and fetchurl outputs are read-only, so the
      # copy must be made writable to allow the real-POM overwrite.
      installArtifact =
        e:
        if consolidate then
          ''
            cp -f ${e.fetched} "$out/${e.rel}"
            chmod u+w "$out/${e.rel}"''
        else
          ''ln -sfn ${e.fetched} "$out/${e.rel}"'';
      installCmds = lib.concatMapStrings (e: ''
        mkdir -p "$out/${dirOf e.rel}"
        ${installArtifact e}
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
  makeInitScript =
    pkgs: mavenRepo:
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
    {
      pkgs,
      lockFile,
      jdk ? pkgs.jdk17,
      gradlePackage ? pkgs.gradle,
      consolidateMavenRepo ? false,
      engineModes ? null,
      ...
    }:
    let
      nodes = scopeEngineNodes engineModes (readNodes lockFile);
      mavenRepo = buildMavenRepo pkgs nodes consolidateMavenRepo;
      initScript = makeInitScript pkgs mavenRepo;
    in
    {
      inherit mavenRepo initScript;
      buildInputs = [
        jdk
        gradlePackage
      ];
      baseGradleFlags = [
        "--offline"
        "--no-daemon"
        "--no-configuration-cache"
        "--init-script"
        "${initScript}"
      ];
    };

  # Returns a shell-hook string that wires the lockfile's offline Maven repo
  # into a dedicated GRADLE_USER_HOME, so dev-shell `flutter run` / `gradle`
  # resolve locked deps from the reproducible repo. The init script injects the
  # offline repo at the front of every RepositoryHandler, so locked artifacts
  # come from file:// while anything not yet locked falls through to the
  # project's own google()/mavenCentral() over the network (no re-lock needed
  # mid-development). Drop the result into devenv enterShell, a mkShell
  # shellHook, or a direnv .envrc.
  #
  # Parameters:
  #   pkgs           — nixpkgs attribute set
  #   lockFile       — flutter2nix.lock / gradle2nix.lock (android section used)
  #   gradleUserHome — shell expression for the GRADLE_USER_HOME to use. A
  #                    dedicated path keeps the offline wiring out of the global
  #                    ~/.gradle (which would force every project on the machine
  #                    to prefer this repo). Default: "$HOME/.gradle-flutter2nix".
  offlineGradleDevHook =
    {
      pkgs,
      lockFile,
      gradleUserHome ? "$HOME/.gradle-flutter2nix",
    }:
    let
      gradle = buildGradleProject { inherit pkgs lockFile; };
    in
    ''
      # flutter2nix: prefer the locked offline Maven repo for Gradle builds.
      export GRADLE_USER_HOME="${gradleUserHome}"
      mkdir -p "$GRADLE_USER_HOME/init.d"
      install -m644 ${gradle.initScript} \
        "$GRADLE_USER_HOME/init.d/flutter2nix-offline.gradle"
    '';

  # Signs an Android App Bundle (.aab) with a release/upload key, OUTSIDE the
  # Nix build, so keystore material never enters the store and the offline
  # buildFlutterAndroidApp output stays pure and reproducible. Returns a runnable
  # writeShellApplication; pass the keystore + passwords via the environment:
  #
  #   KEYSTORE=upload.jks KEYSTORE_PASSWORD=… KEY_ALIAS=… KEY_PASSWORD=… \
  #     flutter2nix-sign-aab <unsigned.aab> <out.aab>
  #
  # AABs are signed with jarsigner using the upload key; Google Play App Signing
  # re-signs the delivered APKs with the app signing key, so no zipalign/apksigner
  # step is needed for the bundle uploaded to the Play Console.
  signAndroidAab =
    {
      pkgs,
      jdk ? pkgs.jdk21,
    }:
    pkgs.writeShellApplication {
      name = "flutter2nix-sign-aab";
      runtimeInputs = [ jdk ];
      text = ''
        in_aab="''${1:?usage: flutter2nix-sign-aab <unsigned.aab> <out.aab>}"
        out_aab="''${2:?usage: flutter2nix-sign-aab <unsigned.aab> <out.aab>}"
        : "''${KEYSTORE:?KEYSTORE (path to the keystore file) must be set}"
        : "''${KEYSTORE_PASSWORD:?KEYSTORE_PASSWORD must be set}"
        : "''${KEY_ALIAS:?KEY_ALIAS must be set}"
        : "''${KEY_PASSWORD:?KEY_PASSWORD must be set}"
        install -D "$in_aab" "$out_aab"
        jarsigner \
          -keystore "$KEYSTORE" \
          -storepass "$KEYSTORE_PASSWORD" \
          -keypass "$KEY_PASSWORD" \
          -sigalg SHA256withRSA -digestalg SHA-256 \
          "$out_aab" "$KEY_ALIAS"
        jarsigner -verify "$out_aab"
        echo "flutter2nix: signed $out_aab"
      '';
    };

in
# rec: buildFlutterAndroidApp recurses into itself to build the incremental
# native-shell variant.
rec {
  inherit
    buildGradleProject
    scopeEngineNodes
    wrapperGradleMajor
    defaultGradlePackage
    offlineGradleDevHook
    signAndroidAab
    ;

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
    {
      pkgs,
      name,
      src,
      lockFile,
      gradleTask ? "assembleRelease",
      gradleFlags ? [ ],
      jdk ? pkgs.jdk17,
      gradlePackage ? defaultGradlePackage pkgs src,
      androidSdk,
      consolidateMavenRepo ? false,
      # engineModes: see buildFlutterAndroidApp. A no-op for pure-Gradle locks
      # (no io.flutter engine nodes to scope); threaded for API consistency.
      engineModes ? null,
      ...
    }:
    let
      nodes = scopeEngineNodes engineModes (readNodes lockFile);
      mavenRepo = buildMavenRepo pkgs nodes consolidateMavenRepo;
      initScript = makeInitScript pkgs mavenRepo;
      allFlags = lib.concatStringsSep " " (
        [
          "--offline"
          "--no-daemon"
          "--no-configuration-cache"
          "--init-script"
          "${initScript}"
        ]
        ++ gradleFlags
      );
    in
    pkgs.stdenv.mkDerivation {
      inherit name src;
      buildInputs = [
        jdk
        gradlePackage
        androidSdk
      ];
      ANDROID_HOME = "${androidSdk}/libexec/android-sdk";
      ANDROID_SDK_ROOT = "${androidSdk}/libexec/android-sdk";
      JAVA_HOME = "${jdk}";
      # Surface the offline Maven repo (a *build* input, absent from the APK/AAB
      # runtime closure) so consumers can cache it on its own: `nix copy <app>`
      # only carries the runtime closure, so the ~GB network-fetched Maven layer
      # would otherwise never reach a binary cache. Caching mavenRepo lets a
      # downstream build substitute it instead of re-fetching the whole graph.
      passthru = {
        inherit mavenRepo initScript;
      };
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
  #   src             — Flutter app source (must contain pubspec.yaml, android/, lib/)
  #   name            — derivation name (default: the pubspec.yaml package name)
  #   lockFile        — flutter2nix.lock with android.nodes (default:
  #                     src/flutter2nix.lock, where `flutter2nix lock` writes it)
  #   pubspecLockFile — pubspec.lock from a real `flutter pub get` run (must record
  #                     hosted-package sha256 hashes). Default: src + "/pubspec.lock".
  #                     NOTE: converted to JSON at evaluation time (import-from-derivation).
  #   gitHashes       — hashes for git-sourced pub dependencies (pub does not record them)
  #   flutterSdk      — Flutter SDK package (default: pkgs.flutter)
  #   jdk             — JDK package (default: pkgs.jdk17)
  #   gradlePackage   — Gradle package; must match the wrapper version the lockfile
  #                     was captured with (default: autodetected from the app's
  #                     gradle-wrapper.properties via defaultGradlePackage)
  #   androidSdk      — Android SDK from androidenv.composeAndroidPackages { }.androidsdk
  #   flutterBuildArgs — extra args for `flutter build appbundle` (e.g. ["--flavor" "stag"])
  #   consolidateMavenRepo — default false (symlink artifacts → deduped closure).
  #                     Set true to copy the whole offline Maven repo into a single
  #                     store path so a cold CI runner substitutes it as one NAR
  #                     rather than ~2900 per-object cache GETs; the tradeoff is
  #                     lost dedup + incremental transfer (see buildMavenRepo).
  #   engineModes     — default null = keep the all-modes Flutter engine superset,
  #                     so one offline repo serves debug `flutter run` AND release
  #                     CI. Set to the build's actual engine mode(s) — e.g.
  #                     [ "release" ] for a release appbundle — to drop the
  #                     io.flutter engine variants this build never links (debug +
  #                     profile native .so are the bulk of a Flutter closure). This
  #                     is the primary cold-CI closure lever: pure selection over
  #                     the lockfile (the lockfile keeps the superset, artifact
  #                     bytes are untouched), so it has zero hash/hermeticity
  #                     impact. ABIs are not scoped — a release appbundle links
  #                     every release ABI — mode is the dominant axis. Leave null
  #                     for any repo reused across modes (e.g. an offlineGradleDevHook
  #                     dev shell that also debug-runs).
  buildFlutterAndroidApp =
    args@{
      pkgs,
      src,
      name ? pubLib.pubspecName src,
      lockFile ? src + "/flutter2nix.lock",
      pubspecLockFile ? src + "/pubspec.lock",
      gitHashes ? { },
      flutterSdk ? pkgs.flutter,
      jdk ? pkgs.jdk17,
      gradlePackage ? defaultGradlePackage pkgs src,
      androidSdk,
      flutterBuildArgs ? [ ],
      consolidateMavenRepo ? false,
      engineModes ? null,
      # "KEY=VALUE" --dart-define strings (mirrors buildFlutterIOSApp).
      dartDefines ? [ ],
      # Dart obfuscation + split-debug-info; symbol files surface as
      # $out/debug-symbols (see installPhase).
      obfuscate ? false,
      splitDebugInfo ? "build/debug-symbols",
      # Split the build into native-shell / dart-aot / assemble derivations so
      # Dart-only edits reuse the cached Gradle shell and only rerun the AOT
      # slice (docs/incremental-dart-builds.md). Sound because the AAB is
      # unsigned — signing happens out-of-build (signAndroidAab re-signs the
      # whole bundle). Do NOT enable if your Gradle build signs the bundle.
      incrementalDart ? false,
      # Dirs (relative to src) excluded from the native shell's inputs.
      nativeShellExcludes ? [
        "lib"
        "test"
        "assets"
      ],
      # Internal (split machinery): build this derivation as the native shell —
      # filtered src + stub libapp.so fed to Gradle directly.
      _dartStub ? false,
      _buildSrc ? null,
      ...
    }:
    let
      gradle = buildGradleProject {
        inherit
          pkgs
          lockFile
          jdk
          gradlePackage
          consolidateMavenRepo
          engineModes
          ;
      };

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
      buildSrc = if _buildSrc != null then _buildSrc else src;

      # --- incrementalDart split machinery (docs/incremental-dart-builds.md) ---
      filteredSrc =
        excludes:
        lib.fileset.toSource {
          root = src;
          fileset = lib.fileset.difference src (
            lib.fileset.unions (map (d: lib.fileset.maybeMissing (src + "/${d}")) excludes)
          );
        };

      # native-shell: the full Gradle bundle build with stub Dart artifacts,
      # keyed on everything EXCEPT nativeShellExcludes. Dart-tier knobs are
      # stripped so they don't key the shell either.
      nativeShell = buildFlutterAndroidApp (
        args
        // {
          name = "${name}-native-shell";
          incrementalDart = false;
          dartDefines = [ ];
          obfuscate = false;
          _dartStub = true;
          _buildSrc = filteredSrc nativeShellExcludes;
        }
      );

      # dart-aot: gen_snapshot per ABI + flutter_assets, with android/ and ios/
      # excluded from its inputs. Needs no Android SDK — the nixpkgs Flutter
      # SDK's gen_snapshot emits ELF libapp.so directly. Mirrors the `flutter
      # assemble` invocation the Flutter Gradle plugin's compileFlutterBuild
      # task makes (BaseFlutterTaskHelper.kt), minus -dMinSdkVersion, which no
      # android AOT/bundle target reads.
      dartAot = pkgs.stdenv.mkDerivation {
        name = "${name}-dart-aot";
        src = filteredSrc [
          "ios"
          "android"
        ];
        buildInputs = [
          flutterSdk
          pkgs.git
        ];
        meta.platforms = lib.platforms.linux;
        buildPhase = ''
          runHook preBuild
          export HOME="$NIX_BUILD_TOP"
          printf '[safe]\n\tdirectory = *\n' > "$HOME/.gitconfig"

          mkdir -p .dart_tool
          cp ${packageConfig} .dart_tool/package_config.json
          chmod u+w .dart_tool/package_config.json
          install -m644 ${pubspecLockFile} pubspec.lock
          ${pkgs.python3.withPackages (ps: [ ps.pyyaml ])}/bin/python3 \
            ${pkgs.path}/pkgs/build-support/dart/pub2nix/package-graph.py \
            > .dart_tool/package_graph.json
          rm -f .flutter-plugins-dependencies
          ${pkgs.python3.withPackages (ps: [ ps.pyyaml ])}/bin/python3 \
            ${./generate-flutter-plugins.py} "${flutterSdk.version}"
          ${pkgs.python3}/bin/python3 ${./check-flutter-sdk.py} "${flutterSdk}"

          # -dTreeShakeIcons=true: `flutter build appbundle` (the monolithic
          # path) tree-shakes icon fonts by default in release mode, so the
          # AOT tier must too or the swapped flutter_assets diverge.
          assemble_flags=(
            --no-version-check
            --output="$NIX_BUILD_TOP/aot"
            -dTargetFile=lib/main.dart
            -dTargetPlatform=android
            -dBuildMode=release
            -dTrackWidgetCreation=true
            -dTreeShakeIcons=true
            -dDartObfuscation=${if obfuscate then "true" else "false"}
            ${lib.optionalString obfuscate "-dSplitDebugInfo=${splitDebugInfo}"}
            -dAndroidArchs="android-arm android-arm64 android-x64"
          )
          dart_defines=(${lib.concatStringsSep " " (map lib.escapeShellArg dartDefines)})
          if [ "''${#dart_defines[@]}" -gt 0 ]; then
            encoded=""
            for d in "''${dart_defines[@]}"; do
              enc=$(printf '%s' "$d" | base64 | tr -d '\n')
              encoded="''${encoded:+$encoded,}$enc"
            done
            assemble_flags+=("--DartDefines=$encoded")
          fi

          flutter assemble "''${assemble_flags[@]}" \
            android_aot_bundle_release_android-arm \
            android_aot_bundle_release_android-arm64 \
            android_aot_bundle_release_android-x64
          runHook postBuild
        '';
        installPhase = ''
          runHook preInstall
          mkdir -p $out
          for abi in armeabi-v7a arm64-v8a x86_64; do
            cp -R "$NIX_BUILD_TOP/aot/$abi" $out/
          done
          cp -R "$NIX_BUILD_TOP/aot/flutter_assets" $out/
          ${lib.optionalString obfuscate ''
            if [ -n "$(find ${splitDebugInfo} -type f 2>/dev/null)" ]; then
              mkdir -p $out/debug-symbols
              cp -R ${splitDebugInfo}/. $out/debug-symbols/
            fi
          ''}
          runHook postInstall
        '';
      };

      # assemble: copy the shell output and swap the stub entries inside the
      # AAB (a zip) for the real dart-aot artifacts. Sound only because the
      # AAB is unsigned — signing happens out-of-build (signAndroidAab).
      assembled =
        pkgs.runCommand name
          {
            nativeBuildInputs = [ pkgs.zip ];
            meta.platforms = lib.platforms.linux;
            passthru = {
              inherit (gradle) mavenRepo initScript;
              inherit packageConfig nativeShell dartAot;
              offlineDeps = nativeShell.offlineDeps;
            };
          }
          ''
            mkdir -p $out
            cp -R ${nativeShell}/. $out/
            chmod -R u+w $out
            aab=$(find $out -maxdepth 1 -name '*.aab' | head -n1)
            if [ -z "$aab" ]; then
              echo "flutter2nix incrementalDart: no .aab in the native-shell output" >&2
              exit 1
            fi

            # Stage the real Dart artifacts under the AAB's entry paths.
            staging="$NIX_BUILD_TOP/staging"
            for abi in armeabi-v7a arm64-v8a x86_64; do
              if [ -e "${dartAot}/$abi/app.so" ]; then
                install -D -m644 "${dartAot}/$abi/app.so" "$staging/base/lib/$abi/libapp.so"
              fi
            done
            mkdir -p "$staging/base/assets"
            cp -R ${dartAot}/flutter_assets "$staging/base/assets/flutter_assets"
            chmod -R u+w "$staging"
            # zip refuses pre-1980 mtimes (Nix store files sit at the epoch);
            # -X keeps the bumped timestamps out of the archive entries anyway.
            find "$staging" -exec touch -t 198001010000 {} +

            # The stub flutter_assets entry set differs from the real one:
            # delete the stale tree, then add/overwrite in place. libapp.so
            # entries are same-name and simply overwritten. No zipalign
            # concerns — alignment applies to the APKs bundletool derives
            # later, not to the AAB itself.
            zip -q -d "$aab" 'base/assets/flutter_assets/*' || true
            (cd "$staging" && zip -q -X -r "$aab" base)

            ${lib.optionalString obfuscate ''
              if [ -d "${dartAot}/debug-symbols" ]; then
                rm -rf "$out/debug-symbols"
                mkdir -p "$out/debug-symbols"
                cp -R "${dartAot}/debug-symbols/." "$out/debug-symbols/"
              fi
            ''}
          '';

      monolithic = pkgs.stdenv.mkDerivation {
      inherit name;
      src = buildSrc;
      # git: flutter_tools requires it on PATH at startup. The nixpkgs Flutter
      # wrapper bundles its own; raw Google-tarball SDKs do not.
      buildInputs = gradle.buildInputs ++ [
        flutterSdk
        androidSdk
        pkgs.git
      ];
      ANDROID_HOME = "${androidSdk}/libexec/android-sdk";
      ANDROID_SDK_ROOT = "${androidSdk}/libexec/android-sdk";
      JAVA_HOME = "${jdk}";
      meta.platforms = lib.platforms.linux;
      # Surface the network-bound offline layers so consumers can cache them
      # independently of the app output. They are *build* inputs of this
      # derivation — absent from the AAB's runtime closure — so a plain
      # `nix copy <app>` never carries the ~GB Maven graph or the pub package
      # set, and a downstream CI rebuild re-fetches both from the network. These
      # are the exact derivations the build consumes, so caching them lets the
      # build substitute them instead of rebuilding/re-fetching.
      passthru = {
        inherit (gradle) mavenRepo initScript;
        inherit packageConfig;
        # A single derivation whose runtime closure unions every offline layer
        # (Maven repo + pub packages): cache this one path to prime both at once.
        offlineDeps = pkgs.linkFarm "${name}-offline-deps" [
          {
            name = "maven-repo";
            path = gradle.mavenRepo;
          }
          {
            name = "package-config";
            path = packageConfig;
          }
        ];
      };
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
                # Raw-tarball Flutter SDKs keep their .git, and git refuses repos owned
                # by another user ("dubious ownership") — which is every store path in
                # the sandbox. Trust everything within this throwaway HOME (same
                # workaround as buildFlutterIOSApp).
                printf '[safe]\n\tdirectory = *\n' > "$HOME/.gitconfig"
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
                # Give the Gradle JVM a generous, deterministic heap. AGP's
                # FinalizeBundleTask runs bundletool in-process, and packaging the
                # release AAB (the fixture's intermediary bundle is ~90MB) routinely
                # exceeds Gradle's ~512MB default heap. With the default it OOMs
                # intermittently — fine when a build runs alone, but flaky under the
                # CPU contention of a parallel `nix build .#e2e` (GC gets starved and
                # tips into "Java heap space" at :app:signReleaseBundle /
                # FinalizeBundleTask$BundleToolRunnable). A fixed 4g heap removes the
                # flakiness; it also covers Kotlin in-process compilation above.
                echo "org.gradle.jvmargs=-Xmx4g" >> "$GRADLE_USER_HOME/gradle.properties"
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
                # Gradle 9+ requires every included project's projectDirectory to be
                # writable, but android plugin paths point into the read-only store.
                # Copy android plugin packages to a writable dir and rewrite their
                # entries in .flutter-plugins-dependencies.
                ${pkgs.python3}/bin/python3 ${./relocate-plugins.py} \
                  .flutter-plugins-dependencies "$NIX_BUILD_TOP/plugin-copies"
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
                ${
                  if _dartStub then
                    ''
                      # Incremental native shell: deterministic stub Dart artifacts in the
                      # intermediate dir the Flutter Gradle plugin reads AOT output from,
                      # then Gradle driven directly with the Dart compile task excluded.
                      # The assemble derivation swaps the stubs for real dart-aot output.
                      ndk_bin="$(find -L "$ANDROID_SDK_ROOT/ndk" -path '*/toolchains/llvm/prebuilt/*/bin' -type d 2>/dev/null | head -n1)"
                      if [ -z "$ndk_bin" ]; then
                        echo "ERROR: incrementalDart needs an NDK under $ANDROID_SDK_ROOT/ndk to build the stub libapp.so" >&2
                        exit 1
                      fi
                      printf 'const unsigned char kFlutter2nixStubApp = 1;\n' > "$NIX_BUILD_TOP/stub.c"
                      flutter_intermediate=build/app/intermediates/flutter/release
                      for tgt in aarch64-linux-android21:arm64-v8a armv7a-linux-androideabi21:armeabi-v7a x86_64-linux-android21:x86_64; do
                        abi="''${tgt##*:}"
                        mkdir -p "$flutter_intermediate/$abi"
                        "$ndk_bin/clang" --target="''${tgt%%:*}" -shared \
                          -o "$flutter_intermediate/$abi/app.so" "$NIX_BUILD_TOP/stub.c"
                      done
                      # Marker so a stub that accidentally ships is identifiable.
                      mkdir -p "$flutter_intermediate/flutter_assets"
                      touch "$flutter_intermediate/flutter_assets/flutter2nix-stub"
                      # flutter_tools normally stamps the pubspec version into
                      # local.properties for the Gradle plugin; driving Gradle
                      # directly means doing it here.
                      version_line="$(sed -n 's/^version:[[:space:]]*//p' pubspec.yaml | head -n1 | tr -d '[:space:]')"
                      version_name="''${version_line%%+*}"
                      version_code="''${version_line##*+}"
                      [ -n "$version_name" ] || version_name=1.0.0
                      { [ -n "$version_code" ] && [ "$version_code" != "$version_line" ]; } || version_code=1
                      printf 'flutter.versionName=%s\nflutter.versionCode=%s\n' \
                        "$version_name" "$version_code" >> android/local.properties
                      # ponytail: assumes the template module name :app; parameterize
                      # only if a consumer ever renames the module.
                      (cd android && ./gradlew :app:bundleRelease \
                        -x :app:compileFlutterBuildRelease \
                        -Ptarget-platform=android-arm,android-arm64,android-x64)
                    ''
                  else
                    ''
                      flutter build appbundle --no-pub ${
                        lib.escapeShellArgs (
                          flutterBuildArgs
                          ++ map (d: "--dart-define=${d}") dartDefines
                          ++ lib.optionals obfuscate [
                            "--obfuscate"
                            "--split-debug-info=${splitDebugInfo}"
                          ]
                        )
                      }
                    ''
                }
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
        # Deploy-time deobfuscation/symbolication upload artifacts. These are NOT
        # part of the app's runtime closure and are emitted only when the relevant
        # build flags are set, so every copy below is guarded and never fails an
        # ordinary (non-minified / non-obfuscated) build:
        #   * mapping/        — R8's mapping.txt (+ seeds/usage/configuration/
        #                       resources .txt) from build/app/outputs/mapping/
        #                       <flavor>Release/. Uploaded to the Play Console and
        #                       Crashlytics so release crash stacks de-minify.
        #                       Present only when isMinifyEnabled (R8) is on.
        #   * debug-symbols/  — Dart split-debug-info symbol files
        #                       (app.android-*.symbols) for `flutter symbolize` of
        #                       obfuscated Dart stack traces. Present only when the
        #                       build passes --obfuscate --split-debug-info=<dir>.
        # The AAB/APK-at-$out-root layout above is left untouched (CI greps
        # result/ for *.aab); these land in dedicated subdirs alongside it.
        mappingDir="$(find . -type d -ipath "*outputs/mapping/*release*" 2>/dev/null | head -n1)"
        if [ -n "$mappingDir" ] && [ -d "$mappingDir" ]; then
          mkdir -p $out/mapping
          cp -R "$mappingDir"/. $out/mapping/
        fi
        # --split-debug-info=${splitDebugInfo} writes app.android-*.symbols
        # there, relative to the flutter build cwd (the unpacked source root),
        # which is also the installPhase cwd.
        if [ -n "$(find ${splitDebugInfo} -type f 2>/dev/null)" ]; then
          mkdir -p $out/debug-symbols
          cp -R ${splitDebugInfo}/. $out/debug-symbols/
        fi
        runHook postInstall
      '';
      };
    in
    if incrementalDart then assembled else monolithic;
}
