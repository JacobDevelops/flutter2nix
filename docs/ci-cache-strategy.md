# CI cache strategy for flutter2nix

How to make clean-CI hermetic Flutter builds fast, and where to spend (and not
spend) caching effort. The short version, in priority order:

1. **Ship fewer bytes.** On a cold ephemeral runner the build is restore-bound, and
   the restore is bandwidth-bound on the dependency closure. The biggest lever is a
   *smaller closure* — it restores faster at **any** bandwidth and shrinks the cache
   for every consumer. See [Lever 1](#lever-1-ship-fewer-bytes-the-primary-lever).
2. **Use the Nix binary cache, not `actions/cache`.** Custom `actions/cache` archives
   of build inputs are almost always the wrong layer (see below).
3. **Then tune the restore** (parallelism, zstd, prefetch). These help the *many
   small* toolchain/SDK/pub paths and make the slow step visible — but they barely
   move a single large, incompressible Maven NAR. Tuning is the last lever, not the
   first.

## Why the binary cache is the right layer

Every hermetic input flutter2nix builds against is already a content-addressed
Nix store path:

- the Flutter SDK (`pkgs.flutter`),
- the Android SDK (`androidenv.composeAndroidPackages`), JDK, and Gradle,
- the offline Maven repo (`buildMavenRepo` in `nix/gradle2nix-lib.nix`),
- Dart pub dependencies (fixed-output derivations from `pubspec.lock`, see
  `nix/pub-lib.nix`),
- CocoaPods inputs for iOS.

Nix already does, for free, everything a hand-rolled CI cache tries to do:

- **Dedup** — identical paths are stored and transferred once, across lockfile
  versions, projects, and builders.
- **Compression** — NARs are compressed by the substituter (xz on
  cache.nixos.org; zstd is configurable on your own cache — see below).
- **Incremental transfer** — only store paths missing from the local store are
  fetched; a dep bump pulls the changed paths, not the world.
- **Integrity** — every path is hash-verified on substitution.

A custom `actions/cache` tar of `/nix/store` (or of `~/.gradle`, `~/.pub-cache`,
etc.) **fights** all of this: it re-compresses what Nix already compressed,
defeats per-path dedup, and risks restoring a store that disagrees with the
build's expectations. Don't do it. Cache *cargo's* `target/` with
`Swatinem/rust-cache` (that's not a Nix path — see `.github/workflows/ci.yml`),
but cache *Nix build inputs* by pointing CI at a binary cache.

### Concretely

1. Add a binary cache substituter the CI runner can read **and** write:
   - [Cachix](https://www.cachix.org/) — `cachix/cachix-action`, zstd NARs.
   - [attic](https://github.com/zhaofengli/attic) — self-hosted, zstd, dedup by
     chunk.
   - `DeterminateSystems/magic-nix-cache-action` — zero-config, backs onto the
     GitHub Actions cache (fine for small/medium closures; the Actions cache has
     a 10 GB repo cap, so watch the SDK + maven-repo closure size).
   - A plain S3/HTTP `nix copy --to` cache.
2. Build once (`nix build .#<app>`), push the closure to the cache.
3. Subsequent clean runs substitute the unchanged closure instead of rebuilding.

cache.nixos.org already serves the toolchain/SDK tier (Flutter, Android SDK,
JDK, Gradle) for most nixpkgs revisions, so even with no private cache the most
expensive *downloads* are already covered — what your private cache adds is the
**project-specific** tier (maven repo, pub deps, CocoaPods).

## Cache classes by stability tier

Split caching effort by how often each class changes. Most-stable first:

| Tier | Contents | Changes when | Where it's served |
|------|----------|--------------|-------------------|
| Toolchain / SDK | `pkgs.flutter`, Android SDK, JDK, Gradle, Rust toolchain | nixpkgs / flake.lock bump | cache.nixos.org (mostly) |
| Dependencies | offline Maven repo, pub deps, CocoaPods pods | `gradle2nix.lock` / `pubspec.lock` / `Podfile.lock` bump | your private cache |
| Build outputs | APK / AAB / IPA, gradle/flutter build dirs | every source edit | don't cache — cheap to rebuild given warm deps |
| Lock-time | gradle2nix resolve-cache (`resolve-cache.json`) | new/changed deps during `lock` | persist by a stable key (tiny JSON) |

Two consequences:

- **Don't cache build outputs.** With the dependency tier warm, an
  `assembleRelease` / `flutter build` is minutes, not the bottleneck. Caching the
  volatile output churns the cache for little benefit (cache save cost > restore
  benefit).
- **The gradle2nix resolve-cache is a *lock-time* cache, not a build cache.** It
  lives at `{gradle-user-home}/caches/gradle2nix/resolve-cache.json` and only
  speeds up regenerating the lockfile (resolved SHA-256s, POM texts, confirmed
  404s — Maven release URLs are immutable). It is small and safe to cache by a
  key over `gradle2nix.lock`. It has nothing to do with the hermetic build.

## Compression: xz vs zstd, and when it matters

NAR compression is a per-cache setting, not a per-build one. The tradeoff is
ratio (smaller transfer) vs. decompression speed (faster restore):

- **xz** — best ratio, slow to decompress. cache.nixos.org default. Good for the
  toolchain/SDK tier: pushed rarely, pulled often, cached on the runner's store
  after the first pull anyway.
- **zstd** — slightly larger, *much* faster to decompress, fast to compress.
  Better for the **dependency tier** on ephemeral runners where **restore time
  dominates** end-to-end CI time. Cachix and attic serve zstd; configure it on a
  self-hosted cache with `compression = zstd` (and tune `compression-level`).

Rule of thumb: **optimise for fastest end-to-end CI, not smallest archive.** On a
cold ephemeral runner the offline Maven repo's ~5.6 GB decompresses on the
critical path, so zstd's faster decompression usually wins even though the NAR is
a bit larger. On a persistent builder the closure is already local, so
compression barely matters.

## The `consolidateMavenRepo` decision

`buildMavenRepo` (`nix/gradle2nix-lib.nix`) has two shapes, selected by the
`consolidateMavenRepo` flag (default `false`):

- **Symlink (default).** Each fetched artifact is symlinked, so the repo's
  closure is ~2900 content-addressed store paths. Dedups across lockfile versions
  and projects; a warm builder transfers only the paths a dep bump changed. **The
  right default for persistent builders and any setup with a private binary
  cache.**
- **Consolidated (`consolidateMavenRepo = true`).** Every artifact is copied into
  a single store path, so the whole repo substitutes as **one NAR in one
  request**. On an ephemeral cold-store runner this avoids ~2900 per-object cache
  GETs (minutes of per-object latency even when every path is a hit). Cost: the
  single NAR shares nothing, so each lockfile version is a full (~5.6 GB) NAR and
  every dep bump re-pushes/re-pulls the whole closure.

Decision:

| Runner | Private cache | Use |
|--------|---------------|-----|
| Persistent (self-hosted, warm store) | any | symlink (default) |
| Ephemeral (fresh store each run) | object-latency-bound | `consolidateMavenRepo = true` |
| Ephemeral | chunked/dedup cache (attic) | measure both — chunking can beat the single NAR |

## Lever 1: Ship fewer bytes (the primary lever)

A cold runner's restore is bandwidth-bound on the dependency closure. Halve the
bytes and you roughly halve the restore — at **any** link speed, and for **every**
consumer of the cache. This dominates the tuning levers below, which barely move a
single large incompressible NAR (a consolidated Maven repo is mostly already-
compressed JARs/AARs; the binary cache's xz/zstd can't shrink it, so ~5.6 GB on
disk transfers as ~5.6 GB over the wire).

Measured with `ci-restore-sim.sh` against an incompressible jfit-scale fixture
(rate 8 MB/s, 4 cores): a 256 MB closure restores in 31.1 s; a 128 MB closure in
15.1 s — **2× fewer bytes ≈ 2× faster restore.** That linear relationship is the
whole argument for shrinking.

What you can safely cut, and what you can't:

- **`-sources.jar` / `-javadoc.jar` are excluded automatically.** A hermetic
  compile/assemble never reads them — they exist for IDEs and docs — so `gradle2nix
  lock` drops any `sources`/`javadoc` classifier artifact before it reaches the
  lockfile (and therefore the offline-deps closure). This is the default; there is
  nothing to configure. Most projects pull none (Gradle's runtime resolution doesn't
  request them), so for them it is a no-op; projects that do pull them get a free
  reduction at zero build cost. Re-run `lock` to pick it up.
- **Symlink mode dedups across versions and projects.** `consolidateMavenRepo =
  false` (the default) makes each artifact its own content-addressed store path, so a
  dep bump re-transfers only the changed paths and a shared artifact is stored once
  across every lockfile and project. Over time this is a much smaller *cache* than
  consolidated mode's full-NAR-per-version (see the table above). It does not shrink
  the *first* cold pull, but it shrinks every subsequent one.
- **What is NOT removable.** The bulk of the closure is genuinely-needed AARs/JARs
  (native libraries for every ABI live inside single AAR files — you cannot prune one
  without breaking the artifact's hash). The multiple `aapt2:<abi>` versions a lock
  may contain are intentional: each matches an `aapt2-proto` version the build can
  select. And the debug+release artifact superset is intentional so one offline repo
  serves both. Don't prune these to chase bytes; you'll break hermeticity or
  correctness for a closure that is mostly irreducible anyway.

Measure your own closure before and after with `measure-closure.sh` (see *Measuring
it*), and prove the restore delta with `ci-restore-sim.sh`.

## Tune the restore (the secondary lever)

These help the **many small** toolchain/SDK/pub paths and make the slow step
visible in the timeline; they do **not** meaningfully speed up a single large
incompressible Maven NAR. Apply them after Lever 1, not instead of it.

On a fresh runner every build input is a cache miss, so **restore — not compile —
dominates** end-to-end time. (Measured on a real consumer: a ~14.5 min cold
Android build step decomposed as ~11 min Nix realising the build closure, ~2.6 min
unpacking SDK derivations, and only ~43 s of actual Gradle/Flutter compile. That
~11 min is overwhelmingly remote-cache *substitution*: the workflow pipes
`nix build` through a `grep -Ev` that strips every `copying path`/`fetching path`
line, and no local build-phase output — which is *not* stripped — appears until
the final ~3 min, so little is being built locally during the silence.) Three
settings move the restore, all of which the `prefetch-nix-closure` action below
sets for you:

1. **Substitution parallelism.** Nix's defaults (`http-connections = 25`,
   `max-substitution-jobs = 16`, `download-buffer-size = 64 MiB`) under-use the
   link to a high-latency remote cache:

   ```
   http-connections = 128
   max-substitution-jobs = 128
   download-buffer-size = 536870912   # 512 MiB
   ```

   Know what each one buys you — they are not interchangeable levers:
   `http-connections` and `max-substitution-jobs` parallelise the **many small**
   toolchain/SDK/pub paths (and are irrelevant to a single consolidated Maven NAR,
   which is one path, downloaded as one sequential stream). `download-buffer-size`
   is the only one of the three that can touch that single big NAR — it keeps the
   download from stalling while the NAR drains to the store — but it only matters at
   link speeds high enough for decompression to become the bottleneck; on a
   bandwidth-starved runner (e.g. ~8 MB/s) the link, not the buffer, is the limit and
   it is effectively moot. Set these in the Nix installer's
   `extra-conf` (the daemon reads them at start), or pass them per-invocation with
   `--option` from a trusted runner user. If your cache returns throttling or
   `RequestCanceled` under high parallelism, dial the first two back down.

2. **zstd — strongly recommended if you set `consolidateMavenRepo = true`.**
   Consolidation collapses ~2900 small NARs into one big NAR: great for object
   latency, but that single NAR then decompresses **single-threaded on the restore
   path**, and the Nix push default is xz (best ratio, slowest to decompress). Push
   your cache with zstd so the big NAR decompresses several-fold faster:

   ```
   nix copy --to 's3://my-cache?compression=zstd&parallel-compression=true&…' .#my-app-offline-deps
   ```

   Cachix and attic already serve zstd. This is a seconds-scale win, not a
   minutes-scale one — xz-decompressing even a multi-GB NAR is on the order of
   seconds, small next to download time — but it is free to claim. Quantify it on
   your repo with `measure-closure.sh --compression` (see *Measuring it*).

3. **Prefetch the heavy closure as its own step.** Realise the offline-deps
   derivation *before* the build, so the slow restore is parallelised, timed, and
   visible in the run timeline rather than buried inside the build step:

   ```yaml
   - uses: <your-org>/flutter2nix/.github/actions/prefetch-nix-closure@<rev>
     with:
       installables: ".#my-app-offline-deps"   # whatever surfaces your offline deps
   - run: nix build .#my-app   # project-tier closure already local
   ```

   The action applies the tuning in (1) by default (override via its inputs) and
   encodes nothing project-specific — you name the installable. **Scope caveat:** an
   app's `offlineDeps` usually covers the project tier (Maven repo + pub packages)
   but **not** the Flutter/Android SDK/JDK/Gradle closure, which is still
   substituted inside `nix build`. To cover that too, either add those installables
   here, or — simplest — put the tuning in the installer's `extra-conf` so it
   applies daemon-wide to the build step as well (also the fallback when the runner
   user is not trusted, since `--option` is then ignored). See
   `.github/actions/prefetch-nix-closure/action.yml`.

## Measuring it

Don't guess — measure restore-dominated vs. build-dominated time:

- **End-to-end build wall-clock**, cold and warm: `fnx bench` (run inside
  `nix develop`). Targets: `lock`, `gradle-build`, `flutter-build`, `ios-lock`,
  `ios-build`. Results append to `benchmarks/BENCHMARKS.md` /
  `benchmarks/history.jsonl`.
- **Closure size and path count** of the offline Maven repo per mode (the
  `consolidateMavenRepo` tradeoff, made data-driven):

  ```bash
  # symlink (default) vs consolidated — build each, then compare:
  nix path-info -Sh ./result            # total closure size (human)
  nix path-info -rS ./result | wc -l    # number of store paths in the closure
  ```

  `benchmarks/measure-closure.sh <result> [label]` wraps these two commands into
  one labeled line — build each mode's `result`, run it on both, and compare.
  Note: measure the **offline Maven repo derivation itself** (exposed via the
  build's `passthru`), not the final APK/AAB — an app output's closure is just
  the artifact (build inputs like the Maven repo are not runtime references, so
  the output closure is tiny and identical across `consolidateMavenRepo` modes).

  Expect ~2900 paths / ~19 MB on-disk for symlink mode vs. a handful of paths /
  ~5.6 GB for consolidated. The question CI answers is whether 2900 small object
  GETs or one 5.6 GB NAR pull is faster on *your* runner — so measure restore
  time on the actual cache backend.

- **NAR compression** (xz vs zstd), the cold-restore-time lever for a consolidated
  repo: `benchmarks/measure-closure.sh --compression <result> [label]` dumps the
  path's NAR once and reports each codec's compressed size and — what actually
  matters — its **decompression** wall-clock. Run it on the consolidated Maven
  repo path to confirm zstd's faster decompress before committing to xz on your
  cache. (It compresses the full NAR locally, so it needs scratch disk and a
  minute on the ~5.6 GB repo; point it at a smaller path first to sanity-check.)

- **Substitution time** on a cold store: `nix copy --from <cache> <path>` against
  a throwaway store (`--store ./tmp-store`) and time it. Never GC or delete
  `/nix/store`, `~/.cache`, `~/.gradle`, `~/.pub-cache`, or `~/.android` to force
  a cold measurement — use an explicit throwaway store/dir instead.

### Reproducing cold-CI slowness locally

CI is the worst place to A/B-test a cache change: every run is a cold cloud
runner you can't attach to. `benchmarks/ci-restore-sim.sh` recreates the
restore-dominated build on one machine, no cloud and no root, so you can measure
the fix before shipping it. It:

- pushes the closure to a `file://` binary cache (one per compression) — the
  *simulated remote cache*;
- serves it over HTTP through a small **throttling** static server (aggregate
  link-level bandwidth cap + per-object latency) — S3/HTTP GET semantics, slowed to a
  runner's link. The cap is a shared token bucket across all connections, so more
  `http-connections` cut serial round-trips but never exceed the link, and one big NAR
  on a single connection saturates it exactly like many small paths do;
- substitutes into a **fresh throwaway store** (every path a cold miss) **pinned to
  N cores** (`taskset`) to mirror an N-vcpu runner;
- runs the matrix `{xz,zstd} × {default,tuned}` and prints each restore time plus
  the `xz/default → zstd/tuned` speedup.

```bash
nix build .#<app>.offlineDeps -o result-deps     # a heavy, real closure
./benchmarks/ci-restore-sim.sh --store-path result-deps --rate 8 --cpus 8
```

No app handy? `benchmarks/fixtures/jfit-scale-offline-deps.nix` synthesises a
jfit-scale closure (~5 GiB across ~2900 incompressible "JAR" files — one store
path, the `consolidateMavenRepo=true` shape). It is deterministic and needs no
network, so the benchmark is self-contained and reproducible:

```bash
nix build --impure -f benchmarks/fixtures/jfit-scale-offline-deps.nix -o result-deps
./benchmarks/ci-restore-sim.sh --store-path result-deps --rate 8 --cpus 8
# totalMiB / numArtifacts / incompressibleFrac are tunable for a quicker run.
```

Measured at small scale (rate 8 MB/s, 4 cores): a 256 MiB fixture restores in
31.1 s and a 128 MiB fixture in 15.1 s — ~8.3 MB/s effective, and 2× bytes ≈ 2×
time. At that throughput a full ~5 GiB closure extrapolates to ~10 min, which is in
line with the ~11 min cold restore observed on a real consumer. (The fixture is
deliberately *incompressible*: each artifact uses an independent AES-CTR keystream,
so the binary cache stores ~5 GiB and the throttled link carries ~5 GiB — modelling
real already-compressed JARs. If you make it compressible, the cache stores far less
and the restore finishes in seconds, which would *not* reproduce the real scenario.)

**Calibrate to your consumer** by effective throughput: closure_bytes ÷
restore_seconds. A ~5.6 GB closure restoring in ~11 min is ~8.5 MB/s, so
`--rate 8` reproduces that per-byte timing; a smaller closure finishes sooner but
the ratios and the per-GB extrapolation hold. Note the link carries the
**compressed** NARs, not the uncompressed closure, so a very compressible closure
restores faster than its on-disk size suggests — use a real, representative
closure (or scale `--rate` down) for a faithful absolute number.

What the matrix tells you that prose can't: **whether your restore is
bandwidth-limited or decompression-limited.** Under a hard bandwidth cap the
*compressed* bytes dominate, so xz's better ratio can beat zstd — but only when the
payload is actually compressible; for already-compressed JARs/AARs (the real case)
xz and zstd transfer ~equal bytes and the ratio advantage vanishes. zstd wins only
when decompression (CPU, single-threaded per NAR) is the bottleneck — fast link,
few cores, one big consolidated NAR. Don't assume which regime you're in; the two
axes (`--rate` vs `--cpus`) let you find it, then pick compression accordingly.

## Anti-patterns

- Tarring `/nix/store` (or `~/.gradle`, `~/.pub-cache`) into `actions/cache`.
- Caching build outputs (APK/AAB/IPA) across runs.
- Forcing cold measurements by deleting shared global caches.
- Optimising for smallest archive instead of fastest end-to-end CI.
- Flipping `consolidateMavenRepo` on for a persistent builder (kills dedup and
  incremental transfer for no benefit).
