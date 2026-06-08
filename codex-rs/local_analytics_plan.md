# Local Analytics Sink and Standalone DuckDB Materializer

## Summary

Add a local-only analytics sink to `codex-analytics` so Codex can append its reduced analytics events into one JSONL file while continuing to POST normal analytics events to codex-backend exactly as it does today when backend analytics are enabled.

Keep the JSONL record contract in `codex-analytics`, where the reduced analytics event types already live. Add a separate `codex-analytics-materializer` crate with its own helper binary so neither `codex-analytics` nor the main `codex` binary build graph pulls in DuckDB.

First usable milestone:

```bash
cargo run -p codex-analytics-materializer -- /path/to/codex-analytics.jsonl \
  --output /path/to/codex-session-viewer.duckdb
```

The helper reduces the JSONL sink into a DuckDB artifact containing the first set of Databricks-shaped tables needed for a future Codex session viewer prototype:

- `viewer_threads_v1`
- `viewer_turns_v1`
- `viewer_turn_events_v1`
- `viewer_responses_calls_v1`
- `viewer_context_windows_v1` as an empty schema-only stub

This pass also captures local-only Responses API attempt summaries needed to populate `viewer_responses_calls_v1`. It does not change the React/FastAPI session viewer, upload artifacts to blob store, synthesize `chat.Conversation` snapshots, or add DuckDB to the main `codex` binary.

## Motivation

We want to iterate on the Unified Codex Session Viewer before the production Kafka/Flink/Databricks pipeline is available. The viewer ultimately needs Databricks-shaped tables derived from real Codex sessions, but hand-authored fixtures are too synthetic and waiting for the full pipeline would slow down the UI and data-model work.

This proposal adds a local-only analytics sink that mirrors the intended production shape: Codex emits an append-only event stream, then a manual reducer materializes queryable tables. Locally, the event stream is one JSONL file and the materialized store is DuckDB; later, the analogous production path can be Kafka/Flink/Databricks.

We intentionally use `codex-analytics` as the entry point because it already owns the normalized Codex thread, turn, tool, compaction, steering, app, plugin, hook, and review events needed for the first viewer tables. We are not building on `codex-rollout-trace`, whose rollout-scoped semantic graph is useful for a different debugging problem but does not match the intended long-lived session/thread/turn warehouse model.

This pass is intentionally narrow: generate believable local tables from Codex analytics events plus local Responses API attempt summaries. React integration, blob-backed sharing, production ingestion, and canonical `chat.Conversation` / context-window materialization are deferred.

## Goals

- Let a developer opt into a local append-only analytics dump with one env var.
- Keep the sink process-global, not session-scoped: one JSONL file may contain events for many sessions, threads, and turns handled by one Codex process.
- Preserve existing analytics behavior; the local sink is an additional best-effort side effect.
- Capture enough reduced analytics events locally to materialize believable thread, turn, and turn-event rows.
- Capture local-only Responses API attempt summaries without routing raw payloads through backend analytics.
- Materialize a local DuckDB file manually from the JSONL sink for later viewer/UI work.

## Delivery Strategy

Land the smallest useful vertical slice first:

1. Add the reduced analytics JSONL sink in `codex-analytics`.
2. Add the standalone `codex-analytics-materializer` helper binary that reads that JSONL file and writes DuckDB.
3. Add local-only Responses API call capture and populate `viewer_responses_calls_v1`.

Keep these boundaries explicit:

- Local JSONL capture is independent from backend analytics POSTing. If `analytics.enabled = false` but `CODEX_ANALYTICS_LOCAL_SINK_PATH` is set, Codex should still reduce analytics facts and append local JSONL records without POSTing them.
- The sink is process-global. Multiple `AnalyticsEventsClient` instances in one Codex process should resolve the same optional local sink handle instead of each opening their own unrelated writer.
- Local-only Responses API payloads must not be modeled as normal backend analytics facts. Use a private queue input variant for local-only Responses facts so raw request/response payloads cannot accidentally reach `send_track_events`.
- DuckDB stays out of the main `codex` binary build graph. Do not add `codex-analytics-materializer` as a dependency of `codex-cli` in v0.

## Out Of Scope

- No `codex-rollout-trace` changes.
- No new `codex-session-trace` crate.
- No React or FastAPI session viewer changes.
- No blob-store upload/download path.
- No `chat.Conversation` synthesis.
- No populated `viewer_context_windows_v1`.
- No `codex debug process-local-analytics` wiring in the main `codex` binary.
- No `--analytics-local-sink` CLI flag in v0; the env var is enough for the first slice.
- No rollout-trace-style semantic graph, interaction edges, code cells, or terminal-session modeling.
- No production Databricks/Flink pipeline work.

