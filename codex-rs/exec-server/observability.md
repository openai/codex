# Exec-server observability proposal

Status: Proposed

## Motivation

`codex exec-server` runs inside execution environments where the surrounding
services can be observed, but the work performed by the exec-server process is
largely opaque. It accepts connections, attaches resumable sessions, spawns
and manages commands, serves filesystem operations, proxies HTTP requests, and
in remote mode maintains a relay connection. Failures in any of these paths
currently have little context beyond the caller-visible error.

The useful Kubernetes analogy is a node agent such as the kubelet: exec-server
is not the workload itself, but a long-running component that receives control
requests and operates workloads within its environment. Kubernetes components
provide component logs, configurable metrics and traces, and health endpoints
without treating workload output as component telemetry. Exec-server should
follow the same boundary.

## Goals

- Make standalone local and remote exec-server instances observable through the
  existing Codex OpenTelemetry and `tracing` infrastructure.
- Provide enough lifecycle context to distinguish connectivity, protocol,
  session, subprocess, filesystem, and HTTP proxy failures.
- Keep stdout reserved for transport behavior and emit component logs only to
  stderr or configured exporters.
- Avoid recording user content, secrets, or unbounded-cardinality data as
  telemetry.
- Require explicit operator opt-in before exporting metrics from a
  containerized exec-server instance.

## Non-goals

- Capturing child-process stdout or stderr as application telemetry.
- Logging commands, environment variables, filesystem contents, HTTP bodies,
  authorization data, or full URLs.
- Adding a separate telemetry backend or exec-server-specific configuration
  format.
- Changing the JSON-RPC or relay protocols for the initial increment.

## Current state

The exec-server crate already emits a small number of `tracing` events for
transport, session, and error conditions. The websocket listener also exposes
`/readyz`. However, the standalone `codex exec-server` CLI path does not
install a tracing subscriber or construct an OTEL provider, so those events
are not useful in the container deployment that most needs them.

Other Codex long-running entrypoints already load `Config`, construct an OTEL
provider through `codex_core::otel_init::build_provider`, attach logger and
tracing layers to `tracing_subscriber`, and record process-start metrics when
metrics are enabled. Exec-server should reuse that path rather than create
telemetry machinery in the protocol and execution crate.

## Signal model

The following Kubernetes component concepts map cleanly to exec-server:

| Kubernetes concept | Exec-server equivalent |
| --- | --- |
| Component logs | Structured events about server, connection, session, relay, and managed-operation lifecycle |
| Component metrics | Bounded counters, gauges, and duration histograms for transport and managed operations |
| Component traces | Spans from an inbound RPC through a process, filesystem, HTTP, or relay action |
| `/readyz` and `/livez` | Whether the service can accept work and whether the process runtime is alive |
| Workload logs | Command stdout/stderr, which remain protocol output and are not component telemetry |

All signals must use low-cardinality operation names and outcomes. Session IDs
and process IDs may be useful for log or trace correlation after a privacy
review, but must not become metric labels. Commands, paths, body content, auth
material, environment values, and full URLs are excluded from emitted data.

## Initial increment: opt-in OTEL bootstrap

The first change should expose the tracing already present in exec-server and
establish the exporter path needed for later instrumentation.

Implementation ownership remains in `codex-rs/cli/src/main.rs`, where
`codex exec-server` is launched and where `codex-core` configuration and OTEL
bootstrap dependencies already exist:

1. Load Codex configuration for every `exec-server` invocation, including
   local listener mode. Remote mode reuses the same loaded config for
   authentication.
2. Build an OTEL provider with service name `codex-exec-server` and
   `default_analytics_enabled = false`.
3. Attach a stderr formatting layer filtered through `RUST_LOG`, plus the
   configured OTEL logger and tracing layers, before entering local serving or
   remote relay operation.
4. Invoke the existing process-start metric helper. It records only when the
   operator has explicitly enabled metrics export.
