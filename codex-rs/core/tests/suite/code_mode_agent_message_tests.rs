//! Exec-spawned tasks retain their originating turn's attribution and plaintext input.

use anyhow::Result;
use codex_features::Feature;
use codex_protocol::openai_models::ToolMode;
use codex_protocol::protocol::EventMsg;
use codex_protocol::turn_input::TurnInputRequest;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_custom_tool_call;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once_match;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::responses::strip_metadata_from_json;
use core_test_support::responses::strip_response_item_ids_from_json;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::collections::HashMap;
use wiremock::matchers::header;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn yielded_code_mode_spawn_preserves_metadata_and_plaintext() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let test = test_codex()
        .with_model("gpt-5.6-sol")
        .with_model_info_override("gpt-5.6-sol", |model| {
            model.tool_mode = Some(ToolMode::CodeMode);
            model.use_responses_lite = true;
            model
                .experimental_supported_tools
                .push("test_sync_tool".to_string());
        })
        .with_config(|config| {
            for feature in [
                Feature::Collab,
                Feature::MultiAgentV2,
                Feature::CodeMode,
                Feature::CodeModeHostGrpc,
            ] {
                config
                    .features
                    .enable(feature)
                    .expect("enable test feature");
            }
            config.multi_agent_v2.non_code_mode_only = false;
        })
        .build_with_auto_env(&server)
        .await?;
    let root_header = test.session_configured.thread_id.to_string();
    let mut created = test.thread_manager.subscribe_thread_created();
    // Keep the old cell blocked until the next turn is executing, then keep that
    // turn active until the old cell has spawned its child.
    let start_barrier = r#"await tools.test_sync_tool({barrier: {
        id: "spawn-origin", participants: 2, timeout_ms: 60000
    }});"#;

    mount_sse_once_match(
        &server,
        header("thread-id", root_header.as_str()),
        sse(vec![
            ev_response_created("spawn"),
            ev_custom_tool_call(
                "spawn-worker",
                "exec",
                &format!(r#"yield_control();
{start_barrier}
text(await tools.collaboration__spawn_agent({{task_name: "worker", message: "Summarize the three stories.", fork_turns: "all"}}));"#),
            ),
            ev_completed("spawn"),
        ]),
    )
    .await;
    mount_sse_once_match(
        &server,
        header("thread-id", root_header.as_str()),
        sse(vec![
            ev_assistant_message("yielded", "Waiting for the next turn."),
            ev_completed("yielded"),
        ]),
    )
    .await;
    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "Delegate the summary task.".to_string(),
                text_elements: Vec::new(),
            }])
            .with_responses_metadata(Some(HashMap::from([(
                "cost_center".to_string(),
                "origin".to_string(),
            )]))),
        )
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    mount_sse_once_match(
        &server,
        header("thread-id", root_header.as_str()),
        sse(vec![
            ev_custom_tool_call("release-worker", "exec", start_barrier),
            ev_completed("release-worker"),
        ]),
    )
    .await;
    // Wait outside nested tool dispatch: another test_sync_tool barrier could
    // hold shared admission while the spawn needs exclusive admission.
    mount_sse_once_match(
        &server,
        header("thread-id", root_header.as_str()),
        sse(vec![
            ev_function_call("wait-worker", "wait", r#"{"cell_id":"1"}"#),
            ev_completed("wait-worker"),
        ]),
    )
    .await;
    mount_sse_once_match(
        &server,
        header("thread-id", root_header.as_str()),
        sse(vec![
            ev_assistant_message("delegated", "The worker started."),
            ev_completed("delegated"),
        ]),
    )
    .await;
    let child_requests = mount_sse_once_match(
        &server,
        move |request: &wiremock::Request| {
            request
                .headers
                .get("thread-id")
                .is_some_and(|id| id != root_header.as_str())
        },
        sse(vec![ev_response_created("child"), ev_completed("child")]),
    )
    .await;
    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "Release the previous turn's cell.".to_string(),
                text_elements: Vec::new(),
            }])
            .with_responses_metadata(Some(HashMap::from([(
                "cost_center".to_string(),
                "current".to_string(),
            )]))),
        )
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    let child_id = created.try_recv()?;
    let child = test.thread_manager.get_thread(child_id).await?;
    wait_for_event(&child, |event| matches!(event, EventMsg::TurnComplete(_))).await;

    let child_request = child_requests
        .requests()
        .into_iter()
        .find(|request| request.header("thread-id") == Some(child_id.to_string()))
        .expect("child receives its task");
    let child_metadata: Value = serde_json::from_str(
        child_request.body_json()["client_metadata"]["x-codex-turn-metadata"]
            .as_str()
            .expect("canonical child metadata"),
    )?;
    assert_eq!(child_metadata["cost_center"], "origin");
    assert_eq!(
        strip_response_item_ids_from_json(strip_metadata_from_json(Value::Array(
            child_request.inputs_of_type("agent_message"),
        ))),
        json!([{
            "type": "agent_message",
            "author": "/root",
            "recipient": "/root/worker",
            "content": [{
                "type": "input_text",
                "text": "Message Type: NEW_TASK\nTask name: /root/worker\nSender: /root\nPayload:\nSummarize the three stories.",
            }],
        }]),
    );
    assert_eq!(test.thread_manager.list_thread_ids().await.len(), 2);
    Ok(())
}
