//! Admitted complete inputs count once per sampling step, including retry preparation.

use super::*;
use codex_otel::MetricsClient;
use codex_otel::MetricsConfig;
use opentelemetry_sdk::metrics::InMemoryMetricExporter;
use opentelemetry_sdk::metrics::data::AggregatedMetrics;
use opentelemetry_sdk::metrics::data::MetricData;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use std::sync::Arc;

#[tokio::test]
#[test_case::test_case(128_000; "admitted")]
#[test_case::test_case(1; "rejected")]
async fn request_histogram_counts_admitted_steps_without_counting_retry_preparation(window: i64) {
    let metrics = MetricsClient::new(
        MetricsConfig::in_memory(
            "test",
            "codex-core",
            env!("CARGO_PKG_VERSION"),
            InMemoryMetricExporter::default(),
        )
        .with_runtime_reader(),
    )
    .unwrap();
    let (session, mut turn) = crate::session::tests::make_session_and_context().await;
    turn.session_telemetry = turn.session_telemetry.clone().with_metrics(metrics.clone());
    Arc::make_mut(&mut turn.config).model_context_window = Some(window);
    let session = Arc::new(session);
    let turn = Arc::new(turn);
    let input = ["retained history", "new review"]
        .map(|role| {
            serde_json::from_value(serde_json::json!({
                "type": "message",
                "role": "user",
                "content": [{
                    "type": "input_text",
                    "text": format!("{role}: {}", "x".repeat(48_000)),
                }],
            }))
            .expect("valid user message")
        })
        .to_vec();
    let mut sum = 0;
    // Identical input in a fresh step must count again without an explicit reset.
    for _ in 0..2 {
        let step = session
            .capture_step_context(
                Arc::clone(&turn),
                &tokio_util::sync::CancellationToken::new(),
            )
            .await
            .unwrap();
        let mut prompt = crate::session::turn::build_prompt(
            input.clone(),
            &step,
            session.get_prompt_base_instructions().await,
            /*incremental_tools*/ false,
        );
        let metadata = session
            .responses_metadata(
                &step,
                crate::responses_metadata::CodexResponsesRequestKind::Turn,
            )
            .await;
        let request = session
            .services
            .model_client
            .build_responses_request(
                &prompt,
                &step.settings.model_info,
                /*effort*/ None,
                codex_protocol::config_types::ReasoningSummary::None,
                /*service_tier*/ None,
                &metadata,
                /*include_internal*/ true,
            )
            .unwrap();
        sum += estimate_request_tokens(&request);
        // The retry loop invokes Guardian preparation again on the same step.
        for _ in 0..2 {
            let result = prepare_prompt(&session, &mut prompt, &step, &metadata).await;
            if window > 1 {
                result.unwrap();
            } else {
                assert!(matches!(
                    result.unwrap_err().details(),
                    codex_protocol::error::CodexErrorDetails::ContextWindowExceeded
                ));
            }
        }
    }
    let snapshot = metrics.snapshot().unwrap();
    let mut histograms = BTreeMap::new();
    for metric in snapshot
        .scope_metrics()
        .flat_map(opentelemetry_sdk::metrics::data::ScopeMetrics::metrics)
        .filter(|metric| metric.name() == REQUEST_TOKENS_METRIC)
    {
        let AggregatedMetrics::F64(MetricData::Histogram(histogram)) = metric.data() else {
            panic!("expected request histogram")
        };
        for point in histogram.data_points() {
            let tags = point
                .attributes()
                .filter(|attr| matches!(attr.key.as_str(), "target" | "component" | "token_bucket"))
                .map(|attr| (attr.key.as_str().to_owned(), attr.value.to_string()))
                .collect::<BTreeMap<_, _>>();
            histograms.insert(tags, (point.count(), point.sum()));
        }
    }
    let expected = if window > 1 {
        BTreeMap::from([(
            BTreeMap::from([
                ("target".to_owned(), "sync".to_owned()),
                ("component".to_owned(), "total".to_owned()),
                ("token_bucket".to_owned(), "02_20k-40k".to_owned()),
            ]),
            (2, sum as f64),
        )])
    } else {
        BTreeMap::new()
    };
    assert_eq!(histograms, expected);
}
