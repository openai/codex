# App-server v2 tracing design

This document proposes a simple, staged tracing design for
`codex-rs/app-server` with these goals:

- support distributed tracing from client-initiated app-server work into
  app-server and `codex-core`
- keep tracing consistent across the app-server v2 surface area
- minimize tracing boilerplate in request handlers
- avoid introducing tracing-owned lifecycle state that duplicates existing
  app-server runtime state

This design explicitly avoids a `RequestKind` taxonomy and avoids
app-server-owned long-lived lifecycle span registries.

## Summary

The design has four major pieces:

1. A transport-level W3C trace carrier on inbound JSON-RPC request envelopes.
2. A centralized app-server request tracing layer that wraps every inbound
   request in the same request span.
3. An internal trace-context handoff through `codex_protocol::Submission` so
   work that continues in `codex-core` inherits the inbound app-server request
   ancestry.
4. A core-owned long-lived turn span for turn-producing operations such as
   `turn/start` and `review/start`.

Every inbound JSON-RPC request gets a standardized request span.

When an app-server request submits work into core, the current span context is
captured into `Submission.trace` when the `Submission` is created. Core then
creates a short-lived dispatch span parented from that carrier and, for
turn-producing operations, creates a long-lived turn span beneath it before
continuing into its existing task and model request tracing.

Important:

- request spans stay short-lived
- long-lived turn spans are owned by core, not app-server
- the design does not add app-server-owned long-lived thread or realtime spans

## Design goals

- **Distributed tracing first**
  - Clients should be able to send trace context to app-server.
  - App-server should preserve that trace ancestry across the async handoff into
    core.
  - Existing core model request tracing should continue to inherit from the
    active core span once the handoff occurs.

- **Consistent request instrumentation**
  - Every inbound request should produce the same request span with the same
    base attributes.
  - Request tracing should be wired at the transport boundary, not repeated in
    individual handlers.

- **Minimal boilerplate**
  - Request handlers should not manually parse carriers or build request spans.
  - Existing calls to `thread.submit(...)` and similar APIs should pick up trace
    propagation automatically because `Submission` creation captures the active
    span context.

- **Minimal business logic pollution**
  - W3C parsing, OTEL conversion, and span-parenting rules should live in
    tracing-specific modules.
  - App-server business logic should stay focused on request handling, not span
    management.

- **Incremental rollout**
  - The first rollout should prove inbound request tracing and app-server ->
    core propagation.
  - Once propagation is in place, core should add a long-lived turn span so a
    single span covers the actual duration of a turn.
  - Thread and realtime lifecycle tracing should wait until there is a concrete
    need.

## Non-goals

- This design does not attempt to make every loaded thread or realtime session
  correspond to a long-lived tracing span.
- This design does not add tracing-owned thread or realtime state stores in the
  initial design.
- This design does not require every app-server v2 `*Params` type to carry
  trace metadata.
- This design does not require outbound JSON-RPC trace propagation in the
  initial rollout.

## Why not `RequestKind`

An earlier direction considered a central `RequestKind` taxonomy such as
`Unary`, `TurnLifecycle`, or `RealtimeLifecycle`.

That is workable, but it makes tracing depend on a classification that can
drift from runtime behavior. The simpler design instead treats tracing as two
generic mechanics:

- every inbound request gets the same request span
- any async work that crosses from app-server into core gets the current span
  context attached to `Submission`

This keeps the initial implementation small and avoids turning tracing into a
taxonomy maintenance problem.

## Terminology

- **Request span**
  - A short-lived span for one inbound JSON-RPC request to app-server.

- **W3C trace context**
  - A serializable representation of distributed trace context based on
    `traceparent` and `tracestate`.

- **Submission trace handoff**
  - The optional serialized trace context attached to
    `codex_protocol::Submission` so core can restore parentage after the
    app-server request handler returns.

- **Dispatch span**
  - A short-lived core span created when the submission loop receives a
    `Submission` with trace context.

- **Turn span**
  - A long-lived core-owned span representing the actual runtime of a turn from
    turn start until completion, interruption, or failure.

