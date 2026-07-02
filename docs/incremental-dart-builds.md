# Incremental Dart-only builds (design + implementation plan)

Status: **implemented** — the three-derivation split (native-shell / dart-aot /
assemble) is live for both platforms behind `incrementalDart = true`, with the
`incremental-app` fixture, `.#incremental-app-mono`/`.#incremental-app-split`
flake packages, eval checks, and `fnx bench --target ios-incremental /
android-incremental`. See the README's "Incremental Dart-only rebuilds" section
for consumer-facing usage.

Audience: an agent working **inside this repo only**. Everything below is
testable with the fixtures in `tests/fixtures/` plus one richer fixture you
will create (spec in "Test fixture" below). You do not need access to any
consumer repo; consumer-measured numbers appear only as motivation.

## Problem

A `buildFlutterApp` derivation is monolithic: `xcodebuild archive` (iOS) or
the Gradle appbundle build (Android) runs end-to-end inside ONE derivation, so
a one-line Dart edit rebuilds the entire native side — pods compile,
Swift/ObjC compile, Gradle configuration, native linking — even though none of
its inputs changed. A real consumer (jfit) measured ~167 s for a full hermetic
iOS archive on an M-series Mac with warm deps, of which the Dart AOT slice is
roughly 60–90 s. Most consumer commits are Dart-only, so most of that native
work is waste.

Nix cannot do incremental work *inside* a derivation — every build starts
clean. The only way to make Dart-only edits cheap is to move the native work
into its own derivation whose inputs do not include `lib/`.

## Design: stub → swap

Split the app into three derivations per platform:

```
native-shell (expensive, cached across Dart edits)
  inputs: ios/ or android/, plugins, lockfiles, engine, Generated.xcconfig
          material — everything EXCEPT lib/, assets/, test/
  build:  the normal platform build, with a deterministic STUB Dart artifact
          (stub App.framework / stub libapp.so per ABI)

dart-aot (cheap, rebuilt on every Dart edit)
  inputs: lib/, assets/, pubspec.yaml+lock, package-config, engine,
          dartDefines, obfuscation/split-debug-info flags,
          ios/Flutter/AppFrameworkInfo.plist (copied into App.framework/Info.plist)
  build:  flutter assemble producing the real App.framework (+flutter_assets)
          or per-ABI libapp.so + assets, plus .symbols / obfuscation map

assemble (trivial)
  inputs: the two above
  build:  copy the shell output, replace the stub artifact(s) with the real
          ones, surface debug-symbols exactly like the monolithic build does
```

Why the swap is sound: `buildFlutterApp` emits an UNSIGNED archive/AAB, and
signing happens out-of-build (`xcodebuild -exportArchive` re-signs every
bundle in the archive; jarsigner signs the AAB). Post-hoc artifact replacement
therefore cannot break signatures — there are none to break yet. Document
this loudly in the API docs: consumers that sign *inside* the build must not
enable the split.

### iOS specifics

- The stub `App.framework` must be structurally valid (Mach-O dylib with the
  right install name + Info.plist) so xcodebuild's embed/thin phases succeed.
  Generate it in the shell derivation with `clang -dynamiclib` from an empty
  translation unit — deterministic, no Dart involved.
- The Run Script phase that normally invokes `flutter assemble` must be
  disabled in the shell build. flutter2nix already owns Generated.xcconfig
  (`nix/flutter2nix-lib.nix`), so gate it there (e.g. emit a flag consumed by
  the xcode_backend shim we control) rather than patching the app's Xcode
  project.
- In release mode everything Dart lives under
  `Runner.app/Frameworks/App.framework/` (binary + `flutter_assets/`), so the
  swap is one directory replace inside the `.xcarchive`, plus replacing the
  `App.framework.dSYM` in `<archive>/dSYMs/` with the one from `dart-aot`.
- `dart-aot` runs `flutter assemble -dBuildMode=release -dTargetPlatform=ios`
  (targets `release_ios_bundle_flutter_assets`) under the same
  CFFIXED_USER_HOME/`__noChroot` regime as the existing archive derivation; it
  links via the system Xcode clang, so it is Darwin-only like the archive.

### Android specifics

