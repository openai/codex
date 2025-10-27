# Multi-Agent Orchestrator Notes

> **Docs in progress** – All multi-agent design material lives in `ai-temp/` while the feature is under active development. **Do not touch files under `docs/` yet;** when the architecture stabilises we will migrate polished sections there.

## Current Capabilities (2025‑10‑20)
- Delegation runs through the `delegate_agent` tool. Payloads can include `conversation_id` to continue an existing delegate session (`codex-rs/core/src/tools/handlers/delegate.rs`).
- Discovery/preview/dismiss flows are served by the `delegate_sessions` tool with `list`, `messages`, and `dismiss` operations (`codex-rs/core/src/tools/handlers/delegate_sessions.rs`).
- `AgentOrchestrator` (`codex-rs/multi-agent/src/orchestrator.rs`) now:
  - Registers every delegate session, exposing summaries (`DelegateSessionSummary`) and event streams per conversation.
  - Emits `DelegateEvent::{Started,Delta,Completed,Failed,Info}` via a `SessionEventBroadcaster`.
  - Supports follow-ups by preserving the original parent run id (`parent_run_for_follow_up`) before re-registering a conversation.
  - Tracks detached runs and reusable sessions, feeding the `/agent` picker and notifications.
- TUI integration (`codex-rs/tui/src/app.rs`, `chatwidget.rs`, `history_cell.rs`) provides:
  - A delegate tree with indentation per depth, status ownership, and summaries.
  - A picker that offers preview/dismiss actions for saved sessions and detached runs.
  - Dedicated history cells for preview output (`new_delegate_preview`) and consistent routing so sessions never leak updates into each other.
- Tests cover the delegate handler, orchestrator follow-up behaviour, and the TUI presentation. See `codex-rs/multi-agent/src/orchestrator/tests.rs` for regression cases on parent linkage and shadow recording.

## Key Modules & Paths
- Loader & facade: `codex-rs/multi-agent/src/lib.rs` (`AgentConfigLoader`, orchestrator re-export).
- Runtime: `codex-rs/multi-agent/src/orchestrator.rs`.
- Shared tool types: `codex-rs/core/src/delegate_tool.rs`.
- Tool handlers/specs: `codex-rs/core/src/tools/handlers/delegate.rs`, `delegate_sessions.rs`, registry wiring in `codex-rs/core/src/tools/spec.rs`.
- UI: `codex-rs/tui/src/app.rs`, `app_event.rs`, `chatwidget.rs`, `history_cell.rs`, `/agent` picker, status helpers.
- Shadow caching architecture: `ai-temp/agents-shadow-client.md`.
- Follow-up design: `ai-temp/agent-follow-up.md`.

## Behaviour Summary
1. **New run** – `delegate_agent` validates input and calls `AgentOrchestrator::delegate`. The orchestrator spins up a conversation through `ConversationManager`, registers it, and streams events back to the UI.
2. **Follow-up** – When `conversation_id` is supplied, `delegate_follow_up` reuses the stored session. Parent run metadata is captured before re-registration so the TUI can keep lineage straight.
3. **Listing & previews** – `delegate_sessions` pulls from `AgentOrchestrator::list_sessions_paginated` and `recent_messages`, which in turn rely on the shadow cache.
4. **Detached runs** – `mode: "detached"` returns immediately; runs are tracked until completion and surfaced in the picker with dismiss actions.
5. **UI routing** – Each event carries `owner_conversation_id`. `App::handle_delegate_update` forwards deltas and completions only to the matching session handle, while parent summaries are enqueued via `ChildSummary`.

## Outstanding / Planned
- Agent switching (temporarily entering a delegate session) remains a future task – see `ai-temp/agent-switching.md`.
- No edits to `docs/advanced.md` (rolled back) or other public docs until this feature graduates.
- Additional end-to-end CLI tests and documentation polish still required before release.

For subsystem-specific details, consult the dedicated docs in `ai-temp/` (shadow client, error handling, parallel delegation, persistence, follow-ups, etc.). Each file references the relevant source paths so updates stay aligned with implementation.
