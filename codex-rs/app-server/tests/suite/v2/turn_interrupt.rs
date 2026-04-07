#![cfg(unix)]

use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::create_mock_responses_server_sequence;
use app_test_support::create_mock_responses_server_sequence_unchecked;
use app_test_support::create_shell_command_sse_response;
use app_test_support::to_response;
use app_test_support::write_mock_responses_config_toml_with_chatgpt_base_url;
use codex_app_server_protocol::JSONRPCNotification;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::ServerRequestResolvedNotification;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnCompletedNotification;
use codex_app_server_protocol::TurnInterruptParams;
use codex_app_server_protocol::TurnInterruptResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::TurnStatus;
use codex_app_server_protocol::UserInput as V2UserInput;
use tempfile::TempDir;
use tokio::time::timeout;

use super::analytics::enable_analytics_capture;
use super::analytics::wait_for_analytics_event;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[tokio::test]
async fn turn_interrupt_aborts_running_turn() -> Result<()> {
    // Use a portable sleep command to keep the turn running.
    #[cfg(target_os = "windows")]
    let shell_command = vec![
        "powershell".to_string(),
        "-Command".to_string(),
        "Start-Sleep -Seconds 10".to_string(),
    ];
    #[cfg(not(target_os = "windows"))]
    let shell_command = vec!["sleep".to_string(), "10".to_string()];

    let tmp = TempDir::new()?;
    let codex_home = tmp.path().join("codex_home");
    std::fs::create_dir(&codex_home)?;
    let working_directory = tmp.path().join("workdir");
    std::fs::create_dir(&working_directory)?;

    // Mock server: long-running shell command then (after abort) nothing else needed.
    let server =
        create_mock_responses_server_sequence_unchecked(vec![create_shell_command_sse_response(
            shell_command.clone(),
            Some(&working_directory),
            Some(10_000),
            "call_sleep",
        )?])
        .await;
    write_mock_responses_config_toml_with_chatgpt_base_url(
        &codex_home,
        &server.uri(),
        &server.uri(),
    )?;
    enable_analytics_capture(&server, &codex_home).await?;

    let mut mcp = McpProcess::new(&codex_home).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    // Start a v2 thread and capture its id.
    let thread_req = mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let thread_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(thread_req)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response::<ThreadStartResponse>(thread_resp)?;

    // Start a turn that triggers a long-running command.
    let turn_req = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![V2UserInput::Text {
                text: "run sleep".to_string(),
                text_elements: Vec::new(),
            }],
            cwd: Some(working_directory.clone()),
            ..Default::default()
        })
        .await?;
    let turn_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_req)),
    )
    .await??;
    let TurnStartResponse { turn } = to_response::<TurnStartResponse>(turn_resp)?;
    let turn_id = turn.id.clone();

    // Give the command a brief moment to start.
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    let thread_id = thread.id.clone();
    // Interrupt the in-progress turn by id (v2 API).
    let interrupt_id = mcp
        .send_turn_interrupt_request(TurnInterruptParams {
            thread_id: thread_id.clone(),
            turn_id: turn_id.clone(),
        })
        .await?;
    let interrupt_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(interrupt_id)),
    )
    .await??;
    let _resp: TurnInterruptResponse = to_response::<TurnInterruptResponse>(interrupt_resp)?;

    let completed_notif: JSONRPCNotification = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;
    let completed: TurnCompletedNotification = serde_json::from_value(
        completed_notif
            .params
            .expect("turn/completed params must be present"),
    )?;
    assert_eq!(completed.thread_id, thread_id);
    assert_eq!(completed.turn.status, TurnStatus::Interrupted);

    let event = wait_for_analytics_event(&server, DEFAULT_READ_TIMEOUT, "codex_turn_event").await?;
    assert_eq!(event["event_params"]["thread_id"], thread_id);
    assert_eq!(event["event_params"]["turn_id"], turn_id);
    assert_eq!(event["event_params"]["status"], "interrupted");
    assert_eq!(event["event_params"]["turn_error"], serde_json::Value::Null);

    Ok(())
}

#[tokio::test]
async fn turn_interrupt_resolves_pending_command_approval_request() -> Result<()> {
    let shell_command = vec![
        "python3".to_string(),
        "-c".to_string(),
        "print(42)".to_string(),
    ];

    let tmp = TempDir::new()?;
    let codex_home = tmp.path().join("codex_home");
    std::fs::create_dir(&codex_home)?;
    let working_directory = tmp.path().join("workdir");
    std::fs::create_dir(&working_directory)?;

    let server = create_mock_responses_server_sequence(vec![create_shell_command_sse_response(
        shell_command.clone(),
        Some(&working_directory),
        Some(10_000),
        "call_python_approval",
    )?])
    .await;
    create_config_toml(&codex_home, &server.uri(), "untrusted", "read-only")?;

    let mut mcp = McpProcess::new(&codex_home).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let thread_req = mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let thread_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(thread_req)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response::<ThreadStartResponse>(thread_resp)?;

    let turn_req = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![V2UserInput::Text {
                text: "run python".to_string(),
                text_elements: Vec::new(),
            }],
            cwd: Some(working_directory),
            ..Default::default()
        })
        .await?;
    let turn_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_req)),
    )
    .await??;
    let TurnStartResponse { turn } = to_response::<TurnStartResponse>(turn_resp)?;

    let request = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_request_message(),
    )
    .await??;
    let ServerRequest::CommandExecutionRequestApproval { request_id, params } = request else {
        panic!("expected CommandExecutionRequestApproval request");
    };
    assert_eq!(params.item_id, "call_python_approval");
    assert_eq!(params.thread_id, thread.id);
    assert_eq!(params.turn_id, turn.id);

    let interrupt_id = mcp
        .send_turn_interrupt_request(TurnInterruptParams {
            thread_id: thread.id.clone(),
            turn_id: turn.id.clone(),
        })
        .await?;
    let interrupt_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(interrupt_id)),
    )
    .await??;
    let _resp: TurnInterruptResponse = to_response::<TurnInterruptResponse>(interrupt_resp)?;

    let resolved_notification = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("serverRequest/resolved"),
    )
    .await??;
    let resolved: ServerRequestResolvedNotification = serde_json::from_value(
        resolved_notification
            .params
            .clone()
            .expect("serverRequest/resolved params must be present"),
    )?;
    assert_eq!(resolved.thread_id, thread.id);
    assert_eq!(resolved.request_id, request_id);

    let completed_notif: JSONRPCNotification = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;
    let completed: TurnCompletedNotification = serde_json::from_value(
        completed_notif
            .params
            .expect("turn/completed params must be present"),
    )?;
    assert_eq!(completed.thread_id, thread.id);
    assert_eq!(completed.turn.status, TurnStatus::Interrupted);

    Ok(())
}

// Helper to create a config.toml pointing at the mock model server.
fn create_config_toml(
    codex_home: &std::path::Path,
    server_uri: &str,
    approval_policy: &str,
    sandbox_mode: &str,
) -> std::io::Result<()> {
    let config_toml = codex_home.join("config.toml");
    std::fs::write(
        config_toml,
        format!(
            r#"
model = "mock-model"
approval_policy = "{approval_policy}"
sandbox_mode = "{sandbox_mode}"

model_provider = "mock_provider"

[model_providers.mock_provider]
name = "Mock provider for test"
base_url = "{server_uri}/v1"
wire_api = "responses"
request_max_retries = 0
stream_max_retries = 0
"#
        ),
    )
}
