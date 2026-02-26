# App-server v2 tracing design

This document proposes a tracing design for `codex-rs/app-server` with these
goals:

- support true distributed tracing across client and server
- keep tracing consistent across the app-server v2 surface area
- minimize tracing boilerplate in request handlers
- minimize OTEL-specific code in business logic
- support both unary and long-lived streaming APIs cleanly

This design explicitly does **not** use a `RequestKind` classification as the
primary driver of tracing behavior. Instead, tracing behavior follows actual
runtime lifecycle events.

## Summary

The design has four major pieces:

1. A transport-level trace carrier on JSON-RPC envelopes.
2. A centralized app-server tracing layer that wraps inbound and outbound
   messages.
3. A lifecycle tracing layer for long-lived app-server operations such as
   active turns, plus lightweight thread and realtime session state with
   lifecycle events/metrics.
4. An internal trace-context handoff through `codex_protocol::Submission` so
   background work in `codex-core` runs under the correct parent span.

Every inbound JSON-RPC request gets a standardized request span.

Long-lived operations create additional spans only when the actual business
logic starts those operations. For example:

- `thread/start` creates a request span and emits thread lifecycle
  events/metrics.
- `turn/start` creates a request span and then a long-lived turn span.
- `thread/list` creates only a request span because it does not create any
  long-lived lifecycle.

Handlers do not construct OTEL spans directly. They make small calls into a
central tracing module to announce lifecycle transitions.

## Design goals

- **Distributed tracing first**
  - Clients should be able to send trace context to app-server.
  - App-server should propagate trace context on all outbound JSON-RPC
    messages.
  - Core model requests should continue propagating the active span context to
    upstream HTTP/WebSocket requests.

- **Consistent instrumentation**
  - Every request should produce a standardized request span with the same base
    attributes.
  - Every response, notification, and server-initiated request should inject
    trace context the same way.

- **Minimal boilerplate**
  - Request handlers should not repeat span construction or attribute assembly.
  - Most endpoints should need no tracing-specific code beyond a lifecycle hook
    when they create or finish a long-lived operation.

- **Minimal business logic pollution**
  - OTEL-specific conversion, carrier parsing, and propagation should live in
    dedicated tracing modules.
  - Business code should report lifecycle facts such as "thread started" or
    "turn finished", not manipulate OTEL contexts directly.

- **Good support for streaming APIs**
  - Request spans should not stay open for minutes or hours.
  - Streaming APIs should create separate long-lived lifecycle spans.

## Non-goals

- This design does not try to encode request semantics in a type-level proof.
- This design does not attempt to make the durable rollout lifetime equal to a
  tracing span lifetime across process restarts.
- This design does not require every app-server v2 `*Params` type to carry
  trace metadata.

## Why not `RequestKind`

An earlier direction considered a central `RequestKind` taxonomy such as
`Unary`, `TurnLifecycle`, or `RealtimeLifecycle`.

That is workable, but it makes tracing depend on a label that can drift from
real runtime behavior. The no-`RequestKind` design instead treats tracing as
behavior-driven:

- every request gets the same generic request span
- long-lived spans are created only when the implementation actually starts a
  lifecycle
- long-lived spans are finished only when the implementation actually observes
  lifecycle completion

This reduces the risk of "the spec says unary but the method now streams" and
avoids turning tracing into a taxonomy maintenance problem.

We can still keep an inventory of traced APIs in docs or tests, but that
inventory should not be the primary source of runtime behavior.

## Terminology

- **Request span**
  - A short-lived span for one inbound JSON-RPC request.

- **Thread residency tracker**
  - Lightweight app-server state keyed by `thread_id` that records facts such
    as load time so we can emit thread lifecycle events and metrics without
    keeping an unbounded thread-level span open for days.

- **Turn span**
  - A long-lived span representing an active turn that starts from
  `turn/start` or an equivalent API and ends at `turn/completed` or abort.

- **Realtime session tracker**
  - Lightweight app-server state keyed by session identity that records facts
    such as start time so we can emit realtime lifecycle events and metrics
    without keeping an unbounded realtime-session span open.

