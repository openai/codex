//! Direct-call metadata coverage, including malformed calls and metadata budgets.

use anyhow::Result;
use codex_features::Feature;
use codex_mcp::CODEX_APPS_MCP_SERVER_NAME;
use codex_model_provider::RemoteCompactionSupport;
use core_test_support::apps_test_server::AppsTestServer;
use core_test_support::apps_test_server::SEARCH_CALENDAR_LIST_TOOL;
use core_test_support::apps_test_server::SEARCH_CALENDAR_NAMESPACE;
use core_test_support::apps_test_server::recorded_apps_tool_calls;
use core_test_support::apps_test_server::search_capable_apps_builder;
use core_test_support::responses::ev_function_call_with_namespace;
use core_test_support::wait_for_mcp_server;
use wiremock::Mock;
use wiremock::Request;
use wiremock::ResponseTemplate;
use wiremock::matchers::body_partial_json;
use wiremock::matchers::method;
use wiremock::matchers::path;
use wiremock::matchers::path_regex;

use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use core_test_support::apps_test_server::configure_search_capable_model;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::ev_tool_search_call;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::responses::start_websocket_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use test_case::test_case;

pub(super) fn tool_call_metadata(mut item: Value) -> Value {
    let mut metadata = item["internal_chat_message_metadata_passthrough"].take();
    let fields = metadata.as_object_mut().expect("output metadata");
    fields.remove("turn_id");
    fields.remove("create_time");
    metadata
}

