# Sub‑Sessions: Spawn and Await Child Conversations

This document proposes a first design to let a model spawn a new conversation (a new “session”) from within an existing `codex-core` session, and later await its completion to retrieve the final assistant message.

## Goals

- Give the model two new capabilities via tools:
  - `create_session(session_type, prompt) -> { session_id }`
  - `wait_session(session_id, timeout_ms) -> { result }`
- The spawned child session runs independently and returns ONLY its last assistant message as the result.
- Allow the caller to customize the child’s developer prompt, model, and tools.
- Keep parent and child conversations isolated in history, state, and rollout.
- Make it easy to add specialized child session profiles (e.g., linter fixer, math solver).

## Non-goals (initial version):
- Cross-session message streaming to the parent while the child is running.
- Bidirectional piping of tool output between sessions.
- Multi-turn orchestration inside the child beyond the standard Codex task loop.

## User-Facing Interface (tools)

- `create_session`
  - Inputs:
    - `session_type: SessionType` — an enum string (phase 1 presets below). In phase 1, this maps to a fixed developer prompt + model; tools are inherited from the parent’s normal configuration.
    - `prompt: String` — initial user message for the child.
  - Output: `{ session_id: String }` — a UUID v4 formatted string. Internally this will be the child `ConversationId`.

- `wait_session`
  - Inputs:
    - `session_id: String`
    - `timeout_ms: i32` — total time to wait; `<= 0` means “do not wait”.
  - Output: `{ result: String }` — last assistant message of the child session. If the child failed or timed out, return a tool error.

- `cancel_session`
  - Inputs:
    - `session_id: String`
  - Output: `{ cancelled: boolean }` — true if the child was still running and is now cancelled. Errors if unknown `session_id`.

Notes:
- These are exposed as function tools so the model can orchestrate sub‑workflows.
- `create_session` returns immediately after queuing the child run.
- `wait_session` may block the current turn until completion or timeout.

## High‑Level Flow

1. Parent model calls `create_session(...)` during a turn.
2. Core spawns a new Codex conversation using a derived `Config`:
   - Phase 1 mapping from `session_type` enum to a profile with:
     - `base_instructions` override set from the profile’s developer prompt.
     - `model` set from the profile’s configured model.
   - Tools: in phase 1, inherit the parent’s normal tool configuration unchanged.
   - Inherit `cwd`, `sandbox_policy`, and telemetry from parent.
   - Approval policy: in phase 1, child sessions run without interactive approvals (see “Approvals & Safety (Phase 1)” below).
3. Core starts the child by submitting a single `UserInput` (or `UserTurn`) with `prompt`.
4. A background driver consumes the child’s events until `TaskComplete`, capturing `last_agent_message`.
5. `create_session` returns `{ session_id }` to the parent turn.
6. Later, the parent calls `wait_session(session_id, timeout_ms)` to obtain the `{ result }` string (the child’s final assistant message), or a timeout/error.
7. The parent may cancel a running child with `cancel_session(session_id)`.

## Architecture

### New Runtime Service: SubsessionManager

Add a per‑session orchestrator responsible for child conversations:

- Stored on `SessionServices` as `subsessions: SubsessionManager`.
- Holds `HashMap<ConversationId, ChildState>` guarded by `tokio::sync::Mutex`.
- Spawns and monitors child conversations; exposes:
  - `spawn_child(config, prompt) -> ConversationId`
  - `wait_child(id, timeout) -> Result<String, ChildError>`
  - `cancel_child(id) -> bool`
  - `abort_all_children()` for cleanup on parent drop/interrupt.

Child state lifecycle:
- `Pending { handle }` — background task running; `handle` joins to finish.
- `Done { result: Option<String> }` — captured last assistant message; `None` if no assistant message was produced.
- `Failed { error }` — terminal error captured as string.

### Using Existing Building Blocks

- Conversation creation: reuse `ConversationManager::new_conversation(config)` internally or a lighter inline `Codex::spawn`, then wrap in `CodexConversation`.
- Child run loop: submit a `UserInput`/`UserTurn` with the provided `prompt` and consume events until `EventMsg::TaskComplete(TaskCompleteEvent)`; use the embedded `last_agent_message` as the result. Fall back to scanning the final turn’s `ResponseItem`s if needed.
- Rollout: each child conversation records its own rollout file via its own `RolloutRecorder`. For origin, see “SessionSource for Sub‑Sessions” below.
- Parent observability: use `EventMsg::BackgroundEvent` to optionally notify the parent UI when a child is created/completed.

### Session types (Phase 1)

Represented as an enum string from the model, mapped server‑side in `codex-core` to profiles:

- `tester` — a strict, concise test‑writing assistant.
- `mathematician` — a reasoning‑optimized assistant focused on math problems.
- `linter_fixer` — an assistant focused on fixing lint issues.
- `default` — fallback; mirrors parent’s model and uses a small generic task prompt.

Each profile supplies:
- `developer_instructions: String` — appended as base instructions override.
- `model_name: String` — full model id to use in the child.

