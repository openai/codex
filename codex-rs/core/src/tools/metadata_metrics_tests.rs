use super::*;
use codex_otel::MetricsConfig;
use opentelemetry_sdk::metrics::InMemoryMetricExporter;
use opentelemetry_sdk::metrics::data::AggregatedMetrics;
use opentelemetry_sdk::metrics::data::MetricData;
use opentelemetry_sdk::metrics::data::ScopeMetrics;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;

fn metrics() -> MetricsClient {
    MetricsClient::new(
        MetricsConfig::in_memory(
            "test",
            "codex-core",
            env!("CARGO_PKG_VERSION"),
            InMemoryMetricExporter::default(),
        )
        .with_runtime_reader(),
    )
    .expect("in-memory metrics")
}

fn samples(metrics: &MetricsClient) -> BTreeMap<String, (u64, f64)> {
    let snapshot = metrics.snapshot().expect("metrics snapshot");
    let mut samples = BTreeMap::new();
    for metric in snapshot.scope_metrics().flat_map(ScopeMetrics::metrics) {
        assert_eq!(metric.name(), SHED_BYTES_METRIC);
        let AggregatedMetrics::F64(MetricData::Histogram(histogram)) = metric.data() else {
            panic!("expected shed-byte histogram");
        };
        for point in histogram.data_points() {
            let attributes = point.attributes().collect::<Vec<_>>();
            assert_eq!(attributes.len(), 1);
            assert_eq!(attributes[0].key.as_str(), "stage");
            samples.insert(
                attributes[0].value.as_str().to_string(),
                (point.count(), point.sum()),
            );
        }
    }
    samples
}

#[test]
fn direct_retention_reports_only_positive_known_shedding() {
    let metrics = metrics();
    for (before, after) in [
        (0, 0),
        (10, 10),
        (10, 20),
        (usize::MAX, 0),
        (10, usize::MAX),
    ] {
        record_shedding("direct_retained", before, after, Some(&metrics));
    }
    record_shedding(
        "direct_retained",
        /*before*/ 100,
        /*after*/ 1,
        /*metrics*/ None,
    );
    assert_eq!(samples(&metrics), BTreeMap::new());
    record_shedding(
        "direct_retained",
        /*before*/ 100,
        /*after*/ 27,
        Some(&metrics),
    );
    assert_eq!(
        samples(&metrics),
        BTreeMap::from([("direct_retained".to_string(), (1, 73.0))]),
    );
}
