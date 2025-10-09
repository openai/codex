#[cfg(feature = "otel")]
mod imp {
    use opentelemetry::KeyValue;
    use opentelemetry::global;
    use opentelemetry::metrics::Counter;
    use std::sync::OnceLock;

    const METER_NAME: &str = "codex.keepalive";

    struct KeepaliveMetrics {
        heartbeat_total: Counter<u64>,
        reconnect_total: Counter<u64>,
        idle_timeout_total: Counter<u64>,
    }

    static METRICS: OnceLock<KeepaliveMetrics> = OnceLock::new();

    impl KeepaliveMetrics {
        fn new() -> Self {
            let meter = global::meter(METER_NAME);
            let heartbeat_total = meter
                .u64_counter("codex_keepalive_heartbeat_total")
                .with_description(
                    "Count of synthetic heartbeat activity emitted by Codex transports.",
                )
                .build();
            let reconnect_total = meter
                .u64_counter("codex_keepalive_reconnect_total")
                .with_description(
                    "Count of transport reconnect attempts triggered by idle detection.",
                )
                .build();
            let idle_timeout_total = meter
                .u64_counter("codex_keepalive_idle_timeout_total")
                .with_description(
                    "Count of connections closed after exceeding idle timeout despite keepalives.",
                )
                .build();

            Self {
                heartbeat_total,
                reconnect_total,
                idle_timeout_total,
            }
        }
    }

    fn metrics() -> &'static KeepaliveMetrics {
        METRICS.get_or_init(KeepaliveMetrics::new)
    }

    pub fn record_heartbeat(transport: &str, status: &str, elapsed_ms: Option<u64>) {
        let mut attrs = vec![
            KeyValue::new("transport", transport.to_string()),
            KeyValue::new("status", status.to_string()),
        ];
        if let Some(ms) = elapsed_ms {
            attrs.push(KeyValue::new("elapsed_ms", ms as i64));
        }
        metrics().heartbeat_total.add(1, &attrs);
    }

    pub fn record_reconnect(transport: &str, status: &str) {
        let attrs = [
            KeyValue::new("transport", transport.to_string()),
            KeyValue::new("status", status.to_string()),
        ];
        metrics().reconnect_total.add(1, &attrs);
    }

    pub fn record_idle_timeout(transport: &str) {
        let attrs = [KeyValue::new("transport", transport.to_string())];
        metrics().idle_timeout_total.add(1, &attrs);
    }
}

#[cfg(not(feature = "otel"))]
mod imp {
    #[inline]
    pub fn record_heartbeat(_: &str, _: &str, _: Option<u64>) {}

    #[inline]
    pub fn record_reconnect(_: &str, _: &str) {}

    #[inline]
    pub fn record_idle_timeout(_: &str) {}
}

pub use imp::{record_heartbeat, record_idle_timeout, record_reconnect};