- Stub `libapp.so` per release ABI (arm64-v8a, armeabi-v7a, x86_64) — an
  empty `clang --target=<abi> -shared` object from the NDK already in the
  closure. Feed them to the Gradle build through the same intermediate
  directory the Flutter Gradle plugin reads AOT output from.
- `dart-aot` runs gen_snapshot per ABI (assemble targets
  `android_aot_bundle_release_android-arm64` etc.), emitting `libapp.so`s,
  `flutter_assets`, and — when obfuscation is on — the `.symbols` files and
  obfuscation map that `installPhase` surfaces as `result/debug-symbols`.
- The AAB is a zip: `assemble` replaces `base/lib/<abi>/libapp.so` and
  `base/assets/flutter_assets/**` entry-for-entry (`zip -X` to avoid
  timestamps; entry order preserved by editing in place). No zipalign
  concerns — alignment applies to APKs generated later by bundletool, not to
  the AAB.

### Keying rules (the point of the exercise)

- The shell derivation's `src` must be a fileset EXCLUDING `lib/`, `test/`,
  `assets/` (make the exclusion list a documented, overridable argument —
  consumers with codegen into `ios/`/`android/` need to widen it). If only
  excluded paths change, the shell drv hash must not move. This is testable
  (below).
- `dart-aot` excludes `ios/`/`android/`, except
  `ios/Flutter/AppFrameworkInfo.plist` which flutter copies into
  `App.framework/Info.plist`.
- Keep the monolithic path as the default; expose the split behind
  `incrementalDart = true` on `buildFlutterApp` so consumers opt in.

### Explicit non-goal

This is **not** OTA code push. Stock Flutter's engine cannot hot-swap Dart
AOT code on user devices; that requires Shorebird's forked engine + updater +
patch infra. This feature makes Dart-only *builds* fast and cache-friendly;
delivery still goes through the stores.

## Test fixture

The existing `tests/fixtures/flutter/minimal-app` (wired into the flake as
`.#buildFlutterIOSApp-e2e` on Darwin and `.#buildFlutterAndroidApp-e2e` on
Linux, aggregated in `.#e2e` / `fnx check`) is the cheap iteration target,
but it is too bare to prove the swap is correct. Create a second fixture,
`tests/fixtures/flutter/incremental-app`, that exercises everything the split
has to get right:

- **At least one plugin with native code on both platforms** (e.g.
  `shared_preferences` or `path_provider`) — proves the shell's pod/Gradle
  plugin compilation is keyed independently of `lib/`, and that the
  registrant plumbing survives the swap.
- **Bundled assets**: one image and one custom font declared in
  `pubspec.yaml` — proves `flutter_assets` replacement (assets live in the
  Dart tier on both platforms).
- **Two or more Dart files** (`lib/main.dart` importing `lib/feature.dart`)
  so a Dart-only edit is representative.
- **A dart-define consumed at runtime** (`String.fromEnvironment`) — proves
  dartDefines key the AOT tier.
- **Obfuscation + split-debug-info enabled** in the `buildFlutterApp` call —
  proves the `.symbols`/dSYM handling in `assemble`.

Creation: `flutter create incremental-app` inside a devenv shell, add the
plugin/assets/defines, run `flutter pub get`, then generate the lockfile the
same way the minimal fixture did:

```bash
cargo run -p flutter2nix -- lock --project-dir tests/fixtures/flutter/incremental-app
```

