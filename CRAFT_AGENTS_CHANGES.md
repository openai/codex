# Craft Agents Codex Fork

This fork of [openai/codex](https://github.com/openai/codex) adds PreToolUse hook support for [Craft Agent](https://github.com/lukilabs/craft-agents).

## Why This Fork?

OpenAI's Codex does not support client-side tool interception. Craft Agent requires the ability to:

1. **Block** tool execution with custom error messages (guides model retry behavior)
2. **Modify** tool inputs before execution (path expansion, skill qualification)
3. **Allow** tool execution after validation

This is critical for:

- Permission modes (safe/ask/allow-all)
- Config file validation before writes
- Source auto-activation
- MCP tool permission checking

See [Issue #2109](https://github.com/openai/codex/issues/2109) - 367+ upvotes requesting this feature, but OpenAI is not accepting contributions.

## Changes from Upstream

### Protocol Types

- `codex-rs/app-server-protocol/src/protocol/v2.rs`: Added PreToolUse types

  - `ToolCallType`: Enum for tool types (Bash, FileWrite, FileEdit, Mcp, Custom, Function, LocalShell)
  - `ToolCallPreExecuteParams`: Request parameters sent before tool execution
  - `ToolCallPreExecuteDecision`: Allow/Block/Modify decision enum
  - `ToolCallPreExecuteResponse`: Response with decision

- `codex-rs/app-server-protocol/src/protocol/common.rs`: Added request definition

  - `ToolCallPreExecute => "item/toolCall/preExecute"`

- `codex-rs/protocol/src/approvals.rs`: Added core event types

  - `ToolCallType`: Core enum mirroring V2
  - `ToolCallPreExecuteRequestEvent`: Event sent from core to app-server
  - `ToolCallPreExecuteDecision`: Core decision enum
  - `ToolCallPreExecuteResponse`: Core response type

- `codex-rs/protocol/src/protocol.rs`: Added EventMsg variant
  - `EventMsg::ToolCallPreExecuteRequest`

### Core Implementation

- `codex-rs/core/src/codex.rs`:

  - `request_tool_preexecute()`: Emits PreToolUse event and awaits client decision
  - `notify_tool_preexecute()`: Receives response and unblocks pending request

- `codex-rs/core/src/state/turn.rs`:

  - `pending_tool_preexecutes`: HashMap for tracking in-flight requests
  - `insert_pending_tool_preexecute()` / `remove_pending_tool_preexecute()`

- `codex-rs/core/src/tools/router.rs`:

  - Intercepts ALL tool calls before dispatch
  - Extracts tool type, input, and MCP server info
  - Calls `request_tool_preexecute()` and handles decision:
    - `Block`: Returns `FunctionCallError::Blocked` with reason
    - `Modify`: Updates tool payload with modified input
    - `Allow`: Continues with original payload

- `codex-rs/core/src/function_tool.rs`:
  - `FunctionCallError::Blocked`: New variant for blocked tools

### App-Server Implementation

- `codex-rs/app-server/src/bespoke_event_handling.rs`:
  - Handles `EventMsg::ToolCallPreExecuteRequest`
  - Converts core types to V2 protocol types
  - Sends `ServerRequestPayload::ToolCallPreExecute` to client
  - `on_tool_preexecute_response()`: Processes client response
  - V1 API defaults to Allow (backwards compatible)

### CI/CD

- `.github/workflows/craft-release.yml`: Cross-platform release workflow
  - Triggers on `craft-v*.*.*` tags
  - Builds for macOS (arm64/x64), Linux (x64/arm64), Windows (x64)

## Protocol Flow

```
1. Tool call arrives at router.rs dispatch_tool_call()
2. Extract tool info (type, name, input, MCP server)
3. Call session.request_tool_preexecute() → Creates pending entry, emits EventMsg
4. App-server receives EventMsg::ToolCallPreExecuteRequest
5. App-server sends ServerRequestPayload::ToolCallPreExecute to client
6. Client responds with decision (Allow/Block/Modify)
7. on_tool_preexecute_response() → session.notify_tool_preexecute()
8. request_tool_preexecute() unblocks with response
9. Router handles decision:
   - Allow: Continue to registry.dispatch()
   - Block: Return FunctionCallError::Blocked
   - Modify: Update payload, then dispatch
```

## Client Integration

TypeScript clients receive `item/toolCall/preExecute` requests with:

```typescript
interface ToolCallPreExecuteParams {
  threadId: string;
  turnId: string;
  itemId: string; // call_id for matching
  toolType:
    | "bash"
    | "fileWrite"
    | "fileEdit"
    | "mcp"
    | "custom"
    | "function"
    | "localShell";
  toolName: string;
  input: JsonValue;
  mcpServer?: string; // For MCP tools
  mcpTool?: string; // For MCP tools
}
```

Respond with:

```typescript
interface ToolCallPreExecuteResponse {
  decision:
    | { type: "allow" }
    | { type: "block"; reason: string }
    | { type: "modify"; input: JsonValue };
}
```

## Usage

### Building

```bash
cd codex-rs
cargo build --release --bin codex
```

### Release

```bash
git tag -a craft-v0.1.0 -m "Craft Release 0.1.0"
git push origin craft-v0.1.0
```

## Syncing with Upstream

This fork tracks `openai/codex:main`. To sync:

```bash
git fetch upstream
git merge upstream/main
# Resolve conflicts in Craft-specific files
git push origin main
```

## License

Same as upstream OpenAI Codex.