Tools are inherited from the parent in phase 1; we will add per‑profile tool curation in phase 2.

### Tools and Configuration Mapping (Phase 2 – future)

We will add optional per‑profile tool surface selection by mapping a profile’s `tools` to a `ToolsConfig` subset (mirroring `core/src/tools/spec.rs`). MCP tool allow‑lists would also be supported.

## Error Handling and Timeouts

- `create_session` errors if the child fails to spawn; otherwise always returns a `session_id`.
- `wait_session`:
  - If `timeout_ms <= 0`, behave as a non‑blocking check: return an error if not completed; otherwise return the result.
  - If timed out, return a tool error like: `"session {id} did not complete within {timeout_ms}ms"`.
  - If the child task fails, return the captured error string.

## Isolation, Security, and Policies

- The child inherits the parent’s `cwd` and `sandbox_policy`. Shell execution remains sandboxed as in the parent.
- The child’s tool surface is restricted to `SessionType.tools`.
- No history is shared; the child starts with initial context built from its own `ConfigureSession` only (environment context and developer instructions).

### Approvals & Safety (Phase 1)

- Child sessions do not request interactive approvals in phase 1.
- Implementation: force the child’s `approval_policy` to a non‑interactive mode (e.g., equivalent of “never escalate”), regardless of the parent session’s policy.
- We can add opt‑in approval behaviors to specific session types in phase 2.

## Events and Rollout

- Each child conversation has its own rollout path under `sessions/YYYY/MM/DD/...` with its own `SessionMeta` and `TurnContext` items.
- Parent session emit `BackgroundEvent` messages:
  - On create: `"spawned child session {id} with model {model_name}"`.
  - On complete: `"child session {id} completed"`.
  - On cancel: `child session {id} cancelled`.

### SessionSource for Sub‑Sessions

Today, `SessionMeta.source` distinguishes origins like `Cli`, `VSCode`, `Exec`, and `Mcp` (see `protocol/src/protocol.rs`). Adding `SessionSource::SubSession` would:
- Let rollouts clearly identify runs that were spawned by another session, enabling filtering, analytics, and UI affordances (e.g., “show only child runs”).
- Help group parent/child runs in future UX without relying on naming conventions or directory structure.

Trade‑offs:
- Requires updating the `protocol` crate (Rust + generated TS) and any consumers that switch over `SessionSource`.
- Backward compatibility: default unknown values to `Unknown` in older clients; new servers can safely emit `subsession`.

Phase 1 proposal: keep using an existing source (e.g., `Exec`) for minimal surface change, but reserve the enum value and wire it shortly after to avoid churn across downstreams.

## Relationship to Review Threads (Phase 3)

- Review mode today uses an isolated in‑memory thread (no parent history) inside the same session/task, then emits `ExitedReviewMode` with structured output.
- Sub‑sessions generalize isolation by giving a fully separate conversation with its own lifecycle, model, and tool surface, and an explicit await mechanism.
- We can later re‑implement review as a pre‑configured `SessionType` template if desired. (phase 3)

## Testing Strategy

- Unit tests for `SubsessionManager`:
  - Spawns a child, captures `last_agent_message`, handles failure.
  - Timeout behavior and non‑blocking checks.
  - `abort_all_children()` on parent drop/interrupt.
- Integration tests exercising the tools:
  - Model calls `create_session` then `wait_session` and receives the child result.
  - Cancellation via `cancel_session`.
  - Deeper subsession spawns a subsession itself.

## Incremental Implementation Plan (Phase 1)

1. Add `subsessions` module to `codex-core` with `SubsessionManager` on `SessionServices`; APIs: spawn/wait/cancel/abort_all.
2. Implement background driver that runs a child conversation to `TaskComplete` and stores `{ result }`.
3. Add tool specs and handlers:
   - `create_session(session_type, prompt)` → spawn via profile mapping and return id.
   - `wait_session(session_id, timeout_ms)` → await completion with timeout and return result.
   - `cancel_session(session_id)` → cancel a running child.
4. Phase 1: inherit tools from parent; force approval policy to non‑interactive.
5. Emit optional `BackgroundEvent` diagnostics.
6. Add enum profiles and developer prompts for initial types (tester, mathematician, linter_fixer, default).

## Module Layout (Phase 1)

Inside `codex-core`:
- `core/src/subsessions/mod.rs` — manager, child state, profile mappings.
- `core/src/tools/handlers/subsessions.rs` — handlers for `create_session`, `wait_session`, `cancel_session`.
- Minimal changes in `core/src/state/service.rs` to attach the manager.
- No changes required to `tui` for phase 1; no UI exposure.

## Coding Guidelines Note

- Reuse existing primitives and flows (Codex spawn, submission loop, rollout).
- Prefer refactoring shared logic over duplicating or introducing ad‑hoc hacks; any unavoidable interim workaround should be accompanied by a clear TODO comment and a follow‑up refactor task.