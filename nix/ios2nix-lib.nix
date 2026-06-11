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
      ${installCmds}
    '';

  # Full derivation that builds an unsigned iOS app (archive + optional export).
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
    , ...
    }:
    pkgs.stdenv.mkDerivation {
      inherit name src;
      __noChroot = true;
      meta.platforms = lib.platforms.darwin;
      buildInputs = [ pkgs.cocoapods ];
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

        # xcodebuild must never see the Nix toolchain env (CC/LD/SDKROOT/
        # DEVELOPER_DIR/NIX_* — spike Finding 4): run it under env -i with the
        # system PATH; /usr/bin/xcodebuild dispatches through xcode-select.
        env -i HOME="$TMPDIR" PATH=/usr/bin:/bin:/usr/sbin:/sbin \
          xcodebuild archive \
          -workspace ${workspace} \
          -scheme ${scheme} \
          -configuration ${configuration} \
          -archivePath "$TMPDIR/app.xcarchive" \
          -destination 'generic/platform=iOS' \
          CODE_SIGNING_ALLOWED=NO

        # Unsigned export fails upstream ("No Team Found in Archive", spike
        # Phase -1.5); the archive below is the supported Plan 2 output and
        # Plan 3's signed plist turns this step functional.
        env -i HOME="$TMPDIR" PATH=/usr/bin:/bin:/usr/sbin:/sbin \
          xcodebuild -exportArchive \
          -archivePath "$TMPDIR/app.xcarchive" \
          -exportOptionsPlist ${exportOptions} \
          -exportPath "$TMPDIR/export" || true

        runHook postBuild
      '';
      installPhase = ''
        runHook preInstall
        mkdir -p $out

        for ipa in "$TMPDIR"/export/*.ipa; do
          [ -e "$ipa" ] && cp "$ipa" $out/
        done

        # Archive-only fallback until Plan 3 supplies signing for the export.
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
