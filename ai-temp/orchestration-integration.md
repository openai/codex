# Orchestration Integration Overview

This note documents how the multi-agent runtime is currently wired into the Codex CLI/TUI stack. It replaces the older speculative design and mirrors the implementation that shipped on 2025‑10‑20.

---

## 1. Component Map

| Layer | File(s) | Responsibility |
| --- | --- | --- |
| **Loader** | `codex-rs/multi-agent/src/lib.rs` | `AgentConfigLoader` merges global + agent config, exposes `AgentContext`, and re-exports the orchestrator API. |
| **Orchestrator runtime** | `codex-rs/multi-agent/src/orchestrator.rs` | Owns session registry, shadow manager hooks, detached-run registry, follow-up handling, and the `SessionEventBroadcaster`. |
| **Tool handlers** | `codex-rs/core/src/tools/handlers/delegate.rs`, `delegate_sessions.rs` | Translate tool payloads into orchestrator calls, serialize responses/errors, and enforce schema constraints. |
| **Shared types** | `codex-rs/core/src/delegate_tool.rs` | Defines `DelegateToolRequest`, `DelegateSessionsList`, `DelegateToolError`, etc. |
| **TUI integration** | `codex-rs/tui/src/app.rs`, `app_event.rs`, `chatwidget.rs`, `history_cell.rs` | Renders delegate events, maintains per-session handles, offers preview/dismiss actions, and updates history. |

---

## 2. Orchestrator Responsibilities

### Session lifecycle
1. **Creation** – `AgentOrchestrator::delegate` (or `delegate_follow_up`) loads the agent config, spawns a conversation, and registers the run via `register_run_conversation`.
2. **Streaming** – Every conversation gets a `SessionEventBroadcaster`. Event tasks forward individual `Event` values into delegate events scoped to the owning conversation.
3. **Shadow capture** – Recorder hooks record user/agent events into `ShadowManager`. `recent_messages` serves previews from this cache.
4. **Follow-up** – `parent_run_for_follow_up` captures the existing parent id before re-registering the conversation; `delegate_follow_up` reuses the stored `CodexConversation` and emits a fresh `DelegateEvent::Started`.
5. **Detached runs** – `delegate()` records detached runs in `detached_runs`. Completions update status and feed notifications.
6. **Session storage** – `store_session` refreshes `StoredDelegateSession` with the new handle and restarts the event forwarder if needed.

### APIs exposed
- `delegate(...) -> DelegateRunId`
- `list_sessions_paginated(cursor, limit) -> DelegateSessionsList`
- `recent_messages(conversation_id, cursor, limit) -> DelegateSessionMessages`
- `dismiss_session(conversation_id)`
- `subscribe() -> mpsc::UnboundedReceiver<DelegateEvent>`
- Helpers for detached summary, parent lookups, shadow metrics/statistics.

---

## 3. Tool / Model Contract

- **`delegate_agent`**  
  - Requires `prompt`. `agent_id` optional when resuming with `conversation_id`.  
  - Mutually exclusive with `batch`. Batch entries trigger concurrent runs.  
  - Handler subscribes to orchestrator events, waits for completion unless `mode: "detached"`, and returns `{"status":"ok","run_id":...}` or `{"status":"accepted"}` for detached calls.

- **`delegate_sessions`**  
  - `operation: "list"` – paginated session summaries (newest first).  
  - `operation: "messages"` – newest-first message preview with cursor support.  
  - `operation: "dismiss"` – removes the session and cleans up shadow resources.  
  - Responses are serialized as `{ "status": "ok", ... }` with `sessions`, `messages`, and `next_cursor` as appropriate.

Errors map to `DelegateToolError` variants (`AgentBusy`, `SessionNotFound`, `InvalidCursor`, etc.) so the model receives actionable messages.

---

## 4. TUI Flow

1. `App::run` constructs an `AgentOrchestrator` and subscribes to its events via `AppEvent::DelegateUpdate`.
2. `/agent` picker (`ChatWidget::open_delegate_picker`) pulls summaries from `delegate_sessions list`, including detached runs and follow-up sessions.
3. Preview action uses `delegate_sessions messages` and renders the result with `new_delegate_preview` history cells.
4. Dismiss action calls `dismiss_session` through the orchestrator.
5. Delegate events update the active `SessionHandle`:
   - `Started` inserts a running status entry and updates the delegate tree.
   - `Delta` streams through the existing `StreamController`.
   - `Completed`/`Failed` produce history cells, clear status owners, and enqueue `ChildSummary` for the parent conversation.
6. Shadow snapshots hydrate when the user opens a saved session; fallbacks replay from rollout and inform the user.

Detached run notifications surface via the notification system; dismissing them removes the run from the registry.

---

## 5. Follow-Up Handling

- When `delegate_agent` receives `conversation_id`, the handler omits `agent_id` (optional) and sets `caller_conversation_id` so the orchestrator knows which primary conversation owns the request.
- `delegate_follow_up` touches the shadow manager, ensures the conversation is idle, reuses existing `CodexConversation`, and emits a `DelegateEvent::Started` with the original parent id.
- Regression tests (`follow_up_shadow_events_do_not_duplicate`, `follow_up_should_preserve_parent_before_registration`) ensure shadow logging does not double-count and lineage stays intact.

---

## 6. Pending Work / Notes

- **Agent switching** – interactive entry/exit of delegate sessions is implemented; further UX polish is tracked in `ai-temp/agent-switching.md`.
- **docs/** – We intentionally rolled back edits to `docs/advanced.md`. All public documentation will be refreshed once the feature is production-ready.
- **Additional tests** – CLI integration tests and further UX polish (breadcrumbs, status chips) are still on the roadmap.

For subsystem details (shadow cache, error handling, parallel orchestration, persistence), refer to the respective docs in `ai-temp/`.
