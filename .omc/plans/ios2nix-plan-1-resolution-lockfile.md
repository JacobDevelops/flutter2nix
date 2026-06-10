# ios2nix — Plan 1: Resolution & Lockfile (Rust, Linux-provable)

> Reads with: `ios2nix-implementation-plan.md` (overview, principles P1–P5, shared contracts §2,
> options A/B/C, pre-mortem). This plan delivers the pure-Rust dependency-resolution half: the part
> that mirrors gradle2nix and is provable on Linux CI. Status: pending approval.

**Scope:** crate restructure → lib+bin; Podfile.lock parse; podspec fetch + source normalization;
content-hash prefetch; nix-core CocoaPods codegen; `lock`/`check`/`generate` CLI; flutter2nix
`ios.nodes` composition. **Turns green:** the resolution/codegen/lockfile unit stubs
(`test_parse_podfile_lock_*`, `test_resolve_pod_*`, `test_codegen_cocoapods_*`, lockfile roundtrip)
+ the `lock`/`check`/`generate` integration stubs. **NOT owned here:** the `export_opts` and
`keychain` tests — although `export_opts` is pure and Linux-runnable, the module + its tests
(`test_generate_export_options_*`) belong to **Plan 3** for cohesion with signing (overview §2.5).

**Out of scope (→ Plans 2/3):** anything that shells to `xcodebuild`/`security`/`codesign`/`pod`;
`nix/ios2nix-lib.nix`; flake package/checks (except keeping `cargo check --workspace` green).

---

## Phase -1 — Feasibility spike: offline `pod install`  *(BLOCKER — macOS-only, Fable runs FIRST)*

**This gates the entire feature.** It decides whether the resolution model below (Option B) is
viable or must become Option A/C. Run before writing resolver code.

**Procedure** (on a Mac, ~½–1 day):
1. Pick/assemble a representative Flutter iOS app whose `Podfile.lock` includes: ≥1 subspec pod
   (e.g. `Firebase/*`), ≥1 binary xcframework pod (Firebase or Realm), ≥1 git-source pod, and
   several Flutter-plugin path pods.
2. By hand, do what the Rust pipeline will automate: for each third-party pod, fetch its podspec,
   read `source`, prefetch the zip/git tree; assemble a local `file://` Specs repo + source mirror.
3. `pod install --no-repo-update` fully offline (no trunk CDN reachable — block it / `--verbose`).
4. Inspect the resulting `Pods/`: subspecs expanded? resource bundles present + linked? run-script
   phases persisted in the generated `Pods.xcodeproj`? Does `xcodebuild build` then succeed offline?

**Success criterion:** offline-assembled `Pods/` builds and is structurally equal to an online
`pod install`. → Option B confirmed; proceed to Phase 0.
**Failure escalation:** → Option A (snapshot the whole `Pods/` into the lockfile) or C (hybrid).
The `ios.nodes`/`dep_source` schema is forward-compatible with all three, so Phases 0–1 below are
not wasted; only the resolver internals (Phase 1–2 source step) change.
**Deliverable:** a one-page spike report committed at `docs/ios-podinstall-spike.md` recording
pass/fail per pod kind and the chosen v1 option. **Update the overview's ADR with the outcome.**

### Phase -1.5 — Sub-spike: is an *unsigned* export even possible?  *(5 min, same Mac session)*
Determines whether Plan 2 can validate `archive → export` independently of Plan 3's signing, or
whether export intrinsically requires signing (making the 2/3 split a validation-time dependency).
- Using the Phase -1 `Pods/` + a built archive, write a minimal `method=development` ExportOptions
  .plist with **no** `signingCertificate`/`provisioningProfiles`, and run
  `xcodebuild -exportArchive -exportOptionsPlist <it>`.
- **If it yields a valid `.ipa`:** unsigned export is feasible → Plan 2 keeps a real unsigned-export
  e2e.
- **If it fails (likely — Apple generally requires a signing identity + profile at export):** record
  it; **Plan 2's e2e is reframed to "archive (unsigned) + export *plumbing* only" and the first
  functional signed `.ipa` is Plan 3's e2e.** Plans 2 and 3 are then validated *together* on macOS,
  not independently — an honest acknowledgement, not a defect.
- **Deliverable:** one line in the spike report: "unsigned export = feasible | not feasible", which
  selects Plan 2 §5's e2e wording.

---

