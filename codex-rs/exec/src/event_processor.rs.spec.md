## Overview
`exec::event_processor` defines the abstraction that bridges Codex core events to CLI output and shared utilities. Implementations (human-readable and JSONL) conform to this trait, while helper functions handle cross-mode tasks such as writing the last agent message to disk.

## Detailed Behavior
- `CodexStatus` enumerates control-flow signals for the main loop:
  - `Running`: continue streaming events.
  - `InitiateShutdown`: request `Op::Shutdown`.
  - `Shutdown`: exit the event loop.
- `EventProcessor` trait:
  - `print_config_summary` prints initial session info after `SessionConfigured`.
  - `process_event` consumes a `codex_core::protocol::Event` and returns `CodexStatus`.
  - `print_final_output` (default no-op) allows implementations to emit post-run summaries (e.g., final assistant message, token usage).
- Helper functions:
  - `handle_last_message` writes the final agent message (if provided) to a configured path and warns when missing.
  - `write_last_message_file` performs the actual filesystem write, emitting errors to stderr when writing fails.

## Broader Context
- `event_processor_with_human_output` and `event_processor_with_jsonl_output` implement this trait. The main event loop in `exec::lib` relies on the status signals to manage shutdown.
- The last-message helper is shared across both modes so `--output-last-message` behaves consistently regardless of output format.
- Context can't yet be determined for additional event processors (e.g., machine-readable gRPC); future variants should reuse these primitives.

## Technical Debt
- None significant; the abstraction is lightweight and shared logic is encapsulated cleanly.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./event_processor_with_human_output.rs.spec.md
  - ./event_processor_with_jsonl_output.rs.spec.md
  - ./lib.rs.spec.md
