//! Direct-call metadata coverage, including malformed calls and metadata budgets.

use anyhow::Result;
use codex_features::Feature;
use codex_model_provider::RemoteCompactionSupport;
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
