# Model catalog provider requirements

`model/list` and periodic model catalog refreshes check the startup provider against
current managed provider requirements before using the catalog. If that provider no longer
complies, `model/list` returns JSON-RPC error `-32600` asking the client to restart Codex,
and background refreshes skip the old endpoint. Requirement load failures also block these
operations. Checks apply even when the catalog is cached. Existing startup provider selection
and caching behavior remain in effect while the provider satisfies current requirements.

# MCP App UI

`mcpToolCall.mcpAppUi` records the invoked descriptor's `resourceUri`
and `preferredModelDisplayMode` (`inline` or `fullscreen`). Descriptors with a widget
URI default to `inline` when the preference is missing or unsupported. The
UI information is preserved in tool-call events and saved history so clients can
render without waiting for the full MCP catalog.

The field is null for older history and tools that declare widgets only in
result metadata; clients retain catalog discovery for those calls. Existing
resource URI fields remain available for older clients.

# Initial Daybreak choice (experimental)

Persistent threads accept `daybreakEnabled` on `thread/start` with the
`experimentalApi` opt-in. The response and `thread/started` notification both
include the initial choice in `thread.daybreakEnabled`. The choice is staged
with the thread's other initial metadata and saved when the thread is persisted.
An unused thread is not guaranteed to survive restart. Omitted or null leaves
the choice unset. Ephemeral threads cannot save it.
Use `thread/metadata/update` for later changes. This preference does not select
`turn/start.cyberAccessProgram` or grant access to an access program.

# User verification cancellation (experimental)

Local UI clients can cancel a native user-verification RPC by sending
`userVerification/cancel` with `{requestId}` and the `experimentalApi` opt-in.
The result is an empty acknowledgment (`{}`). This API does not enable desktop
verification capability advertisement.

`requestId` is the original status, enroll, delete, or verify RPC's string or
integer ID on the same connection, not the server elicitation ID. Use fresh IDs
for each operation and a distinct ID for the cancel RPC. Unknown, finished,
unrelated, and other-connection requests are no-ops.

The acknowledgment confirms the cancellation signal without waiting for the OS
prompt to close. The original RPC completes independently, with
`cancelled/interrupted` when cancellation prevents completion. Cancellation
cannot roll back completed effects. It remains effective while a proof waits for
outbound queue capacity, but cannot retract a response already enqueued.

Canceling or resolving an elicitation does not itself stop a separate
`userVerification/verify` RPC. Clients must cancel that RPC separately and discard
late proofs after the approval is canceled or resolved. Only one native worker
runs per app-server; if an OS call remains active after cancellation or timeout,
subsequent local operations return `failed/providerError` until that worker exits.

# Hosted Codex Apps MCP protocol

The host-owned HTTP `codex_apps` server uses Legacy by default in app-server and
standalone Codex. To discover the 2026-07-28 protocol, set
`codex_apps_mcp_2026_07_28 = true` under `[features]`, or send a true runtime
override via `experimentalFeature/enablement/set`. Discovery falls back to Legacy
when the server does not support it. Explicit config takes precedence.
The dedicated setting does not apply to third-party HTTP or local `codex_app`
stdio servers. The existing `mcp_2026_07_28` flag still governs eligible other
servers, regardless of whether their names or URLs resemble hosted Apps.
App-server does not persist this selection.

# Project trust

`thread/start` does not persist project trust for a directory where configuration
discovery finds no project-root marker, Git checkout, or project-local `.codex`
directory. Starting a task there does not preapprove project configuration added
later. Existing trust decisions and permission checks for projects are unchanged.

# Thread removal

`thread/archive` and `thread/delete` reject attempts to remove a live internal
worker with JSON-RPC error `-32600`. The worker's owner controls its shutdown.
For example, a Guardian reviewer remains available to its parent conversation
after a client tries to archive or delete it.

After the owner releases the worker, its saved conversation can be archived or
deleted normally. Ordinary client-controlled threads keep their existing behavior.

## User verification (experimental)

Codex app-server advertises `openai/elicitation.userVerification` to the
host-owned plugin service for bundled, in-process TUI sessions (`codex-tui`) and
local stdio desktop sessions (`Codex Desktop`) on devices with supported biometric
hardware and the `experimentalApi` opt-in. This is an app-server decision,
independent of whether a key exists; TUI/Desktop/mobile do not advertise this MCP
capability. Mobile integration requires a separate rollout. Other clients and
network connections do not receive this mode, even with a recognized client name.
Before sending verification requests to desktop sessions, deploy a GUI that
handles the typed verification request, cancellation, and late proofs. The general
`experimentalApi` opt-in does not identify a compatible GUI version.

