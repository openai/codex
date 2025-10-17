# Remote Access Architecture

The dream scenario I always want to do is after housrs working on desktop, I can
walk away and continue my work from the my iphone, dictate and complete my work.
As as models mature, we find there are less time to do code editon in an editor,
that will make operate everything from an mobile phone possible, I believe the
time to code from a mobile phone has finally come!

This is the part one of the project, which bridges json-rpc with websocket, which
allows any remote client to talk to json-rpc through websocket.
I have already created an iOS project which will just do most the TUI client can
do. It is very cool to sit on the sofa and do the work.
Part III is to modify the TUI client to connect to the WS Bridge, so we can share
the same session amont the clients.

## Codex Remote Access WebSocket Bridge

The websocket bridge turns the Codex JSON-RPC API into a WebSocket endpoint so a
remote device can stay in lockstep with the desktop session. Any client that can
open a WebSocket and speak JSON-RPC 2.0 inherits the full Codex feature set:
submit tasks, receive streamed responses, handle approvals, and resume work from
anywhere without installing the full stack.

## Architecture

```
┌────────────────────────┐
│ Remote Client          │
│ (mobile / browser /    │
│  thin CLI)             │
└─────────▲──────────────┘
          │ WebSocket (JSON-RPC text frames)
          ▼
┌─────────┴──────────────┐
│ codex-app-server-ws    │
│ (Axum WebSocket proxy) │
└─────────▲──────────────┘
          │ Shared in-process API
          ▼
┌─────────┴──────────────┐
│ AppServerEngine        │
│ (codex-app-server)     │
└─────────▲──────────────┘
          │ Conversations, tools, sandbox
          ▼
┌────────────────────────┐
│ codex-core + toolchain │
└────────────────────────┘
```

- Each WebSocket session instantiates `AppServerEngine::new_connection()` and
  gets a dedicated `AppServerConnection` while reusing the shared `AuthManager`
  and `ConversationManager` (tagged `SessionSource::WSRemote`).
- Outbound engine events are forwarded to the client as newline-terminated JSON
  strings over a bounded channel, so stalled clients cannot build unbounded
  backlogs.
- Inbound text frames are parsed into `JSONRPCMessage` variants and dispatched to
  the engine (`process_request`, `process_notification`, `process_response`).
- An optional bearer token enforces basic auth on the upgrade request.

## Message Flow

1. Client connects to `ws://<host>:<port>/ws`.
2. If `--auth-token` was supplied, the handshake must include
   `Authorization: Bearer <token>`.
3. The bridge splits the socket: one task relays engine events to the client,
   the other consumes client messages and forwards them to the engine.
4. Binary frames, ping/pong frames, and parse failures are ignored; close frames
   or socket errors terminate the session.

## Running the Bridge

```
cd codex-rs
cargo run -p codex-app-server-ws -- \
    --bind 0.0.0.0:9100 \
    --auth-token my-secret \
    --profile remote \
    --sandbox workspace-write \
    --ask-for-approval on-failure
```

### CLI Flags

| Flag                               | Description                                                                   |
| ---------------------------------- | ----------------------------------------------------------------------------- |
| `--bind <host:port>`               | Address to listen on (default `127.0.0.1:9100`).                              |
| `--auth-token <token>`             | Require `Authorization: Bearer <token>` during handshake.                     |
| `--codex-linux-sandbox-exe <path>` | Override the sandbox binary on Linux.                                         |
| `-c key=value`                     | Apply repeatable config overrides from `config.toml`.                         |
| `-m, --model <name>`               | Force a specific model for the session.                                       |
| `-p, --profile <name>`             | Load defaults from a named profile.                                           |
| `-s, --sandbox <policy>`           | Select sandbox policy (`workspace-write`, etc.).                              |
| `-a, --ask-for-approval <policy>`  | Configure approval gating (`never`, `on-failure`, ...).                       |
| `-C, --cd <path>`                  | Set the working directory for the engine session.                             |
| `--full-auto`                      | Shortcut for `--sandbox workspace-write` and `--ask-for-approval on-failure`. |

All overrides flow through `Config::load_with_cli_overrides`, so the bridge
shares config parsing logic with the other Codex binaries.

## Client Integration Tips

- Speak standard JSON-RPC 2.0; notifications (no `id`) and responses are both
  supported.
- Keep the socket open for streaming updates—Codex emits intermediate progress
  as notifications before sending the final result.
- Approval prompts arrive as JSON-RPC requests; clients must reply with either a
  result or an error.
- Implement reconnect logic: new connections spin up fresh engine sessions, so
  persist UI state locally if you need continuity beyond conversation history.
- Send periodic pings if you expect to run behind NATs or mobile networks that
  silently drop idle TCP connections.

## Security Considerations

- Prefer binding to `127.0.0.1` and routing traffic through SSH, WireGuard, or
  Tailscale tunnels when operating over the internet.
- Always set `--auth-token` for any deployment beyond your laptop.
- Logging is handled via `tracing`; set `RUST_LOG=info` (or `debug`) to observe
  connection events without exposing payload contents.
- The bridge inherits sandbox and approval policies from `codex-core`. Choose a
  conservative policy when the agent runs unattended.

## Roadmap

- **Part II**: Ship the native iOS client that speaks the same WebSocket
  protocol, matching the existing TUI feature set with touch-first UX.
- **Part III**: Update the TUI to connect through this bridge so every frontend
  (desktop, mobile, browser) shares the same live session by default.
