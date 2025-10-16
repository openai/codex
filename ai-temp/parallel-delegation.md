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

## 3. Detached Delegation (Status: future work)
- Requires a non-blocking variant (e.g., `delegate_agent_async`) that returns immediately with a
  `run_id` and relies on the orchestrator’s event stream for progress.
- Must surface background activity in the UI: notification list, optional “attach to run”
  command, and stored sessions leveraging `AgentOrchestrator::store_session`.
- Needs policy for auto-cleanup and rate limiting so runaway agents do not flood the orchestrator.
- Join-on-demand flow could reuse existing session switching helpers once a run finishes or when
  the user opts in.

## 4. Next Decisions
1. Pick a parallelization strategy (simple flag + UI refactor vs. dedicated helper).
2. Specify UX for background runs before adding async variant (notifications, manual join, audit).
3. Extend documentation/prompts once the capabilities land so models know when to choose each path.