## High-level tracing model

### 1. Inbound request

For every inbound JSON-RPC request:

1. parse an optional W3C trace carrier from the JSON-RPC envelope
2. create a standardized request span
3. parent that span from the incoming carrier when present
4. process the request inside that span

This is true for every request, regardless of whether the API is unary or
starts work that continues later.

### 2. Async handoff into core

Some app-server requests submit work that continues in core after the original
request returns. The critical example is `turn/start`, but the mechanism should
be generic.

To preserve trace ancestry:

- add an optional `W3cTraceContext` to `codex_protocol::Submission`
- capture the current span context into that field when constructing a
  `Submission` in core submission APIs such as `Codex::submit()` and
  `Codex::submit_with_id()`
- have `codex-core` create a per-submission dispatch span parented from that
  carrier

This gives a clean causal chain:

- client span
- app-server request span
- core dispatch span
- core turn span for turn-producing operations
- existing core spans such as `run_turn`, sampling, and model request spans

### 3. Core-owned turn spans

For turn-producing operations such as `turn/start` and `review/start`:

- app-server creates the inbound request span
- app-server propagates that request context through `Submission.trace`
- core creates a dispatch span when it receives the submission
- core then creates a long-lived turn span beneath that dispatch span
- existing core work such as `run_turn` and model request tracing runs beneath
  the turn span

This keeps long-lived span ownership with the layer that actually owns turn
execution and completion.

### 4. Defer thread and realtime lifecycle-heavy tracing

The design should not add:

- app-server-owned thread residency stores
- app-server-owned realtime session stores

App-server already maintains thread subscription and runtime state in existing
structures. If later tracing work needs thread loaded-duration or realtime
duration metrics, that data should extend those existing structures rather than
introducing a parallel tracing-only state machine.

## Span model by API shape

The initial implementation keeps the app-server side uniform.

### Unary request/response APIs

Examples:

- `thread/list`
- `thread/read`
- `model/list`
- `config/read`
- `skills/list`
- `app/list`

Behavior:

- create request span
- return response
- no additional app-server span state

### Turn-producing APIs

Examples:

- `turn/start`
- `review/start`
- `thread/compact/start` when it executes as a normal turn lifecycle

Behavior:

- create request span
- submit work under that request span
- capture the current span context into `Submission.trace` when the
  `Submission` is created
- let core create a dispatch span and then a long-lived turn span
- let the turn span remain open until the real core turn lifecycle ends

Important: request spans should not stay open until eventual streamed
completion. The request span ends quickly; the core-owned turn span carries the
long-running work.

### Other APIs that submit work into core

Examples:

- `thread/realtime/start`
- `thread/realtime/appendAudio`
- `thread/realtime/appendText`
- `thread/realtime/stop`

Behavior:

- create request span
- submit work under that request span
- capture the current span context into `Submission.trace` when the
  `Submission` is created
- let core continue tracing from there

These APIs do not automatically imply a long-lived app-server or core lifecycle
span in the initial design.

### Thread lifecycle APIs

Examples:

- `thread/start`
- `thread/resume`
- `thread/fork`
- `thread/unsubscribe`

Behavior in the initial design:

- create request span
- annotate with `thread.id` when known
- do not introduce separate app-server lifecycle spans or tracing-only state

If later work needs thread loaded/unloaded metrics, it should reuse the existing
thread runtime state already maintained by app-server.

## Where the code should live

### `codex-rs/protocol`

Add a small shared `W3cTraceContext` type to
[`codex-rs/protocol/src/protocol.rs`](/Users/owen/repos/codex3/codex-rs/protocol/src/protocol.rs).

Responsibilities:

- define a serializable W3C trace context type
- avoid direct dependence on OTEL runtime types
- be usable from both protocol crates and runtime crates

Suggested contents:

- `W3cTraceContext`
  - `traceparent: Option<String>`
  - `tracestate: Option<String>`

Suggested `Submission` change:

- `Submission { id, op, trace: Option<W3cTraceContext> }`

