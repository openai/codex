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

### Protocol Types (DONE)
- `codex-rs/app-server-protocol/src/protocol/v2.rs`: Added PreToolUse types
  - `ToolCallType`: Enum for tool types (Bash, FileWrite, FileEdit, Mcp, Custom, Function, LocalShell)
  - `ToolCallPreExecuteParams`: Request parameters sent before tool execution
  - `ToolCallPreExecuteDecision`: Allow/Block/Modify decision enum
  - `ToolCallPreExecuteResponse`: Response with decision

- `codex-rs/app-server-protocol/src/protocol/common.rs`: Added request definition
  - `ToolCallPreExecute => "item/toolCall/preExecute"`

### CI/CD (DONE)
- `.github/workflows/craft-release.yml`: Cross-platform release workflow

### Implementation (TODO)

#### 1. Core Event Integration
Location: `codex-rs/core/src/`

Need to add:
- New event type `ToolCallPreExecuteRequestEvent` in `codex-protocol`
- Emit event in tool dispatch pipeline (`tools/router.rs`)
- Wait for response before proceeding with execution

#### 2. App-Server Event Handling
Location: `codex-rs/app-server/src/bespoke_event_handling.rs`

Need to add:
- Handler for `EventMsg::ToolCallPreExecuteRequest`
- Send `ServerRequestPayload::ToolCallPreExecute` to client
- Process response and relay decision back to core

#### 3. Integration Points

The existing approval flow works like this:
```
Core → EventMsg::ExecApprovalRequest → App-Server → Client
Client → Response → App-Server → Core (via pending_approvals channel)
```

PreToolUse needs similar flow but for ALL tools, not just commands/file changes.

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