## Phase 0 — Crate becomes lib+bin; deps; Linux-green skeleton  *(platform-independent)*

`ios2nix` is currently **binary-only** (`Cargo.toml` has only `[[bin]]`; `main.rs` owns the modules).
flutter2nix composes by calling `gradle2nix::cli::lock::build_dependency_graph(...).await` as a crate
dep, so ios2nix **must become lib+bin** like gradle2nix.

**Tasks**
- `crates/ios2nix/Cargo.toml`:
  ```toml
  [lib]
  name = "ios2nix"
  path = "src/lib.rs"
  [[bin]]
  name = "ios2nix"
  path = "src/main.rs"

  [dependencies]
  anyhow = "1"
  clap = { version = "4", features = ["derive"] }
  nix-core = { path = "../nix-core" }
  serde = { version = "1", features = ["derive"] }      # NEW
  serde_json = "1"
  serde_yml = "0.0"          # NEW — Podfile.lock + podspec.json YAML/JSON (see Decision below)
  futures = "0.3"            # NEW — bounded-concurrency prefetch
  reqwest = { version = "0.11", features = ["rustls-tls"] }  # NEW — podspec + artifact fetch
  sha2 = "0.10"             # NEW — content hashing
  tokio = { version = "1", features = ["full"] }        # NEW
  tempfile = "3"            # NEW
  log = "0.4"               # NEW
  [dev-dependencies]
  tokio = { version = "1", features = ["full"] }
  pretty_assertions = "1"
  mockito = "0.31"          # NEW — mock podspec/artifact HTTP
  tempfile = "3"
  ```
  **Decision — YAML crate:** `serde_yaml` is archived. Use maintained `serde_yml` (or
  `serde_norway`). Podfile.lock has nested structures (a pod with its transitive deps as a sub-list
  — `complex-20-pods.lock` L4–9), so a real YAML parser beats a hand-roller. The parsing module is
  isolated; swapping is one file. Fable confirms the exact crate at build time.
- `src/lib.rs` (NEW):
  ```rust
  #![allow(dead_code)]
  pub mod cli;
  pub mod cocoapods;   // Podfile.lock parse + pod source classification
  pub mod podspec;     // NEW — podspec fetch/parse, source normalization
  pub mod lockfile;    // NEW — read/write/diff over nix-core DependencyGraph
  pub mod resolve_cache; // NEW — copy gradle2nix prefetch cache
  pub mod export_opts; // (Plan 3 expands; module exists now)
  pub mod keychain;    // (Plan 3; cfg-gated, compiles on Linux as bail!)
  pub mod xcode;       // (Plan 2; env/assert/output model)
  ```
- `src/main.rs` → thin `#[tokio::main] async fn main()` dispatcher; add `Check` + `Generate` to the
  `Command` enum alongside the existing `Lock/Build/Archive/Export/Sign`.
- **Create the compiling cfg-gated stub for EVERY macOS module now** (bodies arrive in Plans 2–3):
  `keychain`, `export_opts`, `xcode::{env,assert}`, `cli::{build,archive,export,sign}`. Each macOS
  fn ships both arms — `#[cfg(target_os="macos")] { …real-later… }` and
  `#[cfg(not(target_os="macos"))] { anyhow::bail!("ios2nix <op> requires macOS") }` — so the
  workspace is Linux-green from the start. The command structs (`ArchiveCommand`/`ExportCommand`/
  `SigningConfig` — overview §2.5) are also defined here, in Plan 2's eventual home but stubbed now,
  so Plans 2–3 fill bodies without restructuring.

**Acceptance — BLOCKING GATES (all green on Linux before Phase 1):**
1. `cargo check --workspace --all-targets`.
2. `cargo clippy --workspace --all-targets -- -D warnings`.
3. `cargo build -p ios2nix` → CLI prints help (lock/check/generate/build/archive/export/sign).
4. No macOS-only FFI crates added (only § above).
5. Every `#[cfg(target_os="macos")]` item has a compiling Linux `bail!` arm.
**Phase 1 does not begin until 1–5 pass. Gate 5 is RE-RUN after each of Plans 2 and 3** (every macOS
body they add must keep the Linux arm — overview §2.5); a missing arm fails the gate and blocks merge.

---

## Phase 1 — Pure core (the platform-independent unit stubs)  *(offline/mock-testable on CI; real CocoaPods behavior gated by Phase -1)*

