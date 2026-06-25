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
const REMOTE_REGISTRATION_METRICS: OperationMetrics = OperationMetrics {
    total_name: "exec_server_remote_registration_total",
    total_description: "Total number of remote exec-server registration attempts.",
    duration_name: "exec_server_remote_registration_duration_seconds",
    duration_description: "Duration of remote exec-server registration attempts in seconds.",
};
const REMOTE_RENDEZVOUS_METRICS: OperationMetrics = OperationMetrics {
    total_name: "exec_server_remote_rendezvous_connect_total",
    total_description: "Total number of remote exec-server rendezvous connection attempts.",
    duration_name: "exec_server_remote_rendezvous_connect_duration_seconds",
    duration_description: "Duration of remote exec-server rendezvous connection attempts in seconds.",
};
const REMOTE_RECONNECTS_TOTAL_METRIC: &str = "exec_server_remote_reconnects_total";
const REMOTE_RECONNECTS_TOTAL_DESCRIPTION: &str = "Total number of remote exec-server reconnects.";

#[derive(Clone, Copy)]
struct OperationMetrics {
    total_name: &'static str,
    total_description: &'static str,
    duration_name: &'static str,
    duration_description: &'static str,
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
    active: Arc<Mutex<ActiveCounts>>,
}

#[derive(Default)]
struct ActiveCounts {
    relay_connections: i64,
    stdio_connections: i64,
    websocket_connections: i64,
    processes: i64,
}

impl ActiveCounts {
    fn connections(&self, transport: ConnectionTransport) -> i64 {
        match transport {
            ConnectionTransport::Relay => self.relay_connections,
            ConnectionTransport::Stdio => self.stdio_connections,
            ConnectionTransport::WebSocket => self.websocket_connections,
        }
    }
}

pub(crate) struct ConnectionMetricGuard {
    telemetry: ExecServerTelemetry,
    transport: ConnectionTransport,
}

pub(crate) struct ProcessMetricGuard {
    telemetry: ExecServerTelemetry,
    started_at: Instant,
    result: &'static str,
}

impl ExecServerTelemetry {
    pub fn new(metrics: MetricsClient) -> Self {
        let active = Arc::new(Mutex::new(ActiveCounts::default()));
        register_active_gauges(&metrics, &active);
        Self {
            inner: Some(Arc::new(ExecServerTelemetryInner { metrics, active })),
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
                &[("transport", transport.metric_tag())],
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
        self.record_operation(REMOTE_REGISTRATION_METRICS, result, duration);
    }

    pub(crate) fn remote_rendezvous_completed(&self, result: &'static str, duration: Duration) {
        self.record_operation(REMOTE_RENDEZVOUS_METRICS, result, duration);
    }

    pub(crate) fn remote_reconnect(&self, reason: &'static str) {
        self.with_inner(|inner| {
            inner.counter(
                REMOTE_RECONNECTS_TOTAL_METRIC,
                REMOTE_RECONNECTS_TOTAL_DESCRIPTION,
                &[("reason", reason)],
            );
        });
    }

    pub(crate) fn process_started(&self) -> ProcessMetricGuard {
        self.with_inner(|inner| {
            inner.adjust_process_count(/*delta*/ 1);
        });
        ProcessMetricGuard {
            telemetry: self.clone(),
            started_at: Instant::now(),
            result: "unknown",
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

    fn record_operation(
        &self,
        metrics: OperationMetrics,
        result: &'static str,
        duration: Duration,
    ) {
        self.with_inner(|inner| {
            let tags = [("result", result)];
            inner.counter(metrics.total_name, metrics.total_description, &tags);
            inner.duration(
                metrics.duration_name,
                metrics.duration_description,
                duration,
                &tags,
            );
        });
    }
}

impl Drop for ConnectionMetricGuard {
    fn drop(&mut self) {
        self.telemetry.connection_finished(self.transport);
    }
}

impl ProcessMetricGuard {
    pub(crate) fn finish(mut self, result: &'static str) {
        self.result = result;
    }
}

impl Drop for ProcessMetricGuard {
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
    }

    fn adjust_process_count(&self, delta: i64) {
        let mut active = self.active_counts();
        active.processes += delta;
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
}

fn register_active_gauges(metrics: &MetricsClient, active: &Arc<Mutex<ActiveCounts>>) {
    for transport in [
        ConnectionTransport::Relay,
        ConnectionTransport::Stdio,
        ConnectionTransport::WebSocket,
    ] {
        register_active_gauge(
            metrics,
            active,
            CONNECTIONS_ACTIVE_METRIC,
            CONNECTIONS_ACTIVE_DESCRIPTION,
            &[("transport", transport.metric_tag())],
            move |active| active.connections(transport),
        );
    }

    register_active_gauge(
        metrics,
        active,
        PROCESSES_ACTIVE_METRIC,
        PROCESSES_ACTIVE_DESCRIPTION,
        &[],
        |active| active.processes,
    );
}

fn register_active_gauge(
    metrics: &MetricsClient,
    active: &Arc<Mutex<ActiveCounts>>,
    name: &str,
    description: &str,
    tags: &[(&str, &str)],
    read: impl Fn(&ActiveCounts) -> i64 + Send + Sync + 'static,
) {
    let active = Arc::clone(active);
    if metrics
        .register_observable_gauge_with_description(
            name,
            description,
            move || {
                let active = active
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                read(&active)
            },
            tags,
        )
        .is_err()
    {
        warn!(metric = name, "failed to register exec-server gauge");
    }
}