## User Experience

### Capture

Primary enablement:

```bash
CODEX_ANALYTICS_LOCAL_SINK_PATH=/tmp/codex-analytics.jsonl codex
```

The env var is sufficient for v0.

Behavior:

- When unset, Codex behaves exactly as today.
- When set, Codex appends one JSON object per line to the configured file.
- The file may contain all sessions launched by one Codex process with that setting.
- Pointing multiple Codex processes at the same sink path is unsupported in v0.
- The sink is local-only and may contain sensitive tool metadata, paths, errors, and Responses API payloads.
- Sink initialization and writes are best-effort; failures log a warning and must not fail the Codex session.

### Manual Materialization

Developer helper command:

```bash
cargo run -p codex-analytics-materializer -- /tmp/codex-analytics.jsonl \
  --output /tmp/codex-session-viewer.duckdb
```

Default output when `--output` is omitted:

```text
<input_stem>.duckdb
```

Example:

```bash
cargo run -p codex-analytics-materializer -- /tmp/codex-analytics.jsonl
# writes /tmp/codex-analytics.duckdb
```

Keep this as a separate helper binary in v0. Do not route it through `codex debug`, because that would pull DuckDB into the main `codex` binary build graph.

## Architecture

```text
Codex runtime facts
  -> codex-analytics::AnalyticsReducer
  -> TrackEventRequest values
     -> existing POST to codex-backend when backend analytics are enabled
     -> optional append to local JSONL sink

Responses API call lifecycle
  -> local-only Responses API start/terminal facts
  -> AnalyticsEventsQueue worker
  -> codex-analytics local reducer
  -> one optional responses_api_call JSONL record

codex-analytics-materializer helper binary
  -> codex-analytics-materializer reads JSONL
  -> reduce/group by session/thread/turn/call
  -> write materialized.duckdb
```

`codex-analytics` is the right sink entry point because it already owns the thread, turn, resolved-config, compaction, tool, app, plugin, hook, steering, review, and subagent analytics semantics that the initial viewer tables depend on. `codex-analytics-materializer` owns only the local DuckDB reduction path and depends on the generic local JSONL envelope exported by `codex-analytics`.

## Local Sink Design

### Ownership

Add a `local_sink` module under `codex-rs/analytics/src/`.

It owns:

- `CODEX_ANALYTICS_LOCAL_SINK_PATH`
- process-global optional sink initialization
- file opening and append behavior
- generic JSONL envelope schema
- best-effort write path
- serialization helpers for local-only Responses API records

The existing `AnalyticsEventsQueue` worker still owns analytics reduction and is the only caller that emits analytics-derived JSONL records. The optional sink itself is process-global and shared by every `AnalyticsEventsClient` instance in one Codex process, so final file writes are serialized by one shared local sink handle instead of depending on one session-scoped queue existing.

`AnalyticsEventsClient` should create a queue whenever either backend analytics are enabled or the local sink is enabled. The worker should tee reduced `TrackEventRequest` values to the local sink when present, then preserve existing network analytics sending only when backend analytics are enabled. The queue also accepts a private local-only Responses input variant alongside normal `AnalyticsFact` values; those local-only payloads must never enter the backend analytics send path.

`codex-analytics` exports only the generic local sink envelope needed by downstream readers:

```rust
pub struct LocalAnalyticsRecord {
    pub schema_version: u32,
    pub recorded_at_epoch_millis: u64,
    pub record_type: LocalAnalyticsRecordType,
    pub session_id: Option<String>,
    pub thread_id: Option<String>,
    pub turn_id: Option<String>,
    pub payload: serde_json::Value,
}
```

Do not export `TrackEventRequest` or the full reduced analytics event type graph. The materializer should deserialize `LocalAnalyticsRecord` and branch on `record_type`; for `codex_analytics_event` records, it should inspect the generic `payload` value by `event_type`.

### File Semantics

- Open with append semantics.
- Write one complete JSONL line per sink record.
- Flush after each line for prototype reliability.
- Do not rotate, compact, or partition in v0.
- Do not maintain per-session directories.
- Accept that the file may contain interleaved sessions and threads from one Codex process.