#[test_case(false; "http")]
#[test_case(true; "websocket")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn message_budget_sheds_inventory_without_changing_tool_results_or_history(
    websocket: bool,
) -> Result<()> {
    skip_if_no_network!(Ok(()));
    let message_limit = 15 * 1024 * 1024;
    let call_count = 16;
    // Each argument fits 8 KiB and all observations fit Direct's 1 MiB budget.
    // Only the final message budget should shed this inventory. HTTP includes
    // the original call arguments; a WebSocket delta already has those upstream.
    let arguments = json!({"plan": [{"step": "x".repeat(7 * 1024), "status": "in_progress"}]});
    assert!(arguments.to_string().len() < 8 * 1024);
    assert!(call_count * arguments.to_string().len() < 1024 * 1024);
    let next_arguments = json!({"plan": [{"step": "done", "status": "completed"}]});
    let instruction_bytes = message_limit - if websocket { 64 * 1024 } else { 192 * 1024 };
    let instructions = "padding ".repeat(instruction_bytes / 8);
    let mut builder = test_codex().with_config(move |config| {
        config.base_instructions = Some(instructions);
        config.model_context_window = Some(20_000_000);
        config.model_auto_compact_token_limit = Some(20_000_000);
        config
            .features
            .disable(Feature::TokenBudget)
            .expect("disable token budget");
        config
            .features
            .disable(Feature::CodeModeOnly)
            .expect("disable code-mode-only tools");
        config
            .features
            .disable(Feature::CodeMode)
            .expect("disable code mode");
        config
            .features
            .enable(Feature::ExecutedToolCallMetadata)
            .expect("enable tool-call metadata");
        config.update_plan_enabled = true;
        config.model_provider.request_max_retries = Some(0);
        config.model_provider.stream_max_retries = Some(0);
    });
    let mut batch = vec![ev_response_created("resp-1")];
    for index in 0..call_count {
        let mut call = ev_function_call(
            &format!("plan-{index}"),
            "update_plan",
            &arguments.to_string(),
        );
        // Stable server IDs keep the WebSocket continuation on the delta path.
        call["item"]["id"] = json!(format!("fc_plan_{index}"));
        batch.push(call);
    }
    batch.push(ev_completed("resp-1"));
    let mut next_call = ev_function_call("plan-next", "update_plan", &next_arguments.to_string());
    next_call["item"]["id"] = json!("fc_plan_next");
    let events = vec![
        batch,
        vec![
            ev_response_created("resp-2"),
            next_call,
            ev_completed("resp-2"),
        ],
        vec![ev_response_created("resp-3"), ev_completed("resp-3")],
    ];
    let http_server = if websocket {
        None
    } else {
        Some(start_mock_server().await)
    };
    let http_mock = if let Some(server) = &http_server {
        Some(mount_sse_sequence(server, events.iter().cloned().map(sse).collect()).await)
    } else {
        None
    };
    let websocket_server = if websocket {
        let mut responses = vec![vec![ev_response_created("warmup"), ev_completed("warmup")]];
        responses.extend(events);
        Some(start_websocket_server(vec![responses]).await)
    } else {
        None
    };
    let test = if let Some(server) = &websocket_server {
        builder.build_with_websocket_server(server).await?
    } else {
        builder
            .build_with_auto_env(http_server.as_ref().expect("HTTP test server"))
            .await?
    };
    test.submit_turn("Update the plan, then finish it").await?;
    let requests = if let Some(server) = &websocket_server {
        let connection = server.single_connection();
        assert_eq!(connection.len(), 4);
        assert_eq!(connection[0].body_json()["generate"], false);
        connection[1..]
            .iter()
            .map(core_test_support::responses::WebSocketRequest::body_json)
            .collect::<Vec<_>>()
    } else {
        http_mock
            .as_ref()
            .expect("HTTP response sequence")
            .requests()
            .iter()
            .map(core_test_support::responses::ResponsesRequest::body_json)
            .collect()
    };
    assert_eq!(requests.len(), 3);
    if websocket {
        assert_eq!(requests[1]["previous_response_id"], "resp-1");
        assert_eq!(requests[2]["previous_response_id"], "resp-2");
    }
    for (index, request) in requests.iter().enumerate() {
        assert_eq!(
            request["instructions"]
                .as_str()
                .expect("request instructions")
                .len(),
            instruction_bytes
        );
        let request_bytes = serde_json::to_vec(request)?.len();
        assert!(
            request_bytes <= message_limit,
            "request {index} is {request_bytes} bytes (limit {message_limit})"
        );
    }

    let history = test.codex.conversation_history_snapshot().await;
    let history = serde_json::to_value(history.items().collect::<Vec<_>>())?;
    let outputs = history
        .as_array()
        .expect("serialized history items")
        .iter()
        .filter(|item| item["type"] == "function_call_output")
        .collect::<Vec<_>>();
    assert_eq!(outputs.len(), call_count + 1);
    for (index, output) in outputs.iter().enumerate() {
        let (call_id, expected_arguments) = if index < call_count {
            (format!("plan-{index}"), &arguments)
        } else {
            ("plan-next".to_string(), &next_arguments)
        };
        assert_eq!(output["call_id"], call_id);
        assert_eq!(output["output"], "Plan updated");
        assert_eq!(
            tool_call_metadata((**output).clone()),
            json!({
                "executed_tool_calls": [{"name": "update_plan", "arguments": expected_arguments}],
                "tool_calls_complete": true,
            })
        );
    }

    let wire_calls = requests[1]["input"]
        .as_array()
        .expect("second request input")
        .iter()
        .filter(|item| item["type"] == "function_call")
        .collect::<Vec<_>>();
    assert_eq!(wire_calls.len(), if websocket { 0 } else { call_count });
    for (index, call) in wire_calls.iter().enumerate() {
        assert_eq!(call["call_id"], format!("plan-{index}"));
        assert_eq!(call["name"], "update_plan");
        assert_eq!(call["arguments"], arguments.to_string());
    }
    // Inspect the bounded wire copy before rebuilding an unbounded comparison below.
    let wire_outputs = requests[1]["input"]
        .as_array()
        .expect("second request input")
        .iter()
        .filter(|item| item["type"] == "function_call_output")
        .collect::<Vec<_>>();
    assert_eq!(wire_outputs.len(), call_count);
    let argument_bytes = serde_json::to_vec(&arguments)?.len() as u64;
    let mut retained_arguments = 0;
    let mut truncated_arguments = 0;
    for (index, output) in wire_outputs.iter().enumerate() {
        assert_eq!(output["call_id"], format!("plan-{index}"));
        assert_eq!(output["output"], "Plan updated");
        let metadata = tool_call_metadata((*output).clone());
        let calls = metadata["executed_tool_calls"]
            .as_array()
            .expect("wire call inventory");
        assert_eq!(calls.len(), 1);
        let call = &calls[0];
        if call["arguments"] == arguments {
            assert_eq!(
                metadata,
                json!({
                    "executed_tool_calls": [{"name": "update_plan", "arguments": arguments}],
                    "tool_calls_complete": true,
                })
            );
            retained_arguments += 1;
        } else {
            let truncation = &call["arguments"]["_codex_executed_tool_call_truncated"];
            let max_bytes = truncation["max_bytes"]
                .as_u64()
                .expect("wire truncation limit");
            assert!(max_bytes < argument_bytes);
            assert_eq!(
                metadata,
                json!({
                    "executed_tool_calls": [{
                        "name": "update_plan",
                        "arguments": {"_codex_executed_tool_call_truncated": {
                            "original_bytes": argument_bytes,
                            "max_bytes": max_bytes,
                        }},
                    }],
                })
            );
            truncated_arguments += 1;
        }
    }
    assert!(
        retained_arguments > 0,
        "wire request lost every full observation"
    );
    assert!(
        truncated_arguments > 0,
        "wire request did not shed any arguments"
    );

    let mut unbounded = requests[1].clone();
    let mut matched_outputs = 0;
    for item in unbounded["input"]
        .as_array_mut()
        .expect("unbounded request input")
    {
        if item["type"] != "function_call_output" {
            continue;
        }
        let source = outputs
            .iter()
            .find(|output| output["call_id"] == item["call_id"])
            .expect("live history output matching the wire call ID");
        assert_eq!(item["output"], source["output"]);
        item["internal_chat_message_metadata_passthrough"] =
            source["internal_chat_message_metadata_passthrough"].clone();
        matched_outputs += 1;
    }
    assert_eq!(matched_outputs, call_count);
    assert!(serde_json::to_vec(&unbounded)?.len() > message_limit);
    if websocket {
        let delta = requests[2]["input"]
            .as_array()
            .expect("WebSocket continuation input");
        assert_eq!(delta.len(), 1);
        assert_eq!(delta[0]["call_id"], "plan-next");
        assert_eq!(delta[0]["output"], "Plan updated");
        assert_eq!(
            tool_call_metadata(delta[0].clone()),
            tool_call_metadata((*outputs[call_count]).clone())
        );
    }
    test.codex.shutdown_and_wait().await?;
    if let Some(server) = websocket_server {
        server.shutdown().await;
    }
    Ok(())
}

