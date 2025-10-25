## Overview
`lib.rs` implements the Responses API proxy: it parses command-line arguments, reads an upstream authorization header securely, starts an HTTP listener via `tiny_http`, and forwards allowed requests to OpenAI using `reqwest`.

## Detailed Behavior
- `Args` (Clap parser) supports:
  - `--port` to bind a fixed port (default: ephemeral).
  - `--server-info` to write `{ "port": <u16>, "pid": <u32> }` JSON to disk for the caller.
  - `--http-shutdown` to expose `GET /shutdown` for graceful termination.
- `run_main`:
  1. Calls `read_auth_header_from_stdin` to obtain an `Authorization: Bearer <token>` header locked in memory.
  2. Binds a local `TcpListener` (with optional requested port) and writes server info when requested.
  3. Builds a `tiny_http::Server` and a shared blocking `reqwest::Client` with `timeout(None)` for streaming responses.
  4. Loops over incoming requests, spawning per-request threads to forward them.
  5. Supports optional `/shutdown` (when enabled) to exit the process.
- `bind_listener` wraps `TcpListener::bind` and returns the bound socket address.
- `write_server_info` ensures parent directories exist and writes a JSON line with port/pid.
- `forward_request` enforces that only `POST /v1/responses` calls (no query string) are proxied. It:
  - Reads the request body.
  - Copies incoming headers, excluding `Authorization` and `Host`.
  - Injects the stored auth header (marked sensitive) and sets `Host: api.openai.com`.
  - Sends the request to `https://api.openai.com/v1/responses` using `reqwest`.
  - Streams the upstream response back to the client, rewriting headers to remove hop-by-hop items.
- Errors encountered during forwarding are logged to stderr but do not crash the server loop.

## Broader Context
- Designed for tooling that needs a narrow OpenAI proxy without exposing the API key in environment variables or logs. The proxy is intentionally minimal and hardcodes the target path to reduce attack surface.

## Technical Debt
- None beyond potential enhancements to expose metrics or support additional endpoints; currently out of scope.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./read_api_key.rs.spec.md
