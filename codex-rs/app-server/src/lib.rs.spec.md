## Overview
`codex-app-server::lib` implements the Codex app server binary. It reads JSON-RPC messages from stdin, processes them via `MessageProcessor`, and writes responses to stdout. The server acts as a bridge between frontends and Codex core components.

## Detailed Behavior
- Channel setup:
  - `incoming_tx`/`incoming_rx` (bounded channel) for JSON-RPC messages read from stdin.
  - `outgoing_tx`/`outgoing_rx` (unbounded channel) for messages destined to stdout.
- Tasks:
  1. **stdin reader** (`tokio::spawn`):
     - Reads lines from stdin, deserializes into `JSONRPCMessage`, and sends them to `incoming_tx`.
     - Logs errors for malformed JSON and exits on EOF or channel closure.
  2. **message processor**:
     - Parses CLI config overrides, loads `Config`, and initializes OTEL/tracing layers.
     - Constructs `MessageProcessor`, passing `OutgoingMessageSender`, optional `codex_linux_sandbox_exe`, and shared config.
     - Processes JSON-RPC requests/responses/notifications/errors in a loop.
  3. **stdout writer**:
     - Consumes `OutgoingMessage`s, serializes them to JSON, and writes newline-delimited messages to stdout.
     - Logs serialization/writing errors and exits when the channel closes.
- Shutdown:
  - Tasks exit when stdin hits EOF (dropping `incoming_tx`), which propagates through processor and writer.
  - `run_main` awaits all tasks with `tokio::join!`.
- OTEL/tracing:
  - Configures `tracing_subscriber` with stderr formatting and optional OTEL bridge using `codex_core::otel_init::build_provider`.

## Broader Context
- Binary entrypoint (`main.rs`) wraps `run_main` via `arg0_dispatch_or_else` for sandbox compatibility.
- `MessageProcessor` orchestrates specific commands (see submodules) for fuzzy search, message handling, etc.
- Frontends (CLI/TUI, SDK integrations) communicate with this server to offload workspace operations.

## Technical Debt
- None noted; structure cleanly separates I/O and processing tasks.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./main.rs.spec.md
  - ./message_processor.rs.spec.md (future)
  - ../core/src/config.rs.spec.md
