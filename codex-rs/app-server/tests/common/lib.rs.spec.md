## Overview
`app_server::tests::common::lib` re-exports the shared test fixtures for convenient use across integration suites and provides JSON-RPC response decoding helpers.

## Detailed Behavior
- Declares submodules (`auth_fixtures`, `mcp_process`, `mock_model_server`, `responses`) and publicly re-exports:
  - `ChatGptAuthFixture`, `ChatGptIdTokenClaims`, `encode_id_token`, `write_chatgpt_auth` for manipulating ChatGPT auth files.
  - `McpProcess` for launching and interacting with the app server.
  - `create_mock_chat_completions_server` to stub OpenAI chat completions over SSE.
  - SSE helper builders for shell, final assistant messages, and apply-patch calls.
- `to_response` takes a `JSONRPCResponse`, extracts the embedded `result`, and deserializes it into the target type using `serde_json`, simplifying test assertions.

## Broader Context
- Test modules import `codex_app_server_tests_common::*` (via dev-dependency) to set up end-to-end scenarios without duplicating boilerplate.

## Technical Debt
- `to_response` assumes the JSON-RPC response body matches the expected schema; invalid payloads bubble up as deserialization errors without additional context.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./auth_fixtures.rs.spec.md
  - ./mcp_process.rs.spec.md
  - ./responses.rs.spec.md
