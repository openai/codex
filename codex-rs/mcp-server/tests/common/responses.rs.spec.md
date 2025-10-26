## Overview
`responses` supplies SSE payload builders for MCP integration tests, mirroring the format returned by the ChatGPT backend during tool invocation.

## Detailed Behavior
- `create_shell_sse_response` formats shell tool-call payloads with serialized arguments (command, optional workdir, timeout) and packages them into `data:` frames terminated with `DONE`.
- `create_final_assistant_message_sse_response` emits a final assistant delta for completions that end the stream.
- `create_apply_patch_sse_response` wraps a diff in a heredoc-based shell command, matching how Codex issues apply_patch calls via the shell runtime.
- Returns strings ready to enqueue into WireMock responders.

## Broader Context
- Used together with the mock model server to simulate streaming tool outputs that the MCP server consumes.

## Technical Debt
- SSE framing logic duplicates helpers in other crates; centralizing these builders would make format updates easier to maintain.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - DRY up SSE builders across app-server and MCP test fixtures to keep them in sync with production expectations.
related_specs:
  - ../mod.spec.md
  - ./mock_model_server.rs.spec.md
