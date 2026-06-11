# ios2nix — Overview & Plan Index (pending approval)

> Status: consensus-approved plan SET, split into 3 hand-off-able documents. No execution performed.
> Handoff target: Fable, on macOS hardware. Repo is unpublished — pick the best design, no
> backwards-compat concerns (per AGENTS.md).

This is the shared deliberation + the index. The three executable plans:

| Plan | File | Platform | What it delivers |
|---|---|---|---|
| **1 — Resolution & Lockfile** | `ios2nix-plan-1-resolution-lockfile.md` | Linux-provable (+ macOS spike gate) | Crate → lib+bin; Podfile.lock + podspec resolution; content-hash prefetch; nix-core CocoaPods codegen; `lock`/`check`/`generate`; flutter2nix `ios.nodes` composition. |
| **2 — Build & Nix Integration** | `ios2nix-plan-2-build-nix.md` | macOS | `xcode` env/assert; `build`/`archive`/`export` orchestration; offline `pod install` sandbox; `nix/ios2nix-lib.nix` (`buildIOSApp`, unsigned path); flake package + darwin checks. |
| **3 — Signing & Provisioning** | `ios2nix-plan-3-signing-provisioning.md` | macOS | Temp-keychain lifecycle + `set-key-partition-list`; cert import; profile install; full `ExportOptions.plist` model; signed export; `sign` re-sign; secret contract; runbook. |

**Execution order & dependencies:**
`Plan 1 Phase -1 spike (BLOCKER)` → `Plan 1 Phases 0–2 (Linux)` → `Plan 2 (macOS, needs the lockfile)`
→ `Plan 3 (macOS, needs Plan 2's archive/export skeleton)` → optional `buildFlutterIOSApp` (Plan 2 §)
. Plan 1 Phase 0 (crate becomes lib+bin) is the shared foundation all three build on.

---

## 0. The one thing to understand before anything else

`ios2nix` is **not** a clean mirror of `gradle2nix`, even though the scaffold is shaped like one.
Two facts in the existing fixtures are *deliberately simplified stubs* that will mislead an
implementer who treats them as ground truth:

1. **A CocoaPods `SPEC CHECKSUM` is NOT a fetchable artifact's content hash.** The fixtures copy the
   `SPEC CHECKSUMS` hex straight into `fetchurl { sha256 = …; }`. In reality a spec checksum hashes
   the *podspec file*, not the downloadable `.zip`/`.xcframework`. The real `fetchurl.sha256` must be
   computed by **prefetching the actual source artifact**.
2. **Most "pods" in a Flutter app are not downloadable third-party pods.** `complex-20-pods.lock` is
   ~18 Flutter *plugin* pods (path pods vendored in the pub cache, no remote URL, owned by the Dart
   layer) plus a couple of real binary pods. The modular fixture's invented `github.com/…` URLs are
   fakes.

A third reality drives Plans 2–3: **Xcode signing is inherently impure** (Apple network, keychain,
provisioning). Locked pod *inputs* are reproducible; the signed `.ipa` is not.

---

## 1. RALPLAN-DR — Shared Deliberation

### Principles (P1–P5)
- **P1 — Compile + unit-test green on Linux.** CI runs `cargo check/clippy --workspace` on Linux.
  Every macOS syscall (`xcodebuild`, `security`, `codesign`, `pod`) sits behind
  `#[cfg(target_os="macos")]` with a non-macOS arm returning `anyhow::bail!("… requires macOS")`.
  Prefer shelling out (string-in/string-out, sidecar-mockable) over macOS-FFI crates that would
  break the Linux gate.
- **P2 — Single-source the lockfile schema.** The unified `flutter2nix.lock` `ios.nodes` array is the
  *only* Rust↔Nix contract. `nix/ios2nix-lib.nix` reads it exclusively; the standalone `pods.nix`
  (`generate`) is consumer convenience, never a second source of truth. (Directly answers the past
  "lockfile format mismatch" incident in project memory.)
- **P3 — Sidecar-injected determinism.** Mirror gradle2nix's `.gradle2nix-tapi-output.json`:
  `.ios2nix-podspecs.json` (resolution) and `.ios2nix-xcode-output.json` (build) short-circuit the
  real tool, so every test is hermetic and Mac-free.
- **P4 — Honesty about hermeticity.** Locked pod inputs are reproducible; the signed `.ipa` is not
  (timestamps, signatures, Apple network). `buildIOSApp` is explicitly impure (`__noChroot`); e2e
  asserts `.ipa` *structure*, never byte-equality.
