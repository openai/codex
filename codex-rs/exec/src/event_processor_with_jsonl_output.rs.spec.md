## Overview
`event_processor_with_jsonl_output` converts Codex events into the JSONL schema consumed by automation. It tracks in-flight commands, patches, MCP calls, and to-do lists, emitting `ThreadEvent` records defined in `exec_events.rs`. It also mirrors last-message handling and token usage reporting.

## Detailed Behavior
- State:
  - `last_message_path` for `--output-last-message`.
  - `next_event_id` (atomic) to generate deterministic item IDs.
  - Hash maps for running commands (`call_id → command/item_id`), patch applies, MCP tool calls, and last critical error.
  - Optional todo list accumulator (`RunningTodoList`) and cached token usage.
- `collect_thread_events` dispatches on `EventMsg` to produce zero or more `ThreadEvent`s:
  - Session/turn lifecycle: `SessionConfigured` → `ThreadStarted`; `TaskStarted` → `turn.started`; `TaskComplete` → `turn.completed` (with usage) or `turn.failed` if an error was recorded.
  - Agent outputs: message/reasoning events become completed `ThreadItem`s (`AgentMessage`, `Reasoning`).
  - Command execution: `ExecCommandBegin/End` create `CommandExecution` items with aggregated output and status.
  - MCP calls, patch applies, todo plan updates, and web searches produce corresponding `ThreadItemDetails` entries, maintaining running state until completion.
  - Error events record `ThreadErrorEvent` and influence the next turn completion status.
  - Token counts cache usage for later summary; plan updates map `StepStatus` to `TodoItem`s.
- `EventProcessor` implementation:
  - `print_config_summary` fakes a `SessionConfigured` event to ensure consistent JSONL output.
  - `process_event` serializes each aggregated `ThreadEvent` to stdout (`println!`), logging serialization errors. When the source event is `TaskComplete`, writes the last agent message to disk and requests shutdown (`CodexStatus::InitiateShutdown`).
- Helpers ensure consistent ID generation (`get_next_item_id`), patch change conversion (`map_change_kind`), and to-do list updates.

## Broader Context
- Enables `codex-exec --json` mode, allowing CI or tooling to consume structured transcripts. The schema is mirrored in TypeScript via `ts_rs`; downstream consumers rely on these shapes for visualizations or audit logs.
- Shares last-message handling with the human processor; both feed into `run_main`’s final output logic.
- Context can't yet be determined for multi-turn sessions; the to-do list logic assumes one active turn at a time.

## Technical Debt
- State management is complex (multiple hash maps); refactoring into domain-specific structs (commands, patches, MCP) would make the logic clearer and easier to test.
- Error handling warns but otherwise continues when begin/end mismatches occur; a stricter invariant (e.g., synthetic items plus metrics) would help detect protocol regressions earlier.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Factor command/patch/MCP tracking into dedicated structs to simplify `collect_thread_events`.
    - Enhance mismatch handling (missing begin/end pairs) with structured diagnostics or metrics to catch protocol drift.
related_specs:
  - ./event_processor.rs.spec.md
  - ./exec_events.rs.spec.md
  - ./lib.rs.spec.md
