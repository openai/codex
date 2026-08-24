use crate::harness::attributes_to_map;
use crate::harness::find_metric;
use codex_otel::MetricsClient;
use codex_otel::MetricsConfig;
use codex_otel::Result;
use codex_otel::SessionTelemetry;
use codex_otel::TelemetryAuthMode;
use codex_protocol::ThreadId;
use codex_protocol::protocol::SessionSource;
use opentelemetry_sdk::metrics::InMemoryMetricExporter;
use opentelemetry_sdk::metrics::data::AggregatedMetrics;
use opentelemetry_sdk::metrics::data::MetricData;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;

#[test]
fn snapshot_collects_metrics_without_shutdown() -> Result<()> {
    let exporter = InMemoryMetricExporter::default();
    let config = MetricsConfig::in_memory(
        "test",
        "codex-cli",
        env!("CARGO_PKG_VERSION"),
        exporter.clone(),
    )
    .with_tag("service", "codex-cli")?
    .with_runtime_reader();
    let metrics = MetricsClient::new(config)?;

    metrics.counter(
        "codex.tool.call",
        /*inc*/ 1,
        &[("tool", "shell"), ("success", "true")],
    )?;

    let snapshot = metrics.snapshot()?;

    let metric = find_metric(&snapshot, "codex.tool.call").expect("counter metric missing");
    let attrs = match metric.data() {
        AggregatedMetrics::U64(data) => match data {
            MetricData::Sum(sum) => {
                let points: Vec<_> = sum.data_points().collect();
                assert_eq!(points.len(), 1);
                attributes_to_map(points[0].attributes())
            }
            _ => panic!("unexpected counter aggregation"),
        },
        _ => panic!("unexpected counter data type"),
    };

    let expected = BTreeMap::from([
        ("service".to_string(), "codex-cli".to_string()),
        ("success".to_string(), "true".to_string()),
        ("tool".to_string(), "shell".to_string()),
    ]);
    assert_eq!(attrs, expected);

    let finished = exporter
        .get_finished_metrics()
        .expect("finished metrics should be readable");
    assert!(finished.is_empty(), "expected no periodic exports yet");

    Ok(())
}

#[test]
fn observable_gauge_is_collected_on_every_delta_snapshot() -> Result<()> {
    let exporter = InMemoryMetricExporter::default();
    let config = MetricsConfig::in_memory("test", "codex-cli", env!("CARGO_PKG_VERSION"), exporter)
        .with_runtime_reader();
    let metrics = MetricsClient::new(config)?;
    metrics.register_observable_gauge_with_description(
        "codex.active",
        "Number of active operations.",
        || 1,
        &[("component", "test")],
    )?;

    for snapshot in [metrics.snapshot()?, metrics.snapshot()?] {
        let gauge = find_metric(&snapshot, "codex.active").expect("gauge metric missing");
        let point = match gauge.data() {
            AggregatedMetrics::I64(MetricData::Gauge(gauge)) => {
                gauge.data_points().next().expect("gauge point")
            }
            _ => panic!("unexpected gauge metric data type"),
        };
        assert_eq!(point.value(), 1);
        assert_eq!(
            attributes_to_map(point.attributes()),
            BTreeMap::from([("component".to_string(), "test".to_string())])
        );
    }

    metrics.shutdown()?;
    Ok(())
}

