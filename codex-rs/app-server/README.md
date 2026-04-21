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
- off (`--listen off`): do not expose a local transport

When running with `--listen ws://IP:PORT`, the same listener also serves basic HTTP health probes:

- `GET /readyz` returns `200 OK` once the listener is accepting new connections.
- `GET /healthz` returns `200 OK` when no `Origin` header is present.
- Any request carrying an `Origin` header is rejected with `403 Forbidden`.

Websocket transport is currently experimental and unsupported. Do not rely on it for production workloads.

Security note:

- Loopback websocket listeners (`ws://127.0.0.1:PORT`) remain appropriate for localhost and SSH port-forwarding workflows.
- Non-loopback websocket listeners currently allow unauthenticated connections by default during rollout. If you expose one remotely, configure websocket auth explicitly now.
- Supported auth modes are app-server flags:
  - `--ws-auth capability-token --ws-token-file /absolute/path`
  - `--ws-auth capability-token --ws-token-sha256 HEX`
  - `--ws-auth signed-bearer-token --ws-shared-secret-file /absolute/path` for HMAC-signed JWT/JWS bearer tokens, with optional `--ws-issuer`, `--ws-audience`, `--ws-max-clock-skew-seconds`
- Clients present the credential as `Authorization: Bearer <token>` during the websocket handshake. Auth is enforced before JSON-RPC `initialize`.
- When starting `codex app-server` manually, prefer `--ws-token-file` over passing raw bearer tokens on the command line. Store a high-entropy token in a file readable only by your user, then have your client present that token in the websocket `Authorization` header.
- `--ws-token-sha256` is intended for clients that keep the raw token in a separate local secret store and only need the server to know the SHA-256 verifier. The hash may appear in process listings, but it is not sufficient to authenticate; clients still need the original raw token. Only use this mode with randomly generated high-entropy tokens, not passwords or other guessable values.

Tracing/log output:

- `RUST_LOG` controls log filtering/verbosity.
- Set `LOG_FORMAT=json` to emit app-server tracing logs to `stderr` as JSON (one event per line).

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
- Start (or resume) a thread: Call `thread/start` to open a fresh conversation. The response returns the thread object and you’ll also get a `thread/started` notification. If you’re continuing an existing conversation, call `thread/resume` with its ID instead. If you want to branch from an existing conversation, call `thread/fork` to create a new thread id with copied history. Like `thread/start`, `thread/fork` also accepts `ephemeral: true` for an in-memory temporary thread.
  The returned `thread.ephemeral` flag tells you whether the session is intentionally in-memory only; when it is `true`, `thread.path` is `null`.
- Begin a turn: To send user input, call `turn/start` with the target `threadId` and the user's input. Optional fields let you override model, cwd, sandbox policy, approval policy, approvals reviewer, etc. This immediately returns the new turn object. The app-server emits `turn/started` when that turn actually begins running.
- Stream events: After `turn/start`, keep reading JSON-RPC notifications on stdout. You’ll see `item/started`, `item/completed`, deltas like `item/agentMessage/delta`, tool progress, etc. These represent streaming model output plus any side effects (commands, tool calls, reasoning notes).
- Finish the turn: When the model is done (or the turn is interrupted via making the `turn/interrupt` call), the server sends `turn/completed` with the final turn state and token usage.

## Initialization