- **P5 — Mirror gradle2nix where the domain matches; diverge only where CocoaPods/Xcode genuinely
  differ, and document each divergence.** The one structural divergence is a new `podspec.rs`
  module: CocoaPods needs an extra metadata indirection (Podfile.lock names pods but not their
  download sources — the *podspec* holds `source`), whereas Gradle/TAPI yields coordinates directly.

### Decision Drivers (top 3)
1. **Cannot build/run on this machine (Linux).** Maximize what's provable on Linux CI; isolate the
   irreducibly-macOS surface for Fable.
2. **CocoaPods source heterogeneity (http zip / git+rev / Flutter-plugin path).** The model + Nix
   builder must handle all three, or honestly scope to a subset for v1.
3. **Unified-composition parity.** ios2nix must expose async
   `cli::lock::build_dependency_graph(...) -> Result<DependencyGraph>` so `flutter2nix` composes
   `ios.nodes` next to `android.nodes`.

### Options for the lock/build strategy
- **A — Vendor the resolved `Pods/` tree** (record/replay a real `pod install`). Captures reality
  exactly; but locking *requires* a Mac + working CocoaPods, larger lockfiles.
- **B — Podspec-driven resolution** ✅ recommended *contingent on the Phase -1 spike*. Parse
  Podfile.lock → fetch podspecs → normalize `source` → prefetch content hashes; exclude path pods.
  Resolution logic is Linux-unit-testable; smaller lockfiles; matches the repo's gradle2nix model.
  Risk: offline `pod install` reconstruction is unproven (CocoaPods generates build artifacts at
  install time, unlike Maven's file mirror).
- **C — Hybrid** (B default, A as `--from-pod-install` escape hatch). Deferred; schema is
  forward-compatible to add later.

**Recommendation: B for v1, CONTINGENT on Plan 1's Phase -1 macOS spike.** If the spike fails,
escalate to A or C. The `ios.nodes`/`dep_source` schema is forward-compatible across all three, so
Plan 1 Phases 0–1 are not wasted regardless of the outcome.

---

## 2. Shared contracts (referenced by all three plans)

### 2.1 Dependency model (nix-core, unchanged for v1)
`LockedDependency { name, version, url, sha256_hex, dep_source: Option<String> }`. Use `dep_source`
as the pod-source discriminator: `"pod-http"` (url = zip, sha256 = content hash), `"pod-git"` (url
packed as `git+<url>#<rev>`, sha256 = NAR hash). Path pods are excluded from `ios.nodes`. Git
submodules → defer to model option (b) `Option<GitPodMetadata{submodules}>` only if a real app needs
it (additive, `skip_serializing_if`, no churn for gradle2nix).

### 2.2 Unified lockfile (flutter2nix)
```rust
pub struct FlutterLockfile {
    #[serde(skip_serializing_if = "Option::is_none")] pub android: Option<AndroidSection>,
    #[serde(skip_serializing_if = "Option::is_none")] pub ios: Option<IosSection>,   // NEW
}
pub struct IosSection { pub nodes: Vec<LockedDependency> }
```
`skip_serializing_if` keeps the existing `!json.contains("\"ios\"")` assertion valid for android-only
locks. `nix/ios2nix-lib.nix` reads `lock.ios.nodes`.

### 2.3 Sidecar schemas (P3) — defined once, used by Plans 1 & 2
**`ios_dir/.ios2nix-podspecs.json`** (resolution short-circuit):
```json
{ "pods": [
  { "name": "firebase_core", "version": "10.0.0",
    "source": { "type": "http", "url": "https://…/firebase_core.zip", "sha256": "<hex>" } },
  { "name": "Firebase/Auth", "version": "10.0.0",
    "source": { "type": "git", "url": "https://github.com/firebase/firebase-ios-sdk.git",
                "rev": "<commit>", "sha256": "<NAR hash>" } },
  { "name": "path_provider_foundation", "version": "2.3.0",
    "source": { "type": "path", "path": "<pub-cache plugin ios/ dir>" } }
]}
```
`type:"path"` → excluded from `ios.nodes`. **`<proj>/.ios2nix-xcode-output.json`** (build
short-circuit): exactly the `XcodeBuildOutput` struct (`version`, `architectures`, `frameworks`,
`codesign_identity`); existing `tests/fixtures/xcode-outputs/*.json` are the fixtures.

### 2.4 CLI surface (reconciled across plans)
`lock | check | generate` (Plan 1, mirror gradle2nix) + `build | archive | export | sign` (Plans
2–3, macOS). `lock` writes JSON only; `generate` writes `pods.nix`; the scaffold's
`cli/lock_tests.rs::test_lock_write_pods_nix` re-homes under `generate`.

### 2.5 Seam-ownership matrix (single source of authority — resolves cross-plan ambiguity)
Every module/command/test is owned by **exactly one** plan. "Stub (P1)" = Plan 1 Phase 0 creates the
compiling cfg-gated skeleton (macOS arm `todo!`-free placeholder + Linux `bail!` arm) so the
workspace stays Linux-green; the owning plan fills the macOS body.

| Artifact | Phase-0 stub | macOS body owner | Tests owned by |
|---|---|---|---|
| `cocoapods.rs`, `podspec.rs`, `resolve_cache.rs`, `lockfile.rs` | — (pure, P1) | P1 | P1 (`test_parse_podfile_lock_*`, `test_resolve_pod_*`) |
| nix-core `codegen::cocoapods` | — (pure, P1) | P1 | P1 (`test_codegen_cocoapods_*`) |
| `cli::lock` / `cli::check` / `cli::generate` | — (P1) | P1 | P1 |
| `xcode::{env,assert,build_output}` | stub (P1) | P2 (`build_output` pure) | P2 |
| `cli::build` / `cli::archive` | stub (P1) | **P2** (unsigned) | P2 |
| `cli::export` (the `xcodebuild -exportArchive` call) | stub (P1) | **P2** | P2 (plumbing) + P3 (signed e2e) |
| `cli::sign` | stub (P1) | **P3** | P3 (`test_sign_ipa_*`) |
| `export_opts.rs` (ExportOptions model + plist gen) | stub (P1) | **P3** (pure → Linux-runnable) | **P3** (`test_generate_export_options_*`, `test_export_options_*`) |
| `keychain.rs` | stub (P1) | **P3** | P3 (`test_create_temp_keychain_*`, `test_import_certificate_*`) |
| `nix/ios2nix-lib.nix buildIOSApp` | — | **P2** owns `run()`; **P3** adds the `signing` branch | P2 (eval) + P3 (signed e2e) |

**Command structs (defined in Plan 2, populated by Plan 3):**
```rust
pub struct SigningConfig { pub team_id:String, pub identity:String, pub profile_uuid:String, pub keychain:PathBuf }
pub struct ArchiveCommand { pub workspace:PathBuf, pub scheme:String, pub configuration:String,
    pub archive_path:PathBuf, pub signing: Option<SigningConfig> }   // None ⇒ unsigned (P2); Some ⇒ manual-signed (P3)
pub struct ExportCommand  { pub archive_path:PathBuf, pub export_opts_plist:PathBuf, pub output_path:PathBuf }
```
**Plist flow (unambiguous):** P3's `export_opts::write_export_options(&opts, path)` writes the
`ExportOptions.plist`; the caller passes that path as `ExportCommand.export_opts_plist`; P2's
`cli::export::run(cmd)` only invokes `xcodebuild -exportArchive -exportOptionsPlist <path>`. P2 never
constructs plist content; P3 never calls xcodebuild. `cli::archive::run` dispatches on
`cmd.signing`: `None` → unsigned flags only (P2); `Some(s)` → append the manual-signing flags (P3 §5a).

**cfg-gating responsibility (enforces P1):** any function Plans 2–3 add with
`#[cfg(target_os="macos")]` MUST ship a `#[cfg(not(target_os="macos"))]` `bail!` arm in the same
change. Plan 1 Phase 0 gate 5 (`cargo check/clippy --workspace` on Linux) is **re-run after each of
Plans 2 and 3** — a missing Linux arm fails the gate and blocks merge.

---

## 3. Global Pre-Mortem (5 scenarios)

1. **Red on Linux CI** — a macOS-only crate/cfg leaves a symbol undefined → breaks the repo-wide
   `cargo check --workspace` gate. *Mitigation:* P1; Plan 1 Phase 0 blocking gates; prefer shelling
   out over FFI.
2. **Reproducible lockfile, irreproducible IPA** — treating `.ipa` like Android's `.aab` → flaky
   signature/timestamp-dependent e2e. *Mitigation:* P4; impure `buildIOSApp`; e2e asserts structure;
   iOS e2e in allowed-to-fail tier.
3. **Wrong pod classification** — a path pod mistaken for third-party (fetch a non-existent zip) or
   vice-versa (a real binary pod silently dropped → link failure). *Mitigation:* classify off the
   podspec `source` field, not name heuristics; fixtures for all 3 kinds; "refuse empty/Flutter-only
   when third-party pods were declared" guard.
4. **Offline `pod install` doesn't actually work** (the central risk) — CocoaPods reaches the CDN or
   needs install-time generation even with `--no-repo-update`. *Mitigation:* **Plan 1 Phase -1 is a
   hard blocker** validating on a representative app (subspecs + bundles + binary + git + path pods)
   before any resolver code; failure → Option A/C.
5. **Git-pod URL encoding doesn't round-trip into Nix** — `git+url#rev` packed in `url` parsed
   wrongly by `fetchgit`, or a dropped `submodules` flag. *Mitigation:* Plan 1 Linux round-trip unit
   test; Plan 2 Nix-eval split test; switch to model (b) if submodules needed.

---

## 4. ADR (consensus-approved)

> **Phase -1 OUTCOME (2026-06-11, macOS 26.4.1 / Xcode 26.3 / CocoaPods 1.16.2):** spike executed —
> **Option B CONFIRMED**. Offline `pod install --no-repo-update` under poisoned proxies reconstructed
> all 22 third-party pods (subspecs, binary xcframeworks, git pod, path pods excluded) from
> pre-seeded caches, Manifest.lock identical, and the offline `Pods.xcodeproj` **built for the
> device SDK with zero network**. **Phase -1.5: unsigned export = NOT feasible** (empirical:
> unsigned archive succeeds; `-exportArchive` fails `No Team Found in Archive`; Xcode 26 renames
> method `development`→`debugging`) → Plan 2 e2e is "archive (unsigned) + export plumbing only",
> first signed `.ipa` is Plan 3's e2e. New binding findings for Plan 2: xcodebuild needs a
> Nix-sanitized env (strip `NIX_*`/`CC`/`CXX`/`LD`/`SDKROOT`/`DEVELOPER_DIR`); nixpkgs-flutter iOS
> app builds are broken upstream (read-only engine framework vs. ad-hoc codesign). Full evidence:
> `docs/ios-podinstall-spike.md`.

- **Decision:** lib+bin Rust crate mirroring gradle2nix; **podspec-driven resolution (B), confirmed
  by the Phase -1 spike (see outcome above)**; **single-source `ios.nodes`**; **strict
  Linux-compilability via cfg-gating + Phase 0 blocking gates**; **explicitly impure `buildIOSApp`**;
  signing modeled in full (Plan 3), not stubbed.
- **Drivers:** can't build here; CocoaPods source heterogeneity; unified-composition parity.
- **Alternatives:** A (proven but Mac-only locking, heavier — the escape hatch if Phase -1 fails);
  C (hybrid, deferred, schema-compatible).
- **Why chosen:** B maximizes Linux-provable value and matches the repo's metadata-driven model *if*
  offline `pod install` works, which Phase -1 proves before commitment — evidence-gated, not assumed.
- **Consequences:** Plans 1 (mostly) provable on Linux CI (mock-provable, not reality-provable — real
  CocoaPods correctness gated by Phase -1 + Fable). Plans 2–3 are the small, sidecar-mockable macOS
  surface. iOS e2e asserts `.ipa` structure only.
- **Follow-ups:** git-pod submodules (model b); Option C capture mode; `buildFlutterIOSApp`;
  TestFlight/App Store Connect upload (out of v1 scope — `destination: upload` leaves the door open).

---

## 5. Consensus record
Planner → Architect → Critic (deliberate mode), two rounds.
- **Round 1** (single plan): Architect surfaced the offline-`pod install` risk; Critic returned
  ITERATE with 7 fixes (Phase -1 blocker, sidecar schema, Phase 0 gates, pre-mortem scenario 4, e2e
  assertions, Option B contingency, relabel CI-provable→mock-provable). All applied.
- **Round 2** (this 3-plan split + Phase 3 signing deep-dive): re-run across all four files; signing
  specifics verified against current Apple/CI docs. Architect flagged seam-ownership gaps + the
  unsigned-export assumption + missing app-extension signing; **Critic verdict: REJECT** with a
  consolidated 10-item checklist. **All 10 applied:** (1) explicit plist flow + `ExportCommand`
  signature; (2) Phase -1.5 unsigned-export sub-spike + Plan 2 e2e reframe; (3) `ArchiveCommand
  {signing: Option<SigningConfig>}` single-command dispatch; (4) re-sign loop now signs
  `.appex` extensions inside-out + documents known limits; (5) Phase-0 Linux-stub responsibility +
  gate-5 re-run after Plans 2/3; (6) provisioning-profile values forced to UUID; (7) seam-ownership
  matrix (§2.5) as single authority; (8) secret-leak runbook warning; (9) Xcode-version method-name
  auto-detection; (10) `test_codegen_cocoapods_*` rename to kill the test-name collision.
Plan status: **pending approval**. No execution performed.
