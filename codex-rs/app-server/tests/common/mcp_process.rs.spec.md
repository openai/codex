## Overview
`mcp_process` launches the `codex-app-server` binary under test and provides high-level helpers for driving its JSON-RPC interface within integration tests.

## Detailed Behavior
- `McpProcess::new` and `new_with_env` spawn the server via `tokio::process::Command`, wiring stdin/stdout pipes and forwarding stderr lines to the test runner. Environment overrides let tests adjust `CODEX_HOME` or clear variables.
- Internally tracks the next request ID with an `AtomicI64`, uses `VecDeque` to buffer user message notifications, and wraps I/O handles (`ChildStdin`, `BufReader<ChildStdout>`).
- Public methods include:
  - `initialize` to perform the initial JSON-RPC handshake.
  - Request senders such as `send_jsonrpc_request`, `send_initialize_request`, `send_add_conversation_listener_request`, etc., each constructing protocol-specific payloads with the proper `RequestId`.
  - Stream readers (`read_stream_until_response_message`, `read_stream_until_request_message`, `read_stream_until_notification_message`, `read_stream_until_error_message`) that consume JSON-RPC messages until the expected variant arrives while queueing unrelated notifications.
  - Helpers for handling pending user messages and converting them into protocol types for assertions.
- Uses `serde_json` for serialization and `codex_app_server_protocol` enums/structs to keep requests aligned with production protocol.
- Mixes blocking and async tasks: reader loops run under `spawn_blocking` to read the PTY-like stdout, while writer tasks push bytes through Tokio channels.

## Broader Context
- Enables end-to-end validation of the MCP server implementation without embedding server logic inside tests. Specs for `codex-core` unified exec reference this helper as the harness that captures PTY output.

## Technical Debt
- File is large (~500 lines) and couples request construction, logging, and message parsing. Extracting reusable JSON-RPC helpers would simplify future maintenance.

---
tech_debt:
  severity: high
  highest_priority_items:
    - Break the monolithic helper into smaller modules (process management vs. JSON-RPC builders vs. notification buffering) to improve readability and reusability.
related_specs:
  - ../mod.spec.md
  - ./lib.rs.spec.md
