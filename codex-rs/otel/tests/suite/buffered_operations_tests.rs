//! Exercises global operation buffering across shutdown and rejected configuration changes.

use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use codex_otel::MetricsClient;
use codex_otel::MetricsConfig;
use codex_otel::OtelExporter;
use codex_otel::OtelHttpProtocol;
use codex_otel::OtelProvider;
use codex_otel::OtelSettings;
use codex_otel::install_global_metrics;
use codex_otel::record_global_operation;
use opentelemetry_sdk::metrics::data::AggregatedMetrics;
use opentelemetry_sdk::metrics::data::MetricData;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use std::time::Duration;

fn install() -> MetricsClient {
    install_global_metrics(
        MetricsClient::new(
            MetricsConfig::in_memory("test", "test", "1", Default::default()).with_runtime_reader(),
        )
        .expect("build in-memory metrics"),
    )
}

fn observe() {
    record_global_operation(
        "test.operations",
        "test.duration",
        Duration::from_millis(/*millis*/ 7),
        &[],
    )
    .expect("record an operation");
}

fn totals(metrics: &MetricsClient) -> (u64, u64, f64) {
    let snapshot = metrics.snapshot().expect("collect runtime metrics");
    let mut totals = (0, 0, 0.0);
    for metric in snapshot
        .scope_metrics()
        .flat_map(opentelemetry_sdk::metrics::data::ScopeMetrics::metrics)
    {
        match metric.data() {
            AggregatedMetrics::U64(MetricData::Sum(sum)) => {
                totals.0 += sum
                    .data_points()
                    .map(opentelemetry_sdk::metrics::data::SumDataPoint::value)
                    .sum::<u64>();
            }
            AggregatedMetrics::F64(MetricData::Histogram(histogram)) => {
                for point in histogram.data_points() {
                    totals.1 += point.count();
                    totals.2 += point.sum();
                }
            }
            _ => panic!("unexpected metric"),
        }
    }
    totals
}

#[test]
fn shutdown_buffers_until_replacement_and_preserves_newer_installations() {
    let first = install();
    first.shutdown().unwrap();
    observe();
    let second = install();
    assert_eq!(totals(&second), (1, 1, 7.0));

    let third = install();
    second.shutdown().unwrap();
    observe();
    assert_eq!(totals(&third), (1, 1, 7.0));
    third.shutdown().unwrap();
}

#[test]
fn rejected_opt_out_preserves_recording_and_accepted_opt_out_survives_shutdown() {
    let metrics = install();
    let mut settings = OtelSettings {
        http_client_factory: HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
        environment: "test".to_string(),
        service_name: "test".to_string(),
        service_version: "1".to_string(),
        codex_home: ".".into(),
        exporter: OtelExporter::None,
        trace_exporter: OtelExporter::OtlpHttp {
            endpoint: "http://127.0.0.1:1/v1/traces".to_string(),
            headers: Default::default(),
            protocol: OtelHttpProtocol::Json,
            tls: None,
        },
        metrics_exporter: OtelExporter::None,
        runtime_metrics: false,
        span_attributes: BTreeMap::from([(String::new(), "invalid".to_string())]),
        tracestate: BTreeMap::new(),
    };
    assert_eq!(
        OtelProvider::try_new(&settings).err().unwrap().to_string(),
        "configured span attribute key must not be empty"
    );
    observe();
    assert_eq!(totals(&metrics), (1, 1, 7.0));

    metrics.shutdown().unwrap();
    observe();
    assert!(OtelProvider::try_new(&settings).is_err());
    let replacement = install();
    assert_eq!(totals(&replacement), (1, 1, 7.0));
    replacement.shutdown().unwrap();

    settings.span_attributes.clear();
    for trace_exporter in [settings.trace_exporter.clone(), OtelExporter::None] {
        let metrics = install();
        settings.trace_exporter = trace_exporter;
        let _provider = OtelProvider::try_new(&settings).unwrap();
        metrics.shutdown().unwrap();
        observe();
        let replacement = install();
        assert_eq!(totals(&replacement), (0, 0, 0.0));
        replacement.shutdown().unwrap();
    }
}
