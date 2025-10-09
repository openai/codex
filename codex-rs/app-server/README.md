# codex-app-server

`codex app-server` is the harness Codex uses to power rich interfaces such as the [Codex VS Code extension](https://marketplace.visualstudio.com/items?itemName=openai.chatgpt). The message schema is currently unstable, but those who wish to build experimental UIs on top of Codex may find it valuable.

## Protocol

Similar to [MCP](https://modelcontextprotocol.io/), `codex app-server` supports bidirectional communication, streaming JSONL over stdio. The protocol is JSON-RPC 2.0, though the `"jsonrpc":"2.0"` header is omitted.

## Message Schema

Currently, you can dump a TypeScript version of the schema using `codex generate-ts`. It is specific to the version of Codex you used to run `generate-ts`, so the two are guaranteed to be compatible.

```
codex generate-ts --out DIR
```