Implement module-by-module; for each, delete its `#[ignore] + todo!()` and write the real assertion.

### 1a. `cocoapods.rs` — Podfile.lock parser + classifier
```rust
pub struct PodfileLock {
    pub pods: Vec<Pod>,                 // name, version, transitive dep names
    pub spec_checksums: BTreeMap<String,String>,  // NOTE: podspec hash, NOT artifact hash (see overview §0)
    pub podfile_checksum: String,
    pub cocoapods_version: String,
}
pub struct Pod { pub name: String, pub version: String, pub deps: Vec<String> }
pub fn parse_podfile_lock(yaml: &str) -> anyhow::Result<PodfileLock>;
/// Classify by podspec source (NOT name heuristics): path/dev pod → excluded; http/git → locked.
pub enum PodSourceKind { Http{url:String}, Git{url:String, rev:String}, Path{path:String} }
```
Green: `test_parse_podfile_lock_{simple,complex,invalid_yaml,missing_sha256}`.

### 1b. `podspec.rs` (NEW) — podspec fetch/parse + source normalization
```rust
pub struct Podspec { pub name:String, pub version:String, pub source: PodSourceKind, pub subspecs: Vec<String> }
pub fn parse_podspec(json: &str) -> anyhow::Result<Podspec>;     // pure — Linux-testable
pub async fn fetch_podspec(name:&str, version:&str, spec_repos:&[String], client:&reqwest::Client) -> anyhow::Result<Podspec>;
pub fn resolve_pod_source(spec:&Podspec) -> anyhow::Result<PodSourceKind>;
```
Green: `test_resolve_pod_url_{valid,missing_spec}`. (Network paths mocked with `mockito`.)

### 1c. Content-hash prefetch + `resolve_cache.rs`
Copy gradle2nix's `maven.rs` HTTP patterns (pooled `reqwest::Client`, `buffer_unordered`
concurrency, retry-once, 60s ceiling) and `resolve_cache.rs` verbatim, adapted to pod sources.
```rust
pub async fn prefetch_content_hash(src:&PodSourceKind, client:&reqwest::Client, cache:&ResolveCache) -> anyhow::Result<String>;
```
Green: `test_resolve_pod_sha256_{valid,mismatch}` — the **mismatch** test encodes the
"spec-checksum ≠ content-hash" guard: when an expected hash is supplied (stale fixture), prefetch
must error on divergence, proving we hash the *artifact*, not the podspec.

### 1d. `lockfile.rs` (NEW) — read/write/diff
Copy gradle2nix `lockfile.rs`: `write_lockfile`/`read_lockfile` over `DependencyGraph` (pretty
JSON), `LockfileDiff{added,removed,modified}` + `diff_lockfiles` keyed on `LockedDependency::name`.

