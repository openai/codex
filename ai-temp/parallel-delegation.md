# Parallel Delegation Options

This note captures the current state of agent delegation in the Codex CLI and outlines
approaches for synchronous, parallel, and detached (fire-and-forget) sub-agent runs.

## 1. Sequential Delegation (Status: available)
- `delegate_agent` blocks until the sub-agent reports `DelegateEvent::Completed`, so the caller
  naturally waits on each run before deciding the next action (`codex-rs/core/src/tools/handlers/delegate.rs`).
- The orchestrator spawns each delegate in its own task but nothing else proceeds until the handler
  resolves (`codex-rs/multi-agent/src/orchestrator.rs`).
- TUI rendering assumes a single streaming leaf at any time; nested runs show as deeper indentation
  but only the top item streams (`codex-rs/tui/src/app.rs`).
- Use case: pipelines where each sub-agent’s result conditions the next prompt.

## 2. Parallel Delegation (Status: implemented)
- Core runtime now registers `delegate_agent` with parallel support and enforces a configurable
  concurrency cap (`[multi_agent].max_concurrent_delegates`, default 5) so front-ends can launch
  multiple sub-agents at once without overwhelming the orchestrator.
- Parallel tool batching is only available on model families that expose
  `supports_parallel_tool_calls`. Today that includes `test-gpt-5-codex`/`codex-*` internal models;
  production tiers (`gpt-5-codex`, `gpt-5`) still force single-function-call turns, so existing
  agents fall back to sequential delegation unless the CLI handles batching locally.
- The TUI replaces the simple stack with a delegate tree that keeps lineage for every run. Nested and
  sibling delegates now render with indentation (two spaces per depth) so siblings appear grouped
  under their parent, while history entries and status headers stay in sync as roots start and finish.
- Streaming output can hop between active delegates; each run maintains its own capture buffer so
  summaries and transcript snippets remain scoped correctly.
- Single-call models now use the handler’s `batch` payload to trigger all delegates in one tool turn,
  so the orchestration layer fans out work even when the model can’t issue multiple function calls.
- Remaining task: expand prompts/docs so agents understand that parallel delegates are available, how
  the UI surfaces them, and when to leverage concurrency.

## 3. Detached Delegation (Status: implemented)
- Introduce `delegate_agent_async` (or `delegate_agent` with `mode: "detached"`) that returns an
  immediate acknowledgement (`{"status":"accepted"}`) without exposing a `run_id`; the model resumes
  its turn while the user decides when to inspect the run.
- The orchestrator now keeps a detached-run registry keyed by run id (pending/ready/failed) with agent
  id, prompt preview, timestamps, and any conversation handle; completion hooks still fan updates into
  the notification bus so the UI can show progress without blocking the caller.
- Surface detached runs in the TUI via notifications plus the existing `/agent` picker: pending entries
  appear as informational rows (with prompt preview) until they finish, failed entries gain a dismissal
  action, and ready runs appear alongside other delegate sessions with an extra "Dismiss detached run"
  option; returning to the main agent can either move the full conversation back into the primary
  transcript or discard it entirely, matching the current non-detached behavior.
- Reuse `[multi_agent].max_concurrent_delegates` as the throttle for detached work while a run is
  actively executing; once it finishes and awaits user review it no longer counts toward the cap.
  When the cap is reached, return a `queue_full` error through the tool response so agents can
  apologize or retry later; do not auto-expire completed runs—only user dismissal removes them.
- Update prompts/docs so agents know when to choose async delegation (long-running tasks, optional
  review) versus synchronous/parallel paths, and capture open questions about acknowledgement metadata
  (beyond `status`/`session_id`/`conversation_id`) and whether summaries should ever auto-inject into
  the main transcript.
- Hook completion into both notification systems:
  - TUI: add a new `Notification::DetachedRunFinished` variant so unfocused terminals surface the
    alert when detached work completes (subject to `tui.notifications` filters).
  - External: extend `UserNotification` to emit a `detached-run-finished` payload when `config.notify`
    is set, keeping headless/automation users in the loop.
- Surface detached sessions in the existing `/agent` picker by reusing `DelegateSessionSummary`:
  mark summaries spawned from detached runs with a `Detached` mode, prefix their labels (e.g.,
  “Detached · #agent · pending/finished”), keep the existing `last_interacted_at` ordering, and
  dispatch the same `AppEvent::EnterDelegateSession` so users attach to the run through the normal
  agent-switch flow. Returning follows the current summary flow (apply vs. dismiss); no additional
  grouping or status-card integration is planned for v1.
- Implementation status: core/tooling now accepts `mode = "detached"`, the orchestrator both tracks
  run mode and maintains the detached-run registry, synchronous responses still stream summaries, the
  TUI renders detached sessions with prefixed labels plus desktop notifications when they finish or
  fail, and external `config.notify` hooks emit `detached-run-finished` payloads (failures populate the
  `error` field rather than using a distinct type).

## 4. Next Decisions
1. Pick a parallelization strategy (simple flag + UI refactor vs. dedicated helper).
2. Specify UX for background runs before adding async variant (notifications, manual join, audit).
3. Extend documentation/prompts once the capabilities land so models know when to choose each path.
