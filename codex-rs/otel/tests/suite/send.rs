use crate::harness::attributes_to_map;
use crate::harness::build_metrics_with_defaults;
use crate::harness::find_metric;
use crate::harness::histogram_data;
use crate::harness::latest_metrics;
use codex_otel::metrics::Result;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;

// Ensures counters/histograms render with default + per-call tags.
#[test]
fn send_builds_payload_with_tags_and_histograms() -> Result<()> {
    let (metrics, exporter) =
        build_metrics_with_defaults(&[("service", "codex-cli"), ("env", "prod")])?;

    metrics.counter("codex.turns", 1, &[("model", "gpt-5.1"), ("env", "dev")])?;
    metrics.histogram("codex.tool_latency", 25, &[("tool", "shell")])?;
    metrics.shutdown()?;

    let resource_metrics = latest_metrics(&exporter);

    let counter = find_metric(&resource_metrics, "codex.turns").expect("counter metric missing");
    let counter_attributes = match counter.data() {
        opentelemetry_sdk::metrics::data::AggregatedMetrics::U64(data) => match data {
            opentelemetry_sdk::metrics::data::MetricData::Sum(sum) => {
                let points: Vec<_> = sum.data_points().collect();
                assert_eq!(points.len(), 1);
                assert_eq!(points[0].value(), 1);
                attributes_to_map(points[0].attributes())
            }
            _ => panic!("unexpected counter aggregation"),
        },
        _ => panic!("unexpected counter data type"),
    };

    let expected_counter_attributes = BTreeMap::from([
        ("service".to_string(), "codex-cli".to_string()),
        ("env".to_string(), "dev".to_string()),
        ("model".to_string(), "gpt-5.1".to_string()),
    ]);
    assert_eq!(counter_attributes, expected_counter_attributes);

    let (bounds, bucket_counts, sum, count) =
        histogram_data(&resource_metrics, "codex.tool_latency");
    assert!(!bounds.is_empty());
    assert_eq!(bucket_counts.iter().sum::<u64>(), 1);
    assert_eq!(sum, 25.0);
    assert_eq!(count, 1);

    let histogram_attrs = attributes_to_map(
        match find_metric(&resource_metrics, "codex.tool_latency").and_then(|metric| {
            match metric.data() {
                opentelemetry_sdk::metrics::data::AggregatedMetrics::F64(
                    opentelemetry_sdk::metrics::data::MetricData::Histogram(histogram),
                ) => histogram
                    .data_points()
                    .next()
                    .map(opentelemetry_sdk::metrics::data::HistogramDataPoint::attributes),
                _ => None,
            }
        }) {
            Some(attrs) => attrs,
            None => panic!("histogram attributes missing"),
        },
    );
    let expected_histogram_attributes = BTreeMap::from([
        ("service".to_string(), "codex-cli".to_string()),
        ("env".to_string(), "prod".to_string()),
        ("tool".to_string(), "shell".to_string()),
    ]);
    assert_eq!(histogram_attrs, expected_histogram_attributes);

    Ok(())
}

