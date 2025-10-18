# Shadow Client v2 Specification

## Design Goals

- Keep an in-memory “shadow client” for every active delegate conversation so the user can attach/detach instantly with full fidelity—no rollout replay, no missing turns.
- Preserve rollouts as the source of truth; shadows are an optimization guarded by resource limits and fallbacks.

## Required Behaviour

### 1. Continuous Recording

- Spin up a `ShadowRecorder` alongside each delegate conversation. It subscribes to the same `CodexConversation` stream as the UI.
- Do not stop after the first `TaskComplete`. Continue recording until the delegate session is explicitly closed (or we evict the shadow).
- Record every `EventMsg`, merge deltas with the same `StreamController` logic the live ChatWidget uses, and build ready-to-render `HistoryCell`s. The recorder should output the _exact_ transcript the live widget would show.
- Track delegate capture frames (user inputs, agent outputs), tool events, plan updates, etc., so `ChatWidget::apply_delegate_summary` remains accurate.
- Maintain metrics per session: total events, total bytes (compressed + uncompressed), turn count, last updated timestamp.

### 2. Shadow Storage

- Store snapshots via `ShadowSnapshot` (Arc) containing:
  - Rendered history cells.
  - Raw `EventMsg`s (for diagnostics).
  - Delegate capture frames.
  - Metrics listed above.
- Provide cheap `snapshot()` clones on each update; snapshots are immutable views.
- Optional compression (`compress_shadows` flag) reduces footprint using gzip or a custom binary format; track both raw and compressed byte counts.

### 3. Resource Policy

- Configurable `[multi_agent]` knobs:
  - `max_shadow_sessions` (default 5). `0` disables the count cap.
  - `max_shadow_memory_bytes` (default 100 MiB). `0` disables the memory cap.
  - `compress_shadows` (default false).
- `ShadowManager` enforces the caps using LRU by `last_interacted_at`. Evict the oldest snapshots (drop the cached transcript, keep the live conversation) until under both limits.
- On eviction, emit `DelegateEvent::Info` with a clear message (e.g., “Shadow cache evicted for #critic; next attach will replay from rollout”).
- Update aggregate `shadow_memory_bytes` whenever snapshots are added/removed so `/status` can report accurate totals.

### 4. Orchestrator API

- `AgentOrchestrator` owns a `ShadowManager` alongside existing session maps.
- `run_delegate_task` spawns the recorder task that feeds the manager; the recorder loop continues until `ShutdownComplete` and session removal.
- `enter_session` returns:
  - `ActiveDelegateSession` with the live `CodexConversation`, `SessionConfigured`, `Config`.
  - Optional `ShadowSnapshot` (ready-to-render).
  - Latest `DelegateShadowMetrics`.
- `active_sessions()`, `detached_runs()`, and new helper(s) provide structured metrics (session counts, bytes, events) for UI/telemetry.

### 5. UI Integration

- `App::activate_delegate_session`:
  - If snapshot present → call `ChatWidget::hydrate_from_shadow(snapshot)` (no replay). Hydration should be O(1) with respect to the cached cells.
  - If snapshot missing → show an info banner (“Loading #agent from rollout; shadow cache unavailable”), then fall back to `ConversationManager::resume_conversation_from_rollout`.
- `ChatWidget::hydrate_from_shadow` must:
  - Apply cached history cells directly.
  - Seed delegate capture queues.
  - Restore stream controller state so subsequent deltas append seamlessly.
  - Avoid duplicate commit animations or stale status headers.
- `/status`:
  - Add a “Delegates” section summarizing cached sessions vs total, total bytes vs limit, total recorded events, and detached-run counts.
  - Use a helper (`MultiAgentStatusSummary`) exported from `codex_tui::status`.
- Delegate picker entries:
  - Show shadow stats (bytes/events) when available.
  - Show an explicit “rollout replay required” marker when the snapshot is missing.

### 6. Observability

- Libraries expose `ShadowMetrics` through a stable struct consumed by CLI, TUI, or other front ends.
- `/status` helper returns both text lines and machine-readable data for other surfaces (e.g., API, status card).
- Log important lifecycle events (`Shadow snapshot updated`, `Shadow evicted`, `Shadow compression failed -> fallback`).

### 7. Fallback Guarantees

- Shadow is best-effort. If recorder crashes, session is evicted, or compression fails:
  - Emit `DelegateEvent::Info` so front-ends can display a toast/banner.
  - Future attaches replay from rollout as today.
  - Never crash the delegate conversation; return to rollouts and keep going.

### 8. Testing

- Unit tests:
  - Recorder multi-turn coverage (TaskStarted → Delta → TaskComplete → TaskStarted …).
  - Compression pipeline (round-trip + accounting).
  - Eviction logic (count- and memory-based) including info event emission.
- TUI snapshot/unit tests:
  - `/status` delegate section is rendered correctly.
  - Delegate picker shows “shadow” vs “rollout” variants.
  - Chat hydration from snapshot vs rollout fallback.
- Integration tests (async harness):
  - Simulate multiple delegations and detached runs; ensure shadow survives across turns and fallbacks behave as expected.

### 9. Migration / Rollback

- Keep feature-flag support if needed (`enable_shadow_cache`). The default should remain enabled once feature is stable.
- Compression flag separately controllable; if bugs arise, disable compression without losing other functionality.

## Implementation Roadmap

1. Extract `ShadowRecorder` and shared rendering helpers (refactor out of `ChatWidget` into a reusable module).
2. Implement `ShadowManager` with continuous recording, metrics tracking, and eviction.
3. Update orchestrator APIs and events to consume the new manager.
4. Rework TUI hydration, fallback messaging, `/status`, and delegate picker.
5. Expand configuration, documentation, and telemetry.
6. Add unit/integration tests, then roll out behind (optional) feature flag before removing old code paths.
