use super::*;
use codex_otel::MetricsConfig;
use codex_protocol::models::ExecutedToolCall;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::models::ToolResultMetadata;
use codex_protocol::models::bound_executed_tool_calls_for_prompt;
use codex_protocol::models::bound_executed_tool_calls_for_prompt_prioritizing_recent;
use opentelemetry_sdk::metrics::InMemoryMetricExporter;
use opentelemetry_sdk::metrics::data::AggregatedMetrics;
use opentelemetry_sdk::metrics::data::MetricData;
use opentelemetry_sdk::metrics::data::ScopeMetrics;
use pretty_assertions::assert_eq;
use serde_json::json;
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
fn prompt_reports_each_budget_application_without_changing_output() {
    let metrics = metrics();
    let original = (0..96)
        .map(|index| {
            let mut item = ResponseItem::from(ResponseInputItem::FunctionCallOutput {
                call_id: format!("call_{index}"),
                output: FunctionCallOutputPayload::from_text("unchanged output".to_string()),
            });
            let mut call = ExecutedToolCall::new("mcp__apps__read".to_string(), json!({}));
            call.set_tool_result_metadata(ToolResultMetadata::new(&json!({
                "openai/resource_access": {"resources": ["x".repeat(24_000)]},
            })));
            item.append_executed_tool_calls(vec![call]);
            item.mark_tool_calls_complete();
            item
        })
        .collect::<Vec<_>>();
    let mut expected_samples = BTreeMap::new();
    for (stage, bound) in [
        (
            "request",
            bound_executed_tool_calls_for_prompt as fn(&mut [ResponseItem]),
        ),
        (
            "retained",
            bound_executed_tool_calls_for_prompt_prioritizing_recent,
        ),
        (
            "compaction",
            bound_executed_tool_calls_for_prompt_prioritizing_recent,
        ),
    ] {
        let mut expected = original.clone();
        bound(&mut expected);
        let shed = metadata_bytes(&original) - metadata_bytes(&expected);
        assert!(shed > 0);
        let mut actual = original.clone();
        bound_prompt_metadata(&mut actual, bound, stage, Some(&metrics));
        assert_eq!(actual, expected);
        // Reapplying an already-satisfied budget must not add a zero-byte sample.
        bound_prompt_metadata(&mut actual, bound, stage, Some(&metrics));
        assert_eq!(actual, expected);
        let mut without_metrics = original.clone();
        bound_prompt_metadata(&mut without_metrics, bound, stage, /*metrics*/ None);
        assert_eq!(without_metrics, expected);
        expected_samples.insert(stage.to_string(), (1, shed as f64));
    }
    assert_eq!(samples(&metrics), expected_samples);
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