// Ensures defaults merge per line and overrides take precedence.
#[test]
fn send_merges_default_tags_per_line() -> Result<()> {
    let (metrics, exporter) = build_metrics_with_defaults(&[
        ("service", "codex-cli"),
        ("env", "prod"),
        ("region", "us"),
    ])?;

    metrics.counter("codex.alpha", 1, &[("env", "dev"), ("component", "alpha")])?;
    metrics.counter(
        "codex.beta",
        2,
        &[("service", "worker"), ("component", "beta")],
    )?;
    metrics.shutdown()?;

    let resource_metrics = latest_metrics(&exporter);
    let alpha_metric =
        find_metric(&resource_metrics, "codex.alpha").expect("codex.alpha metric missing");
    let alpha_point = match alpha_metric.data() {
        opentelemetry_sdk::metrics::data::AggregatedMetrics::U64(data) => match data {
            opentelemetry_sdk::metrics::data::MetricData::Sum(sum) => {
                let points: Vec<_> = sum.data_points().collect();
                assert_eq!(points.len(), 1);
                points[0]
            }
            _ => panic!("unexpected counter aggregation"),
        },
        _ => panic!("unexpected counter data type"),
    };
    assert_eq!(alpha_point.value(), 1);
    let alpha_attrs = attributes_to_map(alpha_point.attributes());
    let expected_alpha_attrs = BTreeMap::from([
        ("component".to_string(), "alpha".to_string()),
        ("env".to_string(), "dev".to_string()),
        ("region".to_string(), "us".to_string()),
        ("service".to_string(), "codex-cli".to_string()),
    ]);
    assert_eq!(alpha_attrs, expected_alpha_attrs);

    let beta_metric =
        find_metric(&resource_metrics, "codex.beta").expect("codex.beta metric missing");
    let beta_point = match beta_metric.data() {
        opentelemetry_sdk::metrics::data::AggregatedMetrics::U64(data) => match data {
            opentelemetry_sdk::metrics::data::MetricData::Sum(sum) => {
                let points: Vec<_> = sum.data_points().collect();
                assert_eq!(points.len(), 1);
                points[0]
            }
            _ => panic!("unexpected counter aggregation"),
        },
        _ => panic!("unexpected counter data type"),
    };
    assert_eq!(beta_point.value(), 2);
    let beta_attrs = attributes_to_map(beta_point.attributes());
    let expected_beta_attrs = BTreeMap::from([
        ("component".to_string(), "beta".to_string()),
        ("env".to_string(), "prod".to_string()),
        ("region".to_string(), "us".to_string()),
        ("service".to_string(), "worker".to_string()),
    ]);
    assert_eq!(beta_attrs, expected_beta_attrs);

    Ok(())
}

// Verifies enqueued metrics are delivered by the background worker.
#[test]
fn client_sends_enqueued_metric() -> Result<()> {
    let (metrics, exporter) = build_metrics_with_defaults(&[])?;

    metrics.counter("codex.turns", 1, &[("model", "gpt-5.1")])?;
    metrics.shutdown()?;

    let resource_metrics = latest_metrics(&exporter);
    let counter = find_metric(&resource_metrics, "codex.turns").expect("counter metric missing");
    let points = match counter.data() {
        opentelemetry_sdk::metrics::data::AggregatedMetrics::U64(data) => match data {
            opentelemetry_sdk::metrics::data::MetricData::Sum(sum) => {
                sum.data_points().collect::<Vec<_>>()
            }
            _ => panic!("unexpected counter aggregation"),
        },
        _ => panic!("unexpected counter data type"),
    };
    assert_eq!(points.len(), 1);
    let point = points[0];
    assert_eq!(point.value(), 1);
    let attrs = attributes_to_map(point.attributes());
    assert_eq!(attrs.get("model").map(String::as_str), Some("gpt-5.1"));

    Ok(())
}

// Ensures shutdown flushes successfully with in-memory exporters.
#[test]
fn shutdown_flushes_in_memory_exporter() -> Result<()> {
    let (metrics, exporter) = build_metrics_with_defaults(&[])?;

    metrics.counter("codex.turns", 1, &[])?;
    metrics.shutdown()?;

    let resource_metrics = latest_metrics(&exporter);
    let counter = find_metric(&resource_metrics, "codex.turns").expect("counter metric missing");
    let points = match counter.data() {
        opentelemetry_sdk::metrics::data::AggregatedMetrics::U64(data) => match data {
            opentelemetry_sdk::metrics::data::MetricData::Sum(sum) => {
                sum.data_points().collect::<Vec<_>>()
            }
            _ => panic!("unexpected counter aggregation"),
        },
        _ => panic!("unexpected counter data type"),
    };
    assert_eq!(points.len(), 1);

    Ok(())
}

// Ensures shutting down without recording metrics does not export anything.
#[test]
fn shutdown_without_metrics_exports_nothing() -> Result<()> {
    let (metrics, exporter) = build_metrics_with_defaults(&[])?;

    metrics.shutdown()?;

    let finished = exporter.get_finished_metrics().unwrap();
    assert!(finished.is_empty(), "expected no metrics exported");
    Ok(())
}