Wire it into `flake.nix` alongside the existing e2e entries (same
platform/lockfile gates), both monolithic (`incremental-app-mono`) and split
(`incremental-app-split`) so the equivalence test below has both sides.
Commit the fixture WITHOUT `build/`, `.dart_tool/`, `Pods/` (follow
minimal-app's `.gitignore`).

## Implementation phases

1. **iOS prototype on the fixtures**: stub framework generation, Run-Script
   gating, swap assembly. Iterate on `minimal-app`; exit criterion:
   equivalence + keying tests pass on `incremental-app`.
2. **Android**: stub `libapp.so`, gen_snapshot derivation, AAB zip surgery.
   Linux-only (Android SDK gate in the flake) — build on any Linux machine;
   exit criterion same as iOS.
3. **API + docs**: `incrementalDart` flag, exclusion-list argument, README
   section, cache guidance in `docs/ci-cache-strategy.md` (the shell drv is
   the new expensive tier — CI must push it).
4. **Consumer handoff** (out of this repo's scope): consumers validate with
   `--override-input flutter2nix path:...` against their own app and report
   timings. Do not test in consumer repos yourself.

## How to test (all local to this repo)

iOS work needs a Mac with Xcode; Android work needs a Linux machine. Run
every command inside this repo's devenv shell (`devenv shell -- <cmd>`).

### 1. Equivalence (correctness gate)

Build the fixture both ways and compare entry-by-entry:

```bash
nix build .#incremental-app-mono  -o result-mono
nix build .#incremental-app-split -o result-split

# iOS: compare the archives file-by-file
diff <(cd result-mono/*.xcarchive && find . -type f -exec shasum -a 256 {} + | sort -k2) \
     <(cd result-split/*.xcarchive && find . -type f -exec shasum -a 256 {} + | sort -k2)
# Android: unzip both AABs into temp dirs and run the same find|shasum diff.
```

Acceptance: every entry hash matches, EXCEPT a documented allowlist of
known-nondeterministic files (enumerate them — e.g. embedded timestamps — do
not hand-wave). Also assert the swapped `App.framework` and its dSYM share a
debug-id (`dwarfdump --uuid`) or Sentry symbolication silently breaks.

#### Executed results — iOS (macOS arm64, Xcode toolchain, 2026-07)

Ran against `.#incremental-app-mono` vs `.#incremental-app-split`. Result:
**zero files missing on either side; 10 files differ in content, all
accounted for by the allowlist below; within each build the
`App.framework` binary and its dSYM share a debug-id** (mono
`B43B66DF-2600-3DEA-85C6-95376F46A411`, split
`BE48F7E2-37D7-3944-A5E5-CA514A406F4B`), so Sentry symbolication is intact.

Nondeterminism allowlist (every non-matching entry, with cause):

1. `Runner.xcarchive/Info.plist` — `CreationDate` key only: `xcodebuild
   archive` stamps wall-clock time. Differs on every archive, mono or split.
2. LC_UUID in every linked Mach-O and its debug companions — the linker
   derives each binary's UUID from its full input closure (including build
   paths), so two archives never share UUIDs. Covers:
   `Products/Applications/Runner.app/Runner`, its
   `dSYMs/Runner.app.dSYM` DWARF, and
   `Relocations/aarch64/Runner.yml` (15 differing bytes — the embedded UUID).
3. `__LINKEDIT` symbol/string tables embedding the randomized Nix build
   directory (`/nix/var/nix/builds/nix-<pid>-<rand>`): affects
   `shared_preferences_foundation.framework`'s binary, its dSYM, and its
   Relocations yml (8197 differing bytes; file sizes identical).
4. `App.framework/App`, `App.framework.dSYM` DWARF, and
   `debug-symbols/app.ios-arm64.symbols` — `gen_snapshot` (Dart AOT) is
   nondeterministic run-to-run, independent of the mono/split change: 108433
   of 2558224 bytes differ, sizes identical. Proof it is gen_snapshot and not
   the split: rebuilding the *same* dart-aot derivation fails Nix's own
   determinism check —
   `nix build --rebuild '<dart-aot>.drv^*'` →
   `error: derivation '...-incremental-app-split-dart-aot.drv' may not be
   deterministic: output '...' differs`. Byte-diffing the `--keep-failed`
   `.check` output against the registered output shows the identical
   profile: exactly the same three files differ (86436 of 2558224 bytes in
   `App.framework/App`, size unchanged, fresh LC_UUID), everything else
   byte-equal — same-derivation run-to-run noise matches the mono-vs-split
   delta.

Classes 1–3 are inherent to `xcodebuild archive` and reproduce identically
between two *monolithic* builds; only class 4 originates in the Dart AOT
step, and it reproduces between two runs of the *same* derivation.

Runtime smoke: the design constrains output to **unsigned** device-arm64
archives (iphoneos SDK, ad-hoc/no signing). Such an archive cannot boot in a
simulator (wrong platform: simulator requires an `iphonesimulator`-SDK
build, and Flutter has no release-AOT simulator mode) nor install on a
device without signing — the same limitation the mono path has always had;
it is not introduced by the split. The correctness evidence substituting
for first-frame smoke is the byte-equivalence above: the split archive is
identical to the known-good monolithic archive except for the four
enumerated nondeterminism classes, including byte-identical plugin
registration (`GeneratedPluginRegistrant`), plugin frameworks (modulo
class 2/3), and `flutter_assets/` (asset render path). Android emulator
smoke remains open pending a Linux machine (Android phase is Linux-only —
see "Implementation phases").

### 2. Cache-keying proof (the actual feature)

```bash
# capture the native-shell drv path before and after a Dart-only edit:
key() { nix derivation show .#incremental-app-split \
  | python3 -c 'import json,sys; d=json.load(sys.stdin)["derivations"]; v=list(d.values())[0]; print(sorted(i for i in v["inputs"]["drvs"] if "native-shell" in i or "dart-aot" in i))'; }

key                                                             # before
echo '// keying test' >> tests/fixtures/flutter/incremental-app/lib/main.dart
key                                                             # after
git -C . checkout -- tests/fixtures/flutter/incremental-app/lib/main.dart
```

`dart-aot`'s drv path MUST change; `native-shell`'s MUST NOT. Repeat with an
edit under `ios/` (shell changes, AOT does not) and under `assets/` (AOT
changes, shell does not). This same technique verified the `3e641ba`
groundwork (package-config inputs dropped from the full app tree to
`pubspec.yaml` alone) — reuse it.

Note: if the flake evaluates from the git tree, uncommitted fixture edits may
be invisible — use `jj`/`git` scratch commits (this repo is jj-colocated) or
`path:.` when evaluating.

Executed (iOS, 2026-07): 8/8 assertions pass — `lib/feature.dart` edit moves
dart-aot + assemble but not native-shell; `ios/Runner/Info.plist` edit moves
native-shell + assemble but not dart-aot; `ios/Flutter/AppFrameworkInfo.plist`
(the one file under `ios/` that IS a dart-aot input) moves dart-aot; restoring
the edits returns the exact baseline drv hashes.

### 3. Timing benchmark

```bash
# Prime the split (caches the native shell), then edit lib/main.dart and time
# the SAME edit both ways — the fresh drvs force the rebuilds. (`--rebuild`
# can't be the baseline: it diffs against the registered output, and
# xcodebuild is nondeterministic per the class-2/4 allowlist, so it always
# exits 1.)
nix build .#incremental-app-split
echo '// dart edit' >> tests/fixtures/flutter/incremental-app/lib/main.dart
time nix build .#incremental-app-mono   # monolithic baseline: full rebuild
time nix build .#incremental-app-split  # shell cached, AOT+assemble build
```

Record numbers in `benchmarks/BENCHMARKS.md` via `fnx bench` (add
`ios-incremental` / `android-incremental` targets). Target: Dart-only rebuild
≤ 50% of the monolithic wall time on the same machine; worse means the shell
split is leaking inputs — go back to test 2. Use
`benchmarks/ci-restore-sim.sh` / `measure-closure.sh` to check the split
doesn't bloat the closure a cold CI runner must restore.

### 4. Regression safety

The monolithic path stays the default and must stay green: `fnx check`
(builds `.#e2e`) on both platforms after every change to
`nix/flutter2nix-lib.nix` / `nix/gradle2nix-lib.nix`. `nix flake check`
deliberately excludes e2e (runner disk); do not move e2e into `checks`.

## Risks / open questions

- **Run-Script gating**: the exact mechanism to make the shell build skip
  `flutter assemble` without patching consumer Xcode projects needs a spike —
  candidates: Generated.xcconfig flag consumed by our xcode_backend shim, or
  substituting the shim script itself. Decide during phase 1.
- **Stub acceptance**: Gradle's Flutter plugin or Xcode phases may validate
  the Dart artifact (arch checks, symbol presence). The fixture will surface
  this early; the stub may need a token exported symbol.
- **Codegen consumers**: build_runner output under `lib/` is covered (dart-aot
  input set), but codegen writing into `ios/`/`android/` busts the shell —
  hence the overridable exclusion list.
- **Pubspec `version:` bumps** change both tiers (Info.plist/versionCode live
  in the shell). Accepted: release commits rebuild everything; dev iteration
  is the target workload.
