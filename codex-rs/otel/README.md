# codex-otel

`codex-otel` is the OpenTelemetry integration crate for Codex. It provides:

- Provider wiring for log/trace/metric exporters (`codex_otel::OtelProvider`
  and `codex_otel::provider`).
- Session-scoped business event emission via `codex_otel::SessionTelemetry`.
- Low-level metrics APIs via `codex_otel::metrics`.
- Trace-context helpers via `codex_otel::trace_context` and crate-root re-exports.

## Tracing and logs

Create an OTEL provider from `OtelSettings`. The provider also configures
metrics (when enabled), then attach its layers to your `tracing_subscriber`
registry:

```rust
use codex_otel::config::OtelExporter;
use codex_otel::config::OtelHttpProtocol;
use codex_otel::config::OtelSettings;
use codex_otel::OtelProvider;
use tracing_subscriber::prelude::*;

let settings = OtelSettings {
    environment: "dev".to_string(),
    service_name: "codex-cli".to_string(),
    service_version: env!("CARGO_PKG_VERSION").to_string(),
    codex_home: std::path::PathBuf::from("/tmp"),
    exporter: OtelExporter::OtlpHttp {
        endpoint: "https://otlp.example.com".to_string(),
        headers: std::collections::HashMap::new(),
        protocol: OtelHttpProtocol::Binary,
        tls: None,
    },
    trace_exporter: OtelExporter::OtlpHttp {
        endpoint: "https://otlp.example.com".to_string(),
        headers: std::collections::HashMap::new(),
        protocol: OtelHttpProtocol::Binary,
        tls: None,
    },
    metrics_exporter: OtelExporter::None,
    span_attributes: std::collections::BTreeMap::new(),
    tracestate: std::collections::BTreeMap::new(),
};

if let Some(provider) = OtelProvider::try_new(&settings)? {
    let registry = tracing_subscriber::registry()
        .with(provider.logger_layer())
        .with(provider.tracing_layer());
    registry.init();
}
```

Configured span attributes and W3C tracestate member fields are applied to
exported trace spans and propagated trace context:

```toml
[otel.span_attributes]
"example.trace_attr" = "enabled"

[otel.tracestate.example]
alpha = "one"
beta = "two"
```

Configured tracestate members and encoded values must be valid W3C tracestate.
Each nested table is encoded as semicolon-separated `key:value` fields inside
that member. If propagated trace context already has the named member, Codex
upserts configured fields and preserves other fields in that member. This
config shape does not support setting opaque tracestate member values. Invalid
trace metadata entries are ignored during config load and reported as startup
warnings.

## SessionTelemetry (events)

`SessionTelemetry` adds consistent metadata to tracing events and helps record
Codex-specific session events. Rich session/business events should go through
`SessionTelemetry`; subsystem-owned audit events can stay with the owning subsystem.

```rust
use codex_otel::SessionTelemetry;

let manager = SessionTelemetry::new(
    conversation_id,
    model,
    slug,
    account_id,
    account_email,
    auth_mode,
    originator,
    log_user_prompts,
    terminal_type,
    session_source,
);

manager.user_prompt(&prompt_items);
```

### Agent response logs

Set `otel.log_agent_responses = true` with an OTLP HTTP or gRPC log exporter to
emit `codex.agent_response` for completed assistant messages explicitly marked
`final_answer`. The default is false, independent of `otel.log_user_prompt`;
disabled response logging emits no event. Main agents and spawned task agents
are included. Internal/review/compaction/memory agents, commentary, untagged
messages, streaming deltas, tool-generated async messages, and history replay
are excluded. Plans are preserved and memory-citation markup is removed from
the exported copy without changing the stored answer.

Opting in exports potentially sensitive response text, including subagent work
absent from the main answer, to your log destination. Text is sent only to logs,
capped at 65,536 UTF-8 bytes at a character boundary, without a truncation notice.
`response_length` is the original byte count after citation removal, and
`response_truncated` indicates whether the cap removed text.
The log-only target is excluded from trace export, local state logs, and the
feedback log buffer.

Events include standard session attributes, `agent.type` (`main` or `subagent`),
`turn.id`, and `item.id`. `conversation.id` identifies the emitting thread.
Available lineage includes `parent.conversation.id`, `parent.turn.id`,
`root.turn.id`, and `initiating.agent.path`. The parent conversation is the
structural owner; the parent turn is the trigger and may belong to another
agent. They are not a guaranteed matching pair or a supported Compliance API join.
Completion is logged even if the turn later fails. Duplicate item completions
are suppressed within a turn; delivery retains the existing best-effort exporter
behavior, without an exactly-once guarantee.