Native `openai/userVerification` elicitation requests preserve optional `_meta`
JSON through MCP transport and `mcpServer/elicitation/request`. Clients may use
this metadata for extension-specific presentation and must continue to accept
requests without it. Metadata does not change the challenge bytes or the proof
returned in the acceptance response.

Local UI clients use five methods. They require the existing
`experimentalApi` opt-in. The local provider reports
`unavailable/providerUnavailable` on unsupported platforms or without the required
ChatGPT account identity.

| Method | Params | Result |
| --- | --- | --- |
| `userVerification/status` | `{}` | `{credentialId, unavailableReason, unavailableMessage}` |
| `userVerification/enroll` | `{}` | `{credentialId, algorithm?, publicKey?}` |
| `userVerification/delete` | `{}` | `{}` |
| `userVerification/verify` | `{challenge, title, description}` | `{proof: {credentialId, signature}}` |
| `userVerification/cancel` | `{requestId}` | `{}` |

Status reads local readiness without prompting or contacting a backend. A null
`unavailableReason` means local checks passed, not that registration is valid.
Unsupported platforms and missing account identity are reported in the status
response's `unavailableReason` field.
Enrollment creates or reuses the local key and returns its public metadata. The
`publicKey` is unpadded base64url SPKI-DER; `algorithm` is `ecdsaP256Sha256X962`.
During the experimental rollout, `algorithm` and `publicKey` are optional for
compatibility with older app-servers. Current servers populate both fields;
callers must check that both are present and non-null before backend registration.
The trusted UI host owns backend registration: obtain an enrollment challenge,
sign it with `userVerification/verify`, check that the proof's `credentialId`
matches this response, and submit the public metadata and proof to the backend.
Local success is not server enrollment. The caller must preserve the authenticated
account across this flow and reconcile uncertain registration before retrying.
Deletion removes the local key; the caller owns backend revocation.
Enrollment and deletion coordinate credential lifecycle; callers do not issue
separate generate or rotate commands. Identity comes from the authenticated
account; this API exposes no caller-selected scope.