### 1e. nix-core `codegen::cocoapods.rs` — inline + modular
Flesh out the stub mirroring `maven.rs`'s direct-`format!` approach (the `NixExprWriter` is
decorative). Emit exactly the two fixtures:
- **inline** (`simple-2-pods-inline.nix`): `{ lib, fetchurl }:\n{\n  <Pod> = fetchurl { url=…; sha256=…; };\n}`
- **modular** (`complex-20-pods-modular.nix`): `{ lib, fetchurl }:\nlet mkPod = …; in { <Pod> = mkPod { name=…; url=…; sha256=…; }; }`
```rust
pub struct NixCocoaPodsCodegenConfig { pub indent_width: usize, pub sort_deps: bool }
pub fn generate_nix_set(g:&DependencyGraph, c:&NixCocoaPodsCodegenConfig) -> anyhow::Result<String>;     // inline
pub fn generate_nix_overlay(g:&DependencyGraph, c:&NixCocoaPodsCodegenConfig) -> anyhow::Result<String>; // modular
```
Contract locked by fixtures: **raw hex sha256** (NOT SRI — use `dep.sha256_hex()`; `nix fetchurl`
accepts hex). **Attr-name quoting:** bare names unquoted; any name with `/` (subspecs, e.g.
`"Firebase/CoreOnly"`), `.`, `-` → quoted. **Add a subspec-quoting fixture** (none exists). Green via
new nix-core unit tests `test_codegen_cocoapods_{inline,modular,subspec_quoting}` (named distinctly
from Plan 3's `test_generate_export_options_*` to avoid the collision the review flagged — §2.5).

**Acceptance:** all platform-independent unit tests pass on Linux; clippy green.

---

## Phase 2 — lock/check/generate pipeline + sidecar + composition  *(mostly offline/mock-testable on CI; macOS validation of real resolution before Plan 2)*

### 2a. `cli::lock` — the composition contract
```rust
pub struct LockCommand { pub ios_dir: PathBuf, pub output: Option<PathBuf>,
    pub spec_repos: Option<Vec<String>>, pub cache_dir: Option<PathBuf>, pub timeout_secs: u64 }
pub async fn run(cmd: LockCommand) -> anyhow::Result<()>;
pub async fn build_dependency_graph(ios_dir:&Path, spec_repos:&[String],
    cache_dir:Option<&Path>, timeout_secs:u64) -> anyhow::Result<DependencyGraph>;  // flutter2nix calls this
```
Pipeline (mirrors gradle2nix's 7-step shape):
1. Sidecar: if `ios_dir/.ios2nix-podspecs.json` exists, consume it; skip network (overview §2.3).
2. Parse `Podfile.lock`. 3. Classify pods (source-driven). 4. Fetch+parse podspecs (third-party).
5. Prefetch content hash per source (bounded concurrency + cache). 6. Build `DependencyGraph`
   (`format_version "1"`, `dep_source` per kind; path pods excluded). 7. **Refuse to write** when
   Podfile.lock declared third-party pods but the graph is empty/Flutter-only (mirror gradle2nix's
   "0 artifacts" guard) — error names the dropped pods.

### 2b. `cli::check` + `cli::generate`
- `check`: re-resolve, diff vs existing lockfile; exit non-zero with a message containing `"stale"`
  on drift (mirror gradle2nix `check`). `CheckCommand{ ios_dir, lockfile, spec_repos, cache_dir, timeout_secs }`.
- `generate`: JSON lockfile → `pods.nix` (`--format inline|modular`); re-homes
  `test_lock_write_pods_nix`. `GenerateCommand{ lockfile, output, format }` (sync).

### 2c. flutter2nix composition
- `detect.rs`: `pub fn detect_ios(p:&Path) -> bool { p.join("ios").is_dir() && p.join("ios/Podfile.lock").exists() }`.
- `lockfile.rs`: add `ios: Option<IosSection>` (overview §2.2) + ios roundtrip tests mirroring android.
- `cli/lock.rs::generate_lockfile`: after the android block, add an ios block calling
  `ios2nix::cli::lock::build_dependency_graph(&project_dir.join("ios"), …).await` → `IosSection{nodes}`.
- `crates/flutter2nix/Cargo.toml`: add `ios2nix = { path = "../ios2nix" }`.

### 2d. Integration tests (`tests/cli_tests.rs`)
- `lock` full pipeline via `.ios2nix-podspecs.json` sidecar → JSON matches a committed fixture.
- `check` fresh (exit 0) vs stale (exit ≠0, "stale"). `generate` → `pods.nix` matches fixture.
- git-pod round-trip unit test (overview pre-mortem #5): serialize `pod-git` `LockedDependency`,
  parse back, assert `git+url#rev` is unambiguous/extractable.
- The `build/archive/export/sign` integration stubs stay `#[cfg_attr(not(target_os="macos"), ignore)]`
  (their bodies are Plans 2–3).

**Acceptance:** `cargo test -p ios2nix` (lock/check/generate) + nix-core codegen pass on Linux;
unified `flutter2nix lock` on an ios+android fixture emits `{ android, ios }`; clippy green.

---

## New fixtures this plan adds
`tests/fixtures/sidecars/simple.ios2nix-podspecs.json` (+ matching expected lockfile);
a git-source pod podspec; a path/dev pod entry; a subspec-name codegen fixture; an ios+android
unified `flutter2nix.lock`.

## Plan-1 acceptance summary (the Linux-provable gate before handing to Plan 2)
- All BLOCKING GATES (Phase 0) green. All platform-independent unit + lock/check/generate integration
  tests green on Linux. `flutter2nix lock` composes `ios.nodes`. Phase -1 spike report committed and
  ADR updated. Only then does Plan 2 (macOS) begin.

---

### Consensus footer
Round-2 Architect/Critic review applied: Phase -1 retained as blocker; sidecar schema referenced
from overview §2.3 (single definition); Phase 0 blocking gates explicit; "mock-provable not
reality-provable" labeling kept; git-pod round-trip test included. Status: pending approval.