`AgentResponseLogger` owns filtering, text preparation, and per-turn deduplication.
Core supplies completed raw items and step attribution, retaining the logger in
the existing turn extension data; it does not own response-logging policy.

### Guardian assessment logs

Set `otel.log_guardian_assessments = true` with an OTLP HTTP or gRPC log exporter
to emit `codex.guardian_assessment` once a synchronous Guardian review finishes.
The default is false. Events include the reviewed conversation, turn, review and
target item IDs, status, risk level, user authorization and rationale. `outcome`
is `allow` or `deny` for a completed model assessment and absent for review errors,
timeouts and cancellations, including fail-closed denials. Low-risk approvals
may contain the reviewer's generic fallback explanation.

Rationales can contain sensitive content. The exported copy is capped at 65,536
UTF-8 bytes at a character boundary; `rationale_length` records the original byte
count and `rationale_truncated` indicates truncation. The log-only target is
excluded from trace export, local state logs and the feedback log buffer. This
does not add content to the main model's conversation or grant it log read access.
Cached V2 approvals do not create a synchronous assessment and are not included.
Delivery uses the existing best-effort OTLP exporter.

## Metrics (OTLP or in-memory)

Modes:

- OTLP: exports metrics via the OpenTelemetry OTLP exporter (HTTP or gRPC).
- In-memory: records via `opentelemetry_sdk::metrics::InMemoryMetricExporter` for tests/assertions; call `shutdown()` to flush.

`codex-otel` also provides `OtelExporter::Statsig`, a shorthand for exporting OTLP/HTTP JSON metrics
to Statsig using Codex-internal defaults.

Statsig ingestion (OTLP/HTTP JSON) example:

```rust
use codex_otel::config::{OtelExporter, OtelHttpProtocol};

let metrics = MetricsClient::new(MetricsConfig::otlp(
    "dev",
    "codex-cli",
    env!("CARGO_PKG_VERSION"),
    OtelExporter::OtlpHttp {
        endpoint: "https://api.statsig.com/otlp".to_string(),
        headers: std::collections::HashMap::from([(
            "statsig-api-key".to_string(),
            std::env::var("STATSIG_SERVER_SDK_SECRET")?,
        )]),
        protocol: OtelHttpProtocol::Json,
        tls: None,
    },
))?;

metrics.counter("codex.session_started", 1, &[("source", "tui")])?;
metrics.histogram("codex.request_latency", 83, &[("route", "chat")])?;
```

In-memory (tests):

```rust
let exporter = InMemoryMetricExporter::default();
let metrics = MetricsClient::new(MetricsConfig::in_memory(
    "test",
    "codex-cli",
    env!("CARGO_PKG_VERSION"),
    exporter.clone(),
))?;
metrics.counter("codex.turns", 1, &[("model", "gpt-5.1")])?;
metrics.shutdown()?; // flushes in-memory exporter
```

## WebSocket continuation

`codex.websocket.continuation` counts `response.create` send attempts, including
failed sends. It carries existing session tags plus `mode` (`incremental`/`full`),
`phase` (`warmup`/`generation`), and `reason`:

| Reason | Meaning |
| --- | --- |
| `incremental` | Send the previous response ID and new input. |
| `no_previous_request` | First request from a fresh client. |
| `restored_history` | First request after loading resumed or forked history. |
| `connection_closed` | Full input after observing the previous socket closed. |
| `other` | Full input for another reason, such as changed input/settings or unavailable response state. |

Per-socket backend metrics label a resend after reconnect as `initial`; this client
metric retains the first reset reason through reconnect failures and turn boundaries.
Include warmups (`generate=false`), which can send full input before an incremental
generation. Closes may be intentional; restored history includes manual resumes/forks.
This measures client send attempts, not disconnect rates or engine cache reuse.

## Trace context

Trace propagation helpers remain separate from the session event emitter:

```rust
use codex_otel::current_span_w3c_trace_context;
use codex_otel::set_parent_from_w3c_trace_context;
```

## Shutdown

- `OtelProvider::shutdown()` stops the OTEL exporter.
- `SessionTelemetry::shutdown_metrics()` flushes and shuts down the metrics provider.

Both are optional because drop performs best-effort shutdown, but calling them
explicitly gives deterministic flushing (or a shutdown error if flushing does
not complete in time).
