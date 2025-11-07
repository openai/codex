# codex-app-server

`codex app-server` is the interface Codex uses to power rich interfaces such as the [Codex VS Code extension](https://marketplace.visualstudio.com/items?itemName=openai.chatgpt). The message schema is currently unstable, but those who wish to build experimental UIs on top of Codex may find it valuable.

## Protocol

Similar to [MCP](https://modelcontextprotocol.io/), `codex app-server` supports bidirectional communication, streaming JSONL over stdio. The protocol is JSON-RPC 2.0, though the `"jsonrpc":"2.0"` header is omitted.

## Message Schema

Currently, you can dump a TypeScript version of the schema using `codex generate-ts`. It is specific to the version of Codex you used to run `generate-ts`, so the two are guaranteed to be compatible.

```
codex generate-ts --out DIR
```

## Initialization

Clients must send a single `initialize` request before invoking any other method, then acknowledge with an `initialized` notification. The server returns the user agent string it will present to upstream services; subsequent requests issued before initialization receive a `"Not initialized"` error, and repeated `initialize` calls receive an `"Already initialized"` error.

Example:

```json
{ "method": "initialize", "id": 0, "params": {
    "clientInfo": { "name": "codex-vscode", "title": "Codex VS Code Extension", "version": "0.1.0" }
} }
{ "id": 0, "result": { "userAgent": "codex-app-server/0.1.0 codex-vscode/0.1.0" } }
{ "method": "initialized" }
```

## Core primitives

We have 3 top level primitives:
- Thread - a conversation between the Codex agent and a user. Each thread contains multiple turns.
- Turn - one turn of the conversation, typically starting with a user message and finishing with an agent message. Each turn contains multiple items.
- Item - represents user inputs and agent outputs as part of the turn, persisted and used as the context for future conversations.

## Thread & turn endpoints

The JSON-RPC API exposes dedicated methods for managing Codex conversations. Threads store long-lived conversation metadata, and turns store the per-message exchange (input → Codex output, including streamed items). Use the thread APIs to create, list, or archive sessions, then drive the conversation with turn APIs and notifications.

### Quick reference
- `thread/start` — create a new thread; emits `thread/started` and auto-subscribes you to turn/item events for that thread.
- `thread/resume` — reopen an existing thread by id so subsequent `turn/start` calls append to it.
- `thread/list` — page through stored rollouts; supports cursor-based pagination and optional `modelProviders` filtering.
- `thread/archive` — move a thread’s rollout file into the archived directory; returns `{}` on success.
- `turn/start` — add user input to a thread and begin Codex generation; responds with the initial `turn` object and streams `turn/started`, `item/*`, and `turn/completed` notifications.
- `turn/interrupt` — request cancellation of an in-flight turn by `(thread_id, turn_id)`; success is an empty `{}` response and the turn finishes with `status: "interrupted"`.

### 1) Start or resume a thread

Start a fresh thread when you need a new Codex conversation. Optional fields mirror CLI defaults: set `model`, `modelProvider`, `cwd`, `approvalPolicy`, `sandbox`, or custom `config` values. Instructions can be set via `baseInstructions` and `developerInstructions`:

```json
{ "method": "thread/start", "id": 10, "params": {
    "model": "gpt-5-codex",
    "cwd": "/Users/me/project",
    "approvalPolicy": "never",
    "sandbox": "workspace-write",
    "baseInstructions": "You're helping with refactors."
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

### 2) List threads (pagination & filters)

`thread/list` lets you render a history UI. Pass any combination of:
- `cursor` — opaque string from a prior response; omit for the first page.
- `limit` — server defaults to a reasonable page size if unset.
- `modelProviders` — restrict results to specific providers; unset, null, or an empty array will include all providers.

Example:

```json
{ "method": "thread/list", "id": 20, "params": {
    "cursor": null,
    "limit": 25,
    "modelProviders": ["openai"]
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

### 3) Archive a thread

Use `thread/archive` to move the persisted rollout (stored as a JSONL file on disk) into the archived sessions directory.

```json
{ "method": "thread/archive", "id": 21, "params": { "threadId": "thr_b" } }
{ "id": 21, "result": {} }
```

An archived thread will not appear in future calls to `thread/list`.

### 4) Start a turn (send user input)

Turns attach user input (text or images) to a thread and trigger Codex generation. The `input` field is a list of discriminated unions:

- `{"type":"text","text":"Explain this diff"}`
- `{"type":"image","url":"https://…png"}`
- `{"type":"localImage","path":"/tmp/screenshot.png"}`

Override knobs apply to the new turn and become the defaults for subsequent turns on the same thread:

```json
{ "method": "turn/start", "id": 30, "params": {
    "threadId": "thr_123",
    "input": [ { "type": "text", "text": "Run tests" } ],
    "cwd": "/Users/me/project",
    "approvalPolicy": "untrusted",
    "sandboxPolicy": "workspace-write",
    "model": "gpt-5-codex",
    "effort": "medium",
    "summary": "focus-on-test-failures"
} }
{ "id": 30, "result": { "turn": {
    "id": "turn_456",
    "status": "inProgress",
    "items": [],
    "error": null
} } }
```

### 5) Interrupt an active turn

You can cancel a running Turn with `turn/interrupt`.

```json
{ "method": "turn/interrupt", "id": 31, "params": {
    "threadId": "thr_123",
    "turnId": "turn_456"
} }
{ "id": 31, "result": {} }
```

The server requests cancellations for running subprocesses, then emits a `turn/completed` event with `status: "interrupted"`. Rely on the `turn/completed` to know when Codex-side cleanup is done.

## Auth endpoints

The v2 JSON-RPC auth/account surface exposes request/response methods plus server-initiated notifications (no `id`). Use these to determine auth state, start or cancel logins, logout, and inspect ChatGPT rate limits.

### Quick reference
- `account/read` — fetch current account info; optionally refresh tokens.
- `account/login/start` — begin login (`apiKey` or `chatgpt`).
- `account/login/completed` (notify) — emitted when a login attempt finishes (success or error).
- `account/login/cancel` — cancel a pending ChatGPT login by `loginId`.
- `account/logout` — sign out; triggers `account/updated`.
- `account/updated` (notify) — emitted whenever auth mode changes (`authMode`: `apikey`, `chatgpt`, or `null`).
- `account/rateLimits/read` — fetch ChatGPT rate limits; updates arrive via `account/rateLimits/updated` (notify).

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

### Dev notes

- `codex generate-ts --out <dir>` emits v2 types under `v2/`.
- See [“Authentication and authorization” in the config docs](../../docs/config.md#authentication-and-authorization) for configuration knobs.
