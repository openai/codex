# TUI Keymap Additive Rollout Plan

This plan restructures the existing keymap implementation into a reviewable
additive stack while preserving the final behavior from revision `0f03e76c`.

## Goals

- Keep the final behavior effectively identical to `0f03e76c`.
- Reorder history to show intent and risk reduction clearly.
- Make each change compile, format cleanly, and pass scoped tests.
- Establish high-confidence behavior characterization before binding rewrites.

## Commit Sequence

1. **Docs and specification foundation**
   - Add/reshape long-term docs that explain how the keymap system works.
   - Include:
     - conceptual model
     - action matrix
     - default keymap template guidance with explicit URL
     - testing strategy notes
   - No functional code change.

2. **Additive config primitives (not wired)**
   - Introduce keymap config types and schema support in `core`.
   - Keep runtime behavior unchanged by not consuming these config values yet.
   - Ensure parsing/serialization paths are complete and documented.

3. **Behavior characterization tests (pre-rewrite)**
   - Add tests that lock down existing event behavior before switching bindings.
   - Cover key event matching and context-sensitive behaviors that must stay stable.
   - Use these tests as the safety net for subsequent rewiring.
   - Run `cargo llvm-cov` and target full branch coverage for keybinding logic.

4. **Binding replacement using characterized behavior**
   - Introduce keymap-driven binding resolution and wire call sites.
   - Replace legacy binding checks while preserving characterized behavior.
   - Keep docs in sync with any semantics that became explicit.

5. **UX polish and affordances**
   - Apply help/footer/tooltip/key-hint refinements once behavior is stable.
   - Keep affordance text concise and formatted for readability.
   - Update snapshots as needed.

## Validation Gates Per Commit

For each commit in the sequence:

1. Run formatting (`just fmt` in `codex-rs`).
2. Run scoped tests for affected crates (minimum `cargo test -p codex-tui` for TUI
   changes and relevant `core` tests when `core` changes).
3. Ensure branch compiles cleanly (`cargo test` path used as compile gate).
4. For characterization and binding commits, run coverage and track branch
   coverage for keybinding logic (`cargo llvm-cov`).

## JJ Workflow

- Build stack from `trunk()` with explicit ordered changes.
- Use `jj new -A` / `jj new -B` to place changes relative to target revisions.
- Rebase existing implementation changes into the new stack as needed.
- Resolve conflicts immediately and keep each change coherent.
- Use clear `jj describe` messages with title + explanatory body.

## Acceptance Criteria

- Final tip behavior matches `0f03e76c` in practice.
- Stack reads top-to-bottom as: docs -> config -> characterization -> rewiring ->
  UX polish.
- Each commit is independently reviewable with passing checks.