#[test]
fn manager_snapshot_metrics_collects_without_shutdown() -> Result<()> {
    let exporter = InMemoryMetricExporter::default();
    let config = MetricsConfig::in_memory("test", "codex-cli", env!("CARGO_PKG_VERSION"), exporter)
        .with_tag("service", "codex-cli")?
        .with_runtime_reader();
    let metrics = MetricsClient::new(config)?;
    let manager = SessionTelemetry::new(
        ThreadId::new(),
        "gpt-5.1",
        "gpt-5.1",
        Some("account-id".to_string()),
        /*account_email*/ None,
        Some(TelemetryAuthMode::ApiKey),
        "test_originator".to_string(),
        /*log_user_prompts*/ true,
        "tty".to_string(),
        SessionSource::Cli,
    )
    .with_metrics(metrics);

    manager.counter(
        "codex.tool.call",
        /*inc*/ 1,
        &[("tool", "shell"), ("success", "true")],
    );

    let snapshot = manager.snapshot_metrics()?;
    let metric = find_metric(&snapshot, "codex.tool.call").expect("counter metric missing");
    let attrs = match metric.data() {
        AggregatedMetrics::U64(data) => match data {
            MetricData::Sum(sum) => {
                let points: Vec<_> = sum.data_points().collect();
                assert_eq!(points.len(), 1);
                attributes_to_map(points[0].attributes())
            }
            _ => panic!("unexpected counter aggregation"),
        },
        _ => panic!("unexpected counter data type"),
    };

    let expected = BTreeMap::from([
        (
            "app.version".to_string(),
            env!("CARGO_PKG_VERSION").to_string(),
        ),
        (
            "auth_mode".to_string(),
            TelemetryAuthMode::ApiKey.to_string(),
        ),
        ("model".to_string(), "gpt-5.1".to_string()),
        ("originator".to_string(), "test_originator".to_string()),
        ("service".to_string(), "codex-cli".to_string()),
        ("session_source".to_string(), "cli".to_string()),
        ("success".to_string(), "true".to_string()),
        ("tool".to_string(), "shell".to_string()),
    ]);
    assert_eq!(attrs, expected);

    Ok(())
}

#[test]
fn manager_turn_cost_records_microusd_metric() -> Result<()> {
    let exporter = InMemoryMetricExporter::default();
    let config = MetricsConfig::in_memory("test", "codex-cli", env!("CARGO_PKG_VERSION"), exporter)
        .with_runtime_reader();
    let metrics = MetricsClient::new(config)?;
    let thread_id = ThreadId::new();
    let conversation_id = thread_id.to_string();
    let manager = SessionTelemetry::new(
        thread_id,
        "gpt-5.6",
        "gpt-5.6",
        /*account_id*/ None,
        /*account_email*/ None,
        Some(TelemetryAuthMode::ApiKey),
        "test_originator".to_string(),
        /*log_user_prompts*/ false,
        "tty".to_string(),
        SessionSource::Cli,
    )
    .with_metrics(metrics);

    manager.record_turn_cost(
        "turn-123",
        "0.0001245",
        /*interrupted*/ false,
        Some("fast"),
        Some("high"),
    );

    let snapshot = manager.snapshot_metrics()?;
    let metric = find_metric(&snapshot, "codex.turn.cost_microusd")
        .expect("turn-cost microdollar metric missing");
    let point = match metric.data() {
        AggregatedMetrics::U64(MetricData::Sum(sum)) => {
            sum.data_points().next().expect("turn-cost data point")
        }
        _ => panic!("unexpected turn-cost metric data type"),
    };
    assert_eq!(point.value(), 125);
    assert_eq!(
        attributes_to_map(point.attributes()),
        BTreeMap::from([
            (
                "app.version".to_string(),
                env!("CARGO_PKG_VERSION").to_string(),
            ),
            (
                "auth_mode".to_string(),
                TelemetryAuthMode::ApiKey.to_string(),
            ),
            ("conversation.id".to_string(), conversation_id),
            ("model".to_string(), "gpt-5.6".to_string()),
            ("originator".to_string(), "test_originator".to_string()),
            ("reasoning_effort".to_string(), "high".to_string()),
            ("session_source".to_string(), "cli".to_string()),
            ("speed".to_string(), "fast".to_string()),
            ("turn.id".to_string(), "turn-123".to_string()),
            ("turn.interrupted".to_string(), "false".to_string()),
        ])
    );

    Ok(())
}
