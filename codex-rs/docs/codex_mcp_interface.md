# Codex MCP Server Interface [experimental]

This document describes Codex’s experimental MCP server interface: a JSON‑RPC API that runs over the Model Context Protocol (MCP) transport to control a local Codex engine.

- Status: experimental and subject to change without notice
- Server binary: `codex mcp-server` (or `codex-mcp-server`)
- Transport: standard MCP over stdio (JSON‑RPC 2.0, line‑delimited)

## Overview

Codex exposes a small set of MCP‑compatible methods to create and manage conversations, send user input, receive live events, and handle approval prompts. The types are defined in `protocol/src/mcp_protocol.rs` and re‑used by the MCP server implementation in `mcp-server/`.

At a glance:

- Conversations
  - `newConversation` → start a Codex session
  - `sendUserMessage` / `sendUserTurn` → send user input into a conversation
  - `interruptConversation` → stop the current turn
  - `listConversations`, `resumeConversation`, `archiveConversation`
- Configuration and info
  - `getUserSavedConfig`, `setDefaultModel`, `getUserAgent`, `userInfo`
  - `model/list` → enumerate available models and reasoning options
  - `collaborationMode/list` → enumerate collaboration mode presets (experimental)
- Auth
  - `account/read`, `account/login/start`, `account/login/cancel`, `account/logout`, `account/rateLimits/read`
  - notifications: `account/login/completed`, `account/updated`, `account/rateLimits/updated`
- Utilities
  - `gitDiffToRemote`, `execOneOffCommand`
- Approvals (server → client requests)
  - `applyPatchApproval`, `execCommandApproval`
- Notifications (server → client)
  - `loginChatGptComplete`, `authStatusChange`
  - `codex/event` stream with agent events

See code for full type definitions and exact shapes: `protocol/src/mcp_protocol.rs`.

## Starting the server

Run Codex as an MCP server and connect an MCP client:

```bash
codex mcp-server | your_mcp_client
```

For a simple inspection UI, you can also try:

```bash
npx @modelcontextprotocol/inspector codex mcp-server
```

Use the separate `codex mcp` subcommand to manage configured MCP server launchers in `config.toml`.

## Conversations

Start a new session with optional overrides:

Request `newConversation` params (subset):

- `model`: string model id (e.g. "o3", "gpt-5.1", "gpt-5.1-codex")
- `profile`: optional named profile
- `cwd`: optional working directory
- `approvalPolicy`: `untrusted` | `on-request` | `on-failure` | `never`
- `sandbox`: `read-only` | `workspace-write` | `external-sandbox` (honors `networkAccess` restricted/enabled) | `danger-full-access`
- `config`: map of additional config overrides
- `baseInstructions`: optional instruction override
- `compactPrompt`: optional replacement for the default compaction prompt
- `includePlanTool` / `includeApplyPatchTool`: booleans

Response: `{ conversationId, model, reasoningEffort?, rolloutPath }`

Send input to the active turn:

- `sendUserMessage` → enqueue items to the conversation
- `sendUserTurn` → structured turn with explicit `cwd`, `approvalPolicy`, `sandboxPolicy`, `model`, optional `effort`, `summary`, optional `personality`, and optional `outputSchema` (JSON Schema for the final assistant message)

Valid `personality` values are `friendly`, `pragmatic`, and `none`. When `none` is selected, the personality placeholder is replaced with an empty string.

For v2 threads, `turn/start` also accepts `outputSchema` to constrain the final assistant message for that turn.

Interrupt a running turn: `interruptConversation`.

List/resume/archive: `listConversations`, `resumeConversation`, `archiveConversation`.

For v2 threads, use `thread/list` with `archived: true` to list archived rollouts and
`thread/unarchive` to restore them to the active sessions directory (it returns the restored
thread summary).

## Models

Fetch the catalog of models available in the current Codex build with `model/list`. The request accepts optional pagination inputs:

- `pageSize` – number of models to return (defaults to a server-selected value)
- `cursor` – opaque string from the previous response’s `nextCursor`

Each response yields:

- `items` – ordered list of models. A model includes:
  - `id`, `model`, `displayName`, `description`
  - `supportedReasoningEfforts` – array of objects with:
    - `reasoningEffort` – one of `minimal|low|medium|high`
    - `description` – human-friendly label for the effort
  - `defaultReasoningEffort` – suggested effort for the UI
  - `supportsPersonality` – whether the model supports personality-specific instructions
  - `isDefault` – whether the model is recommended for most users
  - `upgrade` – optional recommended upgrade model id
- `nextCursor` – pass into the next request to continue paging (optional)

## Collaboration modes (experimental)

Fetch the built-in collaboration mode presets with `collaborationMode/list`. This endpoint does not accept pagination and returns the full list in one response:

- `data` – ordered list of collaboration mode masks (partial settings to apply on top of the base mode)
  - For tri-state fields like `reasoning_effort` and `developer_instructions`, omit the field to keep the current value, set it to `null` to clear it, or set a concrete value to update it.

## Event stream

While a conversation runs, the server sends notifications:

- `codex/event` with the serialized Codex event payload. The shape matches `core/src/protocol.rs`’s `Event` and `EventMsg` types. Some notifications include a `_meta.requestId` to correlate with the originating request.
- Auth notifications via method names `loginChatGptComplete` and `authStatusChange`.

Clients should render events and, when present, surface approval requests (see next section).

## Tool responses

The `codex` and `codex-reply` tools return standard MCP `CallToolResult` payloads. For
compatibility with MCP clients that prefer `structuredContent`, Codex mirrors the
content blocks inside `structuredContent` alongside the `threadId`.

Example:

```json
{
  "content": [{ "type": "text", "text": "Hello from Codex" }],
  "structuredContent": {
    "threadId": "019bbed6-1e9e-7f31-984c-a05b65045719",
    "content": "Hello from Codex"
  }
}
```

## Approvals (server → client)

When Codex needs approval to apply changes or run commands, the server issues JSON‑RPC requests to the client:

- `applyPatchApproval { conversationId, callId, fileChanges, reason?, grantRoot? }`
- `execCommandApproval { conversationId, callId, command, cwd, reason? }`

The client must reply with `{ decision: "allow" | "deny" }` for each request.

## Auth helpers

For the complete request/response shapes and flow examples, see the [“Auth endpoints (v2)” section in the app‑server README](../app-server/README.md#auth-endpoints-v2).

## Example: start and send a message

```json
{ "jsonrpc": "2.0", "id": 1, "method": "newConversation", "params": { "model": "gpt-5.1", "approvalPolicy": "on-request" } }
```

Server responds:

```json
{ "jsonrpc": "2.0", "id": 1, "result": { "conversationId": "c7b0…", "model": "gpt-5.1", "rolloutPath": "/path/to/rollout.jsonl" } }
```

Then send input:

```json
{ "jsonrpc": "2.0", "id": 2, "method": "sendUserMessage", "params": { "conversationId": "c7b0…", "items": [{ "type": "text", "text": "Hello Codex" }] } }
```

While processing, the server emits `codex/event` notifications containing agent output, approvals, and status updates.

## Compatibility and stability

This interface is experimental. Method names, fields, and event shapes may evolve. For the authoritative schema, consult `protocol/src/mcp_protocol.rs` and the corresponding server wiring in `mcp-server/`.
