"""Preflight: verify a Flutter SDK can run `flutter build` offline.

flutter_tools' PubDependencies artifact (flutter_cache.dart) checks
<sdk>/packages/flutter_tools/.dart_tool/package_config.json before every
build — even with --no-pub. If the file is missing, lists no packages, or
lists a package whose root has no pubspec.yaml, flutter_tools runs an ONLINE
`pub get` for itself, which dies in the Nix sandbox with an opaque
"Got socket error trying to find package <name> at https://pub.dev".

SDKs built by nixpkgs pre-resolve the tool's dependencies; SDKs repackaged
from the raw Google release tarball do not. This script mirrors the
isUpToDate logic exactly so the build fails immediately with an actionable
message instead.

Usage: check-flutter-sdk.py <flutter-sdk-root>
"""
import json
import os
import sys
from urllib.parse import unquote, urlparse

sdk = sys.argv[1]
config_path = os.path.join(
    sdk, "packages", "flutter_tools", ".dart_tool", "package_config.json"
)

FIX_HINT = (
    "flutter2nix: the Flutter SDK at %s cannot build offline:\n"
    "  %%s\n"
    "  flutter_tools' PubDependencies check will run an online `pub get` for\n"
    "  the tool itself (ignoring --no-pub), which the Nix sandbox blocks.\n"
    "  Fix the SDK derivation: write a package_config.json listing at least\n"
    "  flutter_tools itself, e.g.\n"
    '    {"configVersion": 2, "packages": [{"name": "flutter_tools",\n'
    '     "rootUri": "../", "packageUri": "lib/", "languageVersion": "3.10"}]}\n'
    "  into packages/flutter_tools/.dart_tool/ (nixpkgs-built SDKs ship a\n"
    "  fully resolved one)." % sdk
)


def fail(reason):
    print(FIX_HINT % reason, file=sys.stderr)
    sys.exit(1)


try:
    config = json.load(open(config_path))
except FileNotFoundError:
    fail("packages/flutter_tools/.dart_tool/package_config.json is missing")
except ValueError:
    fail("packages/flutter_tools/.dart_tool/package_config.json is not valid JSON")

packages = config.get("packages", [])
if not packages:
    fail(
        "package_config.json lists no packages "
        "(flutter_tools rejects an empty PackageConfig)"
    )

config_dir = os.path.dirname(config_path)
for pkg in packages:
    uri = pkg.get("rootUri", "")
    if uri.startswith("file://"):
        root = unquote(urlparse(uri).path)
    else:
        root = os.path.normpath(os.path.join(config_dir, unquote(uri)))
    if not os.path.isfile(os.path.join(root, "pubspec.yaml")):
        fail(
            "package %r in package_config.json has no pubspec.yaml at its "
            "root (%s)" % (pkg.get("name"), root)
        )

print("flutter2nix: Flutter SDK preflight OK (flutter_tools deps pre-resolved)")