Writes are serialized by the process-global local sink handle. Per-queue event order should be preserved, but cross-queue total ordering inside one process is not a contract beyond the final JSONL line order. Cross-process write coordination is out of scope in v0; do not point multiple Codex processes at the same sink path.

### Record Envelope

Every JSONL line should use one envelope:

```json
{
  "schema_version": 1,
  "recorded_at_epoch_millis": 0,
  "record_type": "codex_analytics_event",
  "session_id": "session_...",
  "thread_id": "thread_...",
  "turn_id": "turn_...",
  "payload": {}
}
```

Common fields:

- `schema_version: u32`
- `recorded_at_epoch_millis: u64`
- `record_type: LocalAnalyticsRecordType`
- `session_id: string | null`
- `thread_id: string | null`
- `turn_id: string | null`
- `payload: object`

Record type contract:

- `codex_analytics_event`
- `responses_api_call`

Do not add a separate manifest record in v0. The reducer can infer covered sessions from the event stream.

## Analytics Event Capture

### Capture Point

Capture reduced analytics events after `AnalyticsReducer::ingest(...)` emits `TrackEventRequest` values and before or alongside `send_track_events(...)`.

That gives the local sink the same normalized event payloads that Codex would otherwise send to codex-backend, instead of duplicating analytics reduction logic.

### Required Refactor

`TrackEventRequest` and related event request types are currently private to `codex-analytics`. Keep them crate-internal for normal callers and serialize them directly into the generic local sink envelope inside the crate.

For each reduced analytics event, `codex-analytics` should serialize the existing `TrackEventRequest` into `serde_json::Value` and store that value as `LocalAnalyticsRecord.payload`. Add crate-internal helpers on `TrackEventRequest` or on the serialized payload to extract envelope metadata:

- `event_type`
- `session_id` when present
- `thread_id` when present
- `turn_id` when present

Avoid making the full event enum part of the public crate API. `codex-analytics-materializer` should consume the exported generic `LocalAnalyticsRecord` envelope and inspect `payload` as JSON instead of redefining Codex analytics event structs.

### Network Behavior

Preserve current network send behavior:

- local sink disabled: no behavior change
- local sink enabled and backend analytics enabled: append locally, then continue existing POST behavior
- local sink enabled and backend analytics disabled: append locally and do not POST
- local sink write failure: warn and continue existing POST behavior
- analytics POST failure: existing behavior unchanged

## Responses API Call Capture

### Why It Lives Here

Responses API calls are not Codex analytics events today, and raw request/response payloads must not be sent as normal analytics telemetry. But they are required for the local viewer-table prototype, so add local-only Responses API facts and reduce them into one local sink event inside `codex-analytics`.

Local-only facts:

```rust
struct LocalResponsesApiCallStartedFact { ... }

enum LocalResponsesApiCallTerminalFact {
    Completed { ... },
    Failed { ... },
    Cancelled { ... },
}
```

No-op-capable capture API, parallel to rollout trace's context/attempt shape:

```rust
pub struct LocalResponsesApiCallCapture { ... }
pub struct LocalResponsesApiCallAttempt { ... }

impl AnalyticsEventsClient {
    pub fn local_responses_api_call_capture(
        &self,
        session_id: String,
        thread_id: String,
        turn_id: String,
    ) -> LocalResponsesApiCallCapture;
}

impl LocalResponsesApiCallCapture {
    pub fn disabled() -> Self;
    pub fn start_attempt(
        &self,
        transport: LocalResponsesApiTransport,
        request: &impl Serialize,
    ) -> LocalResponsesApiCallAttempt;
}

impl LocalResponsesApiCallAttempt {
    pub fn record_completed(...);
    pub fn record_failed(...);
    pub fn record_cancelled(...);
}
```

`LocalResponsesApiCallCapture::disabled()` and disabled attempts are no-ops. When the local sink is enabled, `LocalResponsesApiCallCapture::start_attempt(...)` generates the stable `responses_call_id`, enqueues the start fact through `AnalyticsEventsClient`, and returns the attempt handle that owns terminal recording. Terminal methods enqueue local-only terminal facts through the same client. These facts should use a private queue input variant rather than `AnalyticsFact`, so raw local Responses payloads cannot be sent through normal analytics POSTs. Core should pass this attempt handle through the same Responses stream lifecycle that already carries `InferenceTraceAttempt`; core should not write the JSONL file directly.

### Capture Fields

Each Responses API call should have a generated local `responses_call_id` that is stable across its start and terminal facts.

