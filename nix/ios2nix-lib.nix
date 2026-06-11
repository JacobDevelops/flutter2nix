# ios2nix Nix library functions.
# Mirrors the gradle2nix-lib.nix structure: readPods, buildPodsSandbox, and buildIOSApp.
{ lib }:

let
  # Reads the pods lockfile in DependencyGraph format.
  # Supports both ios.nodes (flutter2nix.lock format) and nodes (standalone format).
  readPods = lockFile:
    let
      lock = builtins.fromJSON (builtins.readFile lockFile);
    in
    if lock ? ios then lock.ios.nodes
    else if lock ? nodes then lock.nodes
    else throw "ios2nix-lib: unrecognized lockfile format in ${toString lockFile}";

  # Splits git+<url>#<rev> URLs into { url; rev; } components.
  # Uses greedy (.*) to capture the last '#' in the URL.
  # Throws on non-matching input.
  splitGitUrl = url:
    let
      m = builtins.match "git\\+(.*)#([^#]*)" url;
    in
    if m != null then
      {
        url = builtins.elemAt m 0;
        rev = builtins.elemAt m 1;
      }
    else throw "ios2nix-lib: invalid git URL format '${url}'";

  # Builds an offline CocoaPods sandbox by fetching all pods and laying them out
  # in a store tree mirroring CocoaPods' expected structure.
  # Each pod is fetched by its locked sha256; git pods use fetchgit, others use fetchurl.
  # Subspecs (names containing '/') are laid out at the full path (e.g., Firebase/Auth)
  # where they naturally nest under the root pod — this is acceptable because subspecs
  # share the root pod's source anyway.
  buildPodsSandbox = pkgs: nodes:
    let
      fetchPod = node:
        if lib.hasPrefix "git+" node.url
        then
          let m = splitGitUrl node.url;
          in pkgs.fetchgit { url = m.url; rev = m.rev; sha256 = node.sha256; }
        else pkgs.fetchurl { url = node.url; sha256 = node.sha256; };

      entries = map (n: { inherit (n) name version; src = fetchPod n; }) nodes;

      installCmds = lib.concatMapStrings (e: ''
        mkdir -p "$out/pods/${e.name}"
        ln -s ${e.src} "$out/pods/${e.name}/${e.version}"
      '') entries;
    in
    pkgs.runCommand "ios-pods-sandbox" { } ''
      # Always create the base tree — a pod-less lockfile is a valid (empty) sandbox.
      mkdir -p "$out/pods"
      ${installCmds}
    '';

  # Full derivation that builds an iOS app (archive + optional export, with optional signing).
  # Signing is impure — it depends on Apple network reachability, keychain state,
  # installed provisioning profiles, and embedded timestamps, none of which are
  # content-addressed. Only the pod inputs (fetched by hash) are deterministic;
  # the `.ipa` is not bit-reproducible.
  #
  # Parameters:
  #   pkgs            — nixpkgs attribute set
  #   name            — derivation name
  #   src             — iOS app source (must contain ios/ with Podfile and Xcode project)
  #   lockFile        — iOS dependencies lockfile (DependencyGraph JSON format)
  #   workspace       — Xcode workspace path relative to the (ios/) source root
  #   scheme          — Xcode scheme name (default: "Runner" for Flutter apps)
  #   configuration   — Build configuration (default: "Release")
  #   exportOptions   — Path to ExportOptions.plist for xcodebuild -exportArchive
  #   signing         — null (unsigned) or an attrset with { teamId, identity, profileSpecifier, ios2nix? }
  #                      For signed builds, signing material is read from the impure environment at
  #                      build time (IOS2NIX_P12_PATH, IOS2NIX_P12_PASSWORD, IOS2NIX_PROFILE_PATH,
  #                      IOS2NIX_KEYCHAIN_PASSWORD), NEVER from the Nix store. The ios2nix bin is
  #                      invoked to set up the temporary keychain and partition list; the archive and
  #                      export use manual-signing flags with the keychain path from setup.
  #   ...             — additional mkDerivation attributes
  buildIOSApp =
    { pkgs
    , name
    , src
    , lockFile
    , workspace ? "Runner.xcworkspace"
    , scheme ? "Runner"
    , configuration ? "Release"
    , exportOptions
    , signing ? null
    , ...
    }:
    pkgs.stdenv.mkDerivation {
      inherit name src;
      __noChroot = true;
      meta.platforms = lib.platforms.darwin;
      buildInputs = [ pkgs.cocoapods ]
        ++ lib.optionals (signing != null) [ (signing.ios2nix or pkgs.ios2nix) ];
      buildPhase = ''
        runHook preBuild
        podsSandbox=${buildPodsSandbox pkgs (readPods lockFile)}

        if [ -d ios ]; then
          cd ios
        fi

        # Make the hash-fetched pod sources visible to pod install. Proper
        # CocoaPods cache seeding (CDN spec cache + download cache, spike
        # Finding 2) lands with the first active e2e fixture.
        mkdir -p Pods
        ln -s "$podsSandbox/pods"/* Pods/ 2>/dev/null || true

        ${pkgs.cocoapods}/bin/pod install --no-repo-update

        # If signing is requested, set up the temporary keychain and partition list.
        # The ios2nix CLI creates a temp keychain, imports the signing cert, sets
        # set-key-partition-list so codesign works non-interactively, and prints the
        # keychain path on stdout for use in the archive step.
        ${lib.optionalString (signing != null) ''
          IOS2NIX_KEYCHAIN_PATH=$(ios2nix sign-setup \
            --p12 "$IOS2NIX_P12_PATH" \
            --profile "$IOS2NIX_PROFILE_PATH")
          export IOS2NIX_KEYCHAIN_PATH
          # Ensure cleanup even if archive/export fails.
          trap '[ -n "''${IOS2NIX_KEYCHAIN_PATH}" ] && security delete-keychain "''${IOS2NIX_KEYCHAIN_PATH}" 2>/dev/null || true' EXIT
        ''}

        # xcodebuild must never see the Nix toolchain env (CC/LD/SDKROOT/
        # DEVELOPER_DIR/NIX_* — spike Finding 4): run it under env -i with the
        # system PATH; /usr/bin/xcodebuild dispatches through xcode-select.
        ${if signing != null then ''
          env -i HOME="$TMPDIR" PATH=/usr/bin:/bin:/usr/sbin:/sbin \
            IOS2NIX_KEYCHAIN_PATH="$IOS2NIX_KEYCHAIN_PATH" \
            xcodebuild archive \
            -workspace ${workspace} \
            -scheme ${scheme} \
            -configuration ${configuration} \
            -archivePath "$TMPDIR/app.xcarchive" \
            -destination 'generic/platform=iOS' \
            DEVELOPMENT_TEAM="${signing.teamId}" \
            CODE_SIGN_STYLE=Manual \
            CODE_SIGN_IDENTITY="${signing.identity}" \
            PROVISIONING_PROFILE_SPECIFIER="${signing.profileSpecifier}" \
            OTHER_CODE_SIGN_FLAGS="--keychain $IOS2NIX_KEYCHAIN_PATH"
        '' else ''
          env -i HOME="$TMPDIR" PATH=/usr/bin:/bin:/usr/sbin:/sbin \
            xcodebuild archive \
            -workspace ${workspace} \
            -scheme ${scheme} \
            -configuration ${configuration} \
            -archivePath "$TMPDIR/app.xcarchive" \
            -destination 'generic/platform=iOS' \
            CODE_SIGNING_ALLOWED=NO
        ''}

        # Export the archive with the provided ExportOptions.plist.
        # For signed builds, the export will perform code signing; failure fails the build.
        # For unsigned builds, export may fail due to missing team info, so we allow it.
        env -i HOME="$TMPDIR" PATH=/usr/bin:/bin:/usr/sbin:/sbin \
          ${lib.optionalString (signing != null) ''IOS2NIX_KEYCHAIN_PATH="$IOS2NIX_KEYCHAIN_PATH"''} \
          xcodebuild -exportArchive \
          -archivePath "$TMPDIR/app.xcarchive" \
          -exportOptionsPlist ${exportOptions} \
          -exportPath "$TMPDIR/export" ${lib.optionalString (signing == null) "|| true"}

        runHook postBuild
      '';
      installPhase = ''
        runHook preInstall
        mkdir -p $out

        for ipa in "$TMPDIR"/export/*.ipa; do
          [ -e "$ipa" ] && cp "$ipa" $out/
        done

        # Archive-only fallback for unsigned builds when export fails.
        if [ -d "$TMPDIR/app.xcarchive" ]; then
          cp -R "$TMPDIR/app.xcarchive" $out/app.xcarchive
        fi

        runHook postInstall
      '';
    };

in
{
  inherit readPods splitGitUrl buildPodsSandbox buildIOSApp;
}
