# Keymap Rewrite Handoff

Updated: February 19, 2026
Repo: `codex-config-keybinds`
Branch: `joshka/keymap-rewrite-start`

## Purpose

This handoff is the authoritative context for finishing and shipping the keymap rewrite commit
stack.

The original implementation existed as a single large commit (`0f03e76c`). The current stack
reorders that work into additive, review-friendly commits while preserving behavioral intent.

Key decisions that still apply:

1. Preserve end-state behavior equivalent to `0f03e76c`.
2. Keep `AGENTS.md` keymap guidance in the stack.
3. Treat keybinding coverage as a keymap-focused gate (not whole-crate perfection).

## Quick Orientation

Summary for incoming implementer:

1. The keymap feature is mostly implemented and wired.
2. The remaining technical gate is branch coverage closure on keybinding branches.
3. Preserve behavior while tightening internals and documentation quality.
4. Keep help affordances succinct and clearly visible.

Read these in order:

1. `docs/keymap-rewrite-handoff.md`
   - Overall constraints, current status, and concrete next steps.
2. `docs/tui-keymap.md`
   - Long-term architecture and behavior model for configurable keymaps.
3. `docs/keymap-action-matrix.md`
   - Action-by-action reference mapping and expected command behavior.
4. `docs/config.md`
   - User-facing config contract and migration notes for presets.
5. `docs/default-keymap.toml`
   - Canonical template that should match the actual runtime/default behavior.
6. `codex-rs/tui/src/keymap.rs`
   - Core resolver and matching logic.
7. `codex-rs/tui/src/bottom_pane/footer.rs`
   - Help/affordance rendering and keymap customization guidance text.

## Keymap Implementation Constraints (Must Hold)

1. Behavioral contract:
   - Preserve existing runtime behavior first, then refactor.
   - End-state behavior should remain equivalent to `0f03e76c`.
2. Keymap type organization:
   - Keep `TuiKeymap`, `TuiKeymapPreset`, and related keymap types out of oversized umbrella files
     (for example `types.rs`) and in dedicated module files.
3. Preset/versioning policy:
   - Do not mutate historical preset defaults.
   - Add new preset versions for behavior changes and keep `latest` as an explicit pointer.
   - Update both `docs/default-keymap.toml` and `docs/config.md` with migration notes whenever
     preset behavior changes.
4. Simplification policy:
   - Prefer reducing keymap repetition with small declarative macros where readability improves.
   - Centralize key-event matching logic on `KeyBinding`/keybinding helpers rather than repeating
     ad-hoc comparisons across callsites.
   - Document new macros and helper abstractions with rustdoc/doc comments.
   - If `key_hint::plain...` helpers are macroized, document invocation patterns and generated
     behavior with rustdoc and examples.
5. Help and affordance UX:
   - Keep keymap customization affordances succinct.
   - Show the keymap affordance on its own line; avoid wrapped wording that buries the action.
6. Documentation expectations:
   - Write long-term "how this works" documentation, not process diary notes.
   - Keep details that affect behavior, contracts, or extension points.
   - Keep docs self-contained for a new engineer without depending on external notes.
   - Run a documentation pass mindset on changed modules: explain invariants and decision
     boundaries, not just mechanics.
7. Template/docs link policy:
   - `docs/default-keymap.toml` must point to a public URL for canonical docs (GitHub for now;
     later `developers.openai.com`).
8. Test and coverage gates:
   - Add characterization tests for pre-existing event behavior before swapping bindings.
   - Maintain compile correctness across all callsites when APIs change (for example, constructor
     arity updates).
   - Use `cargo llvm-cov` branch coverage and push toward 100% on keybinding-related branches.

## Context And Rationale (Self-Contained)

This document is intentionally self-contained for implementation handoff.

Source of truth for intent and behavior:

1. Existing long-term docs in this repo:
   `docs/tui-keymap.md`, `docs/keymap-action-matrix.md`, and `docs/keymap-rollout-plan.md`.
2. Commit stack and diffs listed below.
3. Test and coverage commands listed in this document.

## Current Additive Stack (Authoritative)

