# codex-app-server

`codex app-server` is the harness Codex uses to power rich interfaces such as the [Codex VS Code extension](https://marketplace.visualstudio.com/items?itemName=openai.chatgpt). The message schema is currently unstable, but those who wish to build experimental UIs on top of Codex may find it valuable.

## Protocol

Similar to [MCP](https://modelcontextprotocol.io/), `codex app-server` supports bidirectional communication, streaming JSONL over stdio. The protocol is JSON-RPC 2.0, though the `"jsonrpc":"2.0"` header is omitted.

### Delegating OpenAI Responses HTTP over JSON‑RPC (Experimental)

When the `responses_http_over_jsonrpc` feature is enabled in Codex (see `~/.codex/config.toml`), the app‑server will delegate the actual HTTP `POST /v1/responses` to the JSON‑RPC client. This keeps all agent logic inside app‑server, while allowing an integrating UI to control the network call (e.g., to supply credentials and enforce policy) and stream responses back.

- Server → Client request:
  - Method: `responsesApi/call`
  - Params: `{ conversationId, callId, url, headers: {k: v}, body: <json>, stream: true }`
  - The client should perform the HTTP request and stream SSE events back via notifications (see below).
  - After streaming completes, the client must send a JSON‑RPC response with `{ status, requestId?, error? }`.

- Client → Server notifications (during stream):
  - Method: `responsesApi/event`
  - Params: `{ callId, event }` where `event` is the raw JSON object emitted by the Responses API SSE stream (e.g. `{ "type": "response.output_item.done", ... }`).

- Feature flag (opt‑in): in `~/.codex/config.toml` add
  ```toml
  [features]
  responses_http_over_jsonrpc = true
  ```

The JSON envelopes are mapped 1:1 onto Codex’s internal streaming events, so downstream behavior (streaming assistant text, tool calls, final completion) matches the direct HTTP path.

## Message Schema

Currently, you can dump a TypeScript version of the schema using `codex generate-ts`. It is specific to the version of Codex you used to run `generate-ts`, so the two are guaranteed to be compatible.

```
codex generate-ts --out DIR
```