Clients must send a single `initialize` request per transport connection before invoking any other method on that connection, then acknowledge with an `initialized` notification. The server returns the user agent string it will present to upstream services, `codexHome` for the server's Codex home directory, and `platformFamily` and `platformOs` strings describing the app-server runtime target; subsequent requests issued before initialization receive a `"Not initialized"` error, and repeated `initialize` calls on the same connection receive an `"Already initialized"` error.

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
      "optOutNotificationMethods": ["thread/started", "item/agentMessage/delta"]
    }
  }
}
```

## API Overview

- `thread/start` — create a new thread; emits `thread/started` (including the current `thread.status`) and auto-subscribes you to turn/item events for that thread. When the request includes a `cwd` and the resolved sandbox is `workspace-write` or full access, app-server also marks that project as trusted in the user `config.toml`. Pass `sessionStartSource: "clear"` when starting a replacement thread after clearing the current session so `SessionStart` hooks receive `source: "clear"` instead of the default `"startup"`.
- `thread/resume` — reopen an existing thread by id so subsequent `turn/start` calls append to it.
- `thread/fork` — fork an existing thread into a new thread id by copying the stored history; if the source thread is currently mid-turn, the fork records the same interruption marker as `turn/interrupt` instead of inheriting an unmarked partial turn suffix. The returned `thread.forkedFromId` points at the source thread when known. Accepts `ephemeral: true` for an in-memory temporary fork, emits `thread/started` (including the current `thread.status`), and auto-subscribes you to turn/item events for the new thread.
- `thread/list` — page through stored rollouts; supports cursor-based pagination and optional `modelProviders`, `sourceKinds`, `archived`, `cwd`, and `searchTerm` filters. Each returned `thread` includes `status` (`ThreadStatus`), defaulting to `notLoaded` when the thread is not currently loaded.
- `thread/loaded/list` — list the thread ids currently loaded in memory.
- `thread/read` — read a stored thread by id without resuming it; optionally include turns via `includeTurns`. The returned `thread` includes `status` (`ThreadStatus`), defaulting to `notLoaded` when the thread is not currently loaded.
- `thread/turns/list` — page through a stored thread’s turn history without resuming it; supports cursor-based pagination with `sortDirection`, `nextCursor`, and `backwardsCursor`.
- `thread/metadata/update` — patch stored thread metadata in sqlite; currently supports updating persisted `gitInfo` fields and returns the refreshed `thread`.
- `thread/memoryMode/set` — experimental; set a thread’s persisted memory eligibility to `"enabled"` or `"disabled"` for either a loaded thread or a stored rollout; returns `{}` on success.
- `memory/reset` — experimental; clear the current `CODEX_HOME/memories` directory and reset persisted memory stage data in sqlite while preserving existing thread memory modes; returns `{}` on success.
- `thread/status/changed` — notification emitted when a loaded thread’s status changes (`threadId` + new `status`).
- `thread/archive` — move a thread’s rollout file into the archived directory and attempt to move any spawned descendant thread rollout files; returns `{}` on success and emits `thread/archived` for each archived thread.
- `thread/unsubscribe` — unsubscribe this connection from thread turn/item events. If this was the last subscriber, the server keeps the thread loaded and unloads it only after it has had no subscribers and no thread activity for 30 minutes, then emits `thread/closed`.
- `thread/name/set` — set or update a thread’s user-facing name for either a loaded thread or a persisted rollout; returns `{}` on success and emits `thread/name/updated` to initialized, opted-in clients. Thread names are not required to be unique; name lookups resolve to the most recently updated thread.
- `thread/unarchive` — move an archived rollout file back into the sessions directory; returns the restored `thread` on success and emits `thread/unarchived`.
- `thread/compact/start` — trigger conversation history compaction for a thread; returns `{}` immediately while progress streams through standard turn/item notifications.
- `thread/shellCommand` — run a user-initiated `!` shell command against a thread; this runs unsandboxed with full access rather than inheriting the thread sandbox policy. Returns `{}` immediately while progress streams through standard turn/item notifications and any active turn receives the formatted output in its message stream.
- `thread/backgroundTerminals/clean` — terminate all running background terminals for a thread (experimental; requires `capabilities.experimentalApi`); returns `{}` when the cleanup request is accepted.
- `thread/rollback` — drop the last N turns from the agent’s in-memory context and persist a rollback marker in the rollout so future resumes see the pruned history; returns the updated `thread` (with `turns` populated) on success.
- `turn/start` — add user input to a thread and begin Codex generation; responds with the initial `turn` object and streams `turn/started`, `item/*`, and `turn/completed` notifications. For `collaborationMode`, `settings.developer_instructions: null` means "use built-in instructions for the selected mode".
- `thread/inject_items` — append raw Responses API items to a loaded thread’s model-visible history without starting a user turn; returns `{}` on success.
- `turn/steer` — add user input to an already in-flight regular turn without starting a new turn; returns the active `turnId` that accepted the input. Review and manual compaction turns reject `turn/steer`.
- `turn/interrupt` — request cancellation of an in-flight turn by `(thread_id, turn_id)`; success is an empty `{}` response and the turn finishes with `status: "interrupted"`.
- `thread/realtime/start` — start a thread-scoped realtime session (experimental); pass `outputModality: "text"` or `outputModality: "audio"` to choose model output, returns `{}` and streams `thread/realtime/*` notifications. Omit `transport` for the websocket transport, or pass `{ "type": "webrtc", "sdp": "..." }` to create a WebRTC session from a browser-generated SDP offer; the remote answer SDP is emitted as `thread/realtime/sdp`.
- `thread/realtime/appendAudio` — append an input audio chunk to the active realtime session (experimental); returns `{}`.
- `thread/realtime/appendText` — append text input to the active realtime session (experimental); returns `{}`.
- `thread/realtime/stop` — stop the active realtime session for the thread (experimental); returns `{}`.
- `review/start` — kick off Codex’s automated reviewer for a thread; responds like `turn/start` and emits `item/started`/`item/completed` notifications with `enteredReviewMode` and `exitedReviewMode` items, plus a final assistant `agentMessage` containing the review.
- `command/exec` — run a single command under the server sandbox without starting a thread/turn (handy for utilities and validation).
- `command/exec/write` — write base64-decoded stdin bytes to a running `command/exec` session or close stdin; returns `{}`.
- `command/exec/resize` — resize a running PTY-backed `command/exec` session by `processId`; returns `{}`.
- `command/exec/terminate` — terminate a running `command/exec` session by `processId`; returns `{}`.
- `command/exec/outputDelta` — notification emitted for base64-encoded stdout/stderr chunks from a streaming `command/exec` session.
- `fs/readFile` — read an absolute file path and return `{ dataBase64 }`.
- `fs/writeFile` — write an absolute file path from base64-encoded `{ dataBase64 }`; returns `{}`.
- `fs/createDirectory` — create an absolute directory path; `recursive` defaults to `true`.
- `fs/getMetadata` — return metadata for an absolute path: `isDirectory`, `isFile`, `isSymlink`, `createdAtMs`, and `modifiedAtMs`.
- `fs/readDirectory` — list direct child entries for an absolute directory path; each entry contains `fileName`, `isDirectory`, and `isFile`, and `fileName` is just the child name, not a path.
- `fs/remove` — remove an absolute file or directory tree; `recursive` and `force` default to `true`.
- `fs/copy` — copy between absolute paths; directory copies require `recursive: true`.
- `fs/watch` — subscribe this connection to filesystem change notifications for an absolute file or directory path and caller-provided `watchId`; returns the canonicalized `path`.
- `fs/unwatch` — stop sending notifications for a prior `fs/watch`; returns `{}`.
- `fs/changed` — notification emitted when watched paths change, including the `watchId` and `changedPaths`.
- `model/list` — list available models (set `includeHidden: true` to include entries with `hidden: true`), with reasoning effort options, `additionalSpeedTiers`, optional legacy `upgrade` model ids, optional `upgradeInfo` metadata (`model`, `upgradeCopy`, `modelLink`, `migrationMarkdown`), and optional `availabilityNux` metadata.
- `experimentalFeature/list` — list feature flags with stage metadata (`beta`, `underDevelopment`, `stable`, etc.), enabled/default-enabled state, and cursor pagination. For non-beta flags, `displayName`/`description`/`announcement` are `null`.
- `experimentalFeature/enablement/set` — patch the in-memory process-wide runtime feature enablement for the currently supported feature keys (`apps`, `plugins`). For each feature, precedence is: cloud requirements > --enable <feature_name> > config.toml > experimentalFeature/enablement/set (new) > code default.
- `collaborationMode/list` — list available collaboration mode presets (experimental, no pagination). This response omits built-in developer instructions; clients should either pass `settings.developer_instructions: null` when setting a mode to use Codex's built-in instructions, or provide their own instructions explicitly.
- `skills/list` — list skills for one or more `cwd` values (optional `forceReload`).
- `marketplace/add` — add a remote plugin marketplace from an HTTP(S) Git URL, SSH Git URL, or GitHub `owner/repo` shorthand, then persist it into the user marketplace config. Returns the installed root path plus whether the marketplace was already present.
- `marketplace/remove` — remove a configured marketplace by name from the user marketplace config, and delete its installed marketplace root when one exists.
- `plugin/list` — list discovered plugin marketplaces and plugin state, including effective marketplace install/auth policy metadata, fail-open `marketplaceLoadErrors` entries for marketplace files that could not be parsed or loaded, and best-effort `featuredPluginIds` for the official curated marketplace. `interface.category` uses the marketplace category when present; otherwise it falls back to the plugin manifest category (**under development; do not call from production clients yet**).
- `plugin/read` — read one plugin by `marketplacePath` plus `pluginName`, returning marketplace info, a list-style `summary`, manifest descriptions/interface metadata, and bundled skills/apps/MCP server names. Returned plugin skills include their current `enabled` state after local config filtering. Plugin app summaries also include `needsAuth` when the server can determine connector accessibility (**under development; do not call from production clients yet**).
- `skills/changed` — notification emitted when watched local skill files change.
- `app/list` — list available apps.
- `skills/config/write` — write user-level skill config by name or absolute path.
- `plugin/install` — install a plugin from a discovered marketplace entry, rejecting marketplace entries marked unavailable for install, install MCPs if any, and return the effective plugin auth policy plus any apps that still need auth (**under development; do not call from production clients yet**).
- `plugin/uninstall` — uninstall a plugin by id by removing its cached files and clearing its user-level config entry (**under development; do not call from production clients yet**).
- `mcpServer/oauth/login` — start an OAuth login for a configured MCP server; returns an `authorization_url` and later emits `mcpServer/oauthLogin/completed` once the browser flow finishes.
- `tool/requestUserInput` — prompt the user with 1–3 short questions for a tool call and return their answers (experimental).
- `config/mcpServer/reload` — reload MCP server config from disk and queue a refresh for loaded threads (applied on each thread's next active turn); returns `{}`. Use this after editing `config.toml` without restarting the server.
- `mcpServerStatus/list` — enumerate configured MCP servers with their tools and auth status, plus resources/resource templates for `full` detail; supports cursor+limit pagination. If `detail` is omitted, the server defaults to `full`.
- `mcpServer/resource/read` — read a resource from a configured MCP server by optional `threadId`, `server`, and `uri`, returning text/blob resource `contents`. If `threadId` is omitted, the server reads from the latest MCP config directly.
- `mcpServer/tool/call` — call a tool on a thread's configured MCP server by `threadId`, `server`, `tool`, optional `arguments`, and optional `_meta`, returning the MCP tool result.
- `windowsSandbox/setupStart` — start Windows sandbox setup for the selected mode (`elevated` or `unelevated`); accepts an optional absolute `cwd` to target setup for a specific workspace, returns `{ started: true }` immediately, and later emits `windowsSandbox/setupCompleted`.
- `feedback/upload` — submit a feedback report (classification + optional reason/logs, conversation_id, and optional `extraLogFiles` attachments array); returns the tracking thread id.
- `config/read` — fetch the effective config on disk after resolving config layering.
- `externalAgentConfig/detect` — detect migratable external-agent artifacts with `includeHome` and optional `cwds`; each detected item includes `cwd` (`null` for home), and plugin migration items may additionally include structured `details` grouping plugin ids under each detected marketplace name.
- `externalAgentConfig/import` — apply selected external-agent migration items by passing explicit `migrationItems` with `cwd` (`null` for home) and any plugin `details` returned by detect. When a request includes plugin imports, the server emits `externalAgentConfig/import/completed` after the full import finishes (immediately after the response when everything completed synchronously, or after background remote imports finish).
- `config/value/write` — write a single config key/value to the user's config.toml on disk.
- `config/batchWrite` — apply multiple config edits atomically to the user's config.toml on disk, with optional `reloadUserConfig: true` to hot-reload loaded threads.
- `configRequirements/read` — fetch loaded requirements constraints from `requirements.toml` and/or MDM (or `null` if none are configured), including allow-lists (`allowedApprovalPolicies`, `allowedSandboxModes`, `allowedWebSearchModes`), pinned feature values (`featureRequirements`), `enforceResidency`, and `network` constraints such as canonical domain/socket permissions plus `managedAllowedDomainsOnly`.

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
    "serviceName": "my_app_server_client", // optional metrics tag (`service_name`)
    "sessionStartSource": "startup", // optional: "startup" (default) or "clear"
    // Experimental: requires opt-in
    "dynamicTools": [
        {
            "name": "lookup_ticket",
            "description": "Fetch a ticket by id",
            "deferLoading": true,
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

To continue a stored session, call `thread/resume` with the `thread.id` you previously recorded. The response shape matches `thread/start`. When the stored session includes persisted token usage, the server emits `thread/tokenUsage/updated` immediately after the response so clients can render restored usage before the next turn starts. You can also pass the same configuration overrides supported by `thread/start`, including `approvalsReviewer`.

By default, resume uses the latest persisted `model` and `reasoningEffort` values associated with the thread. Supplying any of `model`, `modelProvider`, `config.model`, or `config.model_reasoning_effort` disables that persisted fallback and uses the explicit overrides plus normal config resolution instead.

Example:

```json
{ "method": "thread/resume", "id": 11, "params": {
    "threadId": "thr_123",
    "personality": "friendly"
} }
{ "id": 11, "result": { "thread": { "id": "thr_123", … } } }
```

To branch from a stored session, call `thread/fork` with the `thread.id`. This creates a new thread id and emits a `thread/started` notification for it. When the source history includes persisted token usage, the server also emits `thread/tokenUsage/updated` for the new thread immediately after the response. If the source thread is actively running, the fork snapshots it as if the current turn had been interrupted first. Pass `ephemeral: true` when the fork should stay in-memory only:

```json
{ "method": "thread/fork", "id": 12, "params": { "threadId": "thr_123", "ephemeral": true } }
{ "id": 12, "result": { "thread": { "id": "thr_456", … } } }
{ "method": "thread/started", "params": { "thread": { … } } }
```

Experimental API: `thread/start`, `thread/resume`, and `thread/fork` accept `persistExtendedHistory: true` to persist a richer subset of ThreadItems for non-lossy history when calling `thread/read`, `thread/resume`, and `thread/fork` later. This does not backfill events that were not persisted previously.

### Example: List threads (with pagination & filters)

`thread/list` lets you render a history UI. Results default to `createdAt` (newest first) descending. Pass any combination of:

- `cursor` — opaque string from a prior response; omit for the first page.
- `limit` — server defaults to a reasonable page size if unset.
- `sortKey` — `created_at` (default) or `updated_at`.
- `sortDirection` — `desc` (default) or `asc`.
- `modelProviders` — restrict results to specific providers; unset, null, or an empty array will include all providers.
- `sourceKinds` — restrict results to specific sources; omit or pass `[]` for interactive sessions only (`cli`, `vscode`).
- `archived` — when `true`, list archived threads only. When `false` or `null`, list non-archived threads (default).
- `cwd` — restrict results to threads whose session cwd exactly matches this path. Relative paths are resolved against the app-server process cwd before matching.
- `searchTerm` — restrict results to threads whose extracted title contains this substring (case-sensitive).
- Responses include `nextCursor` to continue in the same direction and `backwardsCursor` to pass as `cursor` when reversing `sortDirection`.
- Responses include `agentNickname` and `agentRole` for AgentControl-spawned thread sub-agents when available.

Example:

```json
{ "method": "thread/list", "id": 20, "params": {
    "cursor": null,
    "limit": 25,
    "sortKey": "created_at"
} }
{ "id": 20, "result": {
    "data": [
        { "id": "thr_a", "preview": "Create a TUI", "modelProvider": "openai", "createdAt": 1730831111, "updatedAt": 1730831111, "status": { "type": "notLoaded" }, "agentNickname": "Atlas", "agentRole": "explorer" },
        { "id": "thr_b", "preview": "Fix tests", "modelProvider": "openai", "createdAt": 1730750000, "updatedAt": 1730750000, "status": { "type": "notLoaded" } }
    ],
    "nextCursor": "opaque-token-or-null",
    "backwardsCursor": "opaque-token-or-null"
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

`thread/status/changed` is emitted whenever a loaded thread's status changes after it has already been introduced to the client:

- Includes `threadId` and the new `status`.
- Status can be `notLoaded`, `idle`, `systemError`, or `active` (with `activeFlags`; `active` implies running).
- `thread/start`, `thread/fork`, and detached review threads do not emit a separate initial `thread/status/changed`; their `thread/started` notification already carries the current `thread.status`.

```json
{
  "method": "thread/status/changed",
  "params": {
    "threadId": "thr_123",
    "status": { "type": "active", "activeFlags": [] }
  }
}
```

### Example: Unsubscribe from a loaded thread

`thread/unsubscribe` removes the current connection's subscription to a thread. The response status is one of:

- `unsubscribed` when the connection was subscribed and is now removed.
- `notSubscribed` when the connection was not subscribed to that thread.
- `notLoaded` when the thread is not loaded.

If this was the last subscriber, the server does not unload the thread immediately. It unloads the thread after the thread has had no subscribers and no thread activity for 30 minutes, then emits `thread/closed` and a `thread/status/changed` transition to `notLoaded`.

```json
{ "method": "thread/unsubscribe", "id": 22, "params": { "threadId": "thr_123" } }
{ "id": 22, "result": { "status": "unsubscribed" } }
```

Later, after the idle unload timeout:

```json
{ "method": "thread/status/changed", "params": {
    "threadId": "thr_123",
    "status": { "type": "notLoaded" }
} }
{ "method": "thread/closed", "params": { "threadId": "thr_123" } }
```

### Example: Read a thread

Use `thread/read` to fetch a stored thread by id without resuming it. Pass `includeTurns` when you want the full rollout history loaded into `thread.turns`. The returned thread includes `agentNickname` and `agentRole` for AgentControl-spawned thread sub-agents when available.

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

### Example: List thread turns

Use `thread/turns/list` to page a stored thread’s turn history without resuming it. By default, results are sorted descending so clients can start at the present and fetch older turns with `nextCursor`. The response also includes `backwardsCursor`; pass it as `cursor` on a later request with `sortDirection: "asc"` to fetch turns newer than the first item from the earlier page.

```json
{ "method": "thread/turns/list", "id": 24, "params": {
    "threadId": "thr_123",
    "limit": 50,
    "sortDirection": "desc"
} }
{ "id": 24, "result": {
    "data": [ ... ],
    "nextCursor": "older-turns-cursor-or-null",
    "backwardsCursor": "newer-turns-cursor-or-null"
} }
```

### Example: Update stored thread metadata

Use `thread/metadata/update` to patch sqlite-backed metadata for a thread without resuming it. Today this supports persisted `gitInfo`; omitted fields are left unchanged, while explicit `null` clears a stored value.

```json
{ "method": "thread/metadata/update", "id": 24, "params": {
    "threadId": "thr_123",
    "gitInfo": { "branch": "feature/sidebar-pr" }
} }
{ "id": 24, "result": {
    "thread": {
        "id": "thr_123",
        "gitInfo": { "sha": null, "branch": "feature/sidebar-pr", "originUrl": null }
    }
} }

{ "method": "thread/metadata/update", "id": 25, "params": {
    "threadId": "thr_123",
    "gitInfo": { "branch": null }
} }
{ "id": 25, "result": {
    "thread": {
        "id": "thr_123",
        "gitInfo": null
    }
} }
```

Experimental: use `thread/memoryMode/set` to change whether a thread remains eligible for future memory generation.

```json
{ "method": "thread/memoryMode/set", "id": 26, "params": {
    "threadId": "thr_123",
    "mode": "disabled"
} }
{ "id": 26, "result": {} }
```

Experimental: use `memory/reset` to clear local memory artifacts and sqlite-backed memory stage data for the current Codex home. This preserves existing thread memory modes; use `thread/memoryMode/set` separately when a thread's future memory eligibility should change.

```json
{ "method": "memory/reset", "id": 27 }
{ "id": 27, "result": {} }
```

### Example: Archive a thread

Use `thread/archive` to move the persisted rollout (stored as a JSONL file on disk) into the archived sessions directory and attempt to move any spawned descendant thread rollouts.

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

### Example: Run a thread shell command

Use `thread/shellCommand` for the TUI `!` workflow. The request returns immediately with `{}`.
This API runs unsandboxed with full access; it does not inherit the thread
sandbox policy.

If the thread already has an active turn, the command runs as an auxiliary action on that turn. In that case, progress is emitted as standard `item/*` notifications on the existing turn and the formatted output is injected into the turn’s message stream:

- `item/started` with `item: { "type": "commandExecution", "source": "userShell", ... }`
- zero or more `item/commandExecution/outputDelta`
- `item/completed` with the same `commandExecution` item id

If the thread does not already have an active turn, the server starts a standalone turn for the shell command. In that case clients should expect:

- `turn/started`
- `item/started` with `item: { "type": "commandExecution", "source": "userShell", ... }`
- zero or more `item/commandExecution/outputDelta`
- `item/completed` with the same `commandExecution` item id
- `turn/completed`

```json
{ "method": "thread/shellCommand", "id": 26, "params": { "threadId": "thr_b", "command": "git status --short" } }
{ "id": 26, "result": {} }
```

### Example: Start a turn (send user input)

Turns attach user input (text or images) to a thread and trigger Codex generation. The `input` field is a list of discriminated unions:

- `{"type":"text","text":"Explain this diff"}`
- `{"type":"image","url":"https://…png"}`
- `{"type":"localImage","path":"/tmp/screenshot.png"}`

You can optionally specify config overrides on the new turn. If specified, these settings become the default for subsequent turns on the same thread. `outputSchema` applies only to the current turn.

`approvalsReviewer` accepts:

- `"user"` — default. Review approval requests directly in the client.
- `"guardian_subagent"` — route approval requests to a carefully prompted subagent, which gathers relevant context and applies a risk-based decision framework before approving or denying the request.

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

### Example: Start a turn (invoke a plugin)

Invoke a plugin by including a UI mention token such as `@sample` in the text input and adding a `mention` input item with the exact `plugin://<plugin-name>@<marketplace-name>` path returned by `plugin/list`.

```json
{ "method": "turn/start", "id": 35, "params": {
    "threadId": "thr_123",
    "input": [
        { "type": "text", "text": "@sample Summarize the latest updates." },
        { "type": "mention", "name": "Sample Plugin", "path": "plugin://sample@test" }
    ]
} }
{ "id": 35, "result": { "turn": {
    "id": "turn_459",
    "status": "inProgress",
    "items": [],
    "error": null
} } }
```

### Example: Inject raw history items

Use `thread/inject_items` to append prebuilt Responses API items to a loaded thread’s prompt history without starting a user turn. These items are persisted to the rollout and included in subsequent model requests.

```json
{ "method": "thread/inject_items", "id": 36, "params": {
    "threadId": "thr_123",
    "items": [
        {
            "type": "message",
            "role": "assistant",
            "content": [{ "type": "output_text", "text": "Previously computed context." }]
        }
    ]
} }
{ "id": 36, "result": {} }
```

### Example: Start realtime with WebRTC

Use `thread/realtime/start` with `transport.type: "webrtc"` when a browser or webview owns the `RTCPeerConnection` and app-server should create the server-side realtime session. The transport `sdp` must be the offer SDP produced by `RTCPeerConnection.createOffer()`, not a hand-written or minimal SDP string.

The offer should include the media sections the client wants to negotiate. For the standard realtime UI flow, create the audio track/transceiver and the `oai-events` data channel before calling `createOffer()`:

```javascript
const pc = new RTCPeerConnection();

audioElement.autoplay = true;
pc.ontrack = (event) => {
  audioElement.srcObject = event.streams[0];
};

const mediaStream = await navigator.mediaDevices.getUserMedia({ audio: true });
pc.addTrack(mediaStream.getAudioTracks()[0], mediaStream);
pc.createDataChannel("oai-events");

const offer = await pc.createOffer();
await pc.setLocalDescription(offer);
```

Then send `offer.sdp` to app-server. Core uses `experimental_realtime_ws_backend_prompt` for the backend instructions and the thread conversation id for the realtime session id. The start response is `{}`; the remote answer SDP arrives later as `thread/realtime/sdp` and should be passed to `setRemoteDescription()`:

```json
{ "method": "thread/realtime/start", "id": 40, "params": {
    "threadId": "thr_123",
    "outputModality": "audio",
    "prompt": "You are on a call.",
    "sessionId": null,
    "transport": { "type": "webrtc", "sdp": "v=0\r\no=..." }
} }
{ "id": 40, "result": {} }
{ "method": "thread/realtime/sdp", "params": {
    "threadId": "thr_123",
    "sdp": "v=0\r\no=..."
} }
```

Omit `prompt` to use Codex's default realtime backend prompt. Send `prompt: null` or
`prompt: ""` when the session should start without that default backend prompt.

```javascript
await pc.setRemoteDescription({
  type: "answer",
  sdp: notification.params.sdp,
});
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

The server requests cancellation of the active turn, then emits a `turn/completed` event with `status: "interrupted"`. This does not terminate background terminals; use `thread/backgroundTerminals/clean` when you explicitly want to stop those shells. Rely on the `turn/completed` event to know when turn interruption has finished.

### Example: Clean background terminals

Use `thread/backgroundTerminals/clean` to terminate all running background terminals associated with a thread. This method is experimental and requires `capabilities.experimentalApi = true`.

```json
{ "method": "thread/backgroundTerminals/clean", "id": 35, "params": {
    "threadId": "thr_123"
} }
{ "id": 35, "result": {} }
```

### Example: Steer an active turn

Use `turn/steer` to append additional user input to the currently active regular turn. This does
not emit `turn/started` and does not accept turn context overrides.

```json
{ "method": "turn/steer", "id": 32, "params": {
    "threadId": "thr_123",
    "input": [ { "type": "text", "text": "Actually focus on failing tests first." } ],
    "expectedTurnId": "turn_456"
} }
{ "id": 32, "result": { "turnId": "turn_456" } }
```

`expectedTurnId` is required. If there is no active turn, `expectedTurnId` does not match the
active turn, or the active turn kind does not accept same-turn steering (for example review or
manual compaction), the request fails with an `invalid request` error.

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
    "processId": "ls-1",                           // optional string; required for streaming and ability to terminate the process
    "cwd": "/Users/me/project",                    // optional; defaults to server cwd
    "env": { "FOO": "override" },                  // optional; merges into the server env and overrides matching names
    "size": { "rows": 40, "cols": 120 },           // optional; PTY size in character cells, only valid with tty=true
    "sandboxPolicy": { "type": "workspaceWrite" }, // optional; defaults to user config
    "outputBytesCap": 1048576,                     // optional; per-stream capture cap
    "disableOutputCap": false,                     // optional; cannot be combined with outputBytesCap
    "timeoutMs": 10000,                            // optional; ms timeout; defaults to server timeout
    "disableTimeout": false                        // optional; cannot be combined with timeoutMs
} }
{ "id": 32, "result": {
    "exitCode": 0,
    "stdout": "...",
    "stderr": ""
} }
```

- For clients that are already sandboxed externally, set `sandboxPolicy` to `{"type":"externalSandbox","networkAccess":"enabled"}` (or omit `networkAccess` to keep it restricted). Codex will not enforce its own sandbox in this mode; it tells the model it has full file-system access and passes the `networkAccess` state through `environment_context`.

Notes:

- Empty `command` arrays are rejected.
- `sandboxPolicy` accepts the same shape used by `turn/start` (e.g., `dangerFullAccess`, `readOnly`, `workspaceWrite` with flags, `externalSandbox` with `networkAccess` `restricted|enabled`).
- `env` merges into the environment produced by the server's shell environment policy. Matching names are overridden; unspecified variables are left intact.
- When omitted, `timeoutMs` falls back to the server default.
- When omitted, `outputBytesCap` falls back to the server default of 1 MiB per stream.
- `disableOutputCap: true` disables stdout/stderr capture truncation for that `command/exec` request. It cannot be combined with `outputBytesCap`.
- `disableTimeout: true` disables the timeout entirely for that `command/exec` request. It cannot be combined with `timeoutMs`.
- `processId` is optional for buffered execution. When omitted, Codex generates an internal id for lifecycle tracking, but `tty`, `streamStdin`, and `streamStdoutStderr` must stay disabled and follow-up `command/exec/write` / `command/exec/terminate` calls are not available for that command.
- `size` is only valid when `tty: true`. It sets the initial PTY size in character cells.
- Buffered Windows sandbox execution accepts `processId` for correlation, but `command/exec/write` and `command/exec/terminate` are still unsupported for those requests.
- Buffered Windows sandbox execution also requires the default output cap; custom `outputBytesCap` and `disableOutputCap` are unsupported there.
- `tty`, `streamStdin`, and `streamStdoutStderr` are optional booleans. Legacy requests that omit them continue to use buffered execution.
- `tty: true` implies PTY mode plus `streamStdin: true` and `streamStdoutStderr: true`.
- `tty` and `streamStdin` do not disable the timeout on their own; omit `timeoutMs` to use the server default timeout, or set `disableTimeout: true` to keep the process alive until exit or explicit termination.
- `outputBytesCap` applies independently to `stdout` and `stderr`, and streamed bytes are not duplicated into the final response.
- The `command/exec` response is deferred until the process exits and is sent only after all `command/exec/outputDelta` notifications for that connection have been emitted.
- `command/exec/outputDelta` notifications are connection-scoped. If the originating connection closes, the server terminates the process.

Streaming stdin/stdout uses base64 so PTY sessions can carry arbitrary bytes:

```json
{ "method": "command/exec", "id": 33, "params": {
    "command": ["bash", "-i"],
    "processId": "bash-1",
    "tty": true,
    "outputBytesCap": 32768
} }
{ "method": "command/exec/outputDelta", "params": {
    "processId": "bash-1",
    "stream": "stdout",
    "deltaBase64": "YmFzaC00LjQkIA==",
    "capReached": false
} }
{ "method": "command/exec/write", "id": 34, "params": {
    "processId": "bash-1",
    "deltaBase64": "cHdkCg=="
} }
{ "id": 34, "result": {} }
{ "method": "command/exec/write", "id": 35, "params": {
    "processId": "bash-1",
    "closeStdin": true
} }
{ "id": 35, "result": {} }
{ "method": "command/exec/resize", "id": 36, "params": {
    "processId": "bash-1",
    "size": { "rows": 48, "cols": 160 }
} }
{ "id": 36, "result": {} }
{ "method": "command/exec/terminate", "id": 37, "params": {
    "processId": "bash-1"
} }
{ "id": 37, "result": {} }
{ "id": 33, "result": {
    "exitCode": 137,
    "stdout": "",
    "stderr": ""
} }
```

- `command/exec/write` accepts either `deltaBase64`, `closeStdin`, or both.
- Clients may supply a connection-scoped string `processId` in `command/exec`; `command/exec/write`, `command/exec/resize`, and `command/exec/terminate` only accept those client-supplied string ids.
- `command/exec/outputDelta.processId` is always the client-supplied string id from the original `command/exec` request.
- `command/exec/outputDelta.stream` is `stdout` or `stderr`. PTY mode multiplexes terminal output through `stdout`.
- `command/exec/outputDelta.capReached` is `true` on the final streamed chunk for a stream when `outputBytesCap` truncates that stream; later output on that stream is dropped.
- `command/exec.params.env` overrides the server-computed environment per key; set a key to `null` to unset an inherited variable.
- `command/exec/resize` is only supported for PTY-backed `command/exec` sessions.

### Example: Filesystem utilities

These methods operate on absolute paths on the host filesystem and cover reading, writing, directory traversal, copying, removal, and change notifications.

All filesystem paths in this section must be absolute.

```json
{ "method": "fs/createDirectory", "id": 40, "params": {
    "path": "/tmp/example/nested",
    "recursive": true
} }
{ "id": 40, "result": {} }
{ "method": "fs/writeFile", "id": 41, "params": {
    "path": "/tmp/example/nested/note.txt",
    "dataBase64": "aGVsbG8="
} }
{ "id": 41, "result": {} }
{ "method": "fs/getMetadata", "id": 42, "params": {
    "path": "/tmp/example/nested/note.txt"
} }
{ "id": 42, "result": {
    "isDirectory": false,
    "isFile": true,
    "isSymlink": false,
    "createdAtMs": 1730910000000,
    "modifiedAtMs": 1730910000000
} }
{ "method": "fs/readFile", "id": 43, "params": {
    "path": "/tmp/example/nested/note.txt"
} }
{ "id": 43, "result": {
    "dataBase64": "aGVsbG8="
} }
```

- `fs/getMetadata` returns whether the path resolves to a directory or regular file, whether the path itself is a symlink, plus `createdAtMs` and `modifiedAtMs` in Unix milliseconds. If a timestamp is unavailable on the current platform, that field is `0`.
- `fs/createDirectory` defaults `recursive` to `true` when omitted.
- `fs/remove` defaults both `recursive` and `force` to `true` when omitted.
- `fs/readFile` always returns base64 bytes via `dataBase64`, and `fs/writeFile` always expects base64 bytes in `dataBase64`.
- `fs/copy` handles both file copies and directory-tree copies; it requires `recursive: true` when `sourcePath` is a directory. Recursive copies traverse regular files, directories, and symlinks; other entry types are skipped.

### Example: Filesystem watch

`fs/watch` accepts absolute file or directory paths. Watching a file emits `fs/changed` for that file path, including updates delivered via replace or rename operations.

```json
{ "method": "fs/watch", "id": 44, "params": {
    "watchId": "0195ec6b-1d6f-7c2e-8c7a-56f2c4a8b9d1",
    "path": "/Users/me/project/.git/HEAD"
} }
{ "id": 44, "result": {
    "path": "/Users/me/project/.git/HEAD"
} }
{ "method": "fs/changed", "params": {
    "watchId": "0195ec6b-1d6f-7c2e-8c7a-56f2c4a8b9d1",
    "changedPaths": ["/Users/me/project/.git/HEAD"]
} }
{ "method": "fs/unwatch", "id": 45, "params": {
    "watchId": "0195ec6b-1d6f-7c2e-8c7a-56f2c4a8b9d1"
} }
{ "id": 45, "result": {} }
```

## Events

Event notifications are the server-initiated event stream for thread lifecycles, turn lifecycles, and the items within them. After you start or resume a thread, keep reading stdout for `thread/started`, `thread/archived`, `thread/unarchived`, `thread/closed`, `turn/*`, and `item/*` notifications.

Thread realtime uses a separate thread-scoped notification surface. `thread/realtime/*` notifications are ephemeral transport events, not `ThreadItem`s, and are not returned by `thread/read`, `thread/resume`, or `thread/fork`.

Recoverable configuration and initialization warnings use the existing `configWarning` notification: `{ summary, details?, path?, range? }`. App-server may emit it during initialization for config parsing and related setup diagnostics.

Generic runtime warnings use the `warning` notification: `{ threadId?, message }`. App-server emits this for non-fatal warnings from the core event stream, including cases where not all enabled skills are included in the model-visible skills list for a session.

### Notification opt-out

Clients can suppress specific notifications per connection by sending exact method names in `initialize.params.capabilities.optOutNotificationMethods`.

- Exact-match only: `item/agentMessage/delta` suppresses only that method.
- Unknown method names are ignored.
- Applies to app-server typed notifications such as `thread/*`, `turn/*`, `item/*`, and `rawResponseItem/*`.
- Does not apply to requests/responses/errors.

Examples:

- Opt out of thread lifecycle notifications: `thread/started`
- Opt out of streamed agent text deltas: `item/agentMessage/delta`

### Fuzzy file search events (experimental)

The fuzzy file search session API emits per-query notifications:

- `fuzzyFileSearch/sessionUpdated` — `{ sessionId, query, files }` with the current matching files for the active query.
- `fuzzyFileSearch/sessionCompleted` — `{ sessionId, query }` once indexing/matching for that query has completed.

### Thread realtime events (experimental)

The thread realtime API emits thread-scoped notifications for session lifecycle and streaming media:

- `thread/realtime/started` — `{ threadId, sessionId }` once realtime starts for the thread (experimental).
- `thread/realtime/itemAdded` — `{ threadId, item }` for raw non-audio realtime items that do not have a dedicated typed app-server notification, including `handoff_request` (experimental). `item` is forwarded as raw JSON while the upstream websocket item schema remains unstable.
- `thread/realtime/transcript/delta` — `{ threadId, role, delta }` for live realtime transcript deltas (experimental).
- `thread/realtime/transcript/done` — `{ threadId, role, text }` when realtime emits the final full text for a transcript part (experimental).
- `thread/realtime/outputAudio/delta` — `{ threadId, audio }` for streamed output audio chunks (experimental). `audio` uses camelCase fields (`data`, `sampleRate`, `numChannels`, `samplesPerChannel`).
- `thread/realtime/error` — `{ threadId, message }` when realtime encounters a transport or backend error (experimental).
- `thread/realtime/closed` — `{ threadId, reason }` when the realtime transport closes (experimental).

Because audio is intentionally separate from `ThreadItem`, clients can opt out of `thread/realtime/outputAudio/delta` independently with `optOutNotificationMethods`.

### Windows sandbox setup events

- `windowsSandbox/setupCompleted` — `{ mode, success, error }` after a `windowsSandbox/setupStart` request finishes.

### MCP server startup events

- `mcpServer/startupStatus/updated` — `{ name, status, error }` when app-server observes an MCP server startup transition. `status` is one of `starting`, `ready`, `failed`, or `cancelled`. `error` is `null` except for `failed`.

### Turn events

The app-server streams JSON-RPC notifications while a turn is running. Each turn emits `turn/started` when it begins running and ends with `turn/completed` (final `turn` status). Token usage events stream separately via `thread/tokenUsage/updated`. Clients subscribe to the events they care about, rendering each item incrementally as updates arrive. The per-item lifecycle is always: `item/started` → zero or more item-specific deltas → `item/completed`.

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

All items emit shared lifecycle events:

- `item/started` — emits the full `item` when a new unit of work begins so the UI can render it immediately; the `item.id` in this payload matches the `itemId` used by deltas.
- `item/completed` — sends the final `item` once that work itself finishes (for example, after a tool call or message completes); treat this as the authoritative execution/result state.
- `item/autoApprovalReview/started` — [UNSTABLE] temporary auto-review notification carrying `{threadId, turnId, targetItemId, review, action}` when approval auto-review begins. This shape is expected to change soon.
- `item/autoApprovalReview/completed` — [UNSTABLE] temporary auto-review notification carrying `{threadId, turnId, targetItemId, review, action}` when approval auto-review resolves. This shape is expected to change soon.

`review` is [UNSTABLE] and currently has `{status, riskLevel?, userAuthorization?, rationale?}`, where `status` is one of `inProgress`, `approved`, `denied`, or `aborted`. `riskLevel` is one of `"low"`, `"medium"`, `"high"`, or `"critical"` when present. `userAuthorization` is one of `"unknown"`, `"low"`, `"medium"`, or `"high"` when present. `action` is a tagged union with `type: "command" | "execve" | "applyPatch" | "networkAccess" | "mcpToolCall"`. Command-like actions include a `source` discriminator (`"shell"` or `"unifiedExec"`). These notifications are separate from the target item's own `item/completed` lifecycle and are intentionally temporary while the auto-review app protocol is still being designed.

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

- `item/fileChange/patchUpdated` - when `features.apply_patch_streaming_events` is enabled, streams structured file-change snapshots parsed from the model-generated patch before it is executed.
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
- `ActiveTurnNotSteerable { turnKind }`: `turn/start` or `turn/steer` was submitted while the
  current active turn was not steerable, for example `/review` or manual `/compact`
- `BadRequest`
- `Unauthorized`
- `SandboxError`
- `InternalServerError`
- `Other`: all unclassified errors

When an upstream HTTP status is available (for example, from the Responses API or a provider), it is forwarded in `httpStatusCode` on the relevant `codexErrorInfo` variant.

## Approvals

Certain actions (shell commands or modifying files) may require explicit user approval depending on the user's config. When `turn/start` is used, the app-server drives an approval flow by sending a server-initiated JSON-RPC request to the client. The client must respond to tell Codex whether to proceed. UIs should present these requests inline with the active turn so users can review the proposed command or diff before choosing.

- Requests include `threadId` and `turnId`—use them to scope UI state to the active conversation.
- Respond with a single `{ "decision": ... }` payload. Command approvals support `accept`, `acceptForSession`, `acceptWithExecpolicyAmendment`, `applyNetworkPolicyAmendment`, `decline`, or `cancel`. The server resumes or declines the work and ends the item with `item/completed`.

### Command execution approvals

Order of messages:

1. `item/started` — shows the pending `commandExecution` item with `command`, `cwd`, and other fields so you can render the proposed action.
2. `item/commandExecution/requestApproval` (request) — carries the same `itemId`, `threadId`, `turnId`, optionally `approvalId` (for subcommand callbacks), and `reason`. For normal command approvals, it also includes `command`, `cwd`, and `commandActions` for friendly display. When `initialize.params.capabilities.experimentalApi = true`, it may also include experimental `additionalPermissions` describing requested per-command sandbox access; any filesystem paths in that payload are absolute on the wire, and network access is represented as `additionalPermissions.network.enabled`. For network-only approvals, those command fields may be omitted and `networkApprovalContext` is provided instead. Optional persistence hints may also be included via `proposedExecpolicyAmendment` and `proposedNetworkPolicyAmendments`. Clients can prefer `availableDecisions` when present to render the exact set of choices the server wants to expose, while still falling back to the older heuristics if it is omitted.
3. Client response — for example `{ "decision": "accept" }`, `{ "decision": "acceptForSession" }`, `{ "decision": { "acceptWithExecpolicyAmendment": { "execpolicy_amendment": [...] } } }`, `{ "decision": { "applyNetworkPolicyAmendment": { "network_policy_amendment": { "host": "example.com", "action": "allow" } } } }`, `{ "decision": "decline" }`, or `{ "decision": "cancel" }`.
4. `serverRequest/resolved` — `{ threadId, requestId }` confirms the pending request has been resolved or cleared, including lifecycle cleanup on turn start/complete/interrupt.
5. `item/completed` — final `commandExecution` item with `status: "completed" | "failed" | "declined"` and execution output. Render this as the authoritative result.

### File change approvals

Order of messages:

1. `item/started` — emits a `fileChange` item with `changes` (diff chunk summaries) and `status: "inProgress"`. Show the proposed edits and paths to the user.
2. `item/fileChange/requestApproval` (request) — includes `itemId`, `threadId`, `turnId`, an optional `reason`, and may include unstable `grantRoot` when the agent is asking for session-scoped write access under a specific root.
3. Client response — `{ "decision": "accept" }`, `{ "decision": "acceptForSession" }`, `{ "decision": "decline" }`, or `{ "decision": "cancel" }`.
4. `serverRequest/resolved` — `{ threadId, requestId }` confirms the pending request has been resolved or cleared, including lifecycle cleanup on turn start/complete/interrupt.
5. `item/completed` — returns the same `fileChange` item with `status` updated to `completed`, `failed`, or `declined` after the patch attempt. Rely on this to show success/failure and finalize the diff state in your UI.

UI guidance for IDEs: surface an approval dialog as soon as the request arrives. The turn will proceed after the server receives a response to the approval request. The terminal `item/completed` notification will be sent with the appropriate status.

### request_user_input

When the client responds to `item/tool/requestUserInput`, the server emits `serverRequest/resolved` with `{ threadId, requestId }`. If the pending request is cleared by turn start, turn completion, or turn interruption before the client answers, the server emits the same notification for that cleanup.

### MCP server elicitations

MCP servers can interrupt a turn and ask the client for structured input via `mcpServer/elicitation/request`.

Order of messages:

1. `mcpServer/elicitation/request` (request) — includes `threadId`, nullable `turnId`, `serverName`, and either:
   - a form request: `{ "mode": "form", "message": "...", "requestedSchema": { ... } }`
   - a URL request: `{ "mode": "url", "message": "...", "url": "...", "elicitationId": "..." }`
2. Client response — `{ "action": "accept", "content": ... }`, `{ "action": "decline", "content": null }`, or `{ "action": "cancel", "content": null }`.
3. `serverRequest/resolved` — `{ threadId, requestId }` confirms the pending request has been resolved or cleared, including lifecycle cleanup on turn start/complete/interrupt.

`turnId` is best-effort. When the elicitation is correlated with an active turn, the request includes that turn id; otherwise it is `null`.

For MCP tool approval elicitations, form request `meta` includes
`codex_approval_kind: "mcp_tool_call"` and may include `persist: "session"`,
`persist: "always"`, or `persist: ["session", "always"]` to advertise whether
the client can offer session-scoped and/or persistent approval choices.

### Permission requests

The built-in `request_permissions` tool sends an `item/permissions/requestApproval` JSON-RPC request to the client with the requested permission profile. This v2 payload mirrors the command-execution `additionalPermissions` shape: it can request network access and additional filesystem access.

```json
{
  "method": "item/permissions/requestApproval",
  "id": 61,
  "params": {
    "threadId": "thr_123",
    "turnId": "turn_123",
    "itemId": "call_123",
    "reason": "Select a workspace root",
    "permissions": {
      "fileSystem": {
        "write": ["/Users/me/project", "/Users/me/shared"]
      }
    }
  }
}
```

The client responds with `result.permissions`, which should be the granted subset of the requested permission profile. It may also set `result.scope` to `"session"` to make the grant persist for later turns in the same session; omitted or `"turn"` keeps the existing turn-scoped behavior:

```json
{
  "id": 61,
  "result": {
    "scope": "session",
    "permissions": {
      "fileSystem": {
        "write": ["/Users/me/project"]
      }
    }
  }
}
```

Only the granted subset matters on the wire. Any permissions omitted from `result.permissions` are treated as denied. Any permissions not present in the original request are ignored by the server.

Within the same turn, granted permissions are sticky: later shell-like tool calls can automatically reuse the granted subset without reissuing a separate permission request.

If the session approval policy uses `Granular` with `request_permissions: false`, standalone `request_permissions` tool calls are auto-denied and no `item/permissions/requestApproval` prompt is sent. Inline `with_additional_permissions` command requests remain controlled by `sandbox_approval`, and any previously granted permissions remain sticky for later shell-like calls in the same turn.

### Dynamic tool calls (experimental)

`dynamicTools` on `thread/start` and the corresponding `item/tool/call` request/response flow are experimental APIs. To enable them, set `initialize.params.capabilities.experimentalApi = true`.

Each dynamic tool may set `deferLoading`. When omitted, it defaults to `false`. Set it to `true` to keep the tool registered and callable by runtime features such as `js_repl`, while excluding it from the model-facing tool list sent on ordinary turns. When `tool_search` is available, deferred dynamic tools are searchable and can be exposed by a matching search result.

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

The server also emits item lifecycle notifications around the request:

1. `item/started` with `item.type = "dynamicToolCall"`, `status = "inProgress"`, plus `tool` and `arguments`.
2. `item/tool/call` request.
3. Client response.
4. `item/completed` with `item.type = "dynamicToolCall"`, final `status`, and the returned `contentItems`/`success`.

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
The server also emits `skills/changed` notifications when watched local skill files change. Treat this as an invalidation signal and re-run `skills/list` with your current params when needed.

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

```json
{
  "method": "skills/changed",
  "params": {}
}
```

To enable or disable a skill by absolute path:

```json
{
  "method": "skills/config/write",
  "id": 26,
  "params": {
    "path": "/Users/alice/.codex/skills/skill-creator/SKILL.md",
    "name": null,
    "enabled": false
  }
}
```

To enable or disable a skill by name:

```json
{
  "method": "skills/config/write",
  "id": 27,
  "params": {
    "path": null,
    "name": "github:yeet",
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

Invoke an app by inserting `$<app-slug>` in the text input. The slug is derived from the app name and lowercased with non-alphanumeric characters replaced by `-` (for example, "Demo App" becomes `$demo-app`). Add a `mention` input item (recommended) so the server uses the exact `app://<connector-id>` path rather than guessing by name. Plugins use the same `mention` item shape, but with `plugin://<plugin-name>@<marketplace-name>` paths from `plugin/list`.

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

Codex supports these authentication modes. The current mode is surfaced in `account/updated` (`authMode`), which also includes the current ChatGPT `planType` when available, and can be inferred from `account/read`.

- **API key (`apiKey`)**: Caller supplies an OpenAI API key via `account/login/start` with `type: "apiKey"`. The API key is saved and used for API requests.
- **ChatGPT managed (`chatgpt`)** (recommended): Codex owns the ChatGPT OAuth flow and refresh tokens. Start via `account/login/start` with `type: "chatgpt"` for the browser flow or `type: "chatgptDeviceCode"` for device code; Codex persists tokens to disk and refreshes them automatically.

### API Overview

- `account/read` — fetch current account info; optionally refresh tokens.
- `account/login/start` — begin login (`apiKey`, `chatgpt`, `chatgptDeviceCode`).
- `account/login/completed` (notify) — emitted when a login attempt finishes (success or error).
- `account/login/cancel` — cancel a pending managed ChatGPT login by `loginId`.
- `account/logout` — sign out; triggers `account/updated`.
- `account/updated` (notify) — emitted whenever auth mode changes (`authMode`: `apikey`, `chatgpt`, or `null`) and includes the current ChatGPT `planType` when available.
- `account/rateLimits/read` — fetch ChatGPT rate limits; updates arrive via `account/rateLimits/updated` (notify).
- `account/rateLimits/updated` (notify) — emitted whenever a user's ChatGPT rate limits change.
- `account/sendAddCreditsNudgeEmail` — ask ChatGPT to email the workspace owner about depleted credits or a reached usage limit.
- `mcpServer/oauthLogin/completed` (notify) — emitted after a `mcpServer/oauth/login` flow finishes for a server; payload includes `{ name, success, error? }`.
- `mcpServer/startupStatus/updated` (notify) — emitted when a configured MCP server's startup status changes for a loaded thread; payload includes `{ name, status, error }` where `status` is `starting`, `ready`, `failed`, or `cancelled`.

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
   { "method": "account/updated", "params": { "authMode": "apikey", "planType": null } }
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
   { "method": "account/updated", "params": { "authMode": "chatgpt", "planType": "plus" } }
   ```

### 4) Log in with ChatGPT (device code flow)

1. Start:
   ```json
   { "method": "account/login/start", "id": 4, "params": { "type": "chatgptDeviceCode" } }
   { "id": 4, "result": { "type": "chatgptDeviceCode", "loginId": "<uuid>", "verificationUrl": "https://auth.openai.com/codex/device", "userCode": "ABCD-1234" } }
   ```
2. Show `verificationUrl` and `userCode` to the user; the frontend owns the UX.
3. Wait for notifications:
   ```json
   { "method": "account/login/completed", "params": { "loginId": "<uuid>", "success": true, "error": null } }
   { "method": "account/updated", "params": { "authMode": "chatgpt", "planType": "plus" } }
   ```

### 5) Cancel a ChatGPT login

```json
{ "method": "account/login/cancel", "id": 5, "params": { "loginId": "<uuid>" } }
{ "method": "account/login/completed", "params": { "loginId": "<uuid>", "success": false, "error": "…" } }
```

### 6) Logout

```json
{ "method": "account/logout", "id": 6 }
{ "id": 6, "result": {} }
{ "method": "account/updated", "params": { "authMode": null, "planType": null } }
```

### 7) Rate limits (ChatGPT)

```json
{ "method": "account/rateLimits/read", "id": 7 }
{ "id": 7, "result": { "rateLimits": { "primary": { "usedPercent": 25, "windowDurationMins": 15, "resetsAt": 1730947200 }, "secondary": null, "rateLimitReachedType": null } } }
{ "method": "account/rateLimits/updated", "params": { "rateLimits": { … } } }
```

Field notes:

- `usedPercent` is current usage within the OpenAI quota window.
- `windowDurationMins` is the quota window length.
- `resetsAt` is a Unix timestamp (seconds) for the next reset.
- `rateLimitReachedType` identifies the backend-classified limit state when one has been reached.

### 8) Notify a workspace owner about a limit

```json
{ "method": "account/sendAddCreditsNudgeEmail", "id": 8, "params": { "creditType": "credits" } }
{ "id": 8, "result": { "status": "sent" } }
```

Use `creditType: "credits"` when workspace credits are depleted, or `creditType: "usage_limit"` when the workspace usage limit has been reached. If the owner was already notified recently, the response status is `cooldown_active`.

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
- `askForApproval.granular` (enum-variant gate, for `approvalPolicy: { "granular": ... }`)

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

Enum variants can be gated too:

```rust
#[derive(ExperimentalApi)]
enum AskForApproval {
    #[experimental("askForApproval.granular")]
    Granular { /* ... */ },
}
```

If a stable field contains a nested type that may itself be experimental, mark
the field with `#[experimental(nested)]` so `ExperimentalApi` bubbles the nested
reason up through the containing type:

```rust
#[derive(ExperimentalApi)]
struct ProfileV2 {
    #[experimental(nested)]
    approval_policy: Option<AskForApproval>,
}
```

For server-initiated request payloads, annotate the field the same way so schema generation treats it as experimental, and make sure app-server omits that field when the client did not opt into `experimentalApi`.

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