- **Trace carrier**
  - A serializable representation of distributed trace context, based on
    `traceparent` and `tracestate`.

## High-level tracing model

### 1. Inbound request

For every inbound JSON-RPC request:

1. parse an optional trace carrier from the envelope
2. create a request span with standardized attributes
3. parent that request span from the incoming carrier when present
4. process the request inside that request span

This is true for every request, regardless of whether it is unary or
streaming.

### 2. Outbound messages

For every outbound JSON-RPC message:

- response
- notification
- server-initiated request
- error

inject a trace carrier into the envelope from `Span::current()`.

This keeps propagation centralized and consistent.

### 3. Long-lived lifecycle spans

Create long-lived spans only when runtime behavior warrants them. Do not create
unbounded thread-level or realtime-session spans just because something remains
open in memory.

Examples:

- `thread/start`, `thread/resume`, `thread/fork`
  - record thread residency and emit thread lifecycle events/metrics
- `turn/start`, `review/start`, `thread/compact/start` if it runs as a turn
  - create turn spans
- `thread/realtime/start`
  - record realtime session state and emit realtime lifecycle events/metrics

Turn spans are stored by stable IDs and can be re-entered later when sending
notifications or handling completion. Thread and realtime lifecycle use
lightweight session-state trackers rather than long-lived spans.

### 4. Internal async handoff

App-server often starts work that continues on background tasks after the
original request handler returns. The critical example is `turn/start`, which
submits `Op::UserInput` into the core session loop and then returns
immediately.

To preserve trace ancestry:

- add an optional trace carrier to `codex_protocol::Submission`
- populate it when app-server submits work to core
- have `codex-core` create a per-submission dispatch span parented from that
  carrier

This lets existing core spans naturally nest beneath the right app-server turn
or request span without scattering tracing logic throughout core tasks.

## Span model by API shape

The runtime behavior determines which spans exist.

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
- no additional long-lived spans

### Thread lifecycle APIs

Examples:

- `thread/start`
- `thread/resume`
- `thread/fork`
- `thread/unsubscribe` when it unloads a thread

Behavior:

- create request span
- annotate the request span with `thread.id` when known
- emit thread lifecycle events and metrics on load/unload transitions
- update lightweight thread residency state if needed for unload-duration
  metrics

Thread residency is useful as metadata and metrics, but not as a long-lived
tracing span. A thread may remain loaded for minutes, hours, or days, and that
is not a single causal operation.

### Turn lifecycle APIs

Examples:

- `turn/start`
- `review/start`
- `thread/compact/start` when it runs as a normal turn lifecycle

Behavior:

- create request span
- on success, create turn span
- keep the turn span alive until turn completion or abort

Important: request spans should not remain open until `turn/completed`.
Long-running work belongs on the turn span, not the request span.

### Turn control APIs

Examples:

- `turn/steer`
- `turn/interrupt`

Behavior:

- create request span
- link or add events to the active turn span if present
- do not create a new turn span

### Realtime lifecycle APIs

Examples:

- `thread/realtime/start`
- `thread/realtime/appendAudio`
- `thread/realtime/appendText`
- `thread/realtime/stop`

Behavior:

- `start` creates a request span and records realtime session state
- append/stop requests create request spans and use `thread.id` / `session.id`
  attributes for correlation
- completion emits realtime lifecycle events/metrics and clears session state

## Where the code should live

### New crate: trace carrier

Add a small shared crate, tentatively `codex-trace-context`.

Responsibilities:

- define a serializable trace carrier type
- avoid direct dependence on OTEL types
- be usable from protocol crates and runtime crates

Suggested contents:

- `TraceCarrier`
  - `traceparent: Option<String>`
  - `tracestate: Option<String>`
- helper methods for validation that do not depend on OTEL runtime types

Reason:

- `codex-app-server-protocol` needs a serializable envelope type
- `codex-protocol` needs the same type on `Submission`
- `codex-otel` should own OTEL conversion logic, not the plain data type

### `codex-rs/otel`

Add a small helper module, tentatively `trace_context.rs`.