This is the only new internal async handoff needed for the initial rollout.

### `codex-rs/otel`

Add a small helper module or extend existing tracing helpers so OTEL-specific
logic stays centralized.

Responsibilities:

- convert `W3cTraceContext` -> OTEL `Context`
- convert the current tracing span context -> `W3cTraceContext`
- parent a tracing span from an explicit carrier when present
- keep env `TRACEPARENT` / `TRACESTATE` lookup as an explicit helper used at
  defined entrypoints, not as an implicit side effect of generic OTEL provider
  initialization for app-server
- apply precedence rules:
  - app-server inbound request spans: parent from request `trace` when present,
    else from env `TRACEPARENT` / `TRACESTATE` during migration, otherwise
    create a new root span
  - app-server submission dispatch spans: parent from `Submission.trace` when
    present, otherwise inherit naturally from the current span or create a root
    span
  - env `TRACEPARENT` / `TRACESTATE` fallback is not used for app-server
    submission dispatch spans or deeper app-server/core spans
  - env `TRACEPARENT` / `TRACESTATE` remains available for non-server
    entrypoints or process-level startup tracing

Temporary compatibility during rollout:

- Codex Cloud currently injects `TRACEPARENT` /
  `TRACESTATE` into the app-server process environment
- that env-based propagation should be treated as legacy compatibility while
  clients migrate to sending request-level `trace` carriers on every JSON-RPC
  request
- during that migration, only inbound app-server request span creation may fall
  back to env when request `trace` is absent
- `codex-otel` should not keep app-server under ambient env parentage by
  automatically attaching `TRACEPARENT` / `TRACESTATE` during provider
  initialization; app-server request tracing should opt into env fallback only
  when building the inbound request span
- once request-level carriers are in use for app-server clients, remove the
  env-based request propagation path rather than keeping both mechanisms active

Required migration rule:

- app-server submission dispatch spans and downstream spans must not consult env
  `TRACEPARENT` / `TRACESTATE` when determining span parentage
- once app-server inbound request spans or `Submission.trace` provide an
  explicit parent, downstream spans must inherit from the current span and must
  not re-parent themselves from env `TRACEPARENT` / `TRACESTATE`
- update both categories of existing env-based parenting so they do not
  override explicit request/submission ancestry:
  - lower-level callsites such as `apply_traceparent_parent` on `run_turn`
  - provider-init behavior in `codex-otel` that eagerly attaches env trace
    context for the process/thread

Important:

- keep this focused on carrier parsing and span parenting
- do not move app-server runtime state into `codex-otel`
- do not overload `OtelManager` with app-server lifecycle ownership in the
  initial design

### `codex-rs/app-server-protocol`

Extend inbound JSON-RPC request envelopes in
[`codex-rs/app-server-protocol/src/jsonrpc_lite.rs`](/Users/owen/repos/codex3/codex-rs/app-server-protocol/src/jsonrpc_lite.rs)
with a dedicated optional trace carrier field.

Suggested shape:

- `JSONRPCRequest { id, method, params, trace }`

Where:

- `trace: Option<W3cTraceContext>`

Important:

- use a dedicated tracing field, not a generic `meta` bag
- keep tracing transport-level and method-agnostic
- do not add trace fields to individual `*Params` business payloads

### `codex-rs/core`

Make small changes in the submission path in
[`codex-rs/core/src/codex.rs`](/Users/owen/repos/codex3/codex-rs/core/src/codex.rs).

Responsibilities:

- capture the current span context when a `Submission` is created in
  `Codex::submit()` / `Codex::submit_with_id()`
- read `Submission.trace`
- create a per-submission dispatch span parented from that carrier
- run existing submission handling under that span
- do not use env `TRACEPARENT` / `TRACESTATE` fallback in app-server submission
  handling or deeper core spans created from app-server work
- replace lower-level env-based re-parenting where needed so explicit
  `Submission.trace` ancestry remains authoritative

This is enough for existing core tracing to inherit the correct ancestry, and
it is the right place to add the long-lived turn span required for turn
lifecycles.

