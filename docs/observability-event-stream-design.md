# Observability Event Stream

Codex currently has three overlapping observability systems:

- `codex-rs/analytics` reduces runtime facts into remote product analytics.
- `codex-rs/rollout-trace` records rich local diagnostic traces and reduces
  them into a replay/debug graph.
- `codex-rs/otel` emits logs, trace-safe events, and metrics, while native
  `tracing` spans carry execution scope and W3C trace context.

The overlap is real: thread lifecycle, turns, model calls, tools, compaction,
subagents, transport, and feature usage are described more than once. Directly
merging the crates would couple incompatible outputs. The shared layer should
instead define the facts Codex emits; each destination should decide how those
facts are filtered, reduced, stored, or exported.

## Decision

Introduce a shared observation stream for semantic facts.

```text
Observations own semantic facts and metrics derived from facts.
tracing owns spans and trace context propagation.
OTEL is a consumer of observations, not a second event taxonomy.
```

This gives one rule for callers:

- Use observations when a thing happened.
- Use sinks/reducers for analytics, rollout trace, OTEL events, OTEL metrics,
  local buffers, and feedback upload.
- Use `tracing::span!` when code needs active execution scope, parent/child
  causality, span mutation, or W3C context propagation.

The target state should not be a long-lived mix where some facts use
observations and other facts call analytics or OTEL directly. Direct OTEL code
should remain for exporter setup, native spans, context propagation, and
low-level tracing plumbing.

## Goals

- Define canonical observation events that describe what Codex did, not where
  the data is going.
- Keep analytics, rollout trace, OTEL logs, and OTEL metrics as projections.
- Apply filtering at field level so a single observation can contain both safe
  aggregate fields and richer local-only evidence.
- Preserve existing analytics and OTEL output behavior while moving callsites
  toward the shared stream.
- Support local rich retention for feedback/debug upload with explicit policy.
- Prove analytics equivalence with side-by-side conformance tests.

## Non-Goals

- Do not make analytics facts the source of truth for rollout trace.
- Do not make rollout trace raw events the source of truth for analytics.
- Do not encode downstream event names such as `codex_turn_event` or
  `codex.tool_result` into the canonical taxonomy.
- Do not replace native spans with `SpanStarted` / `SpanEnded` observations.
- Do not require full rollout trace graph equivalence before the first version;
  rollout trace is experimental and can use lighter validation initially.

## Shape

```text
codex-core / app-server
  emit observations

codex-observability
  observation traits, field metadata, sink policy, event definitions

codex-observability-derive
  derive macro for field metadata and visitors

sinks/reducers
  analytics projection -> TrackEventRequest
  rollout trace projection -> local trace bundle + reduced graph
  OTEL event projection -> current log and trace-safe event names
  OTEL metric projection -> current counters and histograms
  ringbuffer -> recent rich local observations

native tracing spans
  remain direct tracing instrumentation
```

The derive crate exists because Rust does not expose struct field attributes at
runtime. The public API can still re-export the derive from
`codex-observability` so most callsites only import one crate.

## Why Not Just OTEL?

An alternative is to emit semantic facts as ordinary `tracing` events and let a
local subscriber, OTEL exporter, analytics reducer, or trace reducer all consume
the same event stream. That is attractive because Codex already uses
`tracing`, and local JSON event capture is easy to wire up.

We should still avoid making OTEL/tracing events the canonical schema:

- `tracing` fields are stringly typed at the reducer boundary. Renames or type
  changes become runtime reducer failures instead of Rust compile errors.
- OTEL trace-safe export has a different privacy posture than local diagnostic
  traces. Rich local fields such as tool output, prompts, model payloads, and
  terminal output should not share a target with remotely exported span events.
- Field-level policy needs metadata that `tracing` does not preserve in a
  structured way. We need to know both detail level and data class before a
  sink serializes a field.
- Analytics conformance is easier from typed observations than from flattened
  JSON logs, because reducers can match on Rust event types and fields.