Started fact:

- `responses_call_id`
- `session_id`
- `thread_id`
- `turn_id`
- `transport`: `http` or `websocket`
- `request_started_at_epoch_millis`
- `request_json`

Terminal fact:

- `responses_call_id`
- `completed_at_epoch_millis`
- `status`: `completed`, `failed`, or `cancelled`
- completed: `response_id`, `upstream_request_id`, `token_usage_json`, `output_items`
- failed/cancelled: `upstream_request_id`, `error` or `reason`, `output_items`

Local reducer output:

- one `responses_api_call` JSONL record after the reducer has both start and terminal facts
- payload contains the started metadata plus terminal status and response summary
- if the process exits after a start fact but before a terminal fact, no `responses_api_call` sink record is emitted in v0

### Capture Sites

- Build one `LocalResponsesApiCallCapture` at the existing sampling callsite next to `rollout_thread_trace.inference_trace_context(...)`, using `sess.services.analytics_events_client`, `sess.session_id()`, `sess.thread_id()`, and `turn_context.sub_id`.
- Add one `&LocalResponsesApiCallCapture` parameter next to `&InferenceTraceContext` on `ModelClientSession::stream(...)`. This is the only new capture parameter the normal sampling path should need.
- Pass `LocalResponsesApiCallCapture::disabled()` from the existing compaction `.stream(...)` callsites in v0.
- Inject attempts at the same core hook points as rollout-trace `InferenceTraceAttempt`.
- HTTP Responses path: start the local fact next to `inference_trace_attempt.record_started(&request)`.
- WebSocket Responses path: mirror rollout-trace behavior; skip warmup, record the logical request when reusing an untraced warmup response id, otherwise record the actual websocket request.
- Terminal response: pass `LocalResponsesApiCallAttempt` into `map_response_events` next to `InferenceTraceAttempt`, then emit terminal facts next to `inference_trace_attempt.record_completed`, `record_failed`, and `record_cancelled`.
- v0 records the same non-delta response summary rollout-trace already has: `response_id`, `upstream_request_id`, `token_usage`, and completed `output_items`.

If it keeps `core/src/client.rs` smaller, add one private helper there that holds `InferenceTraceAttempt` plus `LocalResponsesApiCallAttempt` and fans out `record_completed`, `record_failed`, and `record_cancelled`. Do not add a new public combined observer abstraction in v0. Do not preserve exact raw `response.completed.response` JSON in this pass; that would require a broader codex-api shape change and is not needed until `chat.Conversation` / context-window work begins.

## Reducer And DuckDB Materialization

### Ownership

Add an adjacent `codex-rs/analytics-materializer/` crate named `codex-analytics-materializer`.

Expose one public function for the helper binary and tests:

```rust
pub fn process_local_analytics(
    input: impl AsRef<Path>,
    output: impl AsRef<Path>,
) -> anyhow::Result<()>
```

This function:

- reads JSONL sequentially
- deserializes `codex_analytics::LocalAnalyticsRecord`
- validates `schema_version`
- groups records by session/thread/turn/call identifiers
- writes a fresh DuckDB file
- does not mutate the input JSONL

### Dependency

Add DuckDB only to `codex-analytics-materializer`. `codex-analytics` owns the sink and local JSONL record contract but should not depend on DuckDB. `codex-cli` must not depend on `codex-analytics-materializer` in v0; the materializer crate should expose its own standalone helper binary instead.

`codex-analytics-materializer` depends on `codex-analytics` for `LocalAnalyticsRecord` and `LocalAnalyticsRecordType`. It should not redefine `ThreadInitializedEvent`, `CodexTurnEventRequest`, or the other reduced analytics event structs; it branches on `LocalAnalyticsRecord.record_type` and, for `codex_analytics_event` records, reduces `LocalAnalyticsRecord.payload` generically by `event_type`.

If dependencies change, update Cargo/Bazel lockfiles per repo rules.

### Materialized Tables

#### `viewer_threads_v1`

Derived from `codex_analytics_event` records for thread initialization.

Columns:

```text
session_id
thread_id
root_thread_id
parent_thread_id
forked_from_thread_id
thread_source
subagent_source
initialization_mode
thread_created_at_epoch_seconds
model
product_client_id
client_name
client_version
rpc_transport
experimental_api_enabled
codex_rs_version
runtime_os
runtime_os_version
runtime_arch
ephemeral
is_root
```

