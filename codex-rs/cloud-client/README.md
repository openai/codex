# Codex cloud client

Reusable Rust client for native ThreadService gRPC over HTTP/2, with no TUI,
conventional app-server or private monorepo dependency. This slice supports
`Resume` and live `Attach`. It does not use the HTTP `/grpc` JSON gateway.

The caller supplies a trusted native gRPC origin (scheme/host/port, no path),
ChatGPT bearer token, selected account ID and configured `HttpClientFactory`.
HTTPS is required except for direct loopback HTTP with an unrestricted, unmanaged
factory. This exception bypasses proxies to keep plaintext credentials local.
For HTTPS, the shared HTTP client supplies proxy,
custom CA and network policy handling; redirects are rejected. Tonic supplies
gRPC framing, protobuf encoding, message bounds (64 MiB) and status/trailer handling.
Bind factory policy before loading credentials; rebuild on login/account changes.

`ResumeRequest.wait = false` acknowledges admission, not readiness. Requests are
never retried by this crate: a timeout or disconnect can leave work admitted.
`Attach` has no history snapshot or replay guarantee. Dropping it detaches without
stopping the thread or answering approvals. Streams end after their first error.
Resume and Attach setup have a 150-second local deadline; live streams have no idle
deadline. Setup does not send a gRPC deadline that would expire a healthy stream.

`Event::Transcript` decodes selected text, command output, patch, and turn updates.
It validates the thread ID and exposes the decoded fields. Turn updates include
start time, completion time, and duration when the server provides them.
Other notifications and server requests remain opaque protobuf payloads, preserving
unknown fields. These are not ProtoJSON or app-server JSON-RPC. Native status
code, message and binary details are available explicitly; credentials, event
and error diagnostics are redacted. Item lifecycle updates report supported
message, command, and file-change summaries. Agent message summaries retain
structured questions even when their text is empty. Unsupported item kinds stay opaque.
Approval replies and reconnect are subsequent slices.
The minimal envelopes in `wire.rs` and `ResumeRequest` mirror ThreadService field
numbers; no monorepo checkout or protobuf compiler is required to build this crate.

Provide `CODEX_CLOUD_ACCESS_TOKEN` and `CODEX_CLOUD_ACCOUNT_ID` through your environment, then run
this standalone consumer from `codex-rs`:

```sh
cargo run -p codex-cloud-client --example attach -- "$CODEX_CLOUD_GRPC_ENDPOINT" "$CODEX_CLOUD_THREAD_ID"
```

Use an existing authorized thread and native gRPC origin. No production URL is
assumed. Public native ingress is a rollout assumption, not proved by fixture
tests. This change does not publish a package to a registry.

History reads use native `ListTurns` (stored turn metadata) and `ListItems` (one
turn or the whole thread). Calls return one page with forward/backward opaque
cursors, preserve server order and retain raw responses and rows. Reuse a cursor
with the same account, thread and filters. Keep the sort order for `next_cursor`
and reverse it for `backwards_cursor`. Calls never paginate or retry automatically.
Items with an unknown variant or command source remain in the page with no typed summary.

History pages and Attach have no shared revision or replay cursor. A page is not
an atomic history/live boundary, and timestamps are not event offsets. Do not
append buffered deltas to a fetched snapshot: it may already include that text.
Full item snapshots can replace matching account/host/thread/turn/item identities;
they must not create duplicate transcript rows. Disconnects, untyped recovery
notifications and uncertain overlap require authoritative history reload. This
crate exposes the read primitives; a coordinated resync/display state machine is
still required before claiming lossless resume or live/history reconciliation.

`add_user_message` submits ordered text and image parts using the thread's existing
configuration. Use `UserMessageInput::Text` or `UserMessageInput::Image` with an
`ImageReference::Url` (inline data URLs only) or `ImageReference::FileId` and optional
`ImageDetail`. Image-only input is supported; local paths must be converted by the
caller before submission. URL prefixes are case-insensitive and must contain
nonempty data; HTTP image URLs are rejected locally because the backend would
omit them. File IDs must be nonblank. Requests containing images have a 3.75 MiB
encoded-envelope cap, reserving 256 KiB below CCA's 4 MiB forwarding limit. Plain
text retains the 64 MiB encoded-message limit. Text parts carry optional UTF-8
byte spans through `UserMessageTextElement`; empty span lists preserve the plain
text wire encoding. User-message summaries retain ordered content and client IDs
for both text and image echoes.

Choose `StartIfIdle`, `Steer`, or `Queue` explicitly; admission distinguishes
started, steered, duplicate and queued results. Persist a unique client message ID
with each logical input and keep it unchanged when reconciling an uncertain
submission. Success means admission, not completion. `interrupt_turn` addresses
one thread/turn and does not stop the thread or delete queued input. Duplicate
admission exposes only the original turn ID when supplied. Its wire turn status
is synthesized by the backend and does not describe current lifecycle state;
reload history or observe Attach instead of treating the receipt as an active turn.

`MutationError::InvalidRequest` means local validation rejected the request before
sending. `OutcomeUnknown` retains the source error: even a server error can follow
admission, so status codes alone do not prove rejection. Malformed success replies
and mismatched response IDs are also uncertain. No automatic retry is performed;
dropping a pending call does not undo remote work. Reconcile with server state
before deciding whether another attempt is safe. Other input kinds, configuration
overrides and automated outcome reconciliation are not implemented.

The attach example accepts two optional additional arguments to explicitly send
one message after attaching: a persisted client message ID and the text. It uses
`StartIfIdle`, drains events during admission, and prints only redacted event kinds:

```sh
cargo run -p codex-cloud-client --example attach -- "$CODEX_CLOUD_GRPC_ENDPOINT" "$CODEX_CLOUD_THREAD_ID" "$CODEX_CLOUD_MESSAGE_ID" 'Explain this project'
```

`prewarm_default_thread` prepares an ordinary coding thread in the default managed
cloud environment, with optional model/provider selection. It returns the native
thread identity and resolved settings without a separate activation RPC. The
service makes the thread discoverable when it admits the first user message.
The service owns executor selection and paths; the caller supplies no local cwd,
workspace roots, custom configuration, or Aeon mode. This API does not request an
ephemeral thread. Prewarm is sent once; a timeout, disconnect or malformed response
can leave a prepared thread whose identity the caller did not receive. No retry
or cleanup is automatic.
