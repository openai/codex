# Codex app-server v2 notes

This is a practical map of app-server v2 for Python SDK implementation.

## What app-server is

`codex app-server` is a local JSON-RPC server used by rich clients (VS Code, custom UIs) to control Codex conversation threads/turns, receive streaming events, handle approvals, and manage config/auth.

## Transport

- Default: `stdio://` JSONL (one JSON-RPC message per line)
- Experimental: websocket

SDK v0.1 scope: stdio only.

## Required handshake

1. client request: `initialize`
2. server response: `initialize` result
3. client notification: `initialized`

Any non-initialize request before this should be treated as protocol misuse.

## Core primitives

- Thread: durable conversation container
- Turn: one model run (start -> stream -> completed)
- Item: granular artifacts inside a turn (agent deltas, tool calls, file changes, etc.)

## Core methods used in v0.1 SDK

- `thread/start`
- `thread/resume`
- `thread/list`
- `thread/read`
- `turn/start`
- `turn/interrupt`
- `model/list`

## Event flow

Typical turn:

1. `turn/started`
2. `item/started`
3. zero or more deltas (`item/agentMessage/delta`, `item/commandExecution/outputDelta`, ...)
4. `item/completed` (for each item)
5. `turn/completed`

## Server-initiated requests

app-server may send request messages to the client (not notifications), notably:

- `item/commandExecution/requestApproval`
- `item/fileChange/requestApproval`

Client must answer with a JSON-RPC response payload (`accept` / `decline`).

## Error handling

- JSON-RPC error object should raise typed SDK exception with `code`, `message`, `data`.
- backpressure overload can return error `-32001` (retryable).

## Compatibility strategy for Python SDK

- Preserve exact method names from app-server v2
- keep raw dict payload support (escape hatch)
- add typed wrappers incrementally (non-breaking)
- maintain integration tests around handshake + thread/turn + approvals
