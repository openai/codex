use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use std::time::Instant;

use codex_otel::MetricsClient;
use tracing::warn;

const CONNECTIONS_ACTIVE_METRIC: &str = "exec_server_connections_active";
const CONNECTIONS_ACTIVE_DESCRIPTION: &str = "Number of active exec-server connections.";
const CONNECTIONS_TOTAL_METRIC: &str = "exec_server_connections_total";
const CONNECTIONS_TOTAL_DESCRIPTION: &str = "Total number of accepted exec-server connections.";
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
    active: Mutex<ActiveCounts>,
}

#[derive(Default)]
struct ActiveCounts {
    relay_connections: i64,
    stdio_connections: i64,
    websocket_connections: i64,
    processes: i64,
}

pub(crate) struct ConnectionMetricGuard {
    telemetry: ExecServerTelemetry,
    transport: ConnectionTransport,
}

#[derive(Clone)]
pub(crate) struct ProcessMetricGuard {
    measurement: Arc<Mutex<Option<ProcessMetricMeasurement>>>,
}

struct ProcessMetricMeasurement {
    telemetry: ExecServerTelemetry,
    started_at: Instant,
    result: &'static str,
}

impl ExecServerTelemetry {
    pub fn new(metrics: MetricsClient) -> Self {
        Self {
            inner: Some(Arc::new(ExecServerTelemetryInner {
                metrics,
                active: Mutex::new(ActiveCounts::default()),
            })),
        }
    }

    pub(crate) fn connection_started(
        &self,
        transport: ConnectionTransport,
    ) -> ConnectionMetricGuard {
        self.with_inner(|inner| {
            inner.adjust_connection_count(transport, /*delta*/ 1);
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

    pub(crate) fn process_started(&self) -> ProcessMetricGuard {
        self.with_inner(|inner| {
            inner.adjust_process_count(/*delta*/ 1);
        });
        ProcessMetricGuard {
            measurement: Arc::new(Mutex::new(Some(ProcessMetricMeasurement {
                telemetry: self.clone(),
                started_at: Instant::now(),
                result: "unknown",
            }))),
        }
    }

    fn process_finished(&self, result: &'static str, duration: Duration) {
        self.with_inner(|inner| {
            inner.adjust_process_count(/*delta*/ -1);
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
            inner.adjust_connection_count(transport, /*delta*/ -1);
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

impl ProcessMetricGuard {
    pub(crate) fn finish(&self, result: &'static str) {
        let measurement = self
            .measurement
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        if let Some(mut measurement) = measurement {
            measurement.result = result;
        }
    }
}

impl Drop for ProcessMetricMeasurement {
    fn drop(&mut self) {
        self.telemetry
            .process_finished(self.result, self.started_at.elapsed());
    }
}

impl ExecServerTelemetryInner {
    fn active_counts(&self) -> std::sync::MutexGuard<'_, ActiveCounts> {
        // These are independent integer counts, so a panic cannot leave a cross-field invariant
        // half-updated. Recovering a poisoned lock preserves the last completed count update.
        self.active
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn adjust_connection_count(&self, transport: ConnectionTransport, delta: i64) {
        let mut active = self.active_counts();
        let count = match transport {
            ConnectionTransport::Relay => &mut active.relay_connections,
            ConnectionTransport::Stdio => &mut active.stdio_connections,
            ConnectionTransport::WebSocket => &mut active.websocket_connections,
        };
        *count += delta;
        self.gauge(
            CONNECTIONS_ACTIVE_METRIC,
            CONNECTIONS_ACTIVE_DESCRIPTION,
            *count,
            &[("transport", transport.metric_tag())],
        );
    }

    fn adjust_process_count(&self, delta: i64) {
        let mut active = self.active_counts();
        active.processes += delta;
        self.gauge(
            PROCESSES_ACTIVE_METRIC,
            PROCESSES_ACTIVE_DESCRIPTION,
            active.processes,
            &[],
        );
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
