//! Verifies fresh V2 subagents can use inherited client tools when enabled.

use anyhow::Result;
use codex_core::StartThreadOptions;
use codex_features::Feature;
use codex_protocol::dynamic_tools::DynamicToolCallOutputContentItem;
use codex_protocol::dynamic_tools::DynamicToolFunctionSpec;
use codex_protocol::dynamic_tools::DynamicToolResponse;
use codex_protocol::dynamic_tools::DynamicToolSpec;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_function_call_with_namespace;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once_match;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_match;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::time::Duration;

const ROOT_PROMPT: &str = "Spawn the echo worker.";
const CHILD_PROMPT: &str = "Call client_echo with the message child-input.";
const SPAWN_CALL_ID: &str = "spawn-echo-worker";
const ECHO_CALL_ID: &str = "child-echo-call";
const ECHO_RESULT: &str = "distinctive-client-echo-result-7391";

fn body_contains(request: &wiremock::Request, text: &str) -> bool {
    String::from_utf8_lossy(&request.body).contains(text)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fresh_v2_subagent_can_call_parent_dynamic_tool_when_enabled() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let spawn_args =
        json!({"message": CHILD_PROMPT, "task_name": "echo_worker", "fork_turns": "none"});
    mount_sse_once_match(
        &server,
        |request: &wiremock::Request| {
            body_contains(request, ROOT_PROMPT) && !body_contains(request, SPAWN_CALL_ID)
        },
        sse(vec![
            ev_response_created("root-spawn"),
            ev_function_call_with_namespace(
                SPAWN_CALL_ID,
                "collaboration",
                "spawn_agent",
                &spawn_args.to_string(),
            ),
            ev_completed("root-spawn"),
        ]),
    )
    .await;
    mount_sse_once_match(
        &server,
        |request: &wiremock::Request| body_contains(request, SPAWN_CALL_ID),
        sse(vec![
            ev_response_created("root-finished"),
            ev_assistant_message("root-message", "The echo worker has started."),
            ev_completed("root-finished"),
        ]),
    )
    .await;
    let child_request = mount_sse_once_match(
        &server,
        |request: &wiremock::Request| {
            body_contains(request, CHILD_PROMPT)
                && !body_contains(request, SPAWN_CALL_ID)
                && !body_contains(request, ECHO_CALL_ID)
        },
        sse(vec![
            ev_response_created("child-call"),
            ev_function_call(
                ECHO_CALL_ID,
                "client_echo",
                &json!({"message": "child-input"}).to_string(),
            ),
            ev_completed("child-call"),
        ]),
    )
    .await;
    let child_continuation = mount_sse_once_match(
        &server,
        |request: &wiremock::Request| body_contains(request, ECHO_CALL_ID),
        sse(vec![
            ev_response_created("child-finished"),
            ev_assistant_message("child-message", "The client echo result was received."),
            ev_completed("child-finished"),
        ]),
    )
    .await;

    let mut test = test_codex()
        .with_config(|config| {
            for feature in [
                Feature::Collab,
                Feature::MultiAgentV2,
                Feature::MultiAgentV2DynamicTools,
            ] {
                config.features.enable(feature).expect("enable multi-agent");
            }
            config
                .features
                .disable(Feature::EnableRequestCompression)
                .expect("disable request compression for mock matching");
        })
        .build_with_auto_env(&server)
        .await?;
    let tool = DynamicToolFunctionSpec {
        name: "client_echo".to_string(),
        description: "Echo the supplied message through the client.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {"message": {"type": "string"}},
            "required": ["message"],
            "additionalProperties": false,
        }),
        defer_loading: false,
    };
    test.codex.shutdown_and_wait().await?;
    let root = test
        .thread_manager
        .start_thread(StartThreadOptions {
            dynamic_tools: vec![DynamicToolSpec::Function(tool.clone())],
            environments: Some(vec![test.executor_environment().selection().clone()]),
            ..StartThreadOptions::new(test.config.clone())
        })
        .await?;
    test.codex = root.thread;
    test.session_configured = root.session_configured;
    let mut created_threads = test.thread_manager.subscribe_thread_created();
    test.submit_turn(ROOT_PROMPT).await?;
    let child_id = tokio::time::timeout(Duration::from_secs(10), created_threads.recv()).await??;
    let child = test.thread_manager.get_thread(child_id).await?;
    let call = wait_for_event_match(&child, |event| match event {
        EventMsg::DynamicToolCallRequest(call) => Some(call.clone()),
        EventMsg::Error(error) => panic!("child failed before calling echo: {}", error.message),
        EventMsg::TurnComplete(completed) => {
            panic!("child completed without calling echo: {completed:?}")
        }
        _ => None,
    })
    .await;
    assert_eq!(
        (
            call.call_id.as_str(),
            call.namespace.as_deref(),
            call.tool.as_str(),
            &call.arguments
        ),
        (
            ECHO_CALL_ID,
            None,
            "client_echo",
            &json!({"message": "child-input"})
        )
    );

    let expected_tool = json!({
        "type": "function",
        "name": tool.name,
        "description": tool.description,
        "parameters": tool.input_schema,
        "strict": false,
    });
    let child_request = child_request.single_request();
    assert!(
        !child_request
            .message_input_texts("user")
            .iter()
            .any(|text| text.contains(ROOT_PROMPT))
    );
    let body = child_request.body_json();
    let echo_tools: Vec<_> = body["tools"]
        .as_array()
        .expect("model tools")
        .iter()
        .filter(|tool| tool["name"] == "client_echo")
        .collect();
    assert_eq!(echo_tools, vec![&expected_tool]);

    child
        .submit(Op::DynamicToolResponse {
            id: call.call_id,
            response: DynamicToolResponse {
                content_items: vec![DynamicToolCallOutputContentItem::InputText {
                    text: ECHO_RESULT.to_string(),
                }],
                success: true,
            },
        })
        .await?;
    wait_for_event(&child, |event| matches!(event, EventMsg::TurnComplete(_))).await;
    assert_eq!(
        child_continuation
            .single_request()
            .function_call_output_content_and_success(ECHO_CALL_ID),
        Some((Some(ECHO_RESULT.to_string()), None))
    );
    child.shutdown_and_wait().await?;
    test.codex.shutdown_and_wait().await?;
    Ok(())
}