#[test_case(RemoteCompactionSupport::Unsupported, true; "local")]
#[test_case(RemoteCompactionSupport::V2, true; "remote_v2")]
#[test_case(RemoteCompactionSupport::V2, false; "remote_v2_disabled_after_capture")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn direct_call_metadata_during_compaction_respects_provider_support(
    remote_compaction: RemoteCompactionSupport,
    metadata_enabled: bool,
) -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(move |config| {
        config
            .features
            .enable(Feature::ExecutedToolCallMetadata)
            .expect("enable tool-call metadata");
        config.update_plan_enabled = true;
        if remote_compaction == RemoteCompactionSupport::Unsupported {
            config.model_provider.name = "OpenAI-compatible test provider".to_string();
        }
    });
    let test = builder.build_with_auto_env(&server).await?;
    let seed_arguments = json!({"plan": [{"step": "read", "status": "in_progress"}]});
    let arguments = json!({"plan": [{"step": "read", "status": "completed"}]});
    let summary = "The prior call finished.";
    let compact_output = match remote_compaction {
        RemoteCompactionSupport::Unsupported => ev_assistant_message("summary", summary),
        RemoteCompactionSupport::V2 => json!({
            "type": "response.output_item.done",
            "item": {"type": "compaction", "encrypted_content": summary},
        }),
    };
    let mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_function_call("shared", "update_plan", &seed_arguments.to_string()),
                ev_completed("seed-call-response"),
            ]),
            sse(vec![
                ev_assistant_message("seed", "done"),
                ev_completed("seed-response"),
            ]),
            sse(vec![compact_output, ev_completed("compact-response")]),
            sse(vec![
                ev_function_call("shared", "update_plan", &arguments.to_string()),
                ev_completed("reused-response"),
            ]),
            sse(vec![ev_completed("done")]),
        ],
    )
    .await;
    test.submit_turn("Update the plan before compaction")
        .await?;
    // Read the live history: deserializing a rollout intentionally drops host-owned metadata.
    let history = test.codex.conversation_history_snapshot().await;
    let history = serde_json::to_value(history.items().collect::<Vec<_>>())?;
    let history = history.as_array().expect("source history");
    let seed_output = history
        .iter()
        .find(|item| item["type"] == "function_call_output" && item["call_id"] == "shared")
        .expect("direct output before compaction");
    assert_eq!(seed_output["output"], "Plan updated");
    assert_eq!(
        tool_call_metadata(seed_output.clone()),
        json!({
            "executed_tool_calls": [{"name": "update_plan", "arguments": seed_arguments}],
            "tool_calls_complete": true,
        }),
    );
    let user_content = &history
        .iter()
        .find(|item| {
            item["role"] == "user"
                && item["content"][0]["text"] == "Update the plan before compaction"
        })
        .expect("source user message")["content"];
    if !metadata_enabled {
        let mut config = test.config.clone();
        config.features.disable(Feature::ExecutedToolCallMetadata)?;
        test.codex.refresh_runtime_config(config).await;
    }
    test.codex.submit(Op::Compact).await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    let compacted = test.codex.conversation_history_snapshot().await;
    let compacted = serde_json::to_value(compacted.items().collect::<Vec<_>>())?;
    let compacted = compacted.as_array().expect("compacted history");
    // Both paths retain the user message and summary, not the old call or output.
    assert!(compacted.iter().all(|item| item["call_id"] != "shared"));
    assert!(
        compacted
            .iter()
            .any(|item| item["role"] == "user" && &item["content"] == user_content)
    );
    match remote_compaction {
        RemoteCompactionSupport::Unsupported => assert!(compacted.iter().any(|item| {
            item["role"] == "user"
                && item["content"][0]["text"]
                    == format!("{}\n{summary}", codex_core::compact::SUMMARY_PREFIX)
        })),
        RemoteCompactionSupport::V2 => {
            assert!(compacted.iter().any(|item| {
                item["type"] == "compaction" && item["encrypted_content"] == summary
            }))
        }
    }
    test.submit_turn("Update the plan").await?;

    let requests = mock.requests();
    assert_eq!(requests.len(), 5);
    let compact_request = &requests[2];
    assert_eq!(
        compact_request.inputs_of_type("compaction_trigger").len(),
        usize::from(remote_compaction == RemoteCompactionSupport::V2),
    );
    let compact_output = compact_request.function_call_output("shared");
    assert_eq!(compact_output["output"], seed_output["output"]);
    // The local case uses a third-party provider; compact_tests.rs separately covers
    // local compaction with an OpenAI provider that accepts passthrough metadata.
    match remote_compaction {
        RemoteCompactionSupport::Unsupported => assert!(
            compact_output
                .get("internal_chat_message_metadata_passthrough")
                .is_none()
        ),
        RemoteCompactionSupport::V2 => {
            let mut expected = seed_output["internal_chat_message_metadata_passthrough"].clone();
            if !metadata_enabled {
                let metadata = expected.as_object_mut().expect("source metadata");
                metadata.remove("executed_tool_calls");
                metadata.remove("tool_calls_complete");
            }
            assert_eq!(
                compact_output["internal_chat_message_metadata_passthrough"],
                expected
            );
        }
    }
    assert!(
        requests[3]
            .input()
            .iter()
            .all(|item| item["call_id"] != "shared")
    );
    let output = requests[4].function_call_output("shared");
    assert_eq!(output["output"], "Plan updated");
    let captured = test.codex.conversation_history_snapshot().await;
    let captured = serde_json::to_value(captured.items().collect::<Vec<_>>())?;
    let captured_output = captured
        .as_array()
        .expect("captured history")
        .iter()
        .find(|item| item["type"] == "function_call_output" && item["call_id"] == "shared")
        .expect("captured direct output");
    assert_eq!(
        tool_call_metadata(captured_output.clone()),
        if metadata_enabled {
            json!({
                "executed_tool_calls": [{"name": "update_plan", "arguments": arguments}],
                "tool_calls_complete": true,
            })
        } else {
            json!({})
        },
    );
    Ok(())
}

