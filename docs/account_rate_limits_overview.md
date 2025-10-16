## Adding the `account/rateLimits/read` JSON-RPC method

1. Extend the app-server protocol (`codex-rs/app-server-protocol/src/protocol.rs`) with the `GetAccountRateLimits` request/response types and method string, and re-export them from `lib.rs`.
2. Wire the method into the app server by routing `"account/rateLimits/read"` in `codex-rs/app-server/src/codex_message_processor.rs`, implementing the handler, and returning the new response type.
3. Update the backend client (`codex-rs/backend-client/src/client.rs` and related traits/impls) to fetch rate limits from the downstream service.
4. If other surfaces consume the API (CLI/TUI/status views), add plumbing in their respective crates (for example `codex-rs/tui/src/status/rate_limits.rs`) to call the new client method and render the response.
5. Add or adjust tests covering protocol serialization, backend-client calls, server handling, and any UI surfaces; regenerate TUI snapshots if the rendered output changes.
