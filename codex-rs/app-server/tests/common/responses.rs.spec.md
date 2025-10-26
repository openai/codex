## Overview
`responses` builds streaming response payloads used by integration tests to simulate ChatGPT tool calls and assistant messages over SSE.

## Detailed Behavior
- `create_shell_sse_response(command, workdir, timeout_ms, call_id)` serializes tool-call arguments into JSON, wraps them in an SSE frame consistent with OpenAI streaming, and ensures the event terminates with `data: DONE`.
- `create_final_assistant_message_sse_response(message)` builds a stop-finished assistant delta with the supplied content.
- `create_apply_patch_sse_response(patch_content, call_id)` formats a heredoc shell invocation (`apply_patch <<'EOF' ...`) and packages it as a shell tool call, mirroring how Codex executes apply patch commands.
- Helpers leverage `serde_json::json` for deterministic structure and reuse by `mock_model_server`.

## Broader Context
- Pairs with `mock_model_server::create_mock_chat_completions_server`, which streams these SSE strings back to the app server during tests.
- Aligns with tool handler specs in `codex-core`, ensuring integration parity.

## Technical Debt
- Functions duplicate SSE framing logic scattered elsewhere; consolidating in a shared utility would reduce divergence if OpenAI streaming format changes.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Deduplicate SSE framing helpers across crates to keep streaming format updates centralized.
related_specs:
  - ../mod.spec.md
  - ./mock_model_server.rs.spec.md