Responsibilities:

- convert `TraceCarrier` -> OTEL `Context`
- convert `Span::current()` -> `TraceCarrier`
- parent a tracing span from an explicit carrier
- centralize precedence rules:
  - explicit carrier from transport
  - fallback to env `TRACEPARENT` / `TRACESTATE`
  - otherwise root span

This keeps OTEL-specific conversion in one place.

### `codex-rs/app-server-protocol`

Extend JSON-RPC envelopes in
[`codex-rs/app-server-protocol/src/jsonrpc_lite.rs`](/Users/owen/repos/codex3/codex-rs/app-server-protocol/src/jsonrpc_lite.rs)
with an optional `meta` field.

Suggested shape:

- `JSONRPCRequest { id, method, params, meta }`
- `JSONRPCNotification { method, params, meta }`
- `JSONRPCResponse { id, result, meta }`
- `JSONRPCError { error, id, meta }`

Where:

- `meta.trace: Option<TraceCarrier>`

Important:

- trace metadata belongs on the JSON-RPC envelope, not inside business payloads
- this keeps tracing transport-level and method-agnostic

### `codex-rs/protocol`

Extend `Submission` in
[`codex-rs/protocol/src/protocol.rs`](/Users/owen/repos/codex3/codex-rs/protocol/src/protocol.rs)
with an optional trace carrier.

Suggested shape:

- `Submission { id, op, trace: Option<TraceCarrier> }`

This is the async trace handoff between app-server and core.

### `codex-rs/core`

Make a small change in the submission dispatch path in
[`codex-rs/core/src/codex.rs`](/Users/owen/repos/codex3/codex-rs/core/src/codex.rs).

Responsibilities:

- read `Submission.trace`
- create a per-submission dispatch span
- parent that span from the carrier
- run existing op handling under that span

This is enough for existing core tracing to inherit the correct ancestry.
Core business logic should not need broad tracing changes.

### `codex-rs/app-server`

Add a dedicated tracing module rather than spreading logic across existing
handlers. A likely shape is:

- `app_server_tracing/mod.rs`
- `app_server_tracing/request_spans.rs`
- `app_server_tracing/registry.rs`
- `app_server_tracing/incoming.rs`
- `app_server_tracing/outgoing.rs`

Responsibilities:

- extract incoming trace carriers
- build standardized request spans
- maintain turn span registries and lightweight thread/realtime session state
- inject outgoing trace carriers
- expose small lifecycle APIs for handlers

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

## Lifecycle registries and lightweight session state

The tracing layer should own registries for true long-lived spans and a small
amount of lightweight thread/realtime session state.

Suggested structures:

- `TurnSpanRegistry`
  - keyed by `turn_id`
- `ThreadResidencyTracker`
  - keyed by `ThreadId`
  - stores load timestamp and any minimal metadata needed to emit unload
    duration metrics or consistency warnings
- `RealtimeSessionTracker`
  - keyed by `(thread_id, session_id)` or the best available stable runtime key
  - stores session start timestamp and any minimal metadata needed to emit
    close duration metrics or consistency warnings

Responsibilities:

- create true long-lived spans where warranted
- store turn spans by ID
- re-enter turn spans later when notifications are emitted
- end or drop turn spans on lifecycle completion
- record thread and realtime session state without keeping unbounded spans open

Suggested span names:

- `app_server.turn`

Suggested span attributes:

- `thread.id`
- `turn.id`
- `session.id`
- `app_server.connection_id` when relevant
- `rpc.method` of the starting request

Suggested thread lifecycle events/metrics:

- `app_server.thread.loaded`
- `app_server.thread.unloaded`
- `app_server.thread.loaded_duration_ms`

Suggested realtime lifecycle events/metrics:

- `app_server.realtime.started`
- `app_server.realtime.closed`
- `app_server.realtime.duration_ms`

## No direct span construction in handlers

Request handlers should not call `info_span!`, `trace_span!`, `set_parent`, or
OTEL APIs directly for app-server lifecycle tracing.

Instead, handlers should call small tracing APIs such as:

