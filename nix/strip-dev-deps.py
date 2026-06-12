"""Remove dev-dependency plugin registrations from GeneratedPluginRegistrant.java.

flutter build --no-pub skips the release-mode regeneration of
GeneratedPluginRegistrant.java, so dev-only plugins (e.g. integration_test)
remain registered.  The Flutter Gradle plugin excludes them from release
variants, causing javac "package does not exist" errors.  This script strips
them, mirroring flutter's own release-mode behaviour.
"""
import json
import re
import sys

deps_path = sys.argv[1] if len(sys.argv) > 1 else ".flutter-plugins-dependencies"
reg_path = (
    sys.argv[2]
    if len(sys.argv) > 2
    else "android/app/src/main/java/io/flutter/plugins/GeneratedPluginRegistrant.java"
)

try:
    deps = json.load(open(deps_path))
except FileNotFoundError:
    sys.exit(0)

dev = [
    p["name"]
    for p in deps.get("plugins", {}).get("android", [])
    if p.get("dev_dependency")
]
if not dev:
    sys.exit(0)

try:
    src = open(reg_path).read()
except FileNotFoundError:
    sys.exit(0)

for name in dev:
    src = re.sub(
        r"    try \{\n[^\n]*\n    \} catch \(Exception e\) \{\n[^\n]*Error registering plugin "
        + re.escape(name)
        + r",[^\n]*\n    \}\n",
        "",
        src,
    )

open(reg_path, "w").write(src)
print(f"flutter2nix: stripped dev-dependency plugins from registrant: {dev}")
