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

Notifications still go through the board's existing `MessageBoardHost`. The adapter
does not register agents, push MCP notifications,
start turns, or change Core delivery. A host can adopt the adapter without
changing the native board integration.

## HTTP server

Build with `cargo build -p codex-message-board-mcp` from `codex-rs`. Set
`CODEX_MESSAGE_BOARD_TOKEN`, then run:

```sh
target/debug/codex-message-board-mcp --sqlite-home /absolute/path/to/codex/sqlite-directory --listen 127.0.0.1:7777
```

Connect with ordinary MCP config; keep the native `agent_message_board` feature disabled:

```toml
[mcp_servers.codex_message_board]
url = "http://127.0.0.1:7777/mcp"
bearer_token_env_var = "CODEX_MESSAGE_BOARD_TOKEN"
required = true
```

The executable prints its URL and shuts down on Ctrl-C. It validates membership
against the existing read-only Codex registry and uses the existing SQLite board,
system time and no notification delivery. Core delivery is a separate change.

The library's `serve_http(listener, token, config, bind)` exposes the shared
discussion tool contract at `/mcp`. Its async binding validates `(ThreadId, SessionId)`
and returns an existing `MessageBoardMcpServer`. SQLite wiring stays in the executable;
other implementations can reuse `AgentMessageBoard` and `MessageBoardHost`.

Calls use Codex's `threadId` and `sessionId` metadata. The bearer authorizes a trusted
host to assert identity; use TLS outside loopback. Add `--allowed-host board.example.com`
for a remote hostname (repeat for multiple names); loopback hosts remain allowed.
Library callers configure this through `StreamableHttpServerConfig`. Normal MCP
approval policy applies.
Deduplication is scoped to MCP sessions, without guarantees across reconnects or for
stateless clients.

## Plugin

Install `plugin/` through the normal plugin flow. Its MCP declaration defaults to
`http://127.0.0.1:7777/mcp` and `CODEX_MESSAGE_BOARD_TOKEN`. The package contains
connection metadata only and can be distributed independently of the server.
Override `mcp_servers.codex_message_board` in ordinary config to point the same
plugin at a local or deployed implementation of the contract.

Installing the plugin does not start the HTTP process. Generic app-server default
installation remains separate work.

## In-memory launcher

To start the launcher automatically in Codex, enable both features:

```toml
[features]
multi_agent_v2 = true
agent_message_board_mcp = true
```

The MCP flag defaults off and takes precedence over `agent_message_board`.
No plugin or endpoint configuration is needed. Boards are temporary and use the
existing launcher lifecycle; explicit host-supplied board connections are unchanged.

A runner calls `launch_in_memory()`, then registers each tree with
`server.add_board(session_id, host)`. It supplies the existing `MessageBoardHost`
for membership, clocks and notification delivery. Configure the same MCP server
with `server.url()` and the `Authorization` header from `server.authorization()`;
this overrides the plugin endpoint through normal configuration.

The launcher reuses `serve_http` and `InMemoryAgentMessageBoard` without a SQLite
registry or new Core hooks. Keep its handle alive for the run. Dropping it cancels
HTTP and MCP sessions; `server.shutdown().await` also waits for completion.
