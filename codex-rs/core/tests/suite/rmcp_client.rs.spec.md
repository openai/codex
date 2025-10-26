## Overview
End-to-end tests for the RMCP client integrations. They spin up real helper binaries (shipped under `codex-rmcp-client`) and mock Codex responses to verify stdio and streamable HTTP transports, environment propagation, and OAuth token handling. The goal is to ensure MCP tool invocations behave identically across transports and that auth data is persisted and refreshed correctly.

## Detailed Behavior
- **Stdio transport**  
  `stdio_server_round_trip` launches the test server via `cargo run` and checks that Codex emits the correct `McpToolCallBegin/End` events, that structured results contain the expected payload (`echo`, `env`), and that the conversation completes without errors.
  `stdio_server_propagates_whitelisted_env_vars` uses `env_vars` to whitelist an environment variable during launch and asserts the server receives it, proving our env propagation honours per-server configuration.
- **Streamable HTTP transport**  
  `streamable_http_tool_call_round_trip` binds a local HTTP server binary, registers the transport as `StreamableHttp`, and confirms Codex forwards tool calls, captures env snapshots, and streams completions. The helper routine `wait_for_streamable_http_server` polls the spawned process until it is ready or times out.
- **HTTP + OAuth**  
  `streamable_http_with_oauth_round_trip` pre-populates fallback credentials in `CODEX_HOME`, configures the server with OAuth metadata, and ensures the access token is injected via headers. The test also validates that refresh tokens persist across runs and the mock SSE stream completes cleanly.
- Tests rely on `core_test_support::responses` to stub OpenAI-style SSE traffic, `test_codex()` to assemble a full Codex instance, and `skip_if_no_network!` guards so they no-op in restricted environments.

## Broader Context
- Backs up `core/src/mcp` and RMCP client configuration paths, demonstrating that both stdio and streamable transports work end-to-end with approval policies, sandbox settings, and environment forwarding.
- Provides confidence that MCP auth flows (env var whitelist, OAuth refresh store) remain compatible with the user-facing configuration schema.

## Technical Debt
- Tests build and launch helper binaries on the fly (`codex-rmcp-client`), which adds execution time and assumes a local toolchain; targeted unit tests could catch failures earlier.
- OAuth test writes to `.credentials.json` in `CODEX_HOME` and serialises runs via `serial` attribute; refactoring credential handling to accept in-memory stores would remove the filesystem dependency.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Refactor OAuth handling to accept in-memory credential stores so tests can avoid touching the real filesystem.
related_specs:
  - ../../src/mcp/mod.rs.spec.md
  - ../../../rmcp-client/mod.spec.md