Verify signs 1–4096 decoded challenge bytes using P-256 ECDSA with SHA-256. The
challenge and DER signature use unpadded base64url. Title is 1–256 UTF-8 bytes;
description is at most 4096 bytes. The UI obtains approval for that display
context before calling. Verify does not require a pending elicitation; a UI with
its own authenticator can return proof directly in elicitation response content.
The calling flow owns pending-request checks and discards late proofs.
Native enroll, delete, and verify accept local stdio and in-process connections.
WebSocket and remote-control peers must use their own device authenticator;
status remains available for local readiness. Dropping an embedded RPC, disconnecting,
or changing authentication cancels its native operation. Responses recheck the
captured identity after waiting for outbound queue capacity.
Canceling or resolving an elicitation does not itself stop a separate
`userVerification/verify` RPC. The GUI must use `userVerification/cancel` to
cancel that RPC and discard late proofs when an approval is canceled or resolved.
See [User verification cancellation](#user-verification-cancellation-experimental)
for request ID and acknowledgment semantics.
Only one native worker runs per app-server. If an OS call remains active after
cancellation or timeout, subsequent local operations return `failed/providerError`
until that worker exits.

Failures use the normal JSON-RPC error envelope with closed `{type, reason}` data:
`invalidRequest`, `unavailable`, `cancelled`, or `failed`. UI clients branch on
these values rather than message text. Native diagnostic payloads stay private.

## Local rollout compression

The experimental `rollout/compress` method takes no parameters and immediately
returns `{}` after scheduling one best-effort background pass over the app-server's
local rollout storage. It does not change `features.local_thread_store_compression`
or require that startup flag to be enabled. Non-local thread stores do not support
this method.

The worker retains its existing cold-file checks, maintenance and writer locks,
concurrency limit, and cooldown. Acknowledgement does not imply completion or that
any files were compressed; failures are reported through existing logs and metrics.
There are no progress notifications or cancellation API. Clients sharing this
Codex home must support compressed rollout files, including shared histories.

## Managed model provider requirements

Existing threads retain their provider configuration. Input RPCs reject requests when managed
`model_provider` or `model_providers` requirements no longer match that configuration, or cannot
be loaded. This covers turn start/steer, review, compaction, manual queue start, and active goal
updates. Realtime connections use separate routing configuration and are not checked here.
Interrupt, realtime stop, and goal pause/clear remain available. User and project
configuration changes alone do not invalidate existing threads.

# Amazon Bedrock authentication

If `model_providers.amazon-bedrock.aws.credential_export` is configured, Bedrock setup and
Bedrock login return an error without changing configuration or saved credentials. Remove the
exporter configuration before selecting another credential source. `aws.credential_export` and
`aws.profile` cannot be configured together.

## Stored thread attachments

- `thread/attachment/add` — add a durable resource reference to a stored thread without loading it. Repeated writes with the same attachment type and identity key return the existing attachment.
- `thread/attachment/list` — list attachments for one stored thread in a cursor-paginated request, including a thread that is not loaded.
- `thread/attachment/remove` — remove an attachment by its thread, attachment type, and identity key; returns `{}`.
- `thread/attachment/updated` — notification broadcast after an attachment is created or removed; contains the thread, attachment identity, attachment id, and operation.
### Example: Manage stored thread attachments

Attachments record the resources currently associated with a thread, independently of conversation history. Clients can add, remove, and list attachments for one stored thread at a time without resuming those threads. Adding or removing an attachment does not create or delete the underlying resource or rewrite history. An attachment is idempotently identified by its thread, `attachmentType`, and `identityKey`. For pull requests, clients should reuse the canonical application identity `JSON.stringify([canonicalHostname, lowercaseOwner, lowercaseRepository, pullRequestNumber])` so addition and removal agree across surfaces.

```json
{ "method": "thread/attachment/add", "id": 20, "params": {
    "threadId": "thr_123",
    "attachmentType": "pull_request",
    "identityKey": "[\"github.com\",\"openai\",\"codex\",123]",
    "payload": { "url": "https://github.com/openai/codex/pull/123" }
} }
{ "id": 20, "result": {
    "outcome": "created",
    "attachment": {
        "id": "01984de2-8f74-7c91-a3b2-5c5e937cf318",
        "attachmentType": "pull_request",
        "identityKey": "[\"github.com\",\"openai\",\"codex\",123]",
        "payload": { "url": "https://github.com/openai/codex/pull/123" },
        "createdAt": 1750000000
    }
} }

{ "method": "thread/attachment/list", "id": 21, "params": {
    "threadId": "thr_123",
    "limit": 100
} }
{ "id": 21, "result": {
    "data": [{
        "id": "01984de2-8f74-7c91-a3b2-5c5e937cf318",
        "attachmentType": "pull_request",
        "identityKey": "[\"github.com\",\"openai\",\"codex\",123]",
        "payload": { "url": "https://github.com/openai/codex/pull/123" },
        "createdAt": 1750000000
    }],
    "nextCursor": null
} }

{ "method": "thread/attachment/remove", "id": 22, "params": {
    "threadId": "thr_123",
    "attachmentType": "pull_request",
    "identityKey": "[\"github.com\",\"openai\",\"codex\",123]"
} }
{ "id": 22, "result": {} }

{ "method": "thread/attachment/updated", "params": {
    "threadId": "thr_123",
    "attachmentType": "pull_request",
    "identityKey": "[\"github.com\",\"openai\",\"codex\",123]",
    "attachmentId": "01984de2-8f74-7c91-a3b2-5c5e937cf318",
    "operation": "deleted"
} }
```

`thread/attachment/list` accepts one `threadId` and returns at most 100 attachments per page, ordered by creation time and attachment id. Continue with `nextCursor` and the same `threadId` until the cursor is `null`. Each thread can retain up to 100 attachments. Removing an attachment frees a slot for a new attachment.

A non-ephemeral fork copies the source thread's current attachments, even when forking at an earlier turn. The copies have new attachment IDs and creation timestamps, but retain the same resource identities and payloads. Clients use `forkedFromId` on `thread/started` to detect forks and call `thread/attachment/list` with the new thread ID to load their attachments. Fork copying does not emit per-attachment updates; explicit add/remove operations still do. Copying is awaited before publishing the fork, but is best effort: a copy failure is logged and the conversation fork succeeds without attachments. Membership can then change independently on either thread; the referenced resources themselves are not copied. Resuming a fork does not repeat the copy.

Attachment creation and deletion requests using the same thread ID are serialized across connections. The requesting client receives its response before the compact update is broadcast, and duplicate creates or absent deletes do not emit updates. Deleting the owning thread removes its attachments under the same lifecycle exclusion; queued attachment mutations then report that the thread was not found.

# Thread plugin settings

`thread/settings/update` and `turn/start` accept `disabledPluginIds`, a list of
`PluginSummary.id` values from `plugin/list`, in the
`<plugin-name>@<marketplace-name>` format. A supplied list replaces the selection;
omission or `null` preserves it, and `[]` clears it. Saving this selection does
not yet filter plugin capabilities.

Read the selection from `threadSettings.disabledPluginIds` in
`thread/settings/updated` notifications, or from `disabledPluginIds` in
`thread/start`, `thread/resume`, and `thread/fork` responses. Selections persist
across resume. Forks restore the selection from the history retained at the
requested fork boundary.

# Deprecated thread personality setting

`thread/start`, `thread/resume`, `thread/settings/update`, and `turn/start` still
accept `personality`, but `friendly` and `pragmatic` no longer select a style.
`model/list` returns `supportsPersonality: false` for every model.

`none` removes the literal `# Personality` section when Codex prepares
instructions from the model catalog, for example when starting a thread or
switching models. Setting `friendly` or `pragmatic` can replace a previous
`none` setting for that purpose. Changing the setting does not rewrite the
thread's existing instructions or change explicitly supplied base instructions.
The old `features.personality` flag is ignored.

# MCP server capabilities

`mcpServerStatus/list` returns `serverCapabilities` for each initialized MCP server
in both `full` and `toolsAndAuthOnly` detail modes, including thread-scoped reads.
This is the server's advertised MCP capabilities object, including its `extensions`
map. It is null when the connection has not initialized successfully; capabilities
are never inferred from tools or copied from a shared catalog cache.

# Thread rollback

`thread/rollback` has been removed from the API, including its request and response
types. Requests use the generic unknown-method rejection path. Use `thread/revert`
for paginated threads instead.

Existing rollouts may contain historical `ThreadRolledBack` events. Their replay
and migration remain supported so resuming, reading, and forking those threads
preserves the surviving history. This disk compatibility does not require restoring
support for new `thread/rollback` requests.

# Selected workspace routing

The experimental `account/read.workspaceRouting` response field returns the selected ChatGPT workspace's `chatgptAccountId`, resolved HTTPS `backendOrigin`, and backend-provided `accountRoutingOverride`. The routing value is `us`, `us_cr`, or the explicit `NO_CONSTRAINT` value. API-only and signed-out accounts return `null` and do not need `accounts/check`.

App-server discovers routing for saved ChatGPT logins at startup and for new logins or workspace switches. After requirements and routing are ready, it sends the existing `account/updated` notification. Newly initialized connections also receive this notification once saved-workspace routing is ready, including when discovery finished before the connection initialized. Clients then reread `configRequirements/read` and `account/read`. Saved ChatGPT credentials without a selected workspace ID retain their account information and return `workspaceRouting: null`; app-server does not guess a workspace from the backend's default account. Discovery failures for a selected workspace, including missing or null fields from older backends, return an `account/read` error. They never produce a successful unrestricted result. A later read retries failed discovery. Logout clears the cached routing, and results from earlier authentication owners are discarded. Token refreshes for the same known user and workspace invalidate cached routing without cancelling discovery or failing sign-in. Configuration is reloaded after discovery; a changed backend, model provider, or required backend rejects the result so the next read discovers against current configuration. Account notifications recheck the auth owner generation after waiting for outbound queue capacity. Superseded sign-in attempts emit a failed `account/login/completed` event instead of silently dropping completion. Notifications remain snapshots: clients reread current account and requirements state rather than treating a queued notification as authorization.

Routing compares origins by scheme, host, and effective port, ignoring API paths. A required
`chatgpt_base_url` must match discovery; if neither provides an origin, `NO_CONSTRAINT` uses the
configured base URL.

Responses HTTP (including compaction) and WebSockets wait for discovery and preserve API paths.
Guardian v2 classifier HTTP and pooled WebSockets use the same routing.
HTTP redirects are rejected. `us` and `us_cr` set `X-OpenAI-Account-Routing-Override`;
`NO_CONSTRAINT` omits it.

API-key and explicitly external-auth providers bypass discovery. Custom ChatGPT-auth destinations
require discovery before being treated as independent. Changing a workspace-bound thread's
bootstrap origin requires a new thread.

## Windows sandbox implementation selection

`windowsSandbox/setupStart` applies only to the legacy `elevated` and
`unelevated` backends. `windowsSandbox/readiness` reports `ready` when MXC is
selected so clients do not offer legacy setup. The
`allowedWindowsSandboxImplementations` requirement governs only the legacy
backends and does not restrict MXC. Its `mxc` enum member is retained for wire
compatibility but is not emitted. Non-Windows hosts report `notConfigured`.

MXC uses the standard `command/exec` streaming and process-control path, including
ConPTY when `tty` is enabled. The buffered legacy Windows sandbox restrictions on
process control and custom output caps do not apply to MXC.

### Gateway OAuth sign-in

Providers configured with `gateway_oauth` require a secondary OAuth credential in
addition to their primary authentication. Clients with a gateway sign-in UI set
`initialize.capabilities.explicitGatewayOauth: true`, complete initialization, and
successfully call `account/gatewayOAuth/read` before sending authenticated requests,
including startup `model/list` and inference requests. Repeat this probe on each
new connection. A successful `initialize` alone does not confirm support: older
servers can ignore the unknown capability and retain automatic browser login.

Support for `account/gatewayOAuth/read` and `explicitGatewayOauth` is introduced
together, so a successful read confirms support even when `required` is `false`
or `status` is `notReady`. The returned status determines whether sign-in is needed;
it is separate from the capability check. If the probe fails because the method is
unsupported, require a server upgrade. Other errors and timeouts also leave
authenticated requests blocked until a probe succeeds; do not silently fall back
to automatic login.

With explicit login enabled, app-server refreshes existing credentials, but only the
`account/gatewayOAuth/login` RPC starts browser authorization. Requests needing
sign-in fail promptly so the client can offer that flow.

Clients that omit the capability or set it to `false`, including the TUI, retain
automatic browser authorization after initialization. Startup credential reads
cannot open a browser before initialization. Explicit opt-in is shared by gateway
managers using the same home and network configuration within the process and
cannot be undone by a later connection that omits the capability.

- `account/gatewayOAuth/read` returns the current effective `providerId`,
  `providerName`, `required`, `status`, and `error`. `required` indicates that this
  provider uses gateway OAuth, including when already signed in. This operation
  does not refresh tokens or open a browser; `status` is null for other providers.
  `notReady` means credentials are not ready. `succeeded` means saved credentials
  are locally usable, not that a gateway request has been verified. Reads observe
  usable replacement credentials saved by another process sharing the same home.
- `account/gatewayOAuth/login` starts authorization and returns `{}` after
  the credential has been saved. Providers requiring OpenAI authentication need a
  primary account first. A second login request fails while a login is active.
  The initiating connection receives a `started` notification with `authUrl`; the
  client must open that URL in a browser that can reach the server callback port.
  Other status notifications set `authUrl` to null. Login is rejected if the
  initiating connection opted out of `account/gatewayOAuth/changed` notifications.
- `account/gatewayOAuth/cancel` cancels the calling connection's login and returns
  `{}` after the active login releases its slot, so the client can immediately
  start another login. Closing that connection also cancels its login and releases
  the callback listener. Cancellation makes the pending login request fail.
- `account/gatewayOAuth/changed` reports `notReady`, `started`, `succeeded`, or
  `failed`, with an optional `error`. Notifications apply to the current effective
  gateway configuration. Clients can read readiness when connecting and after
  changing configuration. Notifications follow the standard per-connection
  `optOutNotificationMethods` setting.
  These payloads never contain credentials.

Read, login, and cancel take no params. Read and login use the app's current
provider, reloading configuration and returning an error if it cannot be loaded.
Status notifications include `providerId`. During browser sign-in, inference
requests fail promptly and can be retried when sign-in succeeds.

`model/list` also checks gateway authentication before returning cached models.
If authentication fails after the provider configuration changes, it asks the client
to restart Codex so the retained catalog and gateway sign-in use the same provider.
