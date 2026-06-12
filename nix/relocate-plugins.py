"""Relocate Android plugin packages to writable copies for Gradle 9.

Gradle 9+ validates that every included project's projectDirectory exists,
is a directory, AND is writable. Flutter's Gradle plugin sets each plugin's
projectDir to <package>/android using the paths recorded in
.flutter-plugins-dependencies — which, in a hermetic build, point into the
read-only Nix store, so configuration fails with "Configuring project ':x'
without an existing directory is not allowed ... can't be written to".

Copy each android-listed plugin package into a writable directory, lift the
store's read-only file modes, and rewrite the android entries to the copies.
iOS (and other platform) entries keep their store paths: CocoaPods and the
Dart compiler only read them.

Usage: relocate-plugins.py [deps-file] [dest-dir]
"""
import json
import os
import shutil
import stat
import sys

deps_path = sys.argv[1] if len(sys.argv) > 1 else ".flutter-plugins-dependencies"
dest_root = sys.argv[2] if len(sys.argv) > 2 else ".flutter2nix-plugin-copies"

try:
    deps = json.load(open(deps_path))
except FileNotFoundError:
    sys.exit(0)

android = deps.get("plugins", {}).get("android", [])
if not android:
    sys.exit(0)

os.makedirs(dest_root, exist_ok=True)
relocated = []
for entry in android:
    src = entry["path"].rstrip("/")
    dst = os.path.join(dest_root, entry["name"])
    if not os.path.isdir(dst):
        shutil.copytree(src, dst, symlinks=True)
        # Lift the store's read-only modes: Gradle needs the project dir
        # writable, and plugin builds may write intermediates next to sources.
        for root, dirs, files in os.walk(dst):
            for name in dirs + files:
                p = os.path.join(root, name)
                os.chmod(p, os.stat(p).st_mode | stat.S_IWUSR)
        os.chmod(dst, os.stat(dst).st_mode | stat.S_IWUSR)
    entry["path"] = os.path.abspath(dst) + "/"
    relocated.append(entry["name"])

with open(deps_path, "w") as f:
    json.dump(deps, f, indent=2)

print(f"flutter2nix: relocated android plugins to writable copies: {sorted(relocated)}")
