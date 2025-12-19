use super::HistogramBuckets;
use super::MetricsBatch;
use super::MetricsClient;
use super::MetricsConfig;
use super::MetricsError;
use super::Result;
use opentelemetry::KeyValue;
use opentelemetry_sdk::metrics::InMemoryMetricExporter;
use opentelemetry_sdk::metrics::data::AggregatedMetrics;
use opentelemetry_sdk::metrics::data::Metric;
use opentelemetry_sdk::metrics::data::MetricData;
use opentelemetry_sdk::metrics::data::ResourceMetrics;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use std::time::Duration;

fn build_test_client() -> Result<(MetricsClient, InMemoryMetricExporter)> {
    let exporter = InMemoryMetricExporter::default();
    let config = MetricsConfig::new("test-key")
        .with_tag("service", "codex-cli")?
        .with_tag("env", "prod")?
        .with_in_memory_exporter(exporter.clone());
    let metrics = MetricsClient::new(config)?;
    Ok((metrics, exporter))
}

fn latest_metrics(exporter: &InMemoryMetricExporter) -> ResourceMetrics {
    let Ok(metrics) = exporter.get_finished_metrics() else {
        panic!("finished metrics error");
    };
    let Some(metrics) = metrics.into_iter().last() else {
        panic!("metrics export missing");
    };
    metrics
}

fn find_metric<'a>(resource_metrics: &'a ResourceMetrics, name: &str) -> Option<&'a Metric> {
    for scope_metrics in resource_metrics.scope_metrics() {
        for metric in scope_metrics.metrics() {
            if metric.name() == name {
                return Some(metric);
            }
        }
    }
    None
}

fn attributes_to_map<'a>(
    attributes: impl Iterator<Item = &'a KeyValue>,
) -> BTreeMap<String, String> {
    attributes
        .map(|kv| (kv.key.as_str().to_string(), kv.value.as_str().to_string()))
        .collect()
}

