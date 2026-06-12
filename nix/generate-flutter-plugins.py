"""Generate .flutter-plugins-dependencies hermetically for nix builds.

flutter_tools writes this file during `flutter pub get` with developer-machine
absolute paths (pub cache, Flutter SDK). It is gitignored, so a clean-checkout
nix build has no copy — but the Flutter Gradle plugin loader and CocoaPods
podhelper both require it. Synthesize it from inputs the build already has:

  .dart_tool/package_config.json  — package name -> Nix store root (pub2nix)
  <package root>/pubspec.yaml     — flutter.plugin.platforms declarations
  pubspec.lock                    — `dependency: direct dev` -> dev_dependency

Usage: generate-flutter-plugins.py <flutter-version>
Run from the Flutter project root. Writes ./.flutter-plugins-dependencies.

dev_dependency note: flutter_tools marks a plugin as dev when it is not in the
transitive closure of main dependencies. pubspec.lock's `dependency` field only
distinguishes direct main/direct dev/transitive, so direct-dev is used here.
This matches the overwhelmingly common case (integration_test); a transitive
dev-only plugin would be conservatively treated as a main dependency.
"""

import json
import os
import sys
from datetime import datetime, timezone
from urllib.parse import unquote, urlparse

import yaml

PLATFORMS = ["ios", "android", "macos", "linux", "windows", "web"]


def package_roots():
    cfg = json.load(open(".dart_tool/package_config.json"))
    roots = {}
    for p in cfg["packages"]:
        uri = p["rootUri"]
        if uri.startswith("file://"):
            root = unquote(urlparse(uri).path)
        elif uri.startswith("../"):
            # Relative to .dart_tool/
            root = os.path.normpath(os.path.join(".dart_tool", uri))
        else:
            continue
        roots[p["name"]] = root.rstrip("/")
    return roots


def load_yaml(path):
    try:
        with open(path) as f:
            return yaml.safe_load(f) or {}
    except FileNotFoundError:
        return {}


def dev_dependencies():
    lock = load_yaml("pubspec.lock")
    return {
        name
        for name, entry in (lock.get("packages") or {}).items()
        if entry.get("dependency") == "direct dev"
    }


def _implements(decl):
    """A package implements a platform if it declares real plugin code there.

    Pure `default_package` redirects (e.g. app_links -> app_links_linux) belong
    to the referenced package, which declares its own platform entry.
    """
    if not isinstance(decl, dict):
        return False
    return bool(
        (decl.get("pluginClass") and decl.get("pluginClass") != "none")
        or decl.get("dartPluginClass")
        or decl.get("ffiPlugin")
    )


def main():
    flutter_version = sys.argv[1] if len(sys.argv) > 1 else "unknown"
    roots = package_roots()
    dev = dev_dependencies()

    # name -> (root, pubspec dict, platforms dict) for every flutter plugin
    plugins = {}
    for name, root in roots.items():
        spec = load_yaml(os.path.join(root, "pubspec.yaml"))
        platforms = ((spec.get("flutter") or {}).get("plugin") or {}).get("platforms")
        if platforms:
            plugins[name] = (root, spec, platforms)

    def plugin_deps(spec, member_set):
        return sorted(d for d in (spec.get("dependencies") or {}) if d in member_set)

    out_platforms = {}
    for platform in PLATFORMS:
        members = {
            name
            for name, (_, _, platforms) in plugins.items()
            if _implements(platforms.get(platform))
        }
        entries = []
        for name in sorted(members):
            root, spec, platforms = plugins[name]
            decl = platforms[platform]
            entry = {"name": name, "path": root + "/"}
            if decl.get("sharedDarwinSource"):
                entry["shared_darwin_source"] = True
            if platform != "web":
                entry["native_build"] = bool(
                    decl.get("pluginClass") and decl.get("pluginClass") != "none"
                ) or bool(decl.get("ffiPlugin"))
            entry["dependencies"] = plugin_deps(spec, members)
            entry["dev_dependency"] = name in dev
            entries.append(entry)
        out_platforms[platform] = entries

    graph = [
        {"name": name, "dependencies": plugin_deps(plugins[name][1], set(plugins))}
        for name in sorted(plugins)
    ]

    # Deterministic under nix: derive from SOURCE_DATE_EPOCH (epoch in sandbox).
    epoch = int(os.environ.get("SOURCE_DATE_EPOCH", "0"))
    date_created = datetime.fromtimestamp(epoch, tz=timezone.utc).strftime(
        "%Y-%m-%d %H:%M:%S.%f"
    )

    result = {
        "info": "This is a generated file; do not edit or check into version control.",
        "plugins": out_platforms,
        "dependencyGraph": graph,
        "date_created": date_created,
        "version": flutter_version,
        "swift_package_manager_enabled": {"ios": False, "macos": False},
    }
    with open(".flutter-plugins-dependencies", "w") as f:
        json.dump(result, f)
    counts = {p: len(v) for p, v in out_platforms.items() if v}
    print(f"flutter2nix: generated .flutter-plugins-dependencies ({counts})")


if __name__ == "__main__":
    main()
