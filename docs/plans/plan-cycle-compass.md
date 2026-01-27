# Cycle Compass — Feature Integration Plan

## Objective
- Ensure collaboration mode cycling in the TUI automatically returns to Plan mode after cycle-driven Code turns, so plan mode is the default for the next user turn without changing explicit mode selections.

## Scope
- In scope:
  - TUI collaboration mode cycling (Shift+Tab) behavior and state handling.
  - Auto-switch logic on task completion when cycle-driven mode selection is active.
  - TUI tests that cover cycle selection and auto-switch behavior.
- Out of scope:
  - Core protocol changes or new config options.
  - Collaboration mode presets, MCP interface, or non-TUI clients (exec, app-server, MCP).
  - TUI2 feature parity work.

## Assumptions
- "Cycle modes" refers to the TUI collaboration mode cycle (Shift+Tab) between Plan and Code presets.
- Auto-switching should apply only when the active mode was set via cycling, not when selected via /collab, plan implementation prompt, or config defaults.
- Auto-switching should avoid interfering with queued user messages and should not trigger the plan implementation prompt for the just-finished Code turn.
- Lenses applied up front: first-principles (define the minimal state transition needed), no-legacy-compat (single-path behavior, no fallback), line-entropy-audit (minimal state and surface area).

## Findings
- Target codebase:
  - Collaboration modes are modeled in `codex-rs/protocol/src/config_types.rs` via `ModeKind`, `CollaborationMode`, and `CollaborationModeMask`.
  - TUI restricts collaboration presets to Plan/Code and defines cycle order in `codex-rs/tui/src/collaboration_modes.rs`.
  - ChatWidget handles cycling on Shift+Tab in `codex-rs/tui/src/chatwidget.rs` and updates the active mask via `set_collaboration_mask`.
  - User turn submission attaches collaboration mode only when an active mask is set (`codex-rs/tui/src/chatwidget.rs` around `Op::UserTurn`).
  - TUI startup uses `tui.experimental_mode` or default Code mode in `codex-rs/tui/src/chatwidget.rs` and config types in `codex-rs/core/src/config/types.rs`.
  - Core session updates collaboration mode from `Op::UserTurn` in `codex-rs/core/src/codex.rs`.
  - Plan mode gates `request_user_input` availability in `codex-rs/core/src/tools/handlers/request_user_input.rs`.
  - Collaboration modes are feature-gated in `codex-rs/core/src/features.rs` and can be selected via `/collab` (`codex-rs/tui/src/slash_command.rs`).
  - Existing tests cover Shift+Tab cycling and defaults in `codex-rs/tui/src/chatwidget/tests.rs`.
  - MCP interface exposes collaboration mode presets in `codex-rs/docs/codex_mcp_interface.md`.
- Reference project (if provided):
  - None.

## Proposed Integration
### Architecture Fit
- Implement the auto-switch as a TUI-only state transition inside `ChatWidget`, preserving core protocol behavior and avoiding new config or API surface.
- Track whether the active collaboration mask was selected via cycling to scope the auto-switch behavior.

### Data & State
- Add a small `ChatWidget` state flag or enum to record the selection source (cycle vs explicit).
- Clear the cycle flag whenever a non-cycle selection is made (`/collab` picker, plan implementation prompt, config default).
- On task completion, if cycle flag is active and the active mode is Code, switch the active mask to Plan after queued inputs are flushed.

### APIs & Contracts
- No protocol changes; continue to rely on existing `Op::UserTurn` and `CollaborationMode` payloads.
- Avoid introducing new config keys or feature flags.

### UI/UX (If Applicable)
- Keep existing footer indicator and Shift+Tab hint unchanged.
- Ensure auto-switch does not trigger the plan implementation prompt for the just-finished Code cycle.

### Background Jobs & Async
- None.

### Config & Feature Flags
- Respect existing `features.collaboration_modes` and `tui.experimental_mode` behavior.

### Observability & Error Handling
- No new telemetry; reuse existing logging and error paths.

## Files To Touch
- `codex-rs/tui/src/chatwidget.rs`
- `codex-rs/tui/src/chatwidget/tests.rs`

## Work Plan
1. Introduce a minimal selection-source flag in `ChatWidget`, and update cycle paths to set it while explicit selection paths clear it.
2. Add auto-switch logic in `on_task_complete` that reverts to Plan only for cycle-driven Code mode, after queued messages are handled and without triggering the plan implementation prompt.
3. Extend `chatwidget` tests to cover auto-switch behavior and queued-message edge cases.

## Risks & Mitigations
- Risk: Auto-switch alters queued message mode unexpectedly.
  Mitigation: Apply auto-switch only when no queued messages remain; otherwise defer until queue is empty.
- Risk: Plan implementation prompt appears after auto-switch.
  Mitigation: Switch after the prompt check or add a suppression flag for the auto-switch path.

## Open Questions
- None (assumptions captured above).

## Test & Validation Plan
- Unit tests: add `chatwidget` tests for cycle-driven auto-switch and deferral with queued messages.
- Integration tests: N/A (TUI-only behavior).
- E2E tests: N/A.
- Manual verification: Start TUI with collaboration modes enabled, use Shift+Tab to select Code, complete a turn, and confirm the mode resets to Plan without extra prompts.

## Rollout
- No migration; behavior gated by existing collaboration modes feature flag.

## Approval
- Status: Pending
- Notes:
