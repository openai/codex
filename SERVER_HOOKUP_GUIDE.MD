# Codex-CLI Server Hookup Guide

This document describes the new HTTP + WebSocket server for `codex-cli`, provided as a separate binary `codex-server`. It runs alongside the existing `codex` CLI without interfering.

## Installation & Build

1. Install dependencies, including the new `ws` web socket library:
   ```bash
   cd codex-cli
   npm install
   ```
2. Build both the existing CLI and the new server entrypoint:
   ```bash
   npm run build
   ```
   - `dist/cli.js`: original `codex` CLI
   - `dist/server.js`: new `codex-server`

## Running the Server (`codex-server`)

Invoke the new server binary:
```bash
OPENAI_API_KEY=<your-key> codex-server
```

By default it listens on port `3000`. Override with:
```bash
PORT=4000 OPENAI_API_KEY=<your-key> codex-server
```

Required environment variables:
- `OPENAI_API_KEY` (your OpenAI API key)
- `OPENAI_BASE_URL` (optional override of OpenAI endpoint)
- `OPENAI_TIMEOUT_MS` (optional timeout in ms for API calls)

The original `codex` CLI remains unaffected and can still be used via:
```bash
codex --help
``` 

## HTTP Endpoints

- **GET /**
  - Returns HTTP 404 (not implemented)
- **GET /state**
  - Returns current agent state as JSON:
    ```json
    {
      "items": [ /* array of OpenAI ResponseItem */ ],
      "cwd": "/path/to/repo",
      "model": "o4-mini"
    }
    ```
- **POST /prompt**
  - JSON body: `{ "prompt": "Write hello world" }`
  - Success: HTTP 200 `{ "success": true }`
  - Failure: HTTP 400 `{ "error": "Invalid prompt" }`
## WebSocket Stream
- **Endpoint**: `ws://<host>:<port>/ws`
- Stream of JSON messages:
  - `{ "type": "loading", "loading": true|false }`
  - `{ "type": "item", "item": <ResponseItem> }` for each new response chunk
  - `{ "type": "lastResponseId", "id": "..." }`

## Example Usage

Terminal 1: Start server
```bash
OPENAI_API_KEY=YOUR_KEY codex-server
```

Terminal 2: Submit a prompt
```bash
curl -X POST http://localhost:3000/prompt \
     -H 'Content-Type: application/json' \
     -d '{"prompt":"Say hello"}'

curl http://localhost:3000/state
```

Terminal 3: Connect WebSocket (e.g. with `wscat`)
```bash
wscat -c ws://localhost:3000/ws
# you will see streaming JSON messages
```

## Implementation Notes
- The server uses the same `AgentLoop` under the hood in **full-auto** approval mode.
- It does _not_ enforce sandbox write restrictions (commands run directly).
- The HTTP/WebSocket code lives in `src/server.ts`.
- `build.mjs` now includes a second `esbuild` call for `src/server.ts` → `dist/server.js`.
- `package.json` adds:
  - dependency: `ws`
  - devDependency: `@types/ws`
  - bin entry: `"codex-server": "dist/server.js"`

You can extend the server by adding more routes, enabling tool toggles, or exposing metrics (token usage, context window).  This setup leaves the original `codex` CLI fully intact.