- Spans and semantic facts have different lifecycles. Spans model active scope
  and context propagation; observations model facts that happened.

OTEL should therefore consume observations for semantic events and derived
metrics, while native `tracing` remains the right tool for spans, context
propagation, and exporter plumbing. A local trace sink can still write JSONL
bundles; it should write observation envelopes rather than raw
`tracing_subscriber` event JSON.

## Observation API

The shared schema API can be small:

```rust
pub trait Observation {
    const NAME: &'static str;

    fn visit_fields<V: ObservationFieldVisitor>(&self, visitor: &mut V);
}

pub trait ObservationFieldVisitor {
    fn field<T: serde::Serialize + ?Sized>(
        &mut self,
        name: &'static str,
        meta: FieldMeta,
        value: &T,
    );
}

pub trait ObservationSink {
    fn observe<E: Observation>(&self, event: &E);
}
```

Example:

```rust
use codex_observability::Observation;

#[derive(Observation)]
#[observation(name = "turn.started", uses = ["analytics"])]
struct TurnStarted<'a> {
    #[obs(level = "basic", class = "identifier")]
    thread_id: &'a str,

    #[obs(level = "basic", class = "identifier")]
    turn_id: &'a str,

    #[obs(level = "basic", class = "operational")]
    started_at: i64,
}
```

The derive only exposes metadata. It should not implement analytics mapping,
trace persistence, redaction, schema generation, or export behavior.

## Callsite API

Product/runtime code should usually call semantic helper methods rather than
inline-constructing large observation structs. Direct struct construction is
fine in tests, very small leaf events, or helper internals, but it should not be
the normal instrumentation style.

The helper layer should do the tedious, failure-prone work near the callsite:
extract common IDs, join session/thread/turn context, compute durations,
normalize status and error fields, avoid trace-only allocations when no sink
can read them, and then emit the typed observation.

Example shape:

```rust
observability.turn().config_resolved(&session, &turn_context, &input).await;
observability.tool().ended(&invocation, status, &response);
observability.transport().api_request_completed(&request, &outcome);
```

This mirrors the useful part of the existing systems:

- rollout trace has `RolloutTraceRecorder::record_*` helpers
- analytics has `AnalyticsEventsClient::track_*` helpers
- OTEL has `SessionTelemetry::record_*` helpers

The helper layer must not become a second taxonomy. It is ergonomic glue over
canonical observation structs. Prefer small domain-specific helper groups such
as turn, tool, inference, transport, compaction, and feature usage over one
large recorder with every method in the system.

## Rich Payloads

Start simple. Ordinary observation fields should be eager values or borrowed
references. Sinks should reject fields by metadata before serializing them, so
analytics and OTEL metrics do not inspect trace/content fields.

Some trace fields point at large objects that usually already exist at the
callsite: model requests, model responses, tool invocations, tool results, or
terminal output. Prefer passing borrowed serializable views of those objects to
the observation helper rather than building `serde_json::Value` eagerly.

Do not introduce a generalized lazy-field system in the first implementation.
Closures such as `|| build_trace_payload(...)` are useful when constructing the
trace view itself is expensive, but they add API complexity. Treat them as a
later optimization for measured hot spots, not as the default observation
style.

## Field Policy

Filtering must be field-level. Event-level labels such as basic/detailed/trace
are too coarse because one event often contains both remote-safe counters and
local-only content.

Each field should carry at least:

- **Use markers**: exact projections intended to consume the field, for example
  `analytics`, `otel`, or `rollout_trace`.
- **Detail level**: `basic`, `detailed`, or `trace`.
- **Data class**: `identifier`, `operational`, `environment`, `content`, or
  `secret_risk`.

Use markers express intent. Detail level and data class are guardrails. A sink
must first select only fields explicitly marked for that sink, then enforce its
detail/class policy before serializing values.
Event structs may define default use markers when most fields feed the same
projection; field-level use markers override that default for mixed events.