For turn-producing operations, core responsibilities should include:

- read `Submission.trace`
- create a per-submission dispatch span parented from that carrier
- create a long-lived turn span beneath the dispatch span when the operation
  actually starts a turn
- finish that turn span when the real core turn lifecycle completes,
  interrupts, or fails

### `codex-rs/app-server`

Add a small dedicated tracing module rather than spreading request tracing logic
across handlers. A likely shape is:

- `app_server_tracing/mod.rs`
- `app_server_tracing/request_spans.rs`
- `app_server_tracing/incoming.rs`

Responsibilities:

- extract incoming W3C trace carriers from JSON-RPC requests
- build standardized request spans
- provide a small API that wraps request handling in the correct span

Non-responsibilities in the initial design:

- no thread residency registry
- no realtime session registry

## Standardized request spans

Every inbound request should use the same request-span builder.

Suggested name:

- `app_server.request`

Suggested attributes:

- `rpc.system = "jsonrpc"`
- `rpc.service = "codex-app-server"`
- `rpc.method`
- `rpc.transport`
  - `stdio`
  - `websocket`
- `rpc.request_id`
- `app_server.connection_id`
- `app_server.api_version = "v2"` when applicable
- `app_server.client_name` when known from initialize
- `app_server.client_version` when known

Optional useful attributes:

- `thread.id` when already known from params
- `turn.id` when already known from params

Important:

- the span factory should be the only place that assembles these fields
- handlers should not manually construct request-span attributes
- for the `initialize` request itself, read `clientInfo.name` and
  `clientInfo.version` directly from the request params when present
- for later requests on the same connection, read client metadata from
  per-connection session state populated during `initialize`

## No app-server tracing registries

The design should not introduce app-server-owned tracing registries for turns,
threads, or realtime sessions.

Why:

- app-server already has thread subscription and runtime state
- core already owns the real task and turn lifecycle
- a second tracing-specific state machine adds more code and more ways for
  lifecycle tracking to drift

Future guidance:

- if thread loaded/unloaded metrics become important, extend existing app-server
  thread state
- keep long-lived turn spans in core
- if realtime lifecycle metrics become important, extend the existing realtime
  runtime path rather than creating a parallel tracing store

## No direct span construction in handlers

Request handlers should not call `info_span!`, `trace_span!`, `set_parent`, or
OTEL APIs directly for app-server request tracing.

Instead:

- `message_processor` should wrap inbound request handling through the
  centralized request-span helper
- `Codex::submit()` / `Codex::submit_with_id()` should capture the current span
  context into `Submission.trace` when constructing the `Submission`

That keeps request tracing transport-level and largely invisible to business
handlers.

## Layering

The intended call graph is:

- `message_processor` -> `app_server_tracing`
  - create and enter the standardized inbound request span
- `Codex::submit()` / `Codex::submit_with_id()` -> `codex-otel` trace-context
  helper
  - snapshot the current span context into `Submission.trace` when the
    `Submission` is created
- `codex-core` submission loop -> `codex-otel` trace-context helper
  - create a dispatch span parented from `Submission.trace`
  - create a long-lived turn span for turn-producing operations

Important:

- app-server owns inbound request tracing
- core owns execution after the async handoff
- core owns long-lived turn spans
- the design does not add app-server-owned long-lived thread or realtime spans

## Inbound flow in app-server

The inbound request path should work like this:

1. Parse the JSON-RPC request envelope, including `trace`.
2. Use the tracing module to create a request span parented from that request
   carrier when present, otherwise fall back to env `TRACEPARENT` /
   `TRACESTATE` during migration, otherwise create a new root request span.
3. Process the request inside that span.
4. If the request submits work into core, capture the active span context into
   `Submission.trace` when the `Submission` is created.

Integration point:

- [`codex-rs/app-server/src/message_processor.rs`](/Users/owen/repos/codex3/codex-rs/app-server/src/message_processor.rs)

## Core handoff flow

The `turn/start` and similar flows cross an async boundary:

- app-server handler submits work
- core submission loop receives `Submission`
- actual work continues later on different tasks

