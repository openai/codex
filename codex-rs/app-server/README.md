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
- [Auth endpoints](#auth-endpoints)

## Protocol

Similar to [MCP](https://modelcontextprotocol.io/), `codex app-server` supports bidirectional communication, streaming JSONL over stdio. The protocol is JSON-RPC 2.0, though the `"jsonrpc":"2.0"` header is omitted.

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

- Initialize once: Immediately after launching the codex app-server process, send an `initialize` request with your client metadata, then emit an `initialized` notification. Any other request before this handshake gets rejected.
- Start (or resume) a thread: Call `thread/start` to open a fresh conversation. The response returns the thread object and you’ll also get a `thread/started` notification. If you’re continuing an existing conversation, call `thread/resume` with its ID instead.
- Begin a turn: To send user input, call `turn/start` with the target `threadId` and the user's input. Optional fields let you override model, cwd, sandbox policy, etc. This immediately returns the new turn object and triggers a `turn/started` notification.
- Stream events: After `turn/start`, keep reading JSON-RPC notifications on stdout. You’ll see `item/started`, `item/completed`, deltas like `item/agentMessage/delta`, tool progress, etc. These represent streaming model output plus any side effects (commands, tool calls, reasoning notes).
- Finish the turn: When the model is done (or the turn is interrupted via making the `turn/interrupt` call), the server sends `turn/completed` with the final turn state and token usage.

## Initialization

Clients must send a single `initialize` request before invoking any other method, then acknowledge with an `initialized` notification. The server returns the user agent string it will present to upstream services; subsequent requests issued before initialization receive a `"Not initialized"` error, and repeated `initialize` calls receive an `"Already initialized"` error.

Applications building on top of `codex app-server` should identify themselves via the `clientInfo` parameter.

Example (from OpenAI's official VSCode extension):
```json
{ "method": "initialize", "id": 0, "params": {
    "clientInfo": { "name": "codex-vscode", "title": "Codex VS Code Extension", "version": "0.1.0" }
} }
```

## API Overview
- `thread/start` — create a new thread; emits `thread/started` and auto-subscribes you to turn/item events for that thread.
- `thread/resume` — reopen an existing thread by id so subsequent `turn/start` calls append to it.
- `thread/list` — page through stored rollouts; supports cursor-based pagination and optional `modelProviders` filtering.
- `thread/archive` — move a thread’s rollout file into the archived directory; returns `{}` on success.
- `turn/start` — add user input to a thread and begin Codex generation; responds with the initial `turn` object and streams `turn/started`, `item/*`, and `turn/completed` notifications.
- `turn/interrupt` — request cancellation of an in-flight turn by `(thread_id, turn_id)`; success is an empty `{}` response and the turn finishes with `status: "interrupted"`.
- `review/start` — kick off Codex’s automated reviewer for a thread; responds like `turn/start` and emits `item/started`/`item/completed` notifications with `enteredReviewMode` and `exitedReviewMode` items, plus a final assistant `agentMessage` containing the review.
- `command/exec` — run a single command under the server sandbox without starting a thread/turn (handy for utilities and validation).
- `model/list` — list available models (with reasoning effort options).
- `mcpServer/oauth/login` — start an OAuth login for a configured MCP server; returns an `authorization_url` and later emits `mcpServer/oauthLogin/completed` once the browser flow finishes.
- `mcpServers/list` — enumerate configured MCP servers with their tools, resources, resource templates, and auth status; supports cursor+limit pagination.
- `feedback/upload` — submit a feedback report (classification + optional reason/logs and conversation_id); returns the tracking thread id.
- `command/exec` — run a single command under the server sandbox without starting a thread/turn (handy for utilities and validation).
- `config/read` — fetch the effective config on disk after resolving config layering.
- `config/value/write` — write a single config key/value to the user's config.toml on disk.
- `config/batchWrite` — apply multiple config edits atomically to the user's config.toml on disk.

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

To continue a stored session, call `thread/resume` with the `thread.id` you previously recorded. The response shape matches `thread/start`, and no additional notifications are emitted:

```json
{ "method": "thread/resume", "id": 11, "params": { "threadId": "thr_123" } }
{ "id": 11, "result": { "thread": { "id": "thr_123", … } } }
```

### Example: List threads (with pagination & filters)

`thread/list` lets you render a history UI. Pass any combination of:
- `cursor` — opaque string from a prior response; omit for the first page.
- `limit` — server defaults to a reasonable page size if unset.
- `modelProviders` — restrict results to specific providers; unset, null, or an empty array will include all providers.

Example:

```json
{ "method": "thread/list", "id": 20, "params": {
    "cursor": null,
    "limit": 25,
} }
{ "id": 20, "result": {
    "data": [
        { "id": "thr_a", "preview": "Create a TUI", "modelProvider": "openai", "createdAt": 1730831111 },
        { "id": "thr_b", "preview": "Fix tests", "modelProvider": "openai", "createdAt": 1730750000 }
    ],
    "nextCursor": "opaque-token-or-null"
} }
```

When `nextCursor` is `null`, you’ve reached the final page.

### Example: Archive a thread

Use `thread/archive` to move the persisted rollout (stored as a JSONL file on disk) into the archived sessions directory.

```json
{ "method": "thread/archive", "id": 21, "params": { "threadId": "thr_b" } }
{ "id": 21, "result": {} }
```

An archived thread will not appear in future calls to `thread/list`.

### Example: Start a turn (send user input)

Turns attach user input (text or images) to a thread and trigger Codex generation. The `input` field is a list of discriminated unions:

- `{"type":"text","text":"Explain this diff"}`
- `{"type":"image","url":"https://…png"}`
- `{"type":"localImage","path":"/tmp/screenshot.png"}`

You can optionally specify config overrides on the new turn. If specified, these settings become the default for subsequent turns on the same thread.

```json
{ "method": "turn/start", "id": 30, "params": {
    "threadId": "thr_123",
    "input": [ { "type": "text", "text": "Run tests" } ],
    // Below are optional config overrides
    "cwd": "/Users/me/project",
    "approvalPolicy": "unlessTrusted",
    "sandboxPolicy": {
        "mode": "workspaceWrite",
        "writableRoots": ["/Users/me/project"],
        "networkAccess": true
    },
    "model": "gpt-5.1-codex",
    "effort": "medium",
    "summary": "concise"
} }
{ "id": 30, "result": { "turn": {
    "id": "turn_456",
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
{ "method": "item/started", "params": { "item": {
    "type": "enteredReviewMode",
    "id": "turn_900",
    "review": "current changes"
} } }
```

When the reviewer finishes, the server emits `item/started` and `item/completed`
containing an `exitedReviewMode` item with the final review text:

```json
{ "method": "item/completed", "params": { "item": {
    "type": "exitedReviewMode",
    "id": "turn_900",
    "review": "Looks solid overall...\n\n- Prefer Stylize helpers — app.rs:10-20\n  ..."
} } }
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

Notes:
- Empty `command` arrays are rejected.
- `sandboxPolicy` accepts the same shape used by `turn/start` (e.g., `dangerFullAccess`, `readOnly`, `workspaceWrite` with flags).
- When omitted, `timeoutMs` falls back to the server default.

## Events

Event notifications are the server-initiated event stream for thread lifecycles, turn lifecycles, and the items within them. After you start or resume a thread, keep reading stdout for `thread/started`, `turn/*`, and `item/*` notifications.

### Turn events

The app-server streams JSON-RPC notifications while a turn is running. Each turn starts with `turn/started` (initial `turn`) and ends with `turn/completed` (final `turn` status). Token usage events stream separately via `thread/tokenUsage/updated`. Clients subscribe to the events they care about, rendering each item incrementally as updates arrive. The per-item lifecycle is always: `item/started` → zero or more item-specific deltas → `item/completed`.

- `turn/started` — `{ turn }` with the turn id, empty `items`, and `status: "inProgress"`.
- `turn/completed` — `{ turn }` where `turn.status` is `completed`, `interrupted`, or `failed`; failures carry `{ error: { message, codexErrorInfo? } }`.
- `turn/diff/updated` — `{ threadId, turnId, diff }` represents the up-to-date snapshot of the turn-level unified diff, emitted after every FileChange item. `diff` is the latest aggregated unified diff across every file change in the turn. UIs can render this to show the full "what changed" view without stitching individual `fileChange` items.
- `turn/plan/updated` — `{ turnId, explanation?, plan }` whenever the agent shares or changes its plan; each `plan` entry is `{ step, status }` with `status` in `pending`, `inProgress`, or `completed`.

Today both notifications carry an empty `items` array even when item events were streamed; rely on `item/*` notifications for the canonical item list until this is fixed.

#### Items

`ThreadItem` is the tagged union carried in turn responses and `item/*` notifications. Currently we support events for the following items:
- `userMessage` — `{id, content}` where `content` is a list of user inputs (`text`, `image`, or `localImage`).
- `agentMessage` — `{id, text}` containing the accumulated agent reply.
- `reasoning` — `{id, summary, content}` where `summary` holds streamed reasoning summaries (applicable for most OpenAI models) and `content` holds raw reasoning blocks (applicable for e.g. open source models).
- `commandExecution` — `{id, command, cwd, status, commandActions, aggregatedOutput?, exitCode?, durationMs?}` for sandboxed commands; `status` is `inProgress`, `completed`, `failed`, or `declined`.
- `fileChange` — `{id, changes, status}` describing proposed edits; `changes` list `{path, kind, diff}` and `status` is `inProgress`, `completed`, `failed`, or `declined`.
- `mcpToolCall` — `{id, server, tool, status, arguments, result?, error?}` describing MCP calls; `status` is `inProgress`, `completed`, or `failed`.
- `webSearch` — `{id, query}` for a web search request issued by the agent.
- `imageView` — `{id, path}` emitted when the agent invokes the image viewer tool.
- `enteredReviewMode` — `{id, review}` sent when the reviewer starts; `review` is a short user-facing label such as `"current changes"` or the requested target description.
- `exitedReviewMode` — `{id, review}` emitted when the reviewer finishes; `review` is the full plain-text review (usually, overall notes plus bullet point findings).
- `compacted` - `{threadId, turnId}` when codex compacts the conversation history. This can happen automatically.

All items emit two shared lifecycle events:
- `item/started` — emits the full `item` when a new unit of work begins so the UI can render it immediately; the `item.id` in this payload matches the `itemId` used by deltas.
- `item/completed` — sends the final `item` once that work finishes (e.g., after a tool call or message completes); treat this as the authoritative state.

There are additional item-specific events:
#### agentMessage
- `item/agentMessage/delta` — appends streamed text for the agent message; concatenate `delta` values for the same `itemId` in order to reconstruct the full reply.
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
`error` event is emitted whenever the server hits an error mid-turn (for example, upstream model errors or quota limits). Carries the same `{ error: { message, codexErrorInfo? } }` payload as `turn.status: "failed"` and may precede that terminal notification.

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
2. `item/commandExecution/requestApproval` (request) — carries the same `itemId`, `threadId`, `turnId`, optionally `reason` or `risk`, plus `parsedCmd` for friendly display.
3. Client response — `{ "decision": "accept", "acceptSettings": { "forSession": false } }` or `{ "decision": "decline" }`.
4. `item/completed` — final `commandExecution` item with `status: "completed" | "failed" | "declined"` and execution output. Render this as the authoritative result.

### File change approvals

Order of messages:
1. `item/started` — emits a `fileChange` item with `changes` (diff chunk summaries) and `status: "inProgress"`. Show the proposed edits and paths to the user.
2. `item/fileChange/requestApproval` (request) — includes `itemId`, `threadId`, `turnId`, and an optional `reason`.
3. Client response — `{ "decision": "accept" }` or `{ "decision": "decline" }`.
4. `item/completed` — returns the same `fileChange` item with `status` updated to `completed`, `failed`, or `declined` after the patch attempt. Rely on this to show success/failure and finalize the diff state in your UI.

UI guidance for IDEs: surface an approval dialog as soon as the request arrives. The turn will proceed after the server receives a response to the approval request. The terminal `item/completed` notification will be sent with the appropriate status.

## Auth endpoints

The JSON-RPC auth/account surface exposes request/response methods plus server-initiated notifications (no `id`). Use these to determine auth state, start or cancel logins, logout, and inspect ChatGPT rate limits.

### API Overview
- `account/read` — fetch current account info; optionally refresh tokens.
- `account/login/start` — begin login (`apiKey` or `chatgpt`).
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
   { "method": "account/login/start", "id": 2, "params": { "type": "apiKey", "apiKey": "sk-…" } }
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
