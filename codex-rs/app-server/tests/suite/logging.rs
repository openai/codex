use anyhow::Result;
use app_test_support::TestAppServer;
use app_test_support::app_server_json_shutdown_event;
use app_test_support::create_exec_command_sse_response;
use app_test_support::create_final_assistant_message_sse_response;
use app_test_support::create_mock_responses_server_sequence;
use app_test_support::to_response;
use app_test_support::write_mock_responses_config_toml;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::UserInput;
use codex_features::Feature;
use core_test_support::skip_if_no_network;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::collections::BTreeMap;
use tempfile::TempDir;
use tokio::time::Duration;
use tokio::time::timeout;

const READ_TIMEOUT: Duration = Duration::from_secs(10);

#[test]
fn standalone_app_server_emits_json_info_events() -> Result<()> {
    let codex_home = TempDir::new()?;
    let event = app_server_json_shutdown_event("codex-app-server", &[], codex_home.path())?;

    assert_eq!(
        event,
        json!({
            "level": "INFO",
            "fields": {
                "message": "processor task exited",
                "exit_reason": "last_connection_closed",
                "remaining_connection_count": 0,
                "shutdown_forced": false,
            },
            "target": "codex_app_server",
        })
    );

    Ok(())
}

#[tokio::test]
async fn app_server_emits_structured_tool_call_timing_event() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = create_mock_responses_server_sequence(vec![
        create_exec_command_sse_response("exec-call-1")?,
        create_final_assistant_message_sse_response("done")?,
    ])
    .await;
    let codex_home = TempDir::new()?;
    write_mock_responses_config_toml(
        codex_home.path(),
        &server.uri(),
        &BTreeMap::from([(Feature::UnifiedExec, true)]),
        /*auto_compact_limit*/ 100_000,
        /*requires_openai_auth*/ None,
        "mock_provider",
        "compact",
    )?;

    let mut app_server = TestAppServer::new_with_auto_env_and_env(
        codex_home.path(),
        &[
            ("LOG_FORMAT", Some("json")),
            ("RUST_LOG", Some("warn,codex_core::tools::parallel=info")),
        ],
    )
    .await?;
    timeout(READ_TIMEOUT, app_server.initialize()).await??;

    let thread_start_id = app_server
        .send_thread_start_request_with_auto_env(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let thread_start_response: JSONRPCResponse = timeout(
        READ_TIMEOUT,
        app_server.read_stream_until_response_message(RequestId::Integer(thread_start_id)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response(thread_start_response)?;

    let turn_start_id = app_server
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![UserInput::Text {
                text: "run a command".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    let turn_start_response: JSONRPCResponse = timeout(
        READ_TIMEOUT,
        app_server.read_stream_until_response_message(RequestId::Integer(turn_start_id)),
    )
    .await??;
    let TurnStartResponse { turn } = to_response(turn_start_response)?;

    timeout(
        READ_TIMEOUT,
        app_server.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let tool_call = app_server
        .wait_for_json_log_event("codex.tool_call")
        .await?;
    assert_eq!(tool_call["level"], "INFO");
    assert_eq!(tool_call["target"], "codex_core::tools::parallel");
    assert_eq!(tool_call["fields"]["message"], "tool call completed");
    assert!(tool_call["fields"]["trace_id"].is_string());
    assert_eq!(tool_call["fields"]["conversation.id"], thread.id);
    assert_eq!(tool_call["fields"]["turn_id"], turn.id);
    assert_eq!(tool_call["fields"]["tool_name"], "exec_command");
    assert_eq!(tool_call["fields"]["call_id"], "exec-call-1");
    assert_eq!(tool_call["fields"]["tool_source"], "direct");
    assert_eq!(tool_call["fields"]["code_mode_cell_id"], "");
    assert_eq!(tool_call["fields"]["code_mode_runtime_tool_call_id"], "");
    assert_eq!(tool_call["fields"]["execution_started"], true);
    assert_nonnegative_duration_fields(
        &tool_call,
        &[
            "dispatch_duration_ms",
            "handler_duration_ms",
            "total_duration_ms",
        ],
    );
    let dispatch_duration = duration_field(&tool_call, "dispatch_duration_ms");
    let handler_duration = duration_field(&tool_call, "handler_duration_ms");
    let total_duration = duration_field(&tool_call, "total_duration_ms");
    assert!(total_duration > 0.0);
    let truncation_delta = total_duration - dispatch_duration - handler_duration;
    assert!(
        (0.0..=1.0).contains(&truncation_delta),
        "dispatch and handler durations must account for total duration within integer truncation: {tool_call}"
    );

    Ok(())
}

fn assert_nonnegative_duration_fields(event: &serde_json::Value, fields: &[&str]) {
    for field in fields {
        let duration = duration_field(event, field);
        assert!(duration >= 0.0, "{field} must be nonnegative: {event}");
    }
}

fn duration_field(event: &serde_json::Value, field: &str) -> f64 {
    event["fields"][field]
        .as_f64()
        .unwrap_or_else(|| panic!("{field} must be a JSON number: {event}"))
}
