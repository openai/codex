use std::sync::Arc;
use std::sync::atomic::AtomicI64;
use std::sync::atomic::Ordering;

use codex_otel::MetricsClient;
use tracing::warn;

const CONNECTIONS_ACTIVE_METRIC: &str = "exec_server_connections_active";
const CONNECTIONS_ACTIVE_DESCRIPTION: &str = "Number of active exec-server connections.";
const CONNECTIONS_TOTAL_METRIC: &str = "exec_server_connections_total";
const CONNECTIONS_TOTAL_DESCRIPTION: &str = "Total number of accepted exec-server connections.";

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
