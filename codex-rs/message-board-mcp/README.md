# Message-board MCP adapter

`MessageBoardMcpServer` implements `rmcp::ServerHandler` over the existing
`codex-message-board` tool factory. It reuses the selected tools' descriptions,
input schemas, validation, permission checks, output budgets and storage behavior.
It has no dependency on `codex-core`.

A host supplies an authorized board connection, caller ID and caller path, then
serves one adapter instance per MCP session using its chosen `rmcp` transport:

```rust,ignore
use codex_message_board::MessageBoardTool;
use codex_message_board_mcp::MessageBoardMcpServer;
use rmcp::ServiceExt;

let server = MessageBoardMcpServer::new(
    &MessageBoardTool::DISCUSSION,
    board,
    caller,
    caller_path,
);
let running = server.serve(transport).await?;
running.waiting().await?;
```

The host owns transport setup and authentication. Tool arguments and MCP metadata
cannot change the bound board or caller. Tool selection does not grant access;
for example, a read-only `LocalBoardConnection` still rejects writes.

Posts retain the board's request deduplication within a session. Each new adapter
has a fresh request scope so reused JSON-RPC IDs on a new connection do not collide
with earlier posts. This does not provide deduplication across reconnects.

Results include JSON text and, for object results, matching structured content.
Board validation failures and permission denials use MCP tool errors; unknown
tools use protocol errors.

Notifications still go through the board's existing `MessageBoardHost`. This
crate does not register agents, launch a server process, push MCP notifications,
start turns, or change Core delivery. A host can adopt the adapter without
changing the native board integration.