#[test]
// Ensures counters/histograms record with default + per-call tags.
fn send_builds_metrics_with_tags_and_histograms() -> Result<()> {
    let (metrics, exporter) = build_test_client()?;
    let buckets = HistogramBuckets::from_values(&[25, 50, 100])?;

    let mut batch = metrics.batch();
    batch.counter("codex.turns", 1, &[("model", "gpt-5.1"), ("env", "dev")])?;
    batch.histogram("codex.tool_latency", 25, &buckets, &[("tool", "shell")])?;
    metrics.send(batch)?;
    metrics.shutdown()?;

    let resource_metrics = latest_metrics(&exporter);

    let Some(counter_metric) = find_metric(&resource_metrics, "codex.turns") else {
        panic!("counter metric missing");
    };
    let attributes = match counter_metric.data() {
        AggregatedMetrics::I64(data) => match data {
            MetricData::Sum(sum) => {
                let points: Vec<_> = sum.data_points().collect();
                assert_eq!(points.len(), 1);
                let point = points[0];
                assert_eq!(point.value(), 1);
                attributes_to_map(point.attributes())
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
    assert_eq!(attributes, expected_counter_attributes);

    let Some(histogram_metric) = find_metric(&resource_metrics, "codex.tool_latency") else {
        panic!("histogram metric missing");
    };
    let attributes = match histogram_metric.data() {
        AggregatedMetrics::F64(data) => match data {
            MetricData::Histogram(histogram) => {
                let points: Vec<_> = histogram.data_points().collect();
                assert_eq!(points.len(), 1);
                let point = points[0];
                assert_eq!(point.count(), 1);
                assert_eq!(point.sum(), 25.0);
                attributes_to_map(point.attributes())
            }
            _ => panic!("unexpected histogram aggregation"),
        },
        _ => panic!("unexpected histogram data type"),
    };

    let expected_histogram_attributes = BTreeMap::from([
        ("service".to_string(), "codex-cli".to_string()),
        ("env".to_string(), "prod".to_string()),
        ("tool".to_string(), "shell".to_string()),
    ]);
    assert_eq!(attributes, expected_histogram_attributes);

    Ok(())
}

#[test]
// Ensures defaults merge per metric and overrides take precedence.
fn send_merges_default_tags_per_metric() -> Result<()> {
    let exporter = InMemoryMetricExporter::default();
    let config = MetricsConfig::new("test-key")
        .with_tag("service", "codex-cli")?
        .with_tag("env", "prod")?
        .with_tag("region", "us")?
        .with_in_memory_exporter(exporter.clone());
    let metrics = MetricsClient::new(config)?;

    let mut batch = metrics.batch();
    batch.counter("codex.alpha", 1, &[("env", "dev"), ("component", "alpha")])?;
    batch.counter(
        "codex.beta",
        2,
        &[("service", "worker"), ("component", "beta")],
    )?;
    metrics.send(batch)?;
    metrics.shutdown()?;

    let resource_metrics = latest_metrics(&exporter);

    let Some(alpha_metric) = find_metric(&resource_metrics, "codex.alpha") else {
        panic!("alpha metric missing");
    };
    let alpha_attributes = match alpha_metric.data() {
        AggregatedMetrics::I64(data) => match data {
            MetricData::Sum(sum) => {
                let points: Vec<_> = sum.data_points().collect();
                assert_eq!(points.len(), 1);
                attributes_to_map(points[0].attributes())
            }
            _ => panic!("unexpected alpha aggregation"),
        },
        _ => panic!("unexpected alpha data type"),
    };
    let expected_alpha_attributes = BTreeMap::from([
        ("service".to_string(), "codex-cli".to_string()),
        ("env".to_string(), "dev".to_string()),
        ("region".to_string(), "us".to_string()),
        ("component".to_string(), "alpha".to_string()),
    ]);
    assert_eq!(alpha_attributes, expected_alpha_attributes);

    let Some(beta_metric) = find_metric(&resource_metrics, "codex.beta") else {
        panic!("beta metric missing");
    };
    let beta_attributes = match beta_metric.data() {
        AggregatedMetrics::I64(data) => match data {
            MetricData::Sum(sum) => {
                let points: Vec<_> = sum.data_points().collect();
                assert_eq!(points.len(), 1);
                attributes_to_map(points[0].attributes())
            }
            _ => panic!("unexpected beta aggregation"),
        },
        _ => panic!("unexpected beta data type"),
    };
    let expected_beta_attributes = BTreeMap::from([
        ("service".to_string(), "worker".to_string()),
        ("env".to_string(), "prod".to_string()),
        ("region".to_string(), "us".to_string()),
        ("component".to_string(), "beta".to_string()),
    ]);
    assert_eq!(beta_attributes, expected_beta_attributes);

    Ok(())
}

#[test]
// Ensures duration recording maps to histogram output.
fn record_duration_uses_histogram() -> Result<()> {
    let (metrics, exporter) = build_test_client()?;
    let buckets = HistogramBuckets::from_values(&[10, 20])?;

    metrics.record_duration(
        "codex.request_latency",
        Duration::from_millis(15),
        &buckets,
        &[("route", "chat")],
    )?;
    metrics.shutdown()?;

    let resource_metrics = latest_metrics(&exporter);
    let Some(metric) = find_metric(&resource_metrics, "codex.request_latency") else {
        panic!("request latency histogram missing");
    };
    let attributes = match metric.data() {
        AggregatedMetrics::F64(data) => match data {
            MetricData::Histogram(histogram) => {
                let points: Vec<_> = histogram.data_points().collect();
                assert_eq!(points.len(), 1);
                let point = points[0];
                assert_eq!(point.count(), 1);
                assert_eq!(point.sum(), 15.0);
                attributes_to_map(point.attributes())
            }
            _ => panic!("unexpected histogram aggregation"),
        },
        _ => panic!("unexpected histogram data type"),
    };

    let expected_attributes = BTreeMap::from([
        ("service".to_string(), "codex-cli".to_string()),
        ("env".to_string(), "prod".to_string()),
        ("route".to_string(), "chat".to_string()),
    ]);
    assert_eq!(attributes, expected_attributes);

    Ok(())
}

#[test]
// Ensures time_result propagates errors but still records timing.
fn time_result_records_on_error() -> Result<()> {
    let (metrics, exporter) = build_test_client()?;
    let buckets = HistogramBuckets::from_values(&[10, 20])?;

    let Err(err) = metrics.time_result(
        "codex.request_latency",
        &buckets,
        &[("route", "chat")],
        || -> Result<&'static str> { Err(MetricsError::EmptyMetricName) },
    ) else {
        panic!("expected error");
    };
    assert!(matches!(err, MetricsError::EmptyMetricName));
    metrics.shutdown()?;

    let resource_metrics = latest_metrics(&exporter);
    let Some(metric) = find_metric(&resource_metrics, "codex.request_latency") else {
        panic!("request latency histogram missing");
    };
    match metric.data() {
        AggregatedMetrics::F64(data) => match data {
            MetricData::Histogram(histogram) => {
                let points: Vec<_> = histogram.data_points().collect();
                assert_eq!(points.len(), 1);
                assert_eq!(points[0].count(), 1);
            }
            _ => panic!("unexpected histogram aggregation"),
        },
        _ => panic!("unexpected histogram data type"),
    }

    Ok(())
}

#[test]
// Validates invalid tag components are rejected during config build.
fn invalid_tag_component_is_rejected() -> Result<()> {
    let Err(err) = MetricsConfig::default().with_tag("bad key", "value") else {
        panic!("expected error");
    };
    assert!(matches!(
        err,
        MetricsError::InvalidTagComponent { label, value }
            if label == "tag key" && value == "bad key"
    ));
    Ok(())
}

#[test]
// Ensures the reserved histogram bucketing tag key is rejected in config defaults.
fn reserved_tag_key_is_rejected_in_config() -> Result<()> {
    let Err(err) = MetricsConfig::default().with_tag("le", "10") else {
        panic!("expected error");
    };
    assert!(matches!(err, MetricsError::ReservedTagKey { key } if key == "le"));
    Ok(())
}

#[test]
// Ensures per-metric tag keys are validated.
fn counter_rejects_invalid_tag_key() {
    let mut batch = MetricsBatch::new();
    let Err(err) = batch.counter("codex.turns", 1, &[("bad key", "value")]) else {
        panic!("expected error");
    };
    assert!(matches!(
        err,
        MetricsError::InvalidTagComponent { label, value }
            if label == "tag key" && value == "bad key"
    ));
}

#[test]
// Ensures per-metric tag keys cannot use reserved histogram bucketing keys.
fn counter_rejects_reserved_tag_key() {
    let mut batch = MetricsBatch::new();
    let Err(err) = batch.counter("codex.turns", 1, &[("le", "10")]) else {
        panic!("expected error");
    };
    assert!(matches!(err, MetricsError::ReservedTagKey { key } if key == "le"));
}

#[test]
// Ensures per-metric tag values are validated.
fn histogram_rejects_invalid_tag_value() -> Result<()> {
    let mut batch = MetricsBatch::new();
    let buckets = HistogramBuckets::from_values(&[10])?;
    let Err(err) = batch.histogram(
        "codex.request_latency",
        3,
        &buckets,
        &[("route", "bad value")],
    ) else {
        panic!("expected error");
    };
    assert!(matches!(
        err,
        MetricsError::InvalidTagComponent { label, value }
            if label == "tag value" && value == "bad value"
    ));
    Ok(())
}

#[test]
// Ensures histogram calls reject reserved tag keys even though they no longer add `le`.
fn histogram_rejects_reserved_tag_key() -> Result<()> {
    let mut batch = MetricsBatch::new();
    let buckets = HistogramBuckets::from_values(&[10])?;
    let Err(err) = batch.histogram("codex.request_latency", 3, &buckets, &[("le", "10")]) else {
        panic!("expected error");
    };
    assert!(matches!(err, MetricsError::ReservedTagKey { key } if key == "le"));
    Ok(())
}

#[test]
// Ensures invalid metric names are rejected when building a batch.
fn counter_rejects_invalid_metric_name() -> Result<()> {
    let mut batch = MetricsBatch::new();
    let Err(err) = batch.counter("bad name", 1, &[]) else {
        panic!("expected error");
    };
    assert!(matches!(
        err,
        MetricsError::InvalidMetricName { name } if name == "bad name"
    ));
    Ok(())
}

#[test]
// Validates missing API key is rejected early.
fn empty_api_key_is_rejected() {
    let Err(err) = MetricsClient::new(MetricsConfig::new("")) else {
        panic!("expected error");
    };
    assert!(matches!(err, MetricsError::EmptyApiKey));
}

#[test]
// Validates missing endpoint is rejected early.
fn empty_endpoint_is_rejected() {
    let Err(err) = MetricsClient::new(MetricsConfig::new("test").with_endpoint("")) else {
        panic!("expected error");
    };
    assert!(matches!(err, MetricsError::EmptyEndpoint));
}
