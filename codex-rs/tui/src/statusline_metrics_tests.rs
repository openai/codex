#![cfg(test)]
//! Collect the full label set so identifying attributes fail the same assertions as wrong counts.

use super::*;
use codex_otel::MetricsConfig;
use opentelemetry_sdk::metrics::InMemoryMetricExporter;
use opentelemetry_sdk::metrics::data::AggregatedMetrics;
use opentelemetry_sdk::metrics::data::MetricData;
use opentelemetry_sdk::metrics::data::ScopeMetrics;
use pretty_assertions::assert_eq;

pub(crate) struct Collector {
    client: MetricsClient,
}

impl Collector {
    pub(crate) fn new() -> Self {
        let config = MetricsConfig::in_memory(
            "test",
            "codex-tui",
            "test",
            InMemoryMetricExporter::default(),
        )
        .with_runtime_reader();
        Self {
            client: MetricsClient::new(config).unwrap(),
        }
    }

    pub(crate) fn metrics(&self, config: &mut Config) -> StatuslineMetrics {
        config.otel.metrics_exporter = OtelExporterKind::OtlpGrpc {
            endpoint: "http://localhost:4317".into(),
            headers: Default::default(),
            tls: None,
        };
        StatuslineMetrics::new(config, Some(self.client.clone()), "codex-tui")
    }

    pub(crate) fn snapshot(&self) -> Vec<String> {
        let batch = self.client.snapshot().unwrap();
        let mut result = Vec::new();
        for metric in batch.scope_metrics().flat_map(ScopeMetrics::metrics) {
            let Some(name) = metric.name().strip_prefix("codex.statusline.") else {
                continue;
            };
            let AggregatedMetrics::U64(MetricData::Sum(sum)) = metric.data() else {
                panic!("expected a counter for {name}");
            };
            for point in sum.data_points() {
                let labels: String = point
                    .attributes()
                    .map(|a| format!(" {}={}", a.key, a.value))
                    .collect();
                result.push(format!("{name}{labels} {}", point.value()));
            }
        }
        result.sort_unstable();
        result
    }
}

#[tokio::test]
async fn custom_resources_and_otlp_headers_suppress_builtin_reporting() {
    let mut app = crate::app::test_support::make_test_app().await;
    app.config.otel.metrics_exporter = OtelExporterKind::Statsig;
    let collector = Collector::new();
    for key in [
        None,
        Some("OTEL_RESOURCE_ATTRIBUTES"),
        Some("OTEL_EXPORTER_OTLP_METRICS_HEADERS"),
        Some("OTEL_EXPORTER_OTLP_HEADERS"),
    ] {
        let metrics = StatuslineMetrics::with_env(
            &app.config,
            Some(collector.client.clone()),
            "codex-tui",
            |name| (key == Some(name)).then(|| "private=value".into()),
        );
        metrics.record(&app.config, "codex.statusline.picker_opened", &[]);
        let expected = if key.is_none() {
            vec!["picker_opened 1"]
        } else {
            vec![]
        };
        assert_eq!(collector.snapshot(), expected);
    }
}

#[tokio::test]
async fn metrics_skip_changed_destinations_and_custom_builtin_resources() {
    let mut app = crate::app::test_support::make_test_app().await;
    let collector = Collector::new();
    let metrics = collector.metrics(&mut app.config);
    metrics.record(&app.config, "codex.statusline.picker_opened", &[]);
    app.config.otel.metrics_exporter = OtelExporterKind::Statsig;
    metrics.record(&app.config, "codex.statusline.picker_opened", &[]);
    app.config.otel.environment = "custom".into();
    let metrics = StatuslineMetrics::new(&app.config, Some(collector.client.clone()), "codex-tui");
    metrics.record(&app.config, "codex.statusline.picker_opened", &[]);
    assert_eq!(collector.snapshot(), ["picker_opened 1"]);
}
