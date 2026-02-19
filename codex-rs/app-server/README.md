# codex-app-server

`codex app-server` is the interface Codex uses to power rich interfaces such as the [Codex VS Code extension](https://marketplace.visualstudio.com/items?itemName=openai.chatgpt).

## Table of Contents

- [Protocol](#protocol)
- [Message Schema](#message-schema)
- [Core Primitives](#core-primitives)
- [Lifecycle Overview](#lifecycle-overview)
- [Initialization](#initialization)
- [API Overview](#api-overview)
- [Events](#events)
- [Approvals](#approvals)
- [Skills](#skills)
- [Apps](#apps)
- [Auth endpoints](#auth-endpoints)
- [Experimental API Opt-in](#experimental-api-opt-in)

## Protocol

Similar to [MCP](https://modelcontextprotocol.io/), `codex app-server` supports bidirectional communication using JSON-RPC 2.0 messages (with the `"jsonrpc":"2.0"` header omitted on the wire).

Supported transports:

- stdio (`--listen stdio://`, default): newline-delimited JSON (JSONL)
- websocket (`--listen ws://IP:PORT`): one JSON-RPC message per websocket text frame (**experimental / unsupported**)

Websocket transport is currently experimental and unsupported. Do not rely on it for production workloads.

Backpressure behavior:

- The server uses bounded queues between transport ingress, request processing, and outbound writes.
- When request ingress is saturated, new requests are rejected with a JSON-RPC error code `-32001` and message `"Server overloaded; retry later."`.
- Clients should treat this as retryable and use exponential backoff with jitter.

## Message Schema

Currently, you can dump a TypeScript version of the schema using `codex app-server generate-ts`, or a JSON Schema bundle via `codex app-server generate-json-schema`. Each output is specific to the version of Codex you used to run the command, so the generated artifacts are guaranteed to match that version.

```
codex app-server generate-ts --out DIR
codex app-server generate-json-schema --out DIR
```

## Core Primitives

The API exposes three top level primitives representing an interaction between a user and Codex:

- **Thread**: A conversation between a user and the Codex agent. Each thread contains multiple turns.
- **Turn**: One turn of the conversation, typically starting with a user message and finishing with an agent message. Each turn contains multiple items.
- **Item**: Represents user inputs and agent outputs as part of the turn, persisted and used as the context for future conversations. Example items include user message, agent reasoning, agent message, shell command, file edit, etc.

Use the thread APIs to create, list, or archive conversations. Drive a conversation with turn APIs and stream progress via turn notifications.

## Lifecycle Overview

- Initialize once per connection: Immediately after opening a transport connection, send an `initialize` request with your client metadata, then emit an `initialized` notification. Any other request on that connection before this handshake gets rejected.
- Start (or resume) a thread: Call `thread/start` to open a fresh conversation. The response returns the thread object and you’ll also get a `thread/started` notification. If you’re continuing an existing conversation, call `thread/resume` with its ID instead. If you want to branch from an existing conversation, call `thread/fork` to create a new thread id with copied history.
- Begin a turn: To send user input, call `turn/start` with the target `threadId` and the user's input. Optional fields let you override model, cwd, sandbox policy, etc. This immediately returns the new turn object and triggers a `turn/started` notification.
- Stream events: After `turn/start`, keep reading JSON-RPC notifications on stdout. You’ll see `item/started`, `item/completed`, deltas like `item/agentMessage/delta`, tool progress, etc. These represent streaming model output plus any side effects (commands, tool calls, reasoning notes).
- Finish the turn: When the model is done (or the turn is interrupted via making the `turn/interrupt` call), the server sends `turn/completed` with the final turn state and token usage.

## Initialization

Clients must send a single `initialize` request per transport connection before invoking any other method on that connection, then acknowledge with an `initialized` notification. The server returns the user agent string it will present to upstream services; subsequent requests issued before initialization receive a `"Not initialized"` error, and repeated `initialize` calls on the same connection receive an `"Already initialized"` error.

`initialize.params.capabilities` also supports per-connection notification opt-out via `optOutNotificationMethods`, which is a list of exact method names to suppress for that connection. Matching is exact (no wildcards/prefixes). Unknown method names are accepted and ignored.

Applications building on top of `codex app-server` should identify themselves via the `clientInfo` parameter.

**Important**: `clientInfo.name` is used to identify the client for the OpenAI Compliance Logs Platform. If
you are developing a new Codex integration that is intended for enterprise use, please contact us to get it
added to a known clients list. For more context: https://chatgpt.com/admin/api-reference#tag/Logs:-Codex

Example (from OpenAI's official VSCode extension):

```json
{
  "method": "initialize",
  "id": 0,
  "params": {
    "clientInfo": {
      "name": "codex_vscode",
      "title": "Codex VS Code Extension",
      "version": "0.1.0"
    }
  }
}
```

Example with notification opt-out:

```json
{
  "method": "initialize",
  "id": 1,
  "params": {
    "clientInfo": {
      "name": "my_client",
      "title": "My Client",
      "version": "0.1.0"
    },
    "capabilities": {
      "experimentalApi": true,
      "optOutNotificationMethods": [
        "codex/event/session_configured",
        "item/agentMessage/delta"
      ]
    }
  }
}
```

## API Overview

- `thread/start` — create a new thread; emits `thread/started` and auto-subscribes you to turn/item events for that thread.
- `thread/resume` — reopen an existing thread by id so subsequent `turn/start` calls append to it.
- `thread/fork` — fork an existing thread into a new thread id by copying the stored history; emits `thread/started` and auto-subscribes you to turn/item events for the new thread.
- `thread/list` — page through stored rollouts; supports cursor-based pagination and optional `modelProviders`, `sourceKinds`, `archived`, and `cwd` filters. Each returned `thread` includes `status` (`ThreadStatus`), defaulting to `notLoaded` when the thread is not currently loaded.
- `thread/loaded/list` — list the thread ids currently loaded in memory.
- `thread/read` — read a stored thread by id without resuming it; optionally include turns via `includeTurns`. The returned `thread` includes `status` (`ThreadStatus`), defaulting to `notLoaded` when the thread is not currently loaded.
- `thread/status/changed` — notification emitted when a loaded thread’s status changes (`threadId` + new `status`).
- `thread/archive` — move a thread’s rollout file into the archived directory; returns `{}` on success and emits `thread/archived`.
- `thread/name/set` — set or update a thread’s user-facing name; returns `{}` on success. Thread names are not required to be unique; name lookups resolve to the most recently updated thread.
- `thread/unarchive` — move an archived rollout file back into the sessions directory; returns the restored `thread` on success and emits `thread/unarchived`.
- `thread/compact/start` — trigger conversation history compaction for a thread; returns `{}` immediately while progress streams through standard turn/item notifications.
- `thread/backgroundTerminals/clean` — terminate all running background terminals for a thread (experimental; requires `capabilities.experimentalApi`); returns `{}` when the cleanup request is accepted.
- `thread/rollback` — drop the last N turns from the agent’s in-memory context and persist a rollback marker in the rollout so future resumes see the pruned history; returns the updated `thread` (with `turns` populated) on success.
- `turn/start` — add user input to a thread and begin Codex generation; responds with the initial `turn` object and streams `turn/started`, `item/*`, and `turn/completed` notifications. For `collaborationMode`, `settings.developer_instructions: null` means "use built-in instructions for the selected mode".
- `turn/steer` — add user input to an already in-flight turn without starting a new turn; returns the active `turnId` that accepted the input.
- `turn/interrupt` — request cancellation of an in-flight turn by `(thread_id, turn_id)`; success is an empty `{}` response and the turn finishes with `status: "interrupted"`.
- `review/start` — kick off Codex’s automated reviewer for a thread; responds like `turn/start` and emits `item/started`/`item/completed` notifications with `enteredReviewMode` and `exitedReviewMode` items, plus a final assistant `agentMessage` containing the review.
- `command/exec` — run a single command under the server sandbox without starting a thread/turn (handy for utilities and validation).
- `model/list` — list available models (set `includeHidden: true` to include entries with `hidden: true`), with reasoning effort options and optional `upgrade` model ids.
- `experimentalFeature/list` — list feature flags with stage metadata (`beta`, `underDevelopment`, `stable`, etc.), enabled/default-enabled state, and cursor pagination. For non-beta flags, `displayName`/`description`/`announcement` are `null`.
- `collaborationMode/list` — list available collaboration mode presets (experimental, no pagination).
- `skills/list` — list skills for one or more `cwd` values (optional `forceReload`).
- `skills/remote/list` — list public remote skills (**under development; do not call from production clients yet**).
- `skills/remote/export` — download a remote skill by `hazelnutId` into `skills` under `codex_home` (**under development; do not call from production clients yet**).
- `app/list` — list available apps.
- `skills/config/write` — write user-level skill config by path.
- `mcpServer/oauth/login` — start an OAuth login for a configured MCP server; returns an `authorization_url` and later emits `mcpServer/oauthLogin/completed` once the browser flow finishes.
- `tool/requestUserInput` — prompt the user with 1–3 short questions for a tool call and return their answers (experimental).
- `config/mcpServer/reload` — reload MCP server config from disk and queue a refresh for loaded threads (applied on each thread's next active turn); returns `{}`. Use this after editing `config.toml` without restarting the server.
- `mcpServerStatus/list` — enumerate configured MCP servers with their tools, resources, resource templates, and auth status; supports cursor+limit pagination.
- `windowsSandbox/setupStart` — start Windows sandbox setup for the selected mode (`elevated` or `unelevated`); returns `{ started: true }` immediately and later emits `windowsSandbox/setupCompleted`.
- `feedback/upload` — submit a feedback report (classification + optional reason/logs and conversation_id); returns the tracking thread id.
- `command/exec` — run a single command under the server sandbox without starting a thread/turn (handy for utilities and validation).
- `config/read` — fetch the effective config on disk after resolving config layering.
- `config/value/write` — write a single config key/value to the user's config.toml on disk.
- `config/batchWrite` — apply multiple config edits atomically to the user's config.toml on disk.
- `configRequirements/read` — fetch loaded requirements constraints from `requirements.toml` and/or MDM (or `null` if none are configured), including allow-lists (`allowedApprovalPolicies`, `allowedSandboxModes`, `allowedWebSearchModes`), `enforceResidency`, and `network` constraints.

### Example: Start or resume a thread

Start a fresh thread when you need a new Codex conversation.

```json
{ "method": "thread/start", "id": 10, "params": {
    // Optionally set config settings. If not specified, will use the user's
    // current config settings.
    "model": "gpt-5.1-codex",
    "cwd": "/Users/me/project",
    "approvalPolicy": "never",
    "sandbox": "workspaceWrite",
    "personality": "friendly",
    // Experimental: requires opt-in
    "dynamicTools": [
        {
            "name": "lookup_ticket",
            "description": "Fetch a ticket by id",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": { "type": "string" }
                },
                "required": ["id"]
            }
        }
    ],
} }
{ "id": 10, "result": {
    "thread": {
        "id": "thr_123",
        "preview": "",
        "modelProvider": "openai",
        "createdAt": 1730910000
    }
} }
{ "method": "thread/started", "params": { "thread": { … } } }
```

Valid `personality` values are `"friendly"`, `"pragmatic"`, and `"none"`. When `"none"` is selected, the personality placeholder is replaced with an empty string.

To continue a stored session, call `thread/resume` with the `thread.id` you previously recorded. The response shape matches `thread/start`, and no additional notifications are emitted. You can also pass the same configuration overrides supported by `thread/start`, such as `personality`:

```json
{ "method": "thread/resume", "id": 11, "params": {
    "threadId": "thr_123",
    "personality": "friendly"
} }
{ "id": 11, "result": { "thread": { "id": "thr_123", … } } }
```

To branch from a stored session, call `thread/fork` with the `thread.id`. This creates a new thread id and emits a `thread/started` notification for it:

```json
{ "method": "thread/fork", "id": 12, "params": { "threadId": "thr_123" } }
{ "id": 12, "result": { "thread": { "id": "thr_456", … } } }
{ "method": "thread/started", "params": { "thread": { … } } }
```

Experimental API: `thread/start`, `thread/resume`, and `thread/fork` accept `persistExtendedHistory: true` to persist a richer subset of ThreadItems for non-lossy history when calling `thread/read`, `thread/resume`, and `thread/fork` later. This does not backfill events that were not persisted previously.

### Example: List threads (with pagination & filters)

`thread/list` lets you render a history UI. Results default to `createdAt` (newest first) descending. Pass any combination of:

- `cursor` — opaque string from a prior response; omit for the first page.
- `limit` — server defaults to a reasonable page size if unset.
- `sortKey` — `created_at` (default) or `updated_at`.
- `modelProviders` — restrict results to specific providers; unset, null, or an empty array will include all providers.
- `sourceKinds` — restrict results to specific sources; omit or pass `[]` for interactive sessions only (`cli`, `vscode`).
- `archived` — when `true`, list archived threads only. When `false` or `null`, list non-archived threads (default).
- `cwd` — restrict results to threads whose session cwd exactly matches this path.

Example:

```json
{ "method": "thread/list", "id": 20, "params": {
    "cursor": null,
    "limit": 25,
    "sortKey": "created_at"
} }
{ "id": 20, "result": {
    "data": [
        { "id": "thr_a", "preview": "Create a TUI", "modelProvider": "openai", "createdAt": 1730831111, "updatedAt": 1730831111, "status": { "type": "notLoaded" } },
        { "id": "thr_b", "preview": "Fix tests", "modelProvider": "openai", "createdAt": 1730750000, "updatedAt": 1730750000, "status": { "type": "notLoaded" } }
    ],
    "nextCursor": "opaque-token-or-null"
} }
```

When `nextCursor` is `null`, you’ve reached the final page.

### Example: List loaded threads

`thread/loaded/list` returns thread ids currently loaded in memory. This is useful when you want to check which sessions are active without scanning rollouts on disk.

```json
{ "method": "thread/loaded/list", "id": 21 }
{ "id": 21, "result": {
    "data": ["thr_123", "thr_456"]
} }
```

### Example: Track thread status changes

`thread/status/changed` is emitted whenever a loaded thread's status changes:

- Includes `threadId` and the new `status`.
- Status can be `notLoaded`, `idle`, `systemError`, or `active` (with `activeFlags`; `active` implies running).

```json
{ "method": "thread/status/changed", "params": {
    "threadId": "thr_123",
    "status": { "type": "active", "activeFlags": [] }
} }
```

### Example: Read a thread

Use `thread/read` to fetch a stored thread by id without resuming it. Pass `includeTurns` when you want the rollout history loaded into `thread.turns`.

```json
{ "method": "thread/read", "id": 22, "params": { "threadId": "thr_123" } }
{ "id": 22, "result": {
    "thread": { "id": "thr_123", "status": { "type": "notLoaded" }, "turns": [] }
} }
```

```json
{ "method": "thread/read", "id": 23, "params": { "threadId": "thr_123", "includeTurns": true } }
{ "id": 23, "result": {
    "thread": { "id": "thr_123", "status": { "type": "notLoaded" }, "turns": [ ... ] }
} }
```

### Example: Archive a thread

Use `thread/archive` to move the persisted rollout (stored as a JSONL file on disk) into the archived sessions directory.

```json
{ "method": "thread/archive", "id": 21, "params": { "threadId": "thr_b" } }
{ "id": 21, "result": {} }
{ "method": "thread/archived", "params": { "threadId": "thr_b" } }
```

An archived thread will not appear in `thread/list` unless `archived` is set to `true`.

### Example: Unarchive a thread

Use `thread/unarchive` to move an archived rollout back into the sessions directory.

```json
{ "method": "thread/unarchive", "id": 24, "params": { "threadId": "thr_b" } }
{ "id": 24, "result": { "thread": { "id": "thr_b" } } }
{ "method": "thread/unarchived", "params": { "threadId": "thr_b" } }
```

### Example: Trigger thread compaction

Use `thread/compact/start` to trigger manual history compaction for a thread. The request returns immediately with `{}`.

Progress is emitted as standard `turn/*` and `item/*` notifications on the same `threadId`. Clients should expect a single compaction item:

- `item/started` with `item: { "type": "contextCompaction", ... }`
- `item/completed` with the same `contextCompaction` item id

While compaction is running, the thread is effectively in a turn so clients should surface progress UI based on the notifications.

```json
{ "method": "thread/compact/start", "id": 25, "params": { "threadId": "thr_b" } }
{ "id": 25, "result": {} }
```

### Example: Start a turn (send user input)

Turns attach user input (text or images) to a thread and trigger Codex generation. The `input` field is a list of discriminated unions:

- `{"type":"text","text":"Explain this diff"}`
- `{"type":"image","url":"https://…png"}`
- `{"type":"localImage","path":"/tmp/screenshot.png"}`

You can optionally specify config overrides on the new turn. If specified, these settings become the default for subsequent turns on the same thread. `outputSchema` applies only to the current turn.

```json
{ "method": "turn/start", "id": 30, "params": {
    "threadId": "thr_123",
    "input": [ { "type": "text", "text": "Run tests" } ],
    // Below are optional config overrides
    "cwd": "/Users/me/project",
    "approvalPolicy": "unlessTrusted",
    "sandboxPolicy": {
        "type": "workspaceWrite",
        "writableRoots": ["/Users/me/project"],
        "networkAccess": true
    },
    "model": "gpt-5.1-codex",
    "effort": "medium",
    "summary": "concise",
    "personality": "friendly",
    // Optional JSON Schema to constrain the final assistant message for this turn.
    "outputSchema": {
        "type": "object",
        "properties": { "answer": { "type": "string" } },
        "required": ["answer"],
        "additionalProperties": false
    }
} }
{ "id": 30, "result": { "turn": {
    "id": "turn_456",
    "status": "inProgress",
    "items": [],
    "error": null
} } }
```

### Example: Start a turn (invoke a skill)

Invoke a skill explicitly by including `$<skill-name>` in the text input and adding a `skill` input item alongside it.

```json
{ "method": "turn/start", "id": 33, "params": {
    "threadId": "thr_123",
    "input": [
        { "type": "text", "text": "$skill-creator Add a new skill for triaging flaky CI and include step-by-step usage." },
        { "type": "skill", "name": "skill-creator", "path": "/Users/me/.codex/skills/skill-creator/SKILL.md" }
    ]
} }
{ "id": 33, "result": { "turn": {
    "id": "turn_457",
    "status": "inProgress",
    "items": [],
    "error": null
} } }
```

### Example: Start a turn (invoke an app)

Invoke an app by including `$<app-slug>` in the text input and adding a `mention` input item with the app id in `app://<connector-id>` form.

```json
{ "method": "turn/start", "id": 34, "params": {
    "threadId": "thr_123",
    "input": [
        { "type": "text", "text": "$demo-app Summarize the latest updates." },
        { "type": "mention", "name": "Demo App", "path": "app://demo-app" }
    ]
} }
{ "id": 34, "result": { "turn": {
    "id": "turn_458",
    "status": "inProgress",
    "items": [],
    "error": null
} } }
```

### Example: Interrupt an active turn

You can cancel a running Turn with `turn/interrupt`.

```json
{ "method": "turn/interrupt", "id": 31, "params": {
    "threadId": "thr_123",
    "turnId": "turn_456"
} }
{ "id": 31, "result": {} }
```

The server requests cancellations for running subprocesses, then emits a `turn/completed` event with `status: "interrupted"`. Rely on the `turn/completed` to know when Codex-side cleanup is done.

### Example: Clean background terminals

Use `thread/backgroundTerminals/clean` to terminate all running background terminals associated with a thread. This method is experimental and requires `capabilities.experimentalApi = true`.

```json
{ "method": "thread/backgroundTerminals/clean", "id": 35, "params": {
    "threadId": "thr_123"
} }
{ "id": 35, "result": {} }
```

### Example: Steer an active turn

Use `turn/steer` to append additional user input to the currently active turn. This does not emit
`turn/started` and does not accept turn context overrides.

```json
{ "method": "turn/steer", "id": 32, "params": {
    "threadId": "thr_123",
    "input": [ { "type": "text", "text": "Actually focus on failing tests first." } ],
    "expectedTurnId": "turn_456"
} }
{ "id": 32, "result": { "turnId": "turn_456" } }
```

`expectedTurnId` is required. If there is no active turn (or `expectedTurnId` does not match the active turn), the request fails with an `invalid request` error.

### Example: Request a code review

Use `review/start` to run Codex’s reviewer on the currently checked-out project. The request takes the thread id plus a `target` describing what should be reviewed:

- `{"type":"uncommittedChanges"}` — staged, unstaged, and untracked files.
- `{"type":"baseBranch","branch":"main"}` — diff against the provided branch’s upstream (see prompt for the exact `git merge-base`/`git diff` instructions Codex will run).
- `{"type":"commit","sha":"abc1234","title":"Optional subject"}` — review a specific commit.
- `{"type":"custom","instructions":"Free-form reviewer instructions"}` — fallback prompt equivalent to the legacy manual review request.
- `delivery` (`"inline"` or `"detached"`, default `"inline"`) — where the review runs:
  - `"inline"`: run the review as a new turn on the existing thread. The response’s `reviewThreadId` equals the original `threadId`, and no new `thread/started` notification is emitted.
  - `"detached"`: fork a new review thread from the parent conversation and run the review there. The response’s `reviewThreadId` is the id of this new review thread, and the server emits a `thread/started` notification for it before streaming review items.

Example request/response:

```json
{ "method": "review/start", "id": 40, "params": {
    "threadId": "thr_123",
    "delivery": "inline",
    "target": { "type": "commit", "sha": "1234567deadbeef", "title": "Polish tui colors" }
} }
{ "id": 40, "result": {
    "turn": {
        "id": "turn_900",
        "status": "inProgress",
        "items": [
            { "type": "userMessage", "id": "turn_900", "content": [ { "type": "text", "text": "Review commit 1234567: Polish tui colors" } ] }
        ],
        "error": null
    },
    "reviewThreadId": "thr_123"
} }
```

For a detached review, use `"delivery": "detached"`. The response is the same shape, but `reviewThreadId` will be the id of the new review thread (different from the original `threadId`). The server also emits a `thread/started` notification for that new thread before streaming the review turn.

Codex streams the usual `turn/started` notification followed by an `item/started`
with an `enteredReviewMode` item so clients can show progress:

```json
{
  "method": "item/started",
  "params": {
    "item": {
      "type": "enteredReviewMode",
      "id": "turn_900",
      "review": "current changes"
    }
  }
}
```

When the reviewer finishes, the server emits `item/started` and `item/completed`
containing an `exitedReviewMode` item with the final review text:

```json
{
  "method": "item/completed",
  "params": {
    "item": {
      "type": "exitedReviewMode",
      "id": "turn_900",
      "review": "Looks solid overall...\n\n- Prefer Stylize helpers — app.rs:10-20\n  ..."
    }
  }
}
```

The `review` string is plain text that already bundles the overall explanation plus a bullet list for each structured finding (matching `ThreadItem::ExitedReviewMode` in the generated schema). Use this notification to render the reviewer output in your client.

### Example: One-off command execution

Run a standalone command (argv vector) in the server’s sandbox without creating a thread or turn:

```json
{ "method": "command/exec", "id": 32, "params": {
    "command": ["ls", "-la"],
    "cwd": "/Users/me/project",                    // optional; defaults to server cwd
    "sandboxPolicy": { "type": "workspaceWrite" }, // optional; defaults to user config
    "timeoutMs": 10000                             // optional; ms timeout; defaults to server timeout
} }
{ "id": 32, "result": { "exitCode": 0, "stdout": "...", "stderr": "" } }
```

- For clients that are already sandboxed externally, set `sandboxPolicy` to `{"type":"externalSandbox","networkAccess":"enabled"}` (or omit `networkAccess` to keep it restricted). Codex will not enforce its own sandbox in this mode; it tells the model it has full file-system access and passes the `networkAccess` state through `environment_context`.

Notes:

- Empty `command` arrays are rejected.
- `sandboxPolicy` accepts the same shape used by `turn/start` (e.g., `dangerFullAccess`, `readOnly`, `workspaceWrite` with flags, `externalSandbox` with `networkAccess` `restricted|enabled`).
- When omitted, `timeoutMs` falls back to the server default.

## Events

Event notifications are the server-initiated event stream for thread lifecycles, turn lifecycles, and the items within them. After you start or resume a thread, keep reading stdout for `thread/started`, `thread/archived`, `thread/unarchived`, `turn/*`, and `item/*` notifications.

### Notification opt-out

Clients can suppress specific notifications per connection by sending exact method names in `initialize.params.capabilities.optOutNotificationMethods`.

- Exact-match only: `item/agentMessage/delta` suppresses only that method.
- Unknown method names are ignored.
- Applies to both legacy (`codex/event/*`) and v2 (`thread/*`, `turn/*`, `item/*`, etc.) notifications.
- Does not apply to requests/responses/errors.

Examples:

- Opt out of legacy session setup event: `codex/event/session_configured`
- Opt out of streamed agent text deltas: `item/agentMessage/delta`

### Fuzzy file search events (experimental)

The fuzzy file search session API emits per-query notifications:

- `fuzzyFileSearch/sessionUpdated` — `{ sessionId, query, files }` with the current matching files for the active query.
- `fuzzyFileSearch/sessionCompleted` — `{ sessionId, query }` once indexing/matching for that query has completed.

### Windows sandbox setup events

- `windowsSandbox/setupCompleted` — `{ mode, success, error }` after a `windowsSandbox/setupStart` request finishes.

### Turn events

The app-server streams JSON-RPC notifications while a turn is running. Each turn starts with `turn/started` (initial `turn`) and ends with `turn/completed` (final `turn` status). Token usage events stream separately via `thread/tokenUsage/updated`. Clients subscribe to the events they care about, rendering each item incrementally as updates arrive. The per-item lifecycle is always: `item/started` → zero or more item-specific deltas → `item/completed`.

- `turn/started` — `{ turn }` with the turn id, empty `items`, and `status: "inProgress"`.
- `turn/completed` — `{ turn }` where `turn.status` is `completed`, `interrupted`, or `failed`; failures carry `{ error: { message, codexErrorInfo?, additionalDetails? } }`.
- `turn/diff/updated` — `{ threadId, turnId, diff }` represents the up-to-date snapshot of the turn-level unified diff, emitted after every FileChange item. `diff` is the latest aggregated unified diff across every file change in the turn. UIs can render this to show the full "what changed" view without stitching individual `fileChange` items.
- `turn/plan/updated` — `{ turnId, explanation?, plan }` whenever the agent shares or changes its plan; each `plan` entry is `{ step, status }` with `status` in `pending`, `inProgress`, or `completed`.
- `model/rerouted` — `{ threadId, turnId, fromModel, toModel, reason }` when the backend reroutes a request to a different model (for example, due to high-risk cyber safety checks).

Today both notifications carry an empty `items` array even when item events were streamed; rely on `item/*` notifications for the canonical item list until this is fixed.

#### Items

`ThreadItem` is the tagged union carried in turn responses and `item/*` notifications. Currently we support events for the following items:

- `userMessage` — `{id, content}` where `content` is a list of user inputs (`text`, `image`, or `localImage`).
- `agentMessage` — `{id, text}` containing the accumulated agent reply.
- `plan` — `{id, text}` emitted for plan-mode turns; plan text can stream via `item/plan/delta` (experimental).
- `reasoning` — `{id, summary, content}` where `summary` holds streamed reasoning summaries (applicable for most OpenAI models) and `content` holds raw reasoning blocks (applicable for e.g. open source models).
- `commandExecution` — `{id, command, cwd, status, commandActions, aggregatedOutput?, exitCode?, durationMs?}` for sandboxed commands; `status` is `inProgress`, `completed`, `failed`, or `declined`.
- `fileChange` — `{id, changes, status}` describing proposed edits; `changes` list `{path, kind, diff}` and `status` is `inProgress`, `completed`, `failed`, or `declined`.
- `mcpToolCall` — `{id, server, tool, status, arguments, result?, error?}` describing MCP calls; `status` is `inProgress`, `completed`, or `failed`.
- `collabToolCall` — `{id, tool, status, senderThreadId, receiverThreadId?, newThreadId?, prompt?, agentStatus?}` describing collab tool calls (`spawn_agent`, `send_input`, `resume_agent`, `wait`, `close_agent`); `status` is `inProgress`, `completed`, or `failed`.
- `webSearch` — `{id, query, action?}` for a web search request issued by the agent; `action` mirrors the Responses API web_search action payload (`search`, `open_page`, `find_in_page`) and may be omitted until completion.
- `imageView` — `{id, path}` emitted when the agent invokes the image viewer tool.
- `enteredReviewMode` — `{id, review}` sent when the reviewer starts; `review` is a short user-facing label such as `"current changes"` or the requested target description.
- `exitedReviewMode` — `{id, review}` emitted when the reviewer finishes; `review` is the full plain-text review (usually, overall notes plus bullet point findings).
- `contextCompaction` — `{id}` emitted when codex compacts the conversation history. This can happen automatically.
- `compacted` - `{threadId, turnId}` when codex compacts the conversation history. This can happen automatically. **Deprecated:** Use `contextCompaction` instead.

All items emit two shared lifecycle events:

- `item/started` — emits the full `item` when a new unit of work begins so the UI can render it immediately; the `item.id` in this payload matches the `itemId` used by deltas.
- `item/completed` — sends the final `item` once that work finishes (e.g., after a tool call or message completes); treat this as the authoritative state.

There are additional item-specific events:

#### agentMessage

- `item/agentMessage/delta` — appends streamed text for the agent message; concatenate `delta` values for the same `itemId` in order to reconstruct the full reply.

#### plan

- `item/plan/delta` — streams proposed plan content for plan items (experimental); concatenate `delta` values for the same plan `itemId`. These deltas correspond to the `<proposed_plan>` block.

#### reasoning

- `item/reasoning/summaryTextDelta` — streams readable reasoning summaries; `summaryIndex` increments when a new summary section opens.
- `item/reasoning/summaryPartAdded` — marks the boundary between reasoning summary sections for an `itemId`; subsequent `summaryTextDelta` entries share the same `summaryIndex`.
- `item/reasoning/textDelta` — streams raw reasoning text (only applicable for e.g. open source models); use `contentIndex` to group deltas that belong together before showing them in the UI.

#### commandExecution

- `item/commandExecution/outputDelta` — streams stdout/stderr for the command; append deltas in order to render live output alongside `aggregatedOutput` in the final item.
  Final `commandExecution` items include parsed `commandActions`, `status`, `exitCode`, and `durationMs` so the UI can summarize what ran and whether it succeeded.

#### fileChange

- `item/fileChange/outputDelta` - contains the tool call response of the underlying `apply_patch` tool call.

### Errors

`error` event is emitted whenever the server hits an error mid-turn (for example, upstream model errors or quota limits). Carries the same `{ error: { message, codexErrorInfo?, additionalDetails? } }` payload as `turn.status: "failed"` and may precede that terminal notification.

`codexErrorInfo` maps to the `CodexErrorInfo` enum. Common values:

- `ContextWindowExceeded`
- `UsageLimitExceeded`
- `HttpConnectionFailed { httpStatusCode? }`: upstream HTTP failures including 4xx/5xx
- `ResponseStreamConnectionFailed { httpStatusCode? }`: failure to connect to the response SSE stream
- `ResponseStreamDisconnected { httpStatusCode? }`: disconnect of the response SSE stream in the middle of a turn before completion
- `ResponseTooManyFailedAttempts { httpStatusCode? }`
- `BadRequest`
- `Unauthorized`
- `SandboxError`
- `InternalServerError`
- `Other`: all unclassified errors

When an upstream HTTP status is available (for example, from the Responses API or a provider), it is forwarded in `httpStatusCode` on the relevant `codexErrorInfo` variant.

## Approvals

Certain actions (shell commands or modifying files) may require explicit user approval depending on the user's config. When `turn/start` is used, the app-server drives an approval flow by sending a server-initiated JSON-RPC request to the client. The client must respond to tell Codex whether to proceed. UIs should present these requests inline with the active turn so users can review the proposed command or diff before choosing.

- Requests include `threadId` and `turnId`—use them to scope UI state to the active conversation.
- Respond with a single `{ "decision": "accept" | "decline" }` payload (plus optional `acceptSettings` on command executions). The server resumes or declines the work and ends the item with `item/completed`.

### Command execution approvals

Order of messages:

1. `item/started` — shows the pending `commandExecution` item with `command`, `cwd`, and other fields so you can render the proposed action.
2. `item/commandExecution/requestApproval` (request) — carries the same `itemId`, `threadId`, `turnId`, optionally `approvalId` (for subcommand callbacks), `reason`, plus `command`, `cwd`, and `commandActions` for friendly display.
3. Client response — `{ "decision": "accept", "acceptSettings": { "forSession": false } }` or `{ "decision": "decline" }`.
4. `item/completed` — final `commandExecution` item with `status: "completed" | "failed" | "declined"` and execution output. Render this as the authoritative result.

### File change approvals

Order of messages:

1. `item/started` — emits a `fileChange` item with `changes` (diff chunk summaries) and `status: "inProgress"`. Show the proposed edits and paths to the user.
2. `item/fileChange/requestApproval` (request) — includes `itemId`, `threadId`, `turnId`, and an optional `reason`.
3. Client response — `{ "decision": "accept" }` or `{ "decision": "decline" }`.
4. `item/completed` — returns the same `fileChange` item with `status` updated to `completed`, `failed`, or `declined` after the patch attempt. Rely on this to show success/failure and finalize the diff state in your UI.

UI guidance for IDEs: surface an approval dialog as soon as the request arrives. The turn will proceed after the server receives a response to the approval request. The terminal `item/completed` notification will be sent with the appropriate status.

### Dynamic tool calls (experimental)

`dynamicTools` on `thread/start` and the corresponding `item/tool/call` request/response flow are experimental APIs. To enable them, set `initialize.params.capabilities.experimentalApi = true`.

When a dynamic tool is invoked during a turn, the server sends an `item/tool/call` JSON-RPC request to the client:

```json
{
  "method": "item/tool/call",
  "id": 60,
  "params": {
    "threadId": "thr_123",
    "turnId": "turn_123",
    "callId": "call_123",
    "tool": "lookup_ticket",
    "arguments": { "id": "ABC-123" }
  }
}
```

The client must respond with content items. Use `inputText` for text and `inputImage` for image URLs/data URLs:

```json
{
  "id": 60,
  "result": {
    "contentItems": [
      { "type": "inputText", "text": "Ticket ABC-123 is open." },
      { "type": "inputImage", "imageUrl": "data:image/png;base64,AAA" }
    ],
    "success": true
  }
}
```

## Skills

Invoke a skill by including `$<skill-name>` in the text input. Add a `skill` input item (recommended) so the backend injects full skill instructions instead of relying on the model to resolve the name.

```json
{
  "method": "turn/start",
  "id": 101,
  "params": {
    "threadId": "thread-1",
    "input": [
      {
        "type": "text",
        "text": "$skill-creator Add a new skill for triaging flaky CI."
      },
      {
        "type": "skill",
        "name": "skill-creator",
        "path": "/Users/me/.codex/skills/skill-creator/SKILL.md"
      }
    ]
  }
}
```

If you omit the `skill` item, the model will still parse the `$<skill-name>` marker and try to locate the skill, which can add latency.

Example:

```
$skill-creator Add a new skill for triaging flaky CI and include step-by-step usage.
```

Use `skills/list` to fetch the available skills (optionally scoped by `cwds`, with `forceReload`).
You can also add `perCwdExtraUserRoots` to scan additional absolute paths as `user` scope for specific `cwd` entries.
Entries whose `cwd` is not present in `cwds` are ignored.
`skills/list` might reuse a cached skills result per `cwd`; setting `forceReload` to `true` refreshes the result from disk.

```json
{ "method": "skills/list", "id": 25, "params": {
    "cwds": ["/Users/me/project", "/Users/me/other-project"],
    "forceReload": true,
    "perCwdExtraUserRoots": [
      {
        "cwd": "/Users/me/project",
        "extraUserRoots": ["/Users/me/shared-skills"]
      }
    ]
} }
{ "id": 25, "result": {
    "data": [{
        "cwd": "/Users/me/project",
        "skills": [
            {
              "name": "skill-creator",
              "description": "Create or update a Codex skill",
              "enabled": true,
              "interface": {
                "displayName": "Skill Creator",
                "shortDescription": "Create or update a Codex skill",
                "iconSmall": "icon.svg",
                "iconLarge": "icon-large.svg",
                "brandColor": "#111111",
                "defaultPrompt": "Add a new skill for triaging flaky CI."
              }
            }
        ],
        "errors": []
    }]
} }
```

To enable or disable a skill by path:

```json
{
  "method": "skills/config/write",
  "id": 26,
  "params": {
    "path": "/Users/me/.codex/skills/skill-creator/SKILL.md",
    "enabled": false
  }
}
```

## Apps

Use `app/list` to fetch available apps (connectors). Each entry includes metadata like the app `id`, display `name`, `installUrl`, `branding`, `appMetadata`, `labels`, whether it is currently accessible, and whether it is enabled in config.

```json
{ "method": "app/list", "id": 50, "params": {
    "cursor": null,
    "limit": 50,
    "threadId": "thr_123",
    "forceRefetch": false
} }
{ "id": 50, "result": {
    "data": [
        {
            "id": "demo-app",
            "name": "Demo App",
            "description": "Example connector for documentation.",
            "logoUrl": "https://example.com/demo-app.png",
            "logoUrlDark": null,
            "distributionChannel": null,
            "branding": null,
            "appMetadata": null,
            "labels": null,
            "installUrl": "https://chatgpt.com/apps/demo-app/demo-app",
            "isAccessible": true,
            "isEnabled": true
        }
    ],
    "nextCursor": null
} }
```

When `threadId` is provided, app feature gating (`Feature::Apps`) is evaluated using that thread's config snapshot. When omitted, the latest global config is used.

`app/list` returns after both accessible apps and directory apps are loaded. Set `forceRefetch: true` to bypass app caches and fetch fresh data from sources. Cache entries are only replaced when those refetches succeed.

The server also emits `app/list/updated` notifications whenever either source (accessible apps or directory apps) finishes loading. Each notification includes the latest merged app list.

```json
{
  "method": "app/list/updated",
  "params": {
    "data": [
      {
        "id": "demo-app",
        "name": "Demo App",
        "description": "Example connector for documentation.",
        "logoUrl": "https://example.com/demo-app.png",
        "logoUrlDark": null,
        "distributionChannel": null,
        "branding": null,
        "appMetadata": null,
        "labels": null,
        "installUrl": "https://chatgpt.com/apps/demo-app/demo-app",
        "isAccessible": true,
        "isEnabled": true
      }
    ]
  }
}
```

Invoke an app by inserting `$<app-slug>` in the text input. The slug is derived from the app name and lowercased with non-alphanumeric characters replaced by `-` (for example, "Demo App" becomes `$demo-app`). Add a `mention` input item (recommended) so the server uses the exact `app://<connector-id>` path rather than guessing by name.

Example:

```
$demo-app Pull the latest updates from the team.
```

```json
{
  "method": "turn/start",
  "id": 51,
  "params": {
    "threadId": "thread-1",
    "input": [
      {
        "type": "text",
        "text": "$demo-app Pull the latest updates from the team."
      },
      { "type": "mention", "name": "Demo App", "path": "app://demo-app" }
    ]
  }
}
```

## Auth endpoints

The JSON-RPC auth/account surface exposes request/response methods plus server-initiated notifications (no `id`). Use these to determine auth state, start or cancel logins, logout, and inspect ChatGPT rate limits.

### Authentication modes

Codex supports these authentication modes. The current mode is surfaced in `account/updated` (`authMode`) and can be inferred from `account/read`.

- **API key (`apiKey`)**: Caller supplies an OpenAI API key via `account/login/start` with `type: "apiKey"`. The API key is saved and used for API requests.
- **ChatGPT managed (`chatgpt`)** (recommended): Codex owns the ChatGPT OAuth flow and refresh tokens. Start via `account/login/start` with `type: "chatgpt"`; Codex persists tokens to disk and refreshes them automatically.

### API Overview

- `account/read` — fetch current account info; optionally refresh tokens.
- `account/login/start` — begin login (`apiKey`, `chatgpt`).
- `account/login/completed` (notify) — emitted when a login attempt finishes (success or error).
- `account/login/cancel` — cancel a pending ChatGPT login by `loginId`.
- `account/logout` — sign out; triggers `account/updated`.
- `account/updated` (notify) — emitted whenever auth mode changes (`authMode`: `apikey`, `chatgpt`, or `null`).
- `account/rateLimits/read` — fetch ChatGPT rate limits; updates arrive via `account/rateLimits/updated` (notify).
- `account/rateLimits/updated` (notify) — emitted whenever a user's ChatGPT rate limits change.
- `mcpServer/oauthLogin/completed` (notify) — emitted after a `mcpServer/oauth/login` flow finishes for a server; payload includes `{ name, success, error? }`.

### 1) Check auth state

Request:

```json
{ "method": "account/read", "id": 1, "params": { "refreshToken": false } }
```

Response examples:

```json
{ "id": 1, "result": { "account": null, "requiresOpenaiAuth": false } } // No OpenAI auth needed (e.g., OSS/local models)
{ "id": 1, "result": { "account": null, "requiresOpenaiAuth": true } }  // OpenAI auth required (typical for OpenAI-hosted models)
{ "id": 1, "result": { "account": { "type": "apiKey" }, "requiresOpenaiAuth": true } }
{ "id": 1, "result": { "account": { "type": "chatgpt", "email": "user@example.com", "planType": "pro" }, "requiresOpenaiAuth": true } }
```

Field notes:

- `refreshToken` (bool): set `true` to force a token refresh.
- `requiresOpenaiAuth` reflects the active provider; when `false`, Codex can run without OpenAI credentials.

### 2) Log in with an API key

1. Send:
   ```json
   {
     "method": "account/login/start",
     "id": 2,
     "params": { "type": "apiKey", "apiKey": "sk-…" }
   }
   ```
2. Expect:
   ```json
   { "id": 2, "result": { "type": "apiKey" } }
   ```
3. Notifications:
   ```json
   { "method": "account/login/completed", "params": { "loginId": null, "success": true, "error": null } }
   { "method": "account/updated", "params": { "authMode": "apikey" } }
   ```

### 3) Log in with ChatGPT (browser flow)

1. Start:
   ```json
   { "method": "account/login/start", "id": 3, "params": { "type": "chatgpt" } }
   { "id": 3, "result": { "type": "chatgpt", "loginId": "<uuid>", "authUrl": "https://chatgpt.com/…&redirect_uri=http%3A%2F%2Flocalhost%3A<port>%2Fauth%2Fcallback" } }
   ```
2. Open `authUrl` in a browser; the app-server hosts the local callback.
3. Wait for notifications:
   ```json
   { "method": "account/login/completed", "params": { "loginId": "<uuid>", "success": true, "error": null } }
   { "method": "account/updated", "params": { "authMode": "chatgpt" } }
   ```

### 4) Cancel a ChatGPT login

```json
{ "method": "account/login/cancel", "id": 4, "params": { "loginId": "<uuid>" } }
{ "method": "account/login/completed", "params": { "loginId": "<uuid>", "success": false, "error": "…" } }
```

### 5) Logout

```json
{ "method": "account/logout", "id": 5 }
{ "id": 5, "result": {} }
{ "method": "account/updated", "params": { "authMode": null } }
```

### 6) Rate limits (ChatGPT)

```json
{ "method": "account/rateLimits/read", "id": 6 }
{ "id": 6, "result": { "rateLimits": { "primary": { "usedPercent": 25, "windowDurationMins": 15, "resetsAt": 1730947200 }, "secondary": null } } }
{ "method": "account/rateLimits/updated", "params": { "rateLimits": { … } } }
```

Field notes:

- `usedPercent` is current usage within the OpenAI quota window.
- `windowDurationMins` is the quota window length.
- `resetsAt` is a Unix timestamp (seconds) for the next reset.

## Experimental API Opt-in

Some app-server methods and fields are intentionally gated behind an experimental capability with no backwards-compatible guarantees. This lets clients choose between:

- Stable surface only (default): no opt-in, no experimental methods/fields exposed.
- Experimental surface: opt in during `initialize`.

### Generating stable vs experimental client schemas

`codex app-server` schema generation defaults to the stable API surface (experimental fields and methods filtered out). Pass `--experimental` to include experimental methods/fields in generated TypeScript or JSON schema:

```bash
# Stable-only output (default)
codex app-server generate-ts --out DIR
codex app-server generate-json-schema --out DIR

# Include experimental API surface
codex app-server generate-ts --out DIR --experimental
codex app-server generate-json-schema --out DIR --experimental
```

### How clients opt in at runtime

Set `capabilities.experimentalApi` to `true` in your single `initialize` request:

```json
{
  "method": "initialize",
  "id": 1,
  "params": {
    "clientInfo": {
      "name": "my_client",
      "title": "My Client",
      "version": "0.1.0"
    },
    "capabilities": {
      "experimentalApi": true
    }
  }
}
```

Then send the standard `initialized` notification and proceed normally.

Notes:

- If `capabilities` is omitted, `experimentalApi` is treated as `false`.
- This setting is negotiated once at initialization time for the process lifetime (re-initializing is rejected with `"Already initialized"`).

### What happens without opt-in

If a request uses an experimental method or sets an experimental field without opting in, app-server rejects it with a JSON-RPC error. The message is:

`<descriptor> requires experimentalApi capability`

Examples of descriptor strings:

- `mock/experimentalMethod` (method-level gate)
- `thread/start.mockExperimentalField` (field-level gate)

### For maintainers: Adding experimental fields and methods

Use this checklist when introducing a field/method that should only be available when the client opts into experimental APIs.

At runtime, clients must send `initialize` with `capabilities.experimentalApi = true` to use experimental methods or fields.

1. Annotate the field in the protocol type (usually `app-server-protocol/src/protocol/v2.rs`) with:
   ```rust
   #[experimental("thread/start.myField")]
   pub my_field: Option<String>,
   ```
2. Ensure the params type derives `ExperimentalApi` so field-level gating can be detected at runtime.

3. In `app-server-protocol/src/protocol/common.rs`, keep the method stable and use `inspect_params: true` when only some fields are experimental (like `thread/start`). If the entire method is experimental, annotate the method variant with `#[experimental("method/name")]`.

4. Regenerate protocol fixtures:

   ```bash
   just write-app-server-schema
   # Include experimental API fields/methods in fixtures.
   just write-app-server-schema --experimental
   ```

5. Verify the protocol crate:

   ```bash
   cargo test -p codex-app-server-protocol
   ```
