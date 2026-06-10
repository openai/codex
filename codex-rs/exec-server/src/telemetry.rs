use std::sync::Arc;
use std::sync::atomic::AtomicI64;
use std::sync::atomic::Ordering;
use std::time::Duration;

use codex_otel::MetricsClient;
use tracing::warn;

const CONNECTIONS_ACTIVE_METRIC: &str = "exec_server_connections_active";
const CONNECTIONS_ACTIVE_DESCRIPTION: &str = "Number of active exec-server connections.";
const CONNECTIONS_TOTAL_METRIC: &str = "exec_server_connections_total";
const CONNECTIONS_TOTAL_DESCRIPTION: &str = "Total number of accepted exec-server connections.";
const REMOTE_REGISTRATION_TOTAL_METRIC: &str = "exec_server_remote_registration_total";
const REMOTE_REGISTRATION_TOTAL_DESCRIPTION: &str =
    "Total number of remote exec-server registration attempts.";
const REMOTE_REGISTRATION_DURATION_METRIC: &str =
    "exec_server_remote_registration_duration_seconds";
const REMOTE_REGISTRATION_DURATION_DESCRIPTION: &str =
    "Duration of remote exec-server registration attempts in seconds.";
const REQUESTS_TOTAL_METRIC: &str = "exec_server_requests_total";
const REQUESTS_TOTAL_DESCRIPTION: &str = "Total number of exec-server requests.";
const REQUEST_DURATION_METRIC: &str = "exec_server_request_duration_seconds";
const REQUEST_DURATION_DESCRIPTION: &str = "Duration of exec-server requests in seconds.";
const PROCESSES_ACTIVE_METRIC: &str = "exec_server_processes_active";
const PROCESSES_ACTIVE_DESCRIPTION: &str = "Number of active exec-server processes.";
const PROCESSES_FINISHED_TOTAL_METRIC: &str = "exec_server_processes_finished_total";
const PROCESSES_FINISHED_TOTAL_DESCRIPTION: &str =
    "Total number of finished exec-server processes.";
const PROCESS_DURATION_METRIC: &str = "exec_server_process_duration_seconds";
const PROCESS_DURATION_DESCRIPTION: &str = "Duration of exec-server processes in seconds.";

pub fn runtime_span() -> tracing::Span {
    tracing::info_span!("codex.exec_server", otel.kind = "internal")
}

#[derive(Clone, Copy)]
pub(crate) enum ConnectionTransport {
    Relay,
    Stdio,
    WebSocket,
}

impl ConnectionTransport {
    fn metric_tag(self) -> &'static str {
        match self {
            Self::Relay => "relay",
            Self::Stdio => "stdio",
            Self::WebSocket => "websocket",
        }
    }
}

#[derive(Clone, Default)]
pub struct ExecServerTelemetry {
    inner: Option<Arc<ExecServerTelemetryInner>>,
}

struct ExecServerTelemetryInner {
    metrics: MetricsClient,
    relay_connections: AtomicI64,
    stdio_connections: AtomicI64,
    websocket_connections: AtomicI64,
    active_processes: AtomicI64,
}

pub(crate) struct ConnectionMetricGuard {
    telemetry: ExecServerTelemetry,
    transport: ConnectionTransport,
}

impl ExecServerTelemetry {
    pub fn new(metrics: Option<MetricsClient>) -> Self {
        Self {
            inner: metrics.map(|metrics| {
                Arc::new(ExecServerTelemetryInner {
                    metrics,
                    relay_connections: AtomicI64::new(0),
                    stdio_connections: AtomicI64::new(0),
                    websocket_connections: AtomicI64::new(0),
                    active_processes: AtomicI64::new(0),
                })
            }),
        }
    }

    pub(crate) fn connection_started(
        &self,
        transport: ConnectionTransport,
    ) -> ConnectionMetricGuard {
        self.with_inner(|inner| {
            let active = inner
                .connection_counter(transport)
                .fetch_add(1, Ordering::AcqRel)
                + 1;
            inner.gauge(
                CONNECTIONS_ACTIVE_METRIC,
                CONNECTIONS_ACTIVE_DESCRIPTION,
                active,
                &[("transport", transport.metric_tag())],
            );
            inner.counter(
                CONNECTIONS_TOTAL_METRIC,
                CONNECTIONS_TOTAL_DESCRIPTION,
                &[
                    ("transport", transport.metric_tag()),
                    ("result", "accepted"),
                ],
            );
        });
        ConnectionMetricGuard {
            telemetry: self.clone(),
            transport,
        }
    }