`root_thread_id` is derived by walking `parent_thread_id` relationships within one session. If no parent exists, the thread is its own root.

#### `viewer_turns_v1`

Derived from `codex_analytics_event` turn records, using the latest/final reduced turn event per `(session_id, thread_id, turn_id)`.

Columns:

```text
session_id
thread_id
turn_id
turn_ordinal
parent_thread_id
thread_source
subagent_source
product_client_id
client_name
client_version
rpc_transport
experimental_api_enabled
codex_rs_version
runtime_os
runtime_os_version
runtime_arch
model_provider
service_tier
approval_policy
approvals_reviewer
sandbox_network_access
num_input_images
is_first_turn
ephemeral
initialization_mode
workspace_kind
submission_type
model
sandbox_policy
reasoning_effort
reasoning_summary
collaboration_mode
personality
status
turn_error
steer_count
total_tool_call_count
shell_command_count
file_change_count
mcp_tool_call_count
dynamic_tool_call_count
subagent_tool_call_count
web_search_count
image_generation_count
input_tokens
cached_input_tokens
output_tokens
reasoning_output_tokens
total_tokens
duration_ms
started_at_epoch_seconds
completed_at_epoch_seconds
compactions_count
compactions_completed_count
compactions_failed_count
compactions_any_error
tool_calls_count
tool_calls_total_review_count
tool_calls_guardian_review_count
tool_calls_user_review_count
tool_calls_failure_count
tool_calls_requested_network_access_count
tool_calls_requested_additional_permissions_count
tool_calls_total_duration_ms
reviews_event_count
guardian_reviews_event_count
guardian_reviews_tool_call_count
guardian_reviews_total_completion_latency_ms
guardian_reviews_total_tokens
responses_api_calls_total_count
responses_api_calls_failed_count
responses_api_calls_succeeded_count
responses_api_calls_total_latency_ms
```

`turn_ordinal` is derived per thread by start time, then `turn_id` as a stable tie-breaker.

Turns with no reduced Responses records should materialize `responses_api_calls_*` aggregates as zero values.

#### `viewer_turn_events_v1`

Derived from every local sink record with a `turn_id`.

Columns:

```text
session_id
thread_id
turn_id
event_seq
sink_line_number
recorded_at_epoch_millis
event_kind
event_type
responses_call_id
event_summary_json
analytics_event_json
```

Rules:

- `event_seq` is per `(session_id, thread_id, turn_id)` ordered by input line number.
- `event_kind` is `codex_analytics` or `responses_api`.
- Codex analytics rows store the original reduced analytics event in `analytics_event_json`.
- Responses rows store lifecycle summaries in `event_summary_json`.
- Keep this table append-only and paginated-friendly.

#### `viewer_responses_calls_v1`

Populate this table from reduced local `responses_api_call` sink records.

Columns:

```text
session_id
thread_id
turn_id
responses_call_id
call_ordinal
transport
status
request_started_at_epoch_millis
completed_at_epoch_millis
response_id
upstream_request_id
request_json
response_json
token_usage_json
error_json
```

Rules:

- `call_ordinal` is per turn ordered by request start time, then `responses_call_id`.
- `status` is `completed`, `failed`, or `cancelled`.
- `response_json` stores the v0 non-delta response summary, not the exact raw terminal Responses API object.
- Large JSON stays inline for v0; list queries later should avoid selecting those columns.

#### `viewer_context_windows_v1`

Create the table but leave it empty.

Provisional columns:

```text
session_id
thread_id
context_window_id
context_window_ordinal
first_turn_id
last_turn_id
first_responses_call_id
last_responses_call_id
opened_at_epoch_millis
closed_at_epoch_millis
close_reason
message_count
conversation_json
conversation_source
is_synthetic
```

Do not populate it in this pass.

## Helper Binary Changes

Add a standalone helper binary in `codex-rs/analytics-materializer/`:

```text
codex-analytics-materializer <input> [--output <output>]
```

Help text:

```text
Process a local Codex analytics JSONL sink into materialized viewer tables.
```

Behavior:

- Default output to `<input_stem>.duckdb`.
- Print the written output path on success.
- Return a clear error for missing input, malformed JSONL, unsupported schema version, or DuckDB write failure.
- Do not add `codex-cli` wiring in v0.

## Implementation Sequence

### Phase 1: Reduced Analytics JSONL Sink

