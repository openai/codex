//! Verifies reported CoT removals through both sampling transports and the metrics exporter.

use anyhow::Result;
use codex_otel::MetricsClient;
use codex_otel::MetricsConfig;
use codex_otel::REASONING_ITEMS_REMOVED_METRIC;
use codex_otel::REASONING_REPLAY_COUNT_METRIC;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use opentelemetry_sdk::metrics::InMemoryMetricExporter;
use opentelemetry_sdk::metrics::data::AggregatedMetrics;
use opentelemetry_sdk::metrics::data::MetricData;
use opentelemetry_sdk::metrics::data::ScopeMetrics;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::collections::BTreeMap;

#[test_case::test_case(false; "sse")]
#[test_case::test_case(true; "websocket")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reasoning_replay_metrics_preserve_report_availability(use_websocket: bool) -> Result<()> {
    skip_if_no_network!(Ok(()));
    let reports = [
        None,
        Some(json!([])),
        Some(json!([
            {"type": "reasoning", "id": "rs_same", "action": "removed", "reason": "prefix_mismatch"},
            {"type": "reasoning", "id": "rs_same", "action": "removed", "reason": "prefix_mismatch"},
            {"type": "reasoning", "action": "removed", "reason": "organization_mismatch"},
            {"type": "reasoning", "action": "removed", "reason": "end_user_mismatch"},
            {"type": "reasoning", "action": "removed", "reason": "model_mismatch"},
            {"type": "reasoning", "action": "removed", "reason": "turn_boundary"},
            {"type": "reasoning", "action": "removed", "reason": "future-private-reason"},
            {"type": "reasoning", "action": "removed"},
            {"type": "message", "action": "removed", "reason": "prefix_mismatch"},
            {"type": "reasoning", "action": "changed", "reason": "prefix_mismatch"},
            null
        ])),
        Some(json!([{"type": "reasoning", "action": "removed", "reason": "turn_boundary"}])),
        Some(json!(null)),
        Some(json!({"malformed": true})),
    ];
    let events = reports
        .iter()
        .enumerate()
        .map(|(index, report)| {
            let id = format!("resp-{index}");
            let mut completed = responses::ev_completed(&id);
            if let Some(report) = report {
                completed["response"]["input_transformations"] = report.clone();
            }
            let mut in_progress = completed.clone();
            in_progress["type"] = json!("response.in_progress");
            vec![responses::ev_response_created(&id), in_progress, completed]
        })
        .collect::<Vec<_>>();
    let server = responses::start_mock_server().await;
    let http_requests = if use_websocket {
        None
    } else {
        Some(
            responses::mount_sse_sequence(
                &server,
                events.iter().cloned().map(responses::sse).collect(),
            )
            .await,
        )
    };
    let websocket_server = if use_websocket {
        let mut frames = vec![vec![
            responses::ev_response_created("prewarm"),
            responses::ev_completed("prewarm"),
        ]];
        frames.extend(events);
        Some(responses::start_websocket_server(vec![frames]).await)
    } else {
        None
    };
    let metrics = MetricsClient::new(
        MetricsConfig::in_memory(
            "test",
            "reasoning-replay",
            env!("CARGO_PKG_VERSION"),
            InMemoryMetricExporter::default(),
        )
        .with_runtime_reader(),
    )?;
    let mut builder = test_codex().with_config(|config| {
        config.apps_mcp_product_sku = Some("codex".to_string());
    });
    let mut test = builder.build_with_auto_env(&server).await?;
    let mut options = test.start_thread_options().await;
    test.codex.shutdown_and_wait().await?;
    // Only the instrumented thread may prewarm the scripted WebSocket connection.
    if let Some(server) = &websocket_server {
        options.config.model_provider.base_url = Some(format!("{}/v1", server.uri()));
        options.config.model_provider.supports_websockets = true;
    }
    options.thread_extension_init.insert(metrics.clone());
    test.codex = test.thread_manager.start_thread(options).await?.thread;
    for _ in &reports {
        test.submit_text_turn("continue").await?;
    }
    if let Some(requests) = http_requests {
        assert_eq!(requests.requests().len(), reports.len());
    }
    let snapshot = metrics.snapshot()?;
    let mut reports = BTreeMap::new();
    let mut removals = BTreeMap::new();
    for metric in snapshot.scope_metrics().flat_map(ScopeMetrics::metrics) {
        match metric.name() {
            REASONING_REPLAY_COUNT_METRIC => {
                let AggregatedMetrics::U64(MetricData::Sum(sum)) = metric.data() else {
                    panic!("report availability must be a counter");
                };
                for point in sum.data_points() {
                    let tags = point
                        .attributes()
                        .map(|attribute| {
                            (attribute.key.as_str(), attribute.value.as_str().to_string())
                        })
                        .collect::<BTreeMap<_, _>>();
                    assert_eq!(tags["product_sku"], "codex");
                    reports.insert(tags["report"].clone(), point.value());
                }
            }
            REASONING_ITEMS_REMOVED_METRIC => {
                let AggregatedMetrics::F64(MetricData::Histogram(histogram)) = metric.data() else {
                    panic!("reasoning removal counts must be histograms");
                };
                for point in histogram.data_points() {
                    let tags = point
                        .attributes()
                        .map(|attribute| {
                            (attribute.key.as_str(), attribute.value.as_str().to_string())
                        })
                        .collect::<BTreeMap<_, _>>();
                    assert_eq!(tags["product_sku"], "codex");
                    removals.insert(
                        tags["reason"].clone(),
                        (point.count(), point.sum(), point.min(), point.max()),
                    );
                }
            }
            _ => {}
        }
    }
    assert_eq!(
        reports,
        BTreeMap::from([
            ("unavailable".to_string(), 3),
            ("no_removals_reported".to_string(), 1),
            ("prefix_mismatch_reported".to_string(), 1),
            ("removals_reported".to_string(), 1),
        ])
    );
    // Three usable reports contribute to every reason, including their zero counts.
    // Missing, null, and malformed reports contribute no histogram observations.
    assert_eq!(
        removals,
        BTreeMap::from([
            (
                "prefix_mismatch".to_string(),
                (3, 2.0, Some(0.0), Some(2.0))
            ),
            (
                "organization_mismatch".to_string(),
                (3, 1.0, Some(0.0), Some(1.0))
            ),
            (
                "end_user_mismatch".to_string(),
                (3, 1.0, Some(0.0), Some(1.0))
            ),
            ("model_mismatch".to_string(), (3, 1.0, Some(0.0), Some(1.0))),
            ("turn_boundary".to_string(), (3, 2.0, Some(0.0), Some(1.0))),
            ("other".to_string(), (3, 2.0, Some(0.0), Some(2.0))),
        ])
    );
    test.codex.shutdown_and_wait().await?;
    if let Some(server) = websocket_server {
        server.shutdown().await;
    }
    Ok(())
}