1. `b455e26a4a3e`
   `docs(keymap): establish long-term keymap documentation`

   Scope: long-term docs/spec (`docs/tui-keymap.md`, action matrix, template, config docs,
   rollout plan).

2. `1913b55afb75`
   `feat(core): add keymap config schema and types`

   Scope: additive config surface only (`core/src/config/tui_keymap.rs`, schema, config exports).

3. `187aa6969e03`
   `test(tui): add runtime keymap resolver characterization suite`

   Scope: `tui/src/keymap.rs` and `key_hint` helper expansion, with resolver tests and conflict
   guards, but no broad runtime wiring yet.

4. `aeb6caaecbbc`
   `feat(tui): wire runtime keymap into event handling`

   Scope: replace hardcoded key checks across app/composer/pager/approval/onboarding/list/textarea
   routing with runtime keymap usage.

5. `HEAD`
   `feat(tui): surface keymap customization hints`

   Scope: UX/help affordances (`footer.rs`, tooltip text, snapshot updates) plus this handoff doc.

## Validation Evidence (Current)

All commands below were run in this workspace.

1. Config commit (`1913b55afb75`)
   - `cd codex-rs && just fmt`
   - `cd codex-rs && cargo test -p codex-core --lib`
   - Result: pass (`996 passed; 0 failed; 4 ignored`).

2. Resolver/characterization commit (`187aa6969e03`)
   - `cd codex-rs && just fmt`
   - `cd codex-rs && cargo test -p codex-tui`
   - Result: pass (`797 passed; 0 failed; 2 ignored` + integration/doctest pass).

3. Wiring commit (`aeb6caaecbbc`)
   - `cd codex-rs && just fmt`
   - `cd codex-rs && cargo test -p codex-tui`
   - Result: pass (`806 passed; 0 failed; 2 ignored` + integration/doctest pass).

4. UX/hints commit (`HEAD`)
   - `cd codex-rs && just fmt`
   - `cd codex-rs && cargo test -p codex-tui`
   - Result: pass (`806 passed; 0 failed; 2 ignored` + integration/doctest pass).

5. Post-rebase integration validation (current workspace)
   - `cd codex-rs && just fmt`
   - `cd codex-rs && cargo test -p codex-tui`
   - Result: pass (`932 passed; 0 failed; 2 ignored` + integration/doctest pass).

## Coverage Status (Important)

Branch-coverage closure is still pending and is the main remaining technical gate.

What happened:

1. `cargo llvm-cov --branch` on stable failed as expected (needs nightly `-Z` support).
2. Nightly toolchain exists locally, and `llvm-tools-preview` is installed for nightly.
3. A nightly branch-coverage run was started but interrupted before artifact emission.
4. No `/tmp/codex_tui_cov.json` was produced in the latest attempt.

Implication:

- Do not claim branch-coverage completion yet.
- Next dev should run and record a successful branch report before declaring done.

## Remaining Work To Finish This Effort

1. Complete keybinding branch coverage run and record results.
2. Add missing tests if branch gaps remain in `tui/src/keymap.rs` or key-routing branches.
3. Re-run `cargo test -p codex-tui` after any coverage-driven test additions.
4. Decide whether `docs/keymap-rollout-plan.md` remains in the final PR scope or is dropped as
   implementation scaffolding.
5. Final review pass on commit descriptions and stack readability, then push/open PR.

## Recommended Resume Commands

Run these first to rehydrate state:

```bash
git fetch origin
git status --short
git log --oneline --decorate --graph origin/main..HEAD
git diff --stat origin/main...HEAD
git diff origin/main...HEAD
```

Then run coverage work:

```bash
cd codex-rs
cargo +nightly llvm-cov -p codex-tui --tests --branch --json --output-path /tmp/codex_tui_cov.json
cargo +nightly llvm-cov report --summary-only
```

If needed, inspect key files directly in the coverage JSON and add tests before re-running.

## Notes For Reviewer/Implementer Handoff

1. The stack is intentionally additive and readable now; avoid collapsing commits unless explicitly
   requested.
2. Commit 4 contains the behavioral swap; commit 5 should stay UX-only.
3. Keep this file in-tree until PR merge so reviewers and follow-up implementers share one
   authoritative source of context.