1. Add `local_sink` module, generic `LocalAnalyticsRecord` envelope, and env-var enablement in `codex-analytics`.
2. Add process-global optional sink initialization so every `AnalyticsEventsClient` instance in one process shares one local sink handle.
3. Refactor `AnalyticsEventsClient` / `AnalyticsEventsQueue` so the queue exists when either backend analytics or the local sink are enabled, and so backend POSTing remains conditional on backend analytics enablement.
4. Tee reduced `TrackEventRequest` values to the sink by serializing the existing private event values into `LocalAnalyticsRecord.payload`.

### Phase 2: Standalone DuckDB Materializer

5. Add `codex-analytics-materializer` with DuckDB schema creation and its own helper binary.
6. Implement reductions for threads, turns, and turn events; create `viewer_responses_calls_v1` and empty `viewer_context_windows_v1` tables.
7. Add helper-binary argument parsing, default output behavior, and clear errors.
8. Add tests and run scoped formatting/tests.

### Phase 3: Responses API Capture

9. Add a private analytics queue input variant for local-only Responses API facts plus local reducer state in `codex-analytics`.
10. Build `LocalResponsesApiCallCapture` at the sampling callsite, pass it into `ModelClientSession::stream(...)`, and wire HTTP and WebSocket Responses hook points in core to start and finish `LocalResponsesApiCallAttempt` values.
11. Emit one reduced `responses_api_call` JSONL sink record per completed local fact pair.
12. Populate `viewer_responses_calls_v1` and Responses-derived turn aggregates from those reduced records.

## Testing

### `codex-analytics`

- local sink disabled: no file created, existing analytics client behavior unchanged
- local sink enabled: reduced analytics event appends one valid JSONL line
- backend analytics disabled plus local sink enabled: reduced analytics event still appends locally without requiring backend POSTing
- local sink write failure: warning path does not fail caller
- multiple events append complete JSONL lines without interleaving or truncation
- multiple `AnalyticsEventsClient` instances in one process share one local sink handle
- analytics queue worker is the only caller that emits analytics-derived JSONL records
- disabled backend analytics do not suppress local sink reduction when the sink is enabled

### Responses Follow-Up

- Responses start/terminal local facts reduce into one `responses_api_call` JSONL record
- unmatched Responses start fact does not emit a sink record in v0
- disabled `LocalResponsesApiCallCapture` and attempt handles are no-ops

### Responses Integration Follow-Up

- sampling builds local capture with session/thread/turn identity before calling `ModelClientSession::stream(...)`
- compaction passes disabled local capture in v0
- HTTP request starts local capture at the same hook as rollout-trace inference start
- WebSocket request mirrors rollout-trace warmup and untraced-warmup behavior
- terminal completed response records response id, upstream request id, token usage, and output items
- failed and cancelled requests record terminal facts with matching `responses_call_id`

### `codex-analytics-materializer`

- valid fixture JSONL writes DuckDB file
- materializer reads `codex_analytics::LocalAnalyticsRecord` without duplicate reduced event structs
- `viewer_threads_v1` contains root and child thread metadata
- `viewer_turns_v1` contains expected resolved turn row
- `viewer_turn_events_v1` preserves per-turn line order
- `viewer_responses_calls_v1` materializes reduced `responses_api_call` sink records
- `viewer_context_windows_v1` exists and is empty
- malformed JSONL reports line number
- unsupported `schema_version` fails clearly

### Helper Binary

- parser test for `codex-analytics-materializer`
- helper test writes default output path
- helper test respects explicit `--output`

## Operational Notes

- Treat the JSONL sink as sensitive local diagnostic output.
- Do not log raw local sink payloads through tracing.
- Do not send Responses request/response payloads through normal analytics POSTs.
- Do not add DuckDB or `codex-analytics-materializer` to the main `codex` binary build graph in v0.
- Do not promise stable public schema yet; mark the sink and reducer as internal/prototype.
- Prefer deterministic full rebuilds over incremental DuckDB updates in v0.

## Future Follow-Ups

- Populate `viewer_context_windows_v1` from canonical Responses API `chat.Conversation` Kafka payloads.
- Add synthetic local `chat.Conversation` generation only if needed before upstream Kafka support lands.
- Add session viewer local DuckDB drag-and-drop mode.
- Add blob upload and blob-backed artifact loading if sharing local artifacts becomes useful.
- Revisit a `codex debug process-local-analytics` wrapper only if there is a way to keep DuckDB out of the main `codex` binary build graph.
- Revisit payload sidecars only if inline JSONL becomes operationally painful.
