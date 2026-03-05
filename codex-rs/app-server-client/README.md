# codex-app-server-client

Shared in-process app-server client used by conversational CLI surfaces:

- `codex-exec`
- `codex-tui`

## Purpose

This crate centralizes startup and lifecycle management for an in-process
`codex-app-server` runtime, so CLI clients do not need to duplicate:

- app-server bootstrap and initialize handshake
- in-memory request/event transport wiring
- session source selection per client surface
- graceful shutdown behavior

## Client surfaces and session source

`ClientSurface` controls which `SessionSource` is used when starting
app-server:

- `ClientSurface::Exec` -> `SessionSource::Exec`
- `ClientSurface::Tui` -> `SessionSource::Cli`

This ensures thread metadata (for example in `thread/list` and `thread/read`)
matches the originating runtime.

## Transport model

The in-process path uses typed channels:

- client -> server: `ClientRequest` / `ClientNotification`
- server -> client: `InProcessServerEvent`
  - `ServerRequest`
  - `ServerNotification`
  - `LegacyNotification`

JSON serialization is still used at external transport boundaries
(stdio/websocket), but the in-process hot path is typed.

## Backpressure and shutdown

- Queues are bounded and use `DEFAULT_IN_PROCESS_CHANNEL_CAPACITY` by default.
- Full queues return explicit overload behavior instead of unbounded growth.
- `shutdown()` performs a bounded graceful shutdown and then aborts if timeout
  is exceeded.
