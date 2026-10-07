//! Verifies registration telemetry on real sampling requests, including stream retries.

use anyhow::Result;
use codex_core::StartThreadOptions;
use codex_core::TurnInputRequest;
use codex_features::Feature;
use codex_otel::MetricsClient;
use codex_otel::MetricsConfig;
use codex_otel::TOOL_REGISTRATIONS_METRIC;
use codex_protocol::config_types::WebSearchMode;
use codex_protocol::dynamic_tools::DynamicToolFunctionSpec;
use codex_protocol::dynamic_tools::DynamicToolNamespaceSpec;
use codex_protocol::dynamic_tools::DynamicToolNamespaceTool;
use codex_protocol::dynamic_tools::DynamicToolSpec;
use codex_protocol::openai_models::ToolMode;
use codex_protocol::protocol::EventMsg;
use codex_protocol::user_input::UserInput;
use core_test_support::responses;
use core_test_support::responses::namespace_child_tool;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use opentelemetry_sdk::metrics::InMemoryMetricExporter;
use opentelemetry_sdk::metrics::data::AggregatedMetrics;
use opentelemetry_sdk::metrics::data::MetricData;
use opentelemetry_sdk::metrics::data::ScopeMetrics;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::collections::BTreeMap;

/// Transport retries must not inflate registrations or include hosted tools in the inventory.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sampling_records_tool_registrations_once_across_stream_retry() -> Result<()> {
    skip_if_no_network!(Ok(()));
    const MODEL: &str = "tool-registrations-model";
    let server = responses::start_mock_server().await;
    let response = responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![responses::ev_response_created("incomplete")]),
            responses::sse(vec![
                responses::ev_response_created("complete"),
                responses::ev_completed("complete"),
            ]),
        ],
    )
    .await;
    let metrics = MetricsClient::new(
        MetricsConfig::in_memory(
            "test",
            "tool-registrations",
            env!("CARGO_PKG_VERSION"),
            InMemoryMetricExporter::default(),
        )
        .with_runtime_reader(),
    )?;
    let test = test_codex()
        .with_model(MODEL)
        .with_model_info_override(MODEL, |model| {
            model.supports_search_tool = true;
            model.use_responses_lite = false;
            model.tool_mode = Some(ToolMode::Direct);
            model.experimental_supported_tools.clear();
        })
        .with_config(|config| {
            // Keep request_user_input as the only model-only registration.
            for feature in [
                Feature::Collab,
                Feature::SleepTool,
                Feature::TokenBudget,
                Feature::SendMessageToUserAsync,
            ] {
                config.features.disable(feature).expect("disable feature");
            }
            config.experimental_request_user_input_enabled = true;
            config
                .web_search_mode
                .set(WebSearchMode::Cached)
                .expect("enable hosted web search");
            config.model_provider.request_max_retries = Some(0);
            config.model_provider.stream_max_retries = Some(1);
        })
        .build_with_auto_env(&server)
        .await?;
    let input_schema = json!({
        "type": "object",
        "properties": {},
        "additionalProperties": false,
    });
    let mut options = StartThreadOptions {
        dynamic_tools: vec![DynamicToolSpec::Namespace(DynamicToolNamespaceSpec {
            name: "workspace_tools".to_string(),
            description: "Tools for this workspace.".to_string(),
            tools: [("status", false), ("search", true)]
                .into_iter()
                .map(|(name, defer_loading)| {
                    DynamicToolNamespaceTool::Function(DynamicToolFunctionSpec {
                        name: name.to_string(),
                        description: format!("Workspace {name}."),
                        input_schema: input_schema.clone(),
                        defer_loading,
                    })
                })
                .collect(),
        })],
        ..StartThreadOptions::new(test.config.clone())
    };
    options.thread_extension_init.insert(metrics.clone());
    let thread = test.thread_manager.start_thread(options).await?.thread;
    thread
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "Describe the available workspace tools.".to_string(),
            text_elements: Vec::new(),
        }]))
        .await?;
    let EventMsg::TurnComplete(completed) =
        wait_for_event(&thread, |event| matches!(event, EventMsg::TurnComplete(_))).await
    else {
        unreachable!("event predicate guarantees turn completion");
    };
    assert_eq!(completed.error, None);

    let requests = response.requests();
    assert_eq!(requests.len(), 2, "incomplete stream must retry");
    let body = requests[0].body_json();
    assert_eq!(requests[1].body_json()["tools"], body["tools"]);
    assert!(namespace_child_tool(&body, "workspace_tools", "status").is_some());
    assert_eq!(
        namespace_child_tool(&body, "workspace_tools", "search"),
        None
    );
    let tools = body["tools"].as_array().expect("request tools");
    assert!(tools.iter().any(|tool| tool["type"] == "web_search"));
    assert!(
        tools
            .iter()
            .any(|tool| tool["name"] == "request_user_input")
    );
    // The direct-mode request exposes every direct registration, plus the one
    // model-only registration and hosted web search. Namespace children count separately.
    let direct_count = tools
        .iter()
        .filter(|tool| tool["type"] != "web_search")
        .map(|tool| {
            tool.get("tools")
                .and_then(Value::as_array)
                .map_or(1, Vec::len)
        })
        .sum::<usize>()
        - 1;

    let snapshot = metrics.snapshot()?;
    let mut samples = BTreeMap::new();
    for metric in snapshot.scope_metrics().flat_map(ScopeMetrics::metrics) {
        if metric.name() != TOOL_REGISTRATIONS_METRIC {
            continue;
        }
        let AggregatedMetrics::F64(MetricData::Histogram(histogram)) = metric.data() else {
            panic!("tool registration metrics must be histograms");
        };
        for point in histogram.data_points() {
            let attributes = point
                .attributes()
                .map(|attribute| (attribute.key.as_str(), attribute.value.as_str().to_string()))
                .collect::<BTreeMap<_, _>>();
            assert_eq!(attributes.get("model").map(String::as_str), Some(MODEL));
            assert_eq!(attributes["tool_mode"], "direct");
            assert!(
                attributes.keys().all(|key| matches!(
                    *key,
                    "auth_mode"
                        | "session_source"
                        | "originator"
                        | "service_name"
                        | "model"
                        | "app.version"
                        | "exposure"
                        | "tool_mode"
                )),
                "registration telemetry must not label tool names or descriptions"
            );
            assert_eq!(
                samples.insert(attributes["exposure"].clone(), (point.count(), point.sum()),),
                None,
            );
        }
    }
    assert_eq!(
        samples,
        BTreeMap::from([
            ("direct".to_string(), (1, direct_count as f64)),
            ("direct_model_only".to_string(), (1, 1.0)),
            ("deferred".to_string(), (1, 1.0)),
            ("deferred_model_only".to_string(), (1, 0.0)),
            ("code_mode_only".to_string(), (1, 0.0)),
            ("hidden".to_string(), (1, 0.0)),
        ])
    );
    metrics.shutdown()?;
    Ok(())
}