To preserve parentage:

1. app-server request handling runs inside `app_server.request`
2. `Codex::submit()` / `Codex::submit_with_id()` capture that active context
   into `Submission.trace` when constructing the `Submission`
3. core submission loop creates a dispatch span parented from `Submission.trace`
   without consulting env `TRACEPARENT` / `TRACESTATE`
4. if the submission starts a turn, core creates a long-lived turn span beneath
   that dispatch span
5. existing core spans naturally nest under the turn span

This lets:

- submission handling
- a single long-lived turn span for turn-producing APIs
- `run_turn`
- model client request tracing

inherit the app-server request trace without broad tracing changes across core.

## Behavior for key v2 APIs

### `thread/start`

- create request span
- annotate with `thread.id` once known
- send response and `thread/started`
- no separate thread lifecycle span in the initial design

### `thread/resume`

- create request span
- annotate with `thread.id` when known
- no separate lifecycle span

### `thread/fork`

- create request span
- annotate with the new `thread.id`
- no separate lifecycle span

### `thread/unsubscribe`

- create request span
- no separate unload span
- if later thread unload metrics are needed, reuse existing thread state rather
  than adding a tracing-only registry

### `turn/start`

- create request span
- submit work into core under that request span
- propagate the active span context through `Submission.trace` when the
  `Submission` is created
- let core create a dispatch span and then a long-lived turn span
- let that turn span cover the full duration until completion, interruption, or
  failure

### `turn/steer`

- create request span
- if the request submits core work, propagate via `Submission.trace`
- otherwise request span only

### `turn/interrupt`

- create request span
- request span only unless core submission is involved

### `review/start`

- treat like `turn/start`
- let core create the same kind of long-lived turn span

### `thread/realtime/start`, `appendAudio`, `appendText`, `stop`

- create request span
- if the API submits work into core, propagate via `Submission.trace` when the
  `Submission` is created
- do not introduce separate realtime lifecycle spans in the initial design

### Unary methods such as `thread/list`

- create request span only

## Runtime checks

Keep runtime checks narrowly scoped in the initial rollout:

- warn when an inbound trace carrier is present but invalid
- test that `Submission.trace` is set when work is submitted from a traced
  request

Do not add lifecycle consistency checks for tracing registries that do not
exist yet.

## Tests

Add tests for the initial mechanics:

- inbound request tracing accepts a valid W3C carrier
- invalid carriers are ignored cleanly
- unary methods create request spans without needing any extra handler changes
- `turn/start` propagates request ancestry through `Submission.trace` into core
- `turn/start` creates a long-lived core-owned turn span
- the turn span closes on completion, interruption, or failure
- existing core spans inherit from the propagated parent
- inbound app-server request spans prefer request `trace` and may temporarily
  fall back to env `TRACEPARENT` / `TRACESTATE` during client migration
- app-server submission and downstream spans do not use env `TRACEPARENT` /
  `TRACESTATE` fallback
- explicit transport or `Submission.trace` parents are not overwritten by
  lower-level env-based re-parenting

The goal is to verify the centralized propagation behavior, not to exhaustively
test OTEL internals.

## Suggested PR sequence

### PR 1: Foundation plus inbound request spans

Scope:

1. Introduce a shared `W3cTraceContext` type in `codex-protocol`.
2. Add `trace` to inbound JSON-RPC request envelopes in app-server protocol.
3. Add focused trace-context helpers in `codex-rs/otel`.
4. Add the centralized app-server request tracing module.
5. Wrap inbound request handling in `message_processor.rs`.

Why this PR:

- proves the transport and request-span shape with minimal scope
- gives all inbound app-server APIs consistent request tracing immediately
- avoids mixing lifecycle questions into the initial plumbing review

### PR 2: Async handoff into core via `Submission`

Scope:

1. Add `trace` to `Submission`.
2. Capture the current span context automatically when constructing
   `Submission` values in `Codex::submit()` / `Codex::submit_with_id()`.
