## Overview
`core::codex` implements the primary runtime loop for the Codex agent. It owns session startup, submission handling, turn orchestration, history persistence, rollout logging, MCP bridge wiring, and notification/event fan-out to clients. The module exposes the `Codex` facade used by front ends to submit operations and subscribe to protocol events, and holds the internal `Session` state machine that drives tool execution and model interaction.

## Detailed Behavior
- `Codex::spawn` initializes bounded/unbounded channels for submissions and events, gathers user instructions, and assembles a `SessionConfiguration` from the resolved `Config`. It launches the async `submission_loop`, records initial history, surfaces a `SessionConfigured` event (plus any deferred errors), and returns a `CodexSpawnOk` containing the façade and generated `ConversationId`.
- `Codex` provides `submit`, `submit_with_id`, and `next_event` helpers around the submission/event channels, incrementing `next_id` atomically to guarantee unique submission identifiers.
- `Session` encapsulates per-session state: the outbound event sender, `SessionState` (history, configuration, token usage), active turn tracking, shared services (`SessionServices` with MCP connection manager, unified exec manager, notifier, rollout recorder, auth manager, tooling approvals), and a counter for internally generated submission IDs.
- `Session::new` performs startup routines in parallel (rollout recorder initialization, MCP connections, default shell detection, history metadata discovery, MCP auth status lookup). It builds `SessionServices`, kicks off telemetry (`OtelEventManager`), emits initial events, and handles MCP startup errors by queuing `Error` events for the client.
- Turn lifecycle helpers:
  - `new_turn` / `new_turn_with_sub_id` apply `SessionSettingsUpdate`s, produce a `TurnContext` loaded with model client, sandbox policy, instructions, runtime flags, and optional JSON schema for final output.
  - `build_environment_update_item`, `build_initial_context`, and `record_initial_history` prepare environment deltas, bootstrap instructions, and reconstruct history when resuming from rollout artifacts.
  - `send_event`, `send_event_raw`, `notify_stream_error`, and related methods persist events to rollout logs, update token/rate-limit stats, and forward protocol messages through the outbound channel.
- History management utilities (`record_conversation_items`, `record_into_history`, `replace_history`, `persist_rollout_items`, `history_snapshot`, `clone_history`) coordinate in-memory history with persisted rollout entries, enabling features like auto-compaction to reconstruct state.
- Approval routing (`notify_approval`) resolves pending tool approvals within the active turn, wiring user decisions back to waiting tasks.
- Auto-compaction integration delegates to `codex::compact` helpers to summarize history when context limits are reached.
- Submission processing, task dispatch (`RegularTask`, `ReviewTask`, `CompactTask`), MCP resource handlers, and plan updates live further down the module (not shown here) and coordinate via `TurnContext`, `ToolRouter`, and `SessionServices`.

## Broader Context
- `codex-core` consumers treat `Codex` as the sole entry point to the agent loop; CLI/TUI layers surface user actions by calling `submit` and streaming `Event`s. Specs for UI crates should reference the submission IDs and event sequencing described here.
- The module ties together many subsystems (tasks, tools, sandboxing, telemetry); documentation in those modules should cross-link back to the relevant helper functions in this file to clarify call flow.
- Context can't yet be determined for planned refactors mentioned in inline TODOs (removing `Config` from `SessionConfiguration`, migrating history helpers to `ConversationHistory` APIs). Follow-up specs should document the new architecture when implemented.

## Technical Debt
- `SessionConfiguration` still stores a full `Arc<Config>` (`original_config_do_not_use`); the comment indicates this should be removed to reduce coupling once dependent code migrates.
- `Session::history_snapshot` returns `Vec<ResponseItem>` instead of relying solely on `ConversationHistory`, as noted in the inline TODO; converging on `ConversationHistory` would simplify history management.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Remove `original_config_do_not_use` from `SessionConfiguration` once call sites consume explicit fields.
    - Replace `Session::history_snapshot` with direct `ConversationHistory` usage to avoid duplicated history representations.
related_specs:
  - ../mod.spec.md
  - ./lib.rs.spec.md
  - ./conversation_manager.rs.spec.md
  - ./state/mod.spec.md
  - ./tasks/mod.spec.md
  - ./tools/mod.spec.md
  - ./codex/compact.rs.spec.md
