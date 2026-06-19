use codex_otel::MetricsClient;
use codex_otel::MetricsConfig;
use codex_otel::Result;
use opentelemetry::KeyValue;
use opentelemetry_sdk::metrics::InMemoryMetricExporter;
use opentelemetry_sdk::metrics::data::Gauge;
use opentelemetry_sdk::metrics::data::Histogram;
use opentelemetry_sdk::metrics::data::Metric;
use opentelemetry_sdk::metrics::data::ResourceMetrics;
use opentelemetry_sdk::metrics::data::Sum;
use std::collections::BTreeMap;

pub(crate) fn build_metrics_with_defaults(
    default_tags: &[(&str, &str)],
) -> Result<(MetricsClient, InMemoryMetricExporter)> {
    let exporter = InMemoryMetricExporter::default();
    let mut config = MetricsConfig::in_memory(
        "test",
        "codex-cli",
        env!("CARGO_PKG_VERSION"),
        exporter.clone(),
    );
    for (key, value) in default_tags {
        config = config.with_tag(*key, *value)?;
    }
    let metrics = MetricsClient::new(config)?;
    Ok((metrics, exporter))
}

pub(crate) fn latest_metrics(exporter: &InMemoryMetricExporter) -> ResourceMetrics {
    exporter
        .get_finished_metrics()
        .expect("finished metrics should be available")
        .into_iter()
        .last()
        .expect("metrics export should exist")
}

pub(crate) fn find_metric<'a>(
    resource_metrics: &'a ResourceMetrics,
    name: &str,
) -> Option<&'a Metric> {
    for scope_metrics in &resource_metrics.scope_metrics {
        for metric in &scope_metrics.metrics {
            if metric.name == name {
                return Some(metric);
            }
        }
    }
    None
}

pub(crate) fn attributes_to_map<'a>(
    attributes: impl Iterator<Item = &'a KeyValue>,
) -> BTreeMap<String, String> {
    attributes
        .map(|kv| (kv.key.as_str().to_string(), kv.value.as_str().to_string()))
        .collect()
}

pub(crate) fn sum_u64(metric: &Metric) -> &Sum<u64> {
    metric
        .data
        .as_any()
        .downcast_ref()
        .expect("metric should contain a u64 sum")
}

pub(crate) fn histogram_f64(metric: &Metric) -> &Histogram<f64> {
    metric
        .data
        .as_any()
        .downcast_ref()
        .expect("metric should contain an f64 histogram")
}

pub(crate) fn gauge_i64(metric: &Metric) -> &Gauge<i64> {
    metric
        .data
        .as_any()
        .downcast_ref()
        .expect("metric should contain an i64 gauge")
}

pub(crate) fn histogram_data(
    resource_metrics: &ResourceMetrics,
    name: &str,
) -> (Vec<f64>, Vec<u64>, f64, u64) {
    let metric = find_metric(resource_metrics, name).expect("metric should exist");
    let histogram = histogram_f64(metric);
    assert_eq!(histogram.data_points.len(), 1);
    let point = &histogram.data_points[0];
    (
        point.bounds.clone(),
        point.bucket_counts.clone(),
        point.sum,
        point.count,
    )
}