    pub(crate) fn request_completed(
        &self,
        method: &'static str,
        result: &'static str,
        duration: Duration,
    ) {
        self.with_inner(|inner| {
            let tags = [("method", method), ("result", result)];
            inner.counter(REQUESTS_TOTAL_METRIC, REQUESTS_TOTAL_DESCRIPTION, &tags);
            inner.duration(
                REQUEST_DURATION_METRIC,
                REQUEST_DURATION_DESCRIPTION,
                duration,
                &tags,
            );
        });
    }

    pub(crate) fn remote_registration_completed(&self, result: &'static str, duration: Duration) {
        self.with_inner(|inner| {
            let tags = [("result", result)];
            inner.counter(
                REMOTE_REGISTRATION_TOTAL_METRIC,
                REMOTE_REGISTRATION_TOTAL_DESCRIPTION,
                &tags,
            );
            inner.duration(
                REMOTE_REGISTRATION_DURATION_METRIC,
                REMOTE_REGISTRATION_DURATION_DESCRIPTION,
                duration,
                &tags,
            );
        });
    }

    pub(crate) fn process_started(&self) {
        self.with_inner(|inner| {
            let active = inner.active_processes.fetch_add(1, Ordering::AcqRel) + 1;
            inner.gauge(
                PROCESSES_ACTIVE_METRIC,
                PROCESSES_ACTIVE_DESCRIPTION,
                active,
                &[],
            );
        });
    }

    pub(crate) fn process_finished(&self, result: &'static str, duration: Duration) {
        self.with_inner(|inner| {
            let active = inner.active_processes.fetch_sub(1, Ordering::AcqRel) - 1;
            inner.gauge(
                PROCESSES_ACTIVE_METRIC,
                PROCESSES_ACTIVE_DESCRIPTION,
                active,
                &[],
            );
            inner.counter(
                PROCESSES_FINISHED_TOTAL_METRIC,
                PROCESSES_FINISHED_TOTAL_DESCRIPTION,
                &[("result", result)],
            );
            inner.duration(
                PROCESS_DURATION_METRIC,
                PROCESS_DURATION_DESCRIPTION,
                duration,
                &[("result", result)],
            );
        });
    }

    fn connection_finished(&self, transport: ConnectionTransport) {
        self.with_inner(|inner| {
            let active = inner
                .connection_counter(transport)
                .fetch_sub(1, Ordering::AcqRel)
                - 1;
            inner.gauge(
                CONNECTIONS_ACTIVE_METRIC,
                CONNECTIONS_ACTIVE_DESCRIPTION,
                active,
                &[("transport", transport.metric_tag())],
            );
        });
    }

    fn with_inner(&self, emit: impl FnOnce(&ExecServerTelemetryInner)) {
        if let Some(inner) = &self.inner {
            emit(inner);
        }
    }
}

impl Drop for ConnectionMetricGuard {
    fn drop(&mut self) {
        self.telemetry.connection_finished(self.transport);
    }
}

impl ExecServerTelemetryInner {
    fn connection_counter(&self, transport: ConnectionTransport) -> &AtomicI64 {
        match transport {
            ConnectionTransport::Relay => &self.relay_connections,
            ConnectionTransport::Stdio => &self.stdio_connections,
            ConnectionTransport::WebSocket => &self.websocket_connections,
        }
    }

    fn counter(&self, name: &str, description: &str, tags: &[(&str, &str)]) {
        if self
            .metrics
            .counter_with_description(name, description, /*inc*/ 1, tags)
            .is_err()
        {
            warn!(metric = name, "failed to emit exec-server counter");
        }
    }

    fn duration(&self, name: &str, description: &str, duration: Duration, tags: &[(&str, &str)]) {
        if self
            .metrics
            .record_duration_seconds_with_description(name, description, duration, tags)
            .is_err()
        {
            warn!(metric = name, "failed to emit exec-server duration");
        }
    }

    fn gauge(&self, name: &str, description: &str, value: i64, tags: &[(&str, &str)]) {
        if self
            .metrics
            .gauge_with_description(name, description, value, tags)
            .is_err()
        {
            warn!(metric = name, "failed to emit exec-server gauge");
        }
    }
}

#[cfg(test)]
#[path = "telemetry_tests.rs"]
mod tests;