3. Have the core submission loop restore parentage with a dispatch span.
4. Remove or gate both lower-level env-based re-parenting and provider-init
   env attachment in `codex-otel` so `Submission.trace` remains the
   authoritative parent when present.
5. Validate the flow with `turn/start`.

Why this PR:

- validates the critical async handoff from app-server into core
- proves that existing core tracing can inherit the app-server request ancestry
- keeps the behavior change focused on one boundary

### PR 3: Core-owned long-lived turn spans

Scope:

1. Add a long-lived turn span in core for `turn/start`.
2. Reuse the same turn-span pattern for `review/start`.
3. Ensure the span closes on completion, interruption, or failure.

Why this PR:

- completes the minimum useful tracing story for turn lifecycles
- keeps long-lived span ownership in the layer that actually owns the turn
- still builds on the simpler propagation model from PR 2 instead of mixing
  everything into one change

### PR 4: Use request-level trace instead of env var in Python

Scope:

1. Update the Python app-server launcher/client to send `trace` on each
   JSON-RPC request.
2. Reuse the existing upstream trace context in request envelopes during the
   initial migration so end-to-end parentage is preserved before env fallback is
   removed.
3. Continue preferring request-level `trace` over env once both exist.

Why this PR:

- preserves end-to-end parentage while the Rust side is migrating away from
  env-based request propagation
- validates the transport-level tracing design with a real client
- moves trace propagation from process scope toward request scope without
  waiting for the final cleanup

### PR 5: Remove support for `TRACEPARENT` / `TRACESTATE` when using codex app-server

Scope:

1. Stop consulting env `TRACEPARENT` / `TRACESTATE` for app-server inbound
   request span creation.
2. Remove app-server-specific env injection from launchers once request-level
   `trace` carriers are in use.
3. Keep env-based propagation only for non-server entrypoints or process-level
   startup tracing if still needed, and require those entrypoints to opt in
   explicitly rather than inheriting env parentage from generic provider init.
4. Validate that app-server requests still preserve parentage through explicit
   request carriers.

Why this PR:

- completes the migration from process-scoped to request-scoped tracing for
  app-server
- removes duplicated propagation mechanisms and future ambiguity
- aligns the implementation with the long-term design boundary

### PR 6: Optional follow-ups

Possible follow-ups:

1. Reuse existing app-server thread state to add thread loaded/unloaded duration
   metrics if needed.
2. Reuse existing realtime runtime state to add realtime duration metrics if
   needed.
3. Add outbound JSON-RPC trace propagation only if there is a concrete
   client-side tracing use case.

## Rollout guidance

Start with:

- inbound request spans for all app-server requests
- `turn/start` request -> core propagation
- a core-owned long-lived turn span for `turn/start`

Migration note:

- the Rust implementation should treat request-level `trace` carriers as the
  real app-server tracing path immediately
- env `TRACEPARENT` / `TRACESTATE` should remain only as temporary compatibility
  for existing launchers, and only at the inbound request boundary, while
  clients are updated to send `trace` on every JSON-RPC request
- after client rollout, remove the env-based app-server request propagation
  path instead of keeping it as a long-term fallback

Those pieces exercise the important mechanics:

- inbound carrier extraction
- request span creation
- async handoff into core
- inherited core tracing beneath the propagated parent
- a single span covering the full duration of a turn

After that, only add more lifecycle-specific tracing if a real debugging or
observability gap remains.

## Bottom line

The recommended initial design is:

- trace context on inbound JSON-RPC request envelopes
- one standardized request span for every inbound request
- automatic propagation through `Submission` into core
- core-owned long-lived turn spans for turn-producing APIs
- OTEL conversion and carrier logic centralized in `codex-otel`
- env `TRACEPARENT` / `TRACESTATE` kept only as temporary compatibility during
  launcher migration, not as the steady-state app-server request propagation
  mechanism
- no app-server-owned tracing registries for turns, threads, or realtime
  sessions in the initial implementation

This gives app-server distributed tracing that is:

- consistent
- low-boilerplate
- modular
- aligned with the existing ownership boundaries in app-server and core
