## Overview
`codex-mcp-server` implements a JSON-RPC bridge that exposes Codex tooling through the Model Context Protocol. The crate’s `run_main` routine wires stdin/stdout transports to the internal `MessageProcessor`, loads configuration, and brokers outbound messages through a dedicated channel.

## Detailed Behavior
- Modules:
  - `codex_tool_config` and `codex_tool_runner` adapt Codex tools to MCP schema.
  - `exec_approval` / `patch_approval` define approval request payloads.
  - `outgoing_message` and `message_processor` provide the IO/dispatch plumbing.
- `run_main`:
  - Initializes `tracing_subscriber` using `RUST_LOG` or defaults.
  - Creates a bounded channel (`incoming_tx/rx`) for parsed JSON-RPC messages and an unbounded channel for outgoing messages.
  - Spawns three asynchronous tasks:
    1. **stdin reader**: reads newline-delimited JSON, deserializes `JSONRPCMessage`, and forwards valid messages to `incoming_tx`.
    2. **processor loop**: builds an `OutgoingMessageSender`, instantiates `MessageProcessor` with CLI-derived `Config`, and handles requests/responses/notifications/errors until the channel closes.
    3. **stdout writer**: serializes `OutgoingMessage` values back into JSON-RPC lines written to stdout.
  - Parses CLI overrides once (`CliConfigOverrides::parse_overrides`) and derives the runtime `Config` with `Config::load_with_cli_overrides`, surfacing IO-style errors on failure.
  - Awaits all tasks (via `tokio::join!`); shutdown typically begins when stdin reaches EOF and the incoming channel is dropped.
- Re-exports MCP-specific approval types so integrators can construct requests/responses without reaching into private modules.

## Broader Context
- Serves as the glue between Codex core conversations and MCP clients; the heavy lifting resides in `message_processor` (`./message_processor.rs.spec.md`) and tool modules.
- Shares the arg0 entrypoint pattern with `main.rs` to enable Seatbelt launches when necessary.
- Uses the same configuration loader as other Codex binaries, keeping CLI overrides consistent across the ecosystem.

## Technical Debt
- None noted; async task structure and configuration parsing cover the current feature set.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./main.rs.spec.md
  - ./message_processor.rs.spec.md
  - ./outgoing_message.rs.spec.md
  - ./codex_tool_runner.rs.spec.md
  - ./exec_approval.rs.spec.md
  - ./patch_approval.rs.spec.md
