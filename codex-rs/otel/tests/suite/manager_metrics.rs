use crate::harness::attributes_to_map;
use crate::harness::build_metrics_with_defaults;
use crate::harness::find_metric;
use crate::harness::latest_metrics;
use codex_app_server_protocol::AuthMode;
use codex_otel::OtelManager;
use codex_otel::metrics::Result;
use codex_protocol::ThreadId;
use codex_protocol::protocol::SessionSource;
use opentelemetry_sdk::metrics::data::AggregatedMetrics;
use opentelemetry_sdk::metrics::data::MetricData;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;

// Ensures OtelManager attaches metadata tags when forwarding metrics.
#[test]
fn manager_attaches_metadata_tags_to_metrics() -> Result<()> {
    let (metrics, exporter) = build_metrics_with_defaults(&[("service", "codex-cli")])?;
    let manager = OtelManager::new(
        ThreadId::new(),
        "gpt-5.1",
        "gpt-5.1",
        Some("account-id".to_string()),
        None,
        Some(AuthMode::ApiKey),
        true,
        "tty".to_string(),
        SessionSource::Cli,
    )
    .with_metrics(metrics);

    manager.counter("codex.session_started", 1, &[("source", "tui")]);
    manager.shutdown_metrics()?;

    let resource_metrics = latest_metrics(&exporter);
    let metric =
        find_metric(&resource_metrics, "codex.session_started").expect("counter metric missing");
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
        ("auth_mode".to_string(), AuthMode::ApiKey.to_string()),
        ("model".to_string(), "gpt-5.1".to_string()),
        ("service".to_string(), "codex-cli".to_string()),
        ("session_source".to_string(), "cli".to_string()),
        ("source".to_string(), "tui".to_string()),
    ]);
    assert_eq!(attrs, expected);

    Ok(())
}

// Ensures metadata tagging can be disabled when recording via OtelManager.
#[test]
fn manager_allows_disabling_metadata_tags() -> Result<()> {
    let (metrics, exporter) = build_metrics_with_defaults(&[])?;
    let manager = OtelManager::new(
        ThreadId::new(),
        "gpt-4o",
        "gpt-4o",
        Some("account-id".to_string()),
        None,
        Some(AuthMode::ApiKey),
        true,
        "tty".to_string(),
        SessionSource::Cli,
    )
    .with_metrics_without_metadata_tags(metrics);

    manager.counter("codex.session_started", 1, &[("source", "tui")]);
    manager.shutdown_metrics()?;

    let resource_metrics = latest_metrics(&exporter);
    let metric =
        find_metric(&resource_metrics, "codex.session_started").expect("counter metric missing");
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

    let expected = BTreeMap::from([("source".to_string(), "tui".to_string())]);
    assert_eq!(attrs, expected);

    Ok(())
}
