## Overview
`codex-tui::app_event` defines `AppEvent`, the central message enum for TUI state transitions. Events flow from widgets, background tasks, and Codex core into `App::handle_event`, enabling typed communication without tightly coupling components.

## Detailed Behavior
- `AppEvent` variants cover:
  - Codex protocol events (`CodexEvent(Event)`) and agent operations (`CodexOp(Op)`).
  - Application control: starting new sessions, exit requests, commit animation ticks, history insertion, overlays, and deferred diff rendering.
  - UI updates:
    - File search (`StartFileSearch`, `FileSearchResult`) and diff results.
    - Model/effort changes (`UpdateModel`, `UpdateReasoningEffort`, `PersistModelSelection`).
    - Approval workflows (`OpenReasoningPopup`, `OpenFullAccessConfirmation`, `OpenApprovalsPopup`, `FullScreenApprovalRequest`).
    - Sandbox policy/approval policy updates and full-access warning acknowledgments.
    - Conversation history snapshots (`ConversationHistory`), review actions (branch/commit picker, custom prompt).
  - Animation and backpressure control (commit animation start/stop/tick).
- Many variants include structured payloads (`FileMatch`, `ModelPreset`, `ApprovalPreset`, `ApprovalRequest`) so handlers can update specific widgets or persist settings.

## Broader Context
- Producers:
  - Widgets (chat composer, bottom pane) push events via `AppEventSender`.
  - Background tasks (file search manager, diff renderer) send results to the app loop.
  - Session logging records inbound events (see `session_log`).
- Consumers:
  - `App::handle_event` matches on `AppEvent` to route updates (redraw UI, persist config, forward ops to Codex).
- Context can't yet be determined for future multi-window features; new events should continue to flow through this typed channel to avoid global state.

## Technical Debt
-, The enum is large and unstructured; grouping related variants or introducing sub-enums (e.g., `ModelEvent`, `ApprovalEvent`) would clarify semantics.
- Exhaustive matching is required across the app; ensuring unit tests cover new variants would prevent silent no-op cases.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Regroup `AppEvent` into thematic enums or modules to reduce match complexity in handlers.
related_specs:
  - ./app.rs.spec.md
  - ./app_event_sender.rs.spec.md
  - ./bottom_pane/mod.rs.spec.md
