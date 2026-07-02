#!/bin/bash
# flutter2nix xcode_backend shim (incremental Dart-only builds).
#
# Installed as packages/flutter_tools/bin/xcode_backend.sh in a symlink view of
# the real Flutter SDK; the native-shell derivation points Generated.xcconfig's
# FLUTTER_ROOT at that view and sets FLUTTER2NIX_DART_STUB=true (xcconfig build
# settings are exported into Xcode run-script environments). In that mode the
# "build" command — normally the full Dart AOT `flutter assemble` — is replaced
# by a deterministic stub App.framework, so the shell derivation never depends
# on lib/. Every other command, and every build without the flag, delegates to
# the real SDK's xcode_backend.sh.
set -euo pipefail

real="@flutterSdk@/packages/flutter_tools/bin/xcode_backend.sh"

if [[ "${FLUTTER2NIX_DART_STUB:-}" != "true" || "${1:-}" != "build" ]]; then
  exec /bin/bash "$real" "$@"
fi

# Runner links Flutter.framework out of BUILT_PRODUCTS_DIR, where the normal
# "build" would have left it. "prepare" runs exactly the no-Dart slice of that
# (`flutter assemble <mode>_unpack_ios`).
/bin/bash "$real" prepare

# Stub App.framework: a structurally valid Mach-O dylib (right install name,
# Info.plist, one exported symbol) so the embed/thin phases and any arch checks
# succeed. The assemble derivation swaps in the real one afterwards.
fw="$BUILT_PRODUCTS_DIR/App.framework"
rm -rf "$fw"
mkdir -p "$fw/flutter_assets"

arch_flags=()
for arch in $ARCHS; do arch_flags+=(-arch "$arch"); done

stub_dir=$(mktemp -d)
printf 'const unsigned char kFlutter2nixStubApp = 1;\n' > "$stub_dir/stub.c"
xcrun clang -dynamiclib "${arch_flags[@]}" \
  -miphoneos-version-min="${IPHONEOS_DEPLOYMENT_TARGET:-12.0}" \
  -isysroot "$SDKROOT" \
  -install_name '@rpath/App.framework/App' \
  -o "$fw/App" "$stub_dir/stub.c"
rm -rf "$stub_dir"

cat > "$fw/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleDevelopmentRegion</key>
	<string>en</string>
	<key>CFBundleExecutable</key>
	<string>App</string>
	<key>CFBundleIdentifier</key>
	<string>io.flutter.flutter.app</string>
	<key>CFBundleInfoDictionaryVersion</key>
	<string>6.0</string>
	<key>CFBundleName</key>
	<string>App</string>
	<key>CFBundlePackageType</key>
	<string>FMWK</string>
	<key>CFBundleShortVersionString</key>
	<string>1.0</string>
	<key>CFBundleVersion</key>
	<string>1.0</string>
	<key>MinimumOSVersion</key>
	<string>12.0</string>
</dict>
</plist>
PLIST

# Marker so a stub that accidentally ships (assemble not run) is identifiable.
touch "$fw/flutter_assets/flutter2nix-stub"