#[test_case(false, 0; "metadata disabled")]
#[test_case(true, 24; "above previous request budget")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn direct_function_and_tool_search_mark_complete_attempts(
    metadata_enabled: bool,
    budget_calls: usize,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let request_budget = 2 * 1024 * 1024;
    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(move |config| {
        configure_search_capable_model(config);
        config.update_plan_enabled = true;
        if metadata_enabled {
            let _ = config.features.enable(Feature::ExecutedToolCallMetadata);
        } else {
            let _ = config.features.disable(Feature::ExecutedToolCallMetadata);
        }
    });
    let test = builder.build_with_auto_env(&server).await?;
    let valid_arguments = json!({"plan": [{"step": "read", "status": "in_progress"}]});
    let malformed_arguments = "{malformed";
    let search_arguments = json!({"query": "nonexistent-completeness-proof-tool", "limit": null});
    // These calls exceed the old request budget but stay below Direct's retained-history limit.
    let budget_arguments =
        json!({"plan": [{"step": "x".repeat(7 * 1024), "status": "in_progress"}]});
    let budget_arguments_json = budget_arguments.to_string();
    assert!(budget_arguments_json.len() < 8 * 1024);
    if budget_calls > 0 {
        assert!(budget_calls * budget_arguments_json.len() > 128 * 1024);
        assert!(budget_calls * budget_arguments_json.len() < 1024 * 1024);
    }
    let mut events = vec![ev_response_created("resp-1")];
    if budget_calls > 0 {
        events.push(ev_function_call(
            "plan-oversized",
            "update_plan",
            &json!({"plan": [{"step": "x".repeat(9 * 1024), "status": "in_progress"}]}).to_string(),
        ));
    }
    for index in 0..budget_calls {
        events.push(ev_function_call(
            &format!("plan-budget-{index}"),
            "update_plan",
            &budget_arguments_json,
        ));
    }
    events.extend([
        ev_function_call("plan-valid", "update_plan", &valid_arguments.to_string()),
        ev_function_call("plan-malformed", "update_plan", malformed_arguments),
        ev_tool_search_call("search", &search_arguments),
        ev_completed("resp-1"),
    ]);
    let mock = mount_sse_sequence(
        &server,
        vec![
            sse(events),
            sse(vec![
                ev_assistant_message("msg-1", "done"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;
    test.submit_turn_with_approval_and_permission_profile(
        "exercise direct attempts",
        AskForApproval::Never,
        PermissionProfile::Disabled,
    )
    .await?;

    let requests = mock.requests();
    assert_eq!(requests.len(), 2);
    let request = &requests[1];
    let valid_output = request.function_call_output("plan-valid");
    let malformed_output = request.function_call_output("plan-malformed");
    let search_output = request.tool_search_output("search");
    assert_eq!(valid_output["output"], json!("Plan updated"));
    assert!(
        malformed_output["output"]
            .as_str()
            .expect("parse error output")
            .starts_with("failed to parse function arguments:"),
    );
    assert_eq!(search_output["execution"], json!("client"));

    let mut metadata_bytes = 0;
    for (output, name, arguments) in [
        (valid_output, "update_plan", valid_arguments),
        (malformed_output, "update_plan", json!(malformed_arguments)),
        (
            search_output,
            "tool_search",
            json!({"query": "nonexistent-completeness-proof-tool"}),
        ),
    ] {
        let expected = if metadata_enabled {
            json!({
                "executed_tool_calls": [{"name": name, "arguments": arguments}],
                "tool_calls_complete": true,
            })
        } else {
            json!({})
        };
        let metadata = tool_call_metadata(output);
        metadata_bytes += serde_json::to_vec(&metadata)?.len();
        assert_eq!(metadata, expected);
    }
    if budget_calls > 0 {
        let output = request.function_call_output("plan-oversized");
        assert_eq!(output["output"], json!("Plan updated"));
        let metadata = tool_call_metadata(output);
        metadata_bytes += serde_json::to_vec(&metadata)?.len();
        assert!(
            metadata["executed_tool_calls"][0]["arguments"]
                .get("_codex_executed_tool_call_truncated")
                .is_some()
        );
        assert!(metadata.get("tool_calls_complete").is_none());
    }
    for index in 0..budget_calls {
        let output = request.function_call_output(&format!("plan-budget-{index}"));
        assert_eq!(output["output"], json!("Plan updated"));
        let metadata = tool_call_metadata(output);
        metadata_bytes += serde_json::to_vec(&metadata)?.len();
        assert_eq!(
            metadata,
            json!({
                "executed_tool_calls": [{"name": "update_plan", "arguments": budget_arguments}],
                "tool_calls_complete": true,
            })
        );
    }
    assert!(metadata_bytes <= request_budget);
    if budget_calls > 0 {
        assert!(metadata_bytes > 128 * 1024);
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn direct_metadata_limit_retains_marker_without_sending_it_to_custom_provider() -> Result<()>
{
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let apps = AppsTestServer::mount(&server).await?;
    let raw_metadata = json!({"openai/resource_access": {"payload": "x".repeat(600 * 1024)}});
    let raw_bytes = serde_json::to_vec(&raw_metadata)?.len();
    assert!(raw_bytes < 1024 * 1024);
    assert!(raw_bytes * 2 > 1024 * 1024);
    assert!(raw_bytes * 2 < 2 * 1024 * 1024);
    let fixture_metadata = raw_metadata.clone();
    Mock::given(method("POST"))
        .and(path_regex("^/api/codex/ps/mcp/?$"))
        .and(body_partial_json(json!({"method": "tools/call"})))
        .respond_with(move |request: &Request| {
            let body: Value = serde_json::from_slice(&request.body).expect("MCP request");
            let query = body["params"]["arguments"]["query"]
                .as_str()
                .expect("query");
            ResponseTemplate::new(/*s*/ 200).set_body_json(json!({
                "jsonrpc": "2.0", "id": body["id"],
                "result": {
                    "content": [{"type": "text", "text": format!("result for {query}")}],
                    "_meta": fixture_metadata,
                    "isError": false
                }
            }))
        })
        .with_priority(/*p*/ 1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/codex/analytics-events/events"))
        .respond_with(ResponseTemplate::new(/*s*/ 200))
        .mount(&server)
        .await;
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call_with_namespace(
                    "first",
                    SEARCH_CALENDAR_NAMESPACE,
                    SEARCH_CALENDAR_LIST_TOOL,
                    r#"{"query":"first"}"#,
                ),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_function_call_with_namespace(
                    "second",
                    SEARCH_CALENDAR_NAMESPACE,
                    SEARCH_CALENDAR_LIST_TOOL,
                    r#"{"query":"second"}"#,
                ),
                ev_completed("resp-2"),
            ]),
            sse(vec![ev_response_created("resp-3"), ev_completed("resp-3")]),
        ],
    )
    .await;
    let mut builder = search_capable_apps_builder(apps.chatgpt_base_url).with_config(|config| {
        config.analytics_enabled = Some(true);
        config
            .features
            .enable(Feature::ExecutedToolCallMetadata)
            .expect("enable executed tool call metadata");
        config
            .features
            .disable(Feature::CodeMode)
            .expect("disable code mode");
        config
            .features
            .disable(Feature::CodeModeOnly)
            .expect("disable code-mode-only tools");
    });
    // build() always selects the local test environment; all network destinations are the mock.
    let test = builder.build(&server).await?;
    assert!(test.codex.analytics_enabled());
    wait_for_mcp_server(&test.codex, CODEX_APPS_MCP_SERVER_NAME).await?;
    test.submit_turn_with_approval_and_permission_profile(
        "Use [$calendar](app://calendar) to list events twice.",
        AskForApproval::OnRequest,
        PermissionProfile::read_only(),
    )
    .await?;

    let calls = recorded_apps_tool_calls(&server).await;
    assert_eq!(calls.len(), 2);
    for (call, query) in calls.iter().zip(["first", "second"]) {
        assert_eq!(call["params"]["name"], "calendar_list_events");
        assert_eq!(call["params"]["arguments"], json!({"query": query}));
    }
    let requests = responses.requests();
    assert_eq!(requests.len(), 3);
    let first_output = requests[2].function_call_output("first");
    let second_output = requests[2].function_call_output("second");
    let history = test.codex.conversation_history_snapshot().await;
    let history = serde_json::to_value(history.items().collect::<Vec<_>>())?;
    let recorded_outputs = [&first_output, &second_output].map(|output| {
        history
            .as_array()
            .expect("history items")
            .iter()
            .find(|item| {
                item["type"] == "function_call_output" && item["call_id"] == output["call_id"]
            })
            .expect("original tool output in history")
    });
    for ((wire, recorded), query) in [&first_output, &second_output]
        .into_iter()
        .zip(recorded_outputs)
        .zip(["first", "second"])
    {
        assert_eq!(wire["output"], recorded["output"]);
        assert!(
            wire["output"]
                .to_string()
                .contains(&format!("result for {query}"))
        );
        let recorded_metadata = tool_call_metadata(recorded.clone());
        let wire_metadata = tool_call_metadata(wire.clone());
        assert_eq!(recorded_metadata["tool_calls_complete"], true);
        assert_eq!(
            recorded_metadata["executed_tool_calls"][0]["name"],
            format!("{SEARCH_CALENDAR_NAMESPACE}{SEARCH_CALENDAR_LIST_TOOL}")
        );
        assert_eq!(
            recorded_metadata["executed_tool_calls"][0]["arguments"],
            json!({"query": query})
        );
        // A custom Responses provider must never receive internal result metadata.
        assert!(
            wire_metadata["executed_tool_calls"][0]
                .get("tool_result_metadata")
                .is_none()
        );
    }
    let first_metadata = tool_call_metadata(recorded_outputs[0].clone());
    let second_metadata = tool_call_metadata(recorded_outputs[1].clone());
    assert_eq!(
        first_metadata["executed_tool_calls"][0]["tool_result_metadata"],
        raw_metadata
    );
    let marker = second_metadata["executed_tool_calls"][0]["tool_result_metadata"]
        .as_str()
        .expect("second result should have an omission marker");
    let overage = marker
        .strip_prefix("omitted_due_to_size_limit (overage_bytes=")
        .and_then(|value| value.strip_suffix(')'))
        .and_then(|value| value.parse::<usize>().ok())
        .expect("marker should report omitted bytes");
    assert!(overage > 0);
    Ok(())
}
