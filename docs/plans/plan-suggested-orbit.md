# Suggested Orbit — Feature Integration Plan

## Objective
- Add an auto-run mode that continuously submits prompt suggestions in a loop when enabled, without user interaction, while preserving existing suggestion UI behavior when the loop is off.

## Scope
- In scope:
  - New config + feature flag for auto-running prompt suggestions.
  - Core loop driver that submits the latest suggestion as a new user turn when idle.
  - TUI + TUI2 wiring to surface/toggle the auto-run mode and keep manual suggestions intact.
  - Safeguards to prevent looping during active tasks, non-empty composer, modals/popups, or review mode.
- Out of scope:
  - Changing suggestion generation prompt or model selection.
  - External API changes or protocol wire format changes.
  - Web/SDK behavior changes outside the Rust CLI/TUI.

## Assumptions
- “Auto-running prompt suggestions in an infinite loop” means: when enabled, each completed turn’s latest suggestion is auto-submitted as the next user message, repeating indefinitely until disabled.
- Loop should respect the same gating as the manual prompt suggestion pane (empty composer, no active task, no modal/popup, not in review mode), plus no queued user messages.
- Feature is CLI/TUI only (SessionSource::Cli), matching existing prompt suggestions behavior.
- A single source of truth for the toggle will live in config features (or a new dedicated config flag if needed), and TUI + TUI2 will read it.
- No legacy compatibility path: new behavior is the only auto-run path and does not preserve any previous “auto-run” behavior (none exists today).

## Findings
- Target codebase:
  - Prompt suggestion generation lives in `codex-rs/core/src/prompt_suggestions.rs` and is triggered from `Session::on_task_finished` in `codex-rs/core/src/tasks/mod.rs`.
  - TUI and TUI2 open suggestion panes on `PromptSuggestionEvent` with gating in `codex-rs/tui/src/chatwidget.rs` and `codex-rs/tui2/src/chatwidget.rs`.
  - Suggestion submission uses `submit_prompt_suggestion` to queue a user message when the suggestion is accepted.
  - Feature flag `prompt_suggestions` exists in `codex-rs/core/src/features.rs` and config schema.
  - Docs for suggestion UX live in `docs/tui-chat-composer.md`.
- Reference project (if provided):
  - Not provided.

## Proposed Integration
### Architecture Fit
- Extend the existing suggestion pipeline: generate suggestion → UI event → optional auto-submit loop. Keep existing UI behavior; add an auto-run controller at the session/UI boundary.

### Data & State
- Introduce a new feature flag (e.g., `prompt_suggestions_autorun`) stored in config and surfaced in feature registry, or reuse an existing config field if available (preferred: new flag for clarity).
- Track latest suggestion (already in chatwidget state) and add an “auto-run enabled” boolean in chatwidget and config.

### APIs & Contracts
- No protocol changes required; reuse `PromptSuggestionEvent`.
- Add TUI app events to toggle auto-run (or reuse feature update flow).

### UI/UX (If Applicable)
- Prompt suggestions view should offer a toggle for auto-run mode (key binding and status line update).
- Auto-run should be visible via status header or hint line to avoid surprise looping.

### Background Jobs & Async
- Auto-run loop should be event-driven, triggered after `PromptSuggestionEvent` or after turn completion when a suggestion exists; avoid busy loops.

### Config & Feature Flags
- Add a new feature flag in `codex-rs/core/src/features.rs` and schema.
- Update config docs if features are documented; document auto-run behavior in `docs/tui-chat-composer.md`.

### Observability & Error Handling
- Log when auto-run submits a suggestion (debug/trace). Ensure no panic on missing suggestion or invalid state.

## Files To Touch
- `codex-rs/core/src/features.rs`
- `codex-rs/core/config.schema.json`
- `codex-rs/tui/src/chatwidget.rs`
- `codex-rs/tui/src/bottom_pane/prompt_suggestions_view.rs`
- `codex-rs/tui2/src/chatwidget.rs`
- `codex-rs/tui2/src/bottom_pane/prompt_suggestions_view.rs`
- `docs/tui-chat-composer.md`

## Work Plan
1. Add a new auto-run prompt suggestions feature flag in core features + schema.
2. Extend TUI/TUI2 prompt suggestion view to toggle auto-run mode and show status.
3. Implement auto-run gating and submission in chatwidget (triggered on suggestion arrival and/or post-turn idle).
4. Update docs to describe auto-run loop behavior and safety gates.

## Risks & Mitigations
- Risk: Infinite loop submits while a task is running or user is typing.
  Mitigation: Gate on task idle, empty composer, no modal/popup, no queued messages, not in review mode.
- Risk: Rapid auto-submits overwhelm rate limits.
  Mitigation: Auto-run only submits one suggestion per completed turn; rely on rate-limit handling already in place.
- Risk: Confusing UX with simultaneous manual suggestion pane.
  Mitigation: When auto-run is enabled, keep pane closed by default and surface an “Auto-run On” hint.

## Open Questions
- Should auto-run disable the suggestion pane entirely or allow it to remain accessible via `/suggestions`?

## Test & Validation Plan
- Unit tests:
  - Add/extend tests in TUI/TUI2 for auto-run gating when suggestion arrives.
- Integration tests:
  - Core event flow: ensure `PromptSuggestionEvent` still delivered.
- E2E tests:
  - Manual: enable auto-run, complete a turn, verify suggestion auto-submits and repeats.
- Manual verification:
  - Toggle auto-run, verify that composing text, modals, or review mode stop auto-submit.

## Rollout
- Feature flag default off (or on if product wants immediate activation). Use config to enable and avoid legacy behavior.

## Approval
- Status: Pending
- Notes: Assumed meaning of “infinite loop” and added gating for safe auto-submit.