Detail level is not privacy by itself. A tiny field can still be unsafe for
remote export, and a trace-level field can be trace-level because it is large
rather than sensitive. Data class is also not enough for selection: analytics
must not consume every basic operational field just because it would be safe.

Expected sink policies:

- Analytics selects only fields marked `analytics`, then allows basic
  identifiers/operational fields plus selected environment fields; it denies
  content and secret-risk fields even if they are marked accidentally.
- Rollout trace allows rich local fields, with explicit redaction rules for
  secret-risk material.
- OTEL logs preserve today's log export behavior, including account/email only
  where today's policy allows them.
- OTEL trace-safe events prefer lengths, counts, status, timing, and coarse
  categories over content.
- OTEL metrics select exact metric dimensions and then apply the OTEL policy.
- Feedback upload applies an explicit user-approved policy over the ringbuffer.

## Event Taxonomy

The taxonomy should be designed from Codex workflows, not from existing
destination schemas. Names should describe facts that occurred in the system:
`turn.ended`, `tool_call.ended`, `transport.api_request_completed`.

Rules:

- Prefer stable domain facts over analytics, OTEL, or trace storage names.
- Model lifecycle facts explicitly when downstream reducers need ordering or
  duration.
- Keep event count small until a consumer needs a distinct fact.
- Put richness in fields, not in parallel event variants.
- Add transport/runtime facts when they are first-class telemetry today, even
  if analytics ignores them.
- Keep an escape hatch only for local experimental tracing, not remote export.

Initial workflow coverage should be chosen by conformance need:

| Workflow | Canonical examples | Primary consumers |
| --- | --- | --- |
| Session/thread | `session.config_resolved`, `thread.started`, `thread.ended` | analytics, OTEL, rollout |
| Turn lifecycle | `turn.requested`, `turn.started`, `turn.ended` | analytics, OTEL, rollout |
| Turn timing | `turn.first_token_observed`, `turn.first_message_observed` | OTEL metrics, rollout |
| Model I/O | `inference.started`, `inference.sse_event_observed`, `inference.completed`, `inference.failed` | OTEL, rollout |
| Tools | `tool_call.started`, `tool_call.approval_resolved`, `tool_call.ended` | OTEL, rollout |
| Compaction | `compaction.started`, `compaction.installed`, `compaction.ended` | analytics, rollout |
| Agents | `agent.task_sent`, `agent.message_sent`, `agent.result_delivered`, `agent.closed` | rollout, analytics subset |
| Product features | `app.mentioned`, `app.used`, `hook.run_completed`, `plugin.used`, `plugin.state_changed`, `guardian.review_completed` | analytics |
| Transport/auth | `transport.api_request_completed`, `transport.websocket_request_completed`, `auth.recovery_step_completed` | OTEL |

This table is not a complete schema. Each event still needs a typed Rust
definition with field annotations before implementation.

## Projections

### Analytics

Analytics becomes a reducer over observations. Existing product events remain
output schemas.

| Observations | Analytics output |
| --- | --- |
| `thread.started` | `codex_thread_initialized` |
| `turn.started` with resolved config, `turn.ended` with token usage | `codex_turn_event` |
| `turn.steer_resolved` | `codex_turn_steer_event` |
| compaction lifecycle observations | `codex_compaction_event` |
| skill/app/plugin/guardian observations | existing feature events |

The analytics crate should keep the observation-to-legacy-schema translation in
a private projection module. The reducer then stays responsible for ingestion
orchestration, batching, deduplication, connection metadata joins, and product
event naming rather than becoming a mixed bag of mapping code.

### Rollout Trace

Rollout trace consumes observations and decides which fields become trace
payloads. Trace storage details such as `RawPayloadRef` should remain a trace
implementation detail, not part of the shared event definitions.

Example: `inference.completed` can include response material as a trace-level
field. The trace sink may write it to `payloads/*.json`; analytics and OTEL
metrics never read it.

### OTEL

OTEL consumes observations for every semantic fact and derived metric. The
projection preserves existing exported names while removing separate callsite
event definitions.

