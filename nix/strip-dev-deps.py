"""Remove dev-dependency plugin registrations from the Flutter build.

flutter build --no-pub skips the release-mode regeneration of both
.flutter-plugins-dependencies and GeneratedPluginRegistrant.java, so
dev-only plugins (e.g. integration_test) remain in both files.  This
causes two separate failures:

1. Flutter's Gradle plugin reads .flutter-plugins-dependencies and runs
   pub for each listed plugin to resolve its deps.  integration_test (an
   sdk-source package) depends on the `test` pub package, which isn't in
   pubspec.lock — the Nix sandbox blocks the resulting pub.dev request.

2. javac fails with "package does not exist" because the stripped plugin
   isn't present in release variants.

Both are fixed by mirroring flutter's own release-mode behaviour: strip
dev-only entries from .flutter-plugins-dependencies first, then strip the
corresponding registrations from GeneratedPluginRegistrant.java.
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

dev = {
    p["name"]
    for p in deps.get("plugins", {}).get("android", [])
    if p.get("dev_dependency")
}
if not dev:
    sys.exit(0)

# --- strip dev-dep entries from .flutter-plugins-dependencies ---
# Flutter's Gradle plugin reads this file and runs pub for each listed
# plugin.  Leaving dev plugins here causes pub to attempt resolving their
# transitive deps (e.g. integration_test -> test), which are absent from
# pubspec.lock and unreachable in the Nix sandbox.
for platform in deps.get("plugins", {}):
    deps["plugins"][platform] = [
        p for p in deps["plugins"][platform] if p["name"] not in dev
    ]

# Also remove dev deps from the dependencyGraph to keep the file consistent.
deps["dependencyGraph"] = [
    entry for entry in deps.get("dependencyGraph", []) if entry["name"] not in dev
]

with open(deps_path, "w") as f:
    json.dump(deps, f, indent=2)

# --- strip dev-dep registrations from GeneratedPluginRegistrant.java ---
try:
    src = open(reg_path).read()
except FileNotFoundError:
    print(f"flutter2nix: stripped dev-dependency plugins from .flutter-plugins-dependencies: {sorted(dev)}")
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
print(f"flutter2nix: stripped dev-dependency plugins from registrant and deps file: {sorted(dev)}")
