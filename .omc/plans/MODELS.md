# Model routing for the ios2nix plans

Purpose: decide when the three executable ios2nix plans can be run with Claude Opus 4.8 and when they should be assigned to Claude Fable 5.

## Research snapshot: Fable 5 vs Opus 4.8

Sources checked: Anthropic launch post, Claude API docs, and the current model overview.

- **Claude Fable 5** launched on **2026-06-09** as Anthropic's generally available Mythos-class model. Anthropic positions it above all prior generally available Claude models, especially for long-running software engineering, difficult reasoning, knowledge work, and autonomous work. Source: <https://www.anthropic.com/news/claude-fable-5-mythos-5>
- Fable 5 is intended for **demanding reasoning and long-horizon agentic work**, with a **1M-token context window**, up to **128k output tokens**, and pricing of **$10 / MTok input** and **$50 / MTok output**. Source: <https://platform.claude.com/docs/en/about-claude/models/introducing-claude-fable-5-and-claude-mythos-5>
- Fable 5 has integration caveats: safety classifiers may refuse or route certain requests, especially around cybersecurity, biology/chemistry, and distillation; API users must handle `stop_reason: "refusal"` and fallback behavior. It also carries 30-day retention and is not zero-data-retention eligible. Source: <https://platform.claude.com/docs/en/about-claude/models/introducing-claude-fable-5-and-claude-mythos-5>
- **Claude Opus 4.8** remains Anthropic's strongest Opus-tier model for complex reasoning, long-horizon coding, and high-autonomy work. The docs say to start there when unsure, and use Fable only when the workload needs the highest available capability. Opus 4.8 is cheaper at **$5 / MTok input** and **$25 / MTok output**. Source: <https://platform.claude.com/docs/en/about-claude/models/overview>

## Decision rule

Use **Opus 4.8** when the plan is already well-specified, mostly deterministic, and has tight tests or sidecar fixtures that will catch mistakes.

Use **Fable 5** when the plan contains a high-leverage unknown, long-horizon cross-system debugging, or a failure mode that can invalidate the architecture rather than just a local implementation detail.

## Routing by plan

| Plan | Recommended model | Can it get away with only Opus? | Why |
|---|---:|---:|---|
| `ios2nix-plan-1-resolution-lockfile.md` | **Fable for Phase -1 / -1.5, then Opus for Phases 0-2** | **No, not as a whole plan.** | The pure Rust resolver, lockfile, codegen, and CLI work is well-scoped and Linux-testable, so Opus is enough after the path is proven. But the Phase -1 offline `pod install` spike is the architecture gate: if CocoaPods cannot be reconstructed from podspec metadata, the entire resolver strategy changes from Option B to Option A/C. That one macOS feasibility decision should use Fable. |
| `ios2nix-plan-2-build-nix.md` | **Opus 4.8** | **Yes.** | This is mostly Xcode command orchestration, sidecar-mockable Rust plumbing, and Nix wiring. Plan 1 supplies the lockfile/sandbox contract; Plan 3 owns the hardest signing details. Opus should be sufficient because the plan has clear seams, concrete tests, and fewer architectural unknowns. Escalate to Fable only if real macOS validation exposes opaque Xcode/CocoaPods behavior not covered by the plan. |
| `ios2nix-plan-3-signing-provisioning.md` | **Fable 5** | **No.** | Signing is the riskiest implementation surface: temporary keychains, provisioning-profile UUID extraction, Xcode export plist semantics, nested `.appex` re-signing order, secret handling, and impure macOS e2e verification. Mistakes are easy to miss on Linux and often fail as vague `codesign`/`xcodebuild` errors. This matches Fable's advertised strengths in long-horizon agentic coding and difficult cross-tool reasoning. |

## Practical execution split

1. **Start with Fable on Plan 1 Phase -1 / -1.5.** Produce `docs/ios-podinstall-spike.md` and update the ADR with whether Option B survives.
2. **If the spike passes, hand Plan 1 Phases 0-2 to Opus.** The remaining work is mostly pure Rust/Nix-core code with Linux gates.
3. **Run Plan 2 on Opus.** Keep Fable in reserve for unexpected macOS-only failures.
4. **Run Plan 3 on Fable.** After Fable lands the core design and macOS path, small pure follow-up tests or doc edits can be delegated to Opus.

## Bottom line

- **Can use just Opus:** Plan 2.
- **Should definitely use Fable somewhere:** Plan 1, because of the blocker spike.
- **Should be Fable-owned by default:** Plan 3.