- `tracing_state.on_thread_loaded(...)`
- `tracing_state.on_thread_unloaded(...)`
- `tracing_state.on_turn_started(...)`
- `tracing_state.on_turn_finished(...)`
- `tracing_state.on_realtime_session_started(...)`
- `tracing_state.on_realtime_session_closed(...)`

Those calls express business facts without embedding OTEL mechanics in handler
logic.

## Inbound flow in app-server

The inbound request path should work like this:

1. Parse the JSON-RPC request envelope, including `meta.trace`.
2. Use the tracing module to create a request span.
3. Process the request inside that span.
4. If the request starts a long-lived lifecycle, call the appropriate
   lifecycle hook.
5. When submitting work to `codex-core`, attach a trace carrier to
   `Submission`.

Integration point:

- [`codex-rs/app-server/src/message_processor.rs`](/Users/owen/repos/codex3/codex-rs/app-server/src/message_processor.rs)

## Outbound flow in app-server

The outbound message path should work like this:

1. When constructing a response, notification, server request, or error,
   capture `Span::current()`.
2. Convert the current span context into a `TraceCarrier`.
3. Attach it to `meta.trace` on the outgoing envelope.
4. Send the envelope through the existing outbound routing.

Integration point:

- [`codex-rs/app-server/src/outgoing_message.rs`](/Users/owen/repos/codex3/codex-rs/app-server/src/outgoing_message.rs)

Important:

- injection should happen when the outgoing envelope is created, not later in
  the transport writer task
- this preserves the correct context from the current handler or event scope

## Core handoff flow

The `turn/start` and similar flows cross an async boundary:

- request handler submits `Op::UserInput`
- core session loop receives `Submission`
- actual work continues later on different tasks

To preserve parentage:

1. app-server creates a turn span
2. app-server attaches that span context to `Submission.trace`
3. core submission loop creates a dispatch span parented from
   `Submission.trace`
4. existing core spans naturally nest under it

This lets:

- `run_turn`
- sampling spans
- model client request tracing

inherit the app-server turn trace without broad tracing changes across core.

## Behavior for key v2 APIs

### `thread/start`

- create request span
- on successful thread creation:
  - annotate request span with `thread.id`
  - record thread residency
  - emit thread lifecycle event/metric
- send response and `thread/started` with injected trace metadata

### `thread/resume`

- create request span
- on successful resume:
  - annotate request span with `thread.id`
  - record thread residency if not already tracked
  - emit thread lifecycle event/metric when appropriate
- no long-lived thread span

### `thread/fork`

- create request span
- on successful fork:
  - annotate request span with the new `thread.id`
  - record thread residency for the new thread
  - emit thread lifecycle event/metric

### `thread/unsubscribe`

- create request span
- when the last subscriber causes unload:
  - use thread residency state to emit unload event/metric and loaded duration
    when `thread/closed` or equivalent unload completion occurs

### `turn/start`

- create request span
- create turn span on successful start
- propagate turn span context through `Submission`
- emit later streamed notifications under the turn span
- finish turn span on `turn/completed` or abort

### `turn/steer`

- create request span
- if active turn exists, optionally add an event or link against the turn span
- do not create a new turn span

### `turn/interrupt`

- create request span
- add an event or link against the active turn span
- let the existing turn span end on abort/completion

### `review/start`

- treat like `turn/start`
- do not introduce a separate tracing architecture for review turns

### `thread/realtime/start`

- create request span
- annotate request span with `thread.id` and `session.id` when known
- record realtime session state on success
- emit realtime lifecycle event/metric

### `thread/realtime/appendAudio` and `appendText`

- create request span
- annotate request span with `thread.id` and `session.id`
- do not create new long-lived spans

### `thread/realtime/stop`

- create request span
- emit realtime lifecycle event/metric and clear session state when the
  realtime lifecycle actually ends

### Unary methods such as `thread/list`

- create request span only
- no lifecycle registry interaction

## Safeguards without `RequestKind`

Without a central `RequestKind` taxonomy, we still want safeguards so tracing
drift is visible.