5. Retain the provider for the server process lifetime so exporters can flush
   on shutdown.

This increment intentionally does not install SQLite telemetry: unlike
app-server and MCP server, standalone exec-server does not initialize the
Codex state database.

### Configuration behavior

The existing configuration surface applies without new CLI flags:

| Signal | Enablement |
| --- | --- |
| Local stderr logs | Controlled by `RUST_LOG`; written to stderr only |
| Exported logs | Configure `otel.exporter` |
| Exported traces | Configure `otel.trace_exporter` |
| Exported metrics | Set `analytics.enabled = true`; optionally override `otel.metrics_exporter` |

When `analytics.enabled` is absent, exec-server does not export metrics. This
differs intentionally from interactive entrypoints that can default analytics
on: exec-server may run as a long-lived container component, so telemetry
export is an operator deployment choice. If analytics is enabled without a
metrics exporter override, existing configuration defaults apply.

### Transport invariants

- Local websocket mode continues to write the bound `ws://` startup URL to
  stdout for existing launch scripts.
- Stdio mode continues to reserve stdout for JSON-RPC frames.
- Subscriber output is always sent to stderr or OTEL exporters.
- Local startup now loads configuration before opening its listener; malformed
  strict configuration continues to fail before serving work.

## Follow-on increments

Once the exporter path is deployed and its operational value is verified,
later changes can add signals at existing component boundaries.

### Lifecycle events and spans

Emit bounded structured events and spans for:

- process startup and shutdown;
- local connections and remote relay registration, connection, disconnection,
  and retry;
- session attach, resume, and eviction;
- RPC completion by method family and outcome;
- managed process start, exit, and termination;
- filesystem and HTTP proxy operation completion by operation family and
  outcome.

These events should identify operation type and status without including
payload content. Trace-context propagation over the exec-server protocol can
be considered separately after local spans are useful.

### Metrics

Add OTEL instruments only with fixed label domains, such as transport,
operation family, result, and HTTP status class:

| Candidate metric | Instrument | Labels |
| --- | --- | --- |
| `exec_server.connections.active` | gauge | `transport` |
| `exec_server.connections.total` | counter | `transport`, `result` |
| `exec_server.requests.total` | counter | `operation`, `result` |
| `exec_server.request.duration` | histogram | `operation`, `result` |
| `exec_server.processes.active` | gauge | none |
| `exec_server.process.duration` | histogram | `result` |
| `exec_server.relay.reconnects` | counter | `reason` |

Metric labels must never include session IDs, process IDs, commands, paths,
hostnames, or URLs.

### Health

Websocket mode already returns success from `/readyz`. A later increment
should separate:

- `/livez`: the server runtime is responsive and has not begun shutdown.
- `/readyz`: the server can accept useful work.

For remote environment mode, readiness should reflect an active registration
and relay connection. Transient downstream failures should affect readiness
rather than liveness, so container restarts do not amplify backend outages.

## Validation and rollout

The initial implementation should test that:

- local websocket startup still writes its endpoint on stdout while an enabled
  `RUST_LOG` filter makes exec-server component logs visible on stderr;
- configured log and trace exporters can be constructed under the
  `codex-exec-server` service name;
- metrics are absent by default and present only when analytics is explicitly
  enabled;
- strict configuration errors still prevent listener startup.

Deployment can start by enabling stderr collection for container instances,
then opting selected environments into OTEL log or trace export. Subsequent
signal additions should use the redaction and cardinality rules above and be
validated against real incident queries before adding further volume.

## References

- [Kubernetes logging architecture](https://kubernetes.io/docs/concepts/cluster-administration/logging/)
- [Kubernetes system component metrics](https://kubernetes.io/docs/concepts/cluster-administration/system-metrics/)
- [Kubernetes system component traces](https://kubernetes.io/docs/concepts/cluster-administration/system-traces/)
- [Kubernetes API health endpoints](https://kubernetes.io/docs/reference/using-api/health-checks/)
