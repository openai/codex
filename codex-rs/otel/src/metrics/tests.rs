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
use serde_json::Value;
use std::collections::BTreeMap;
use std::time::Duration;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

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

fn json_tags(value: &Value) -> BTreeMap<String, String> {
    value
        .as_object()
        .expect("tags should be an object")
        .iter()
        .map(|(key, value)| {
            let value = value
                .as_str()
                .unwrap_or_else(|| panic!("tag {key} should be a string"));
            (key.clone(), value.to_string())
        })
        .collect()
}

#[tokio::test]
// Sends metrics to a Statsig endpoint with merged tags and metadata.
async fn statsig_http_exporter_sends_events() -> Result<()> {
    let server = MockServer::start().await;
    let _mock = Mock::given(method("POST"))
        .and(path("/v1/log_event"))
        .and(header("statsig-api-key", "test-key"))
        .and(header("user-agent", "codex-test-agent"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;

    let config = MetricsConfig::new("test-key")
        .with_endpoint(format!("{}/v1/log_event", server.uri()))
        .with_user_agent("codex-test-agent")
        .with_tag("service", "codex-cli")?
        .with_tag("env", "prod")?;
    let metrics = MetricsClient::new(config)?;

    metrics.counter("codex.turns", 1, &[("model", "gpt-5.1")])?;
    metrics.histogram("codex.tool_latency", 25, &[("tool", "shell")])?;
    metrics.shutdown()?;

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);

    let body: Value = serde_json::from_slice(&requests[0].body).unwrap();
    let events = body
        .get("events")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    assert_eq!(events.len(), 2);

    let statsig_metadata = body
        .get("statsigMetadata")
        .and_then(Value::as_object)
        .expect("statsig metadata missing");
    assert_eq!(
        statsig_metadata.get("sdkType").and_then(Value::as_str),
        Some("codex-otel-rust")
    );
    assert_eq!(
        statsig_metadata.get("sdkVersion").and_then(Value::as_str),
        Some(env!("CARGO_PKG_VERSION"))
    );

    let mut events_by_name = BTreeMap::new();
    for event in events {
        let name = event
            .get("eventName")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        events_by_name.insert(name, event);
    }

    let counter = events_by_name
        .get("codex.turns")
        .expect("counter event missing");
    assert_eq!(counter.get("value").and_then(Value::as_f64), Some(1.0));
    let counter_metadata = counter.get("metadata").expect("counter metadata missing");
    let expected_counter_metadata = BTreeMap::from([
        ("metric_type".to_string(), "counter".to_string()),
        ("service".to_string(), "codex-cli".to_string()),
        ("env".to_string(), "prod".to_string()),
        ("model".to_string(), "gpt-5.1".to_string()),
    ]);
    assert_eq!(json_tags(counter_metadata), expected_counter_metadata);

    let histogram = events_by_name
        .get("codex.tool_latency")
        .expect("histogram event missing");
    assert_eq!(histogram.get("value").and_then(Value::as_f64), Some(25.0));
    let histogram_metadata = histogram
        .get("metadata")
        .expect("histogram metadata missing");
    let expected_histogram_metadata = BTreeMap::from([
        ("metric_type".to_string(), "histogram".to_string()),
        ("service".to_string(), "codex-cli".to_string()),
        ("env".to_string(), "prod".to_string()),
        ("tool".to_string(), "shell".to_string()),
    ]);
    assert_eq!(json_tags(histogram_metadata), expected_histogram_metadata);

    Ok(())
}

#[test]
// Ensures counters/histograms record with default + per-call tags.
fn send_builds_metrics_with_tags_and_histograms() -> Result<()> {
    let (metrics, exporter) = build_test_client()?;

    metrics.counter("codex.turns", 1, &[("model", "gpt-5.1"), ("env", "dev")])?;
    metrics.histogram("codex.tool_latency", 25, &[("tool", "shell")])?;
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

    metrics.counter("codex.alpha", 1, &[("env", "dev"), ("component", "alpha")])?;
    metrics.counter(
        "codex.beta",
        2,
        &[("service", "worker"), ("component", "beta")],
    )?;
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

    metrics.record_duration(
        "codex.request_latency",
        Duration::from_millis(15),
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

    let Err(err) = metrics.time_result(
        "codex.request_latency",
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
// Ensures per-metric tag keys are validated.
fn counter_rejects_invalid_tag_key() -> Result<()> {
    let (metrics, _exporter) = build_test_client()?;
    let Err(err) = metrics.counter("codex.turns", 1, &[("bad key", "value")]) else {
        panic!("expected error");
    };
    assert!(matches!(
        err,
        MetricsError::InvalidTagComponent { label, value }
            if label == "tag key" && value == "bad key"
    ));
    metrics.shutdown()?;
    Ok(())
}

#[test]
// Ensures per-metric tag values are validated.
fn histogram_rejects_invalid_tag_value() -> Result<()> {
    let (metrics, _exporter) = build_test_client()?;
    let Err(err) = metrics.histogram("codex.request_latency", 3, &[("route", "bad value")]) else {
        panic!("expected error");
    };
    assert!(matches!(
        err,
        MetricsError::InvalidTagComponent { label, value }
            if label == "tag value" && value == "bad value"
    ));
    metrics.shutdown()?;
    Ok(())
}

#[test]
// Ensures invalid metric names are rejected.
fn counter_rejects_invalid_metric_name() -> Result<()> {
    let (metrics, _exporter) = build_test_client()?;
    let Err(err) = metrics.counter("bad name", 1, &[]) else {
        panic!("expected error");
    };
    assert!(matches!(
        err,
        MetricsError::InvalidMetricName { name } if name == "bad name"
    ));
    metrics.shutdown()?;
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
