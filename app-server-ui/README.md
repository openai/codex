# Codex App Server UI

Minimal React + Vite client for the codex app-server v2 JSON-RPC protocol.

## Prerequisites

- `codex` CLI available in your PATH (or set `CODEX_BIN`).
- If you are working from this repo, the bridge will prefer the local
  `codex-rs/target/debug/codex-app-server` binary when it exists.
- A configured Codex environment (API key or login) as required by the app-server.

## Quickstart

From the repo root:

```bash
pnpm install
pnpm --filter app-server-ui dev
```

This starts:
- a WebSocket bridge at `ws://localhost:8787` that spawns `codex app-server`
- the Vite dev server at `http://localhost:5173`

## Configuration

- `CODEX_BIN`: path to the `codex` executable (default: `codex`).
- `APP_SERVER_BIN` / `CODEX_APP_SERVER_BIN`: path to a `codex-app-server` binary (overrides `CODEX_BIN`).
- `APP_SERVER_UI_PORT`: port for the bridge server (default: `8787`).
- `VITE_APP_SERVER_WS`: WebSocket URL for the UI (default: `ws://localhost:8787`).