| Observations | OTEL output |
| --- | --- |
| session/thread observations | `codex.conversation_starts`, `codex.thread.started` |
| `turn.input_received` | `codex.user_prompt` |
| `tool_call.approval_resolved` | `codex.tool_decision` |
| `tool_call.ended` | `codex.tool_result`, tool count/duration metrics |
| transport observations | API/websocket events and count/duration metrics |
| `auth.recovery_step_completed` | `codex.auth_recovery` |
| inference/SSE observations | `codex.sse_event`, SSE duration, response timing metrics |
| turn timing/token observations | TTFT, TTFM, E2E duration, token usage metrics |

Log-only and trace-safe OTEL variants can be emitted from the same observation
by applying different field policies.

## Conformance Testing

The highest-value equivalence target is analytics because it is existing
product behavior.

```text
same E2E Codex run
  legacy analytics facts -> current AnalyticsReducer -> TrackEventRequest JSON
  observations -> analytics observation reducer -> TrackEventRequest JSON

assert exact equality after stable JSON normalization
```

At least one conformance path should apply exact analytics use markers plus the
analytics guardrail policy, so missing markers and unsafe annotations fail
tests. Recommended first scenarios:

- thread start
- normal turn
- failed or interrupted turn
- accepted and rejected turn steering
- compaction
- skill, app, plugin, and guardian events
- subagent thread start

Rollout trace can start with observation-to-trace unit tests plus one or two
end-to-end smoke tests that write and reduce a bundle. Full graph equivalence
is not necessary for the initial experimental rollout feature.

OTEL should get projection tests that compare exported event names, fields,
metric names, and tags for representative observations. Native span tests
should remain in the tracing layer.

## Implementation Stages

Build the proof of life on a fresh `main` branch, but keep commits separable so
the working branch can later be split into reviewable PRs.
Each stage should leave a small, documented API surface and a passing local
verification gate. Non-essential behavior should stay as a TODO until a later
stage needs it.

1. **Observation foundation**
   - Add `codex-observability` and `codex-observability-derive`.
   - Verify with crate-local tests that derived observations expose field
     metadata and reject missing annotations at compile time.
   - Run: `cargo test -p codex-observability`.

2. **Analytics conformance slice**
   - Add the first typed observations and an analytics projection for one
     existing analytics flow.
   - Verify legacy analytics facts and observation-derived facts produce
     identical `TrackEventRequest` JSON after stable normalization.
   - Run: targeted `cargo test -p codex-analytics`.

3. **E2E analytics shadowing**
   - Capture legacy facts and observations from the same core/app-server test
     run for selected scenarios.
   - Verify exact analytics output equality for thread start, normal turn,
     failed/interrupted turn, steering, compaction, and one feature event.
   - Run the targeted core/app-server integration tests added for each
     scenario.

4. **Rollout trace projection**
   - Add a rollout trace sink that consumes the same observations for
     thread/turn, inference, and tool lifecycle.
   - Verify with observation-to-trace unit tests and one smoke test that writes
     and reduces a local bundle.

5. **OTEL projection proof**
   - Add a small OTEL projection for a high-value event such as tool result.
   - Verify exported event names, field policy, metric names, and tags match
     the current behavior.

## Migration

Start with a shadow observation stream and conformance tests. Once analytics
equivalence is reliable, move callsites from direct analytics facts to
observations.

Rollout trace can adopt observations directly because it is experimental.

OTEL should be migrated by category rather than left permanently mixed:
tool/user/turn events, transport events, auth events, and derived metrics can
move behind the OTEL projection. Native spans and trace context stay where they
are.

## Open Questions

- What is the smallest useful field-class set?
- Should timestamps and sequence numbers live in the observation envelope or be
  supplied by sinks?
- Where should fanout live so callsites do not grow `codex-core` further?
- How should borrowed trace-level payloads avoid hot-path allocations while
  still allowing sinks to serialize when policy permits?
- Which direct OTEL counters are first-class observations, and which are
  metrics derived from broader observations?
