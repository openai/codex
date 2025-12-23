use codex_otel::metrics::MetricsClient;
use codex_otel::metrics::MetricsConfig;
use codex_otel::metrics::Result;
use opentelemetry::KeyValue;
use opentelemetry_sdk::metrics::InMemoryMetricExporter;
use opentelemetry_sdk::metrics::data::AggregatedMetrics;
use opentelemetry_sdk::metrics::data::Metric;
use opentelemetry_sdk::metrics::data::MetricData;
use opentelemetry_sdk::metrics::data::ResourceMetrics;
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
    let Ok(metrics) = exporter.get_finished_metrics() else {
        panic!("finished metrics error");
    };
    let Some(metrics) = metrics.into_iter().last() else {
        panic!("metrics export missing");
    };
    metrics
}

pub(crate) fn find_metric<'a>(
    resource_metrics: &'a ResourceMetrics,
    name: &str,
) -> Option<&'a Metric> {
    for scope_metrics in resource_metrics.scope_metrics() {
        for metric in scope_metrics.metrics() {
            if metric.name() == name {
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

pub(crate) fn histogram_data(
    resource_metrics: &ResourceMetrics,
    name: &str,
) -> (Vec<f64>, Vec<u64>, f64, u64) {
    let metric =
        find_metric(resource_metrics, name).unwrap_or_else(|| panic!("metric {name} missing"));
    match metric.data() {
        AggregatedMetrics::F64(data) => match data {
            MetricData::Histogram(histogram) => {
                let points: Vec<_> = histogram.data_points().collect();
                assert_eq!(points.len(), 1);
                let point = points[0];
                let bounds = point.bounds().collect();
                let bucket_counts = point.bucket_counts().collect();
                (bounds, bucket_counts, point.sum(), point.count())
            }
            _ => panic!("unexpected histogram aggregation"),
        },
        _ => panic!("unexpected metric data type"),
    }
}