### Runtime checks

Add warnings or `debug_assert!`s for cases like:

- a completion notification arrives for an unknown turn span
- a thread unloads but there is no tracked residency state for it
- a realtime close/error arrives but there is no tracked session state for it

These are strong signals that lifecycle hooks are missing or out of order.

### Tests

Add tests for the nontrivial lifecycle APIs:

- `thread/start` records thread lifecycle state and emits thread lifecycle
  events/metrics correctly
- `turn/start` creates turn span, propagates context, and closes on completion
- `review/start` reuses turn lifecycle tracing
- realtime session start/stop uses the realtime session tracker and emits
  lifecycle events/metrics correctly
- unary methods emit request spans but no lifecycle spans

The goal is not to test OTEL internals exhaustively, but to verify that the
central tracing layer sees the expected lifecycle facts.

## Suggested PR sequence

### PR 1: Foundation plus `thread/start` vertical slice

Scope:

1. Introduce a shared `TraceCarrier` crate.
2. Add `meta.trace` to JSON-RPC envelopes in app-server protocol.
3. Add `trace` to `Submission`.
4. Add OTEL conversion helpers in `codex-rs/otel`.
5. Add the centralized app-server tracing module:
   - request span builder
   - outgoing injector
   - lifecycle registries
6. Wire inbound request spans in `message_processor.rs`.
7. Wire outbound trace injection in `outgoing_message.rs`.
8. Wire `thread/start` lifecycle tracing.

Why this PR:

- shows the complete architecture in one reviewable vertical slice
- makes the protocol and tracing infrastructure easier to evaluate because
  there is a concrete API using them end to end
- validates distributed trace propagation, request spans, outbound injection,
  and thread lifecycle events/metrics without yet taking on the async
  complexity of turns

### PR 2: `turn/start` proof of concept

Scope:

1. Wire `turn/start` lifecycle tracing.
2. Use `Submission` trace propagation into `codex-core`.

Why this PR:

- validates the critical async handoff from app-server into core
- proves the design works for long-lived streamed APIs that complete much later
- exercises turn span creation, streamed notification scoping, and completion
  handling

### PR 3: Roll out to other long-lived v2 APIs

Scope:

1. Extend lifecycle tracing to `review/start`.
2. Extend lifecycle tracing to realtime APIs.
3. Extend the same centralized machinery to the rest of the v2 surface.

Why this PR:

- applies the already-proven infrastructure to the remaining long-lived APIs
- keeps the initial proof-of-concept focused before broadening coverage
- lets unary APIs inherit request tracing automatically while long-lived APIs
  opt into the appropriate lifecycle hooks

## Rollout guidance

Start with:

- `thread/start`
- `turn/start`

Those two endpoints exercise all of the important mechanics:

- inbound carrier extraction
- request span creation
- thread lifecycle events/metrics and turn lifecycle span creation
- async handoff into core
- outbound response/notification injection
- streamed completion

Once those are stable, apply the same centralized machinery to:

- `thread/resume`
- `thread/fork`
- `review/start`
- realtime APIs
- remaining unary endpoints

## Open questions

- Whether client->server and server->client JSON-RPC envelopes should expose
  `meta` as a general extensibility bag or only `meta.trace`.
- Whether to use only parent-child relationships or also OTEL links for
  control APIs such as `turn/interrupt` and `turn/steer`.
- Whether thread residency tracking should also record subscriber-count or
  connection churn as lightweight lifecycle events/metrics.
- Whether some long-lived asynchronous utility APIs such as `command/exec`
  should get their own lifecycle registries if they stream for meaningful
  durations.

## Bottom line

The recommended design is:

- trace context on JSON-RPC envelopes
- one standard request span for every inbound request
- centralized outgoing trace injection for every outbound message
- long-lived spans created from actual runtime lifecycle events
- internal propagation through `Submission` into core
- lifecycle hooks expressed in business terms, not OTEL terms

This gives app-server distributed tracing that is:

- consistent
- low-boilerplate
- unobtrusive in business logic
- suitable for both unary and streaming v2 APIs
