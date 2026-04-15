#![cfg(unix)]

use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::create_mock_responses_server_sequence;
use app_test_support::create_mock_responses_server_sequence_unchecked;
use app_test_support::create_shell_command_sse_response;
use app_test_support::to_response;
use app_test_support::write_mock_responses_config_toml_with_chatgpt_base_url;
use codex_app_server::INPUT_TOO_LARGE_ERROR_CODE;
use codex_app_server::INVALID_PARAMS_ERROR_CODE;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCNotification;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::TurnSteerParams;
use codex_app_server_protocol::TurnSteerResponse;
use codex_app_server_protocol::UserInput as V2UserInput;
use codex_protocol::user_input::MAX_USER_INPUT_TEXT_CHARS;
use tempfile::TempDir;
use tokio::time::timeout;

use super::analytics::enable_analytics_capture;
use super::analytics::wait_for_analytics_event;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

#[tokio::test]
async fn turn_steer_requires_active_turn() -> Result<()> {
    let tmp = TempDir::new()?;
    let codex_home = tmp.path().join("codex_home");
    std::fs::create_dir(&codex_home)?;

    let server = create_mock_responses_server_sequence(vec![]).await;
    write_mock_responses_config_toml_with_chatgpt_base_url(
        &codex_home,
        &server.uri(),
        &server.uri(),
    )?;
    enable_analytics_capture(&server, &codex_home).await?;

    let mut mcp = McpProcess::new_without_managed_config(&codex_home).await?;
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

    let steer_req = mcp
        .send_turn_steer_request(TurnSteerParams {
            thread_id: thread.id.clone(),
            input: vec![V2UserInput::Text {
                text: "steer".to_string(),
                text_elements: Vec::new(),
            }],
            responsesapi_client_metadata: None,
            expected_turn_id: "turn-does-not-exist".to_string(),
        })
        .await?;
    let steer_err: JSONRPCError = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(steer_req)),
    )
    .await??;
    assert_eq!(steer_err.error.code, -32600);

    let event =
        wait_for_analytics_event(&server, DEFAULT_READ_TIMEOUT, "codex_turn_steer_event").await?;
    assert_eq!(event["event_params"]["thread_id"], thread.id);
    assert_eq!(event["event_params"]["result"], "rejected");
    assert_eq!(event["event_params"]["num_input_images"], 0);
    assert_eq!(
        event["event_params"]["expected_turn_id"],
        "turn-does-not-exist"
    );
    assert_eq!(
        event["event_params"]["accepted_turn_id"],
        serde_json::Value::Null
    );
    assert_eq!(event["event_params"]["rejection_reason"], "no_active_turn");

    Ok(())
}

#[tokio::test]
async fn turn_steer_rejects_oversized_text_input() -> Result<()> {
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

    let mut mcp = McpProcess::new_without_managed_config(&codex_home).await?;
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

    let _task_started: JSONRPCNotification = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/started"),
    )
    .await??;

    let oversized_input = "x".repeat(MAX_USER_INPUT_TEXT_CHARS + 1);
    let steer_req = mcp
        .send_turn_steer_request(TurnSteerParams {
            thread_id: thread.id.clone(),
            input: vec![V2UserInput::Text {
                text: oversized_input.clone(),
                text_elements: Vec::new(),
            }],
            responsesapi_client_metadata: None,
            expected_turn_id: turn.id.clone(),
        })
        .await?;
    let steer_err: JSONRPCError = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(steer_req)),
    )
    .await??;

    assert_eq!(steer_err.error.code, INVALID_PARAMS_ERROR_CODE);
    assert_eq!(
        steer_err.error.message,
        format!("Input exceeds the maximum length of {MAX_USER_INPUT_TEXT_CHARS} characters.")
    );
    let data = steer_err
        .error
        .data
        .expect("expected structured error data");
    assert_eq!(data["input_error_code"], INPUT_TOO_LARGE_ERROR_CODE);
    assert_eq!(data["max_chars"], MAX_USER_INPUT_TEXT_CHARS);
    assert_eq!(data["actual_chars"], oversized_input.chars().count());

    mcp.interrupt_turn_and_wait_for_aborted(thread.id, turn.id, DEFAULT_READ_TIMEOUT)
        .await?;

    Ok(())
}

#[tokio::test]
async fn turn_steer_returns_active_turn_id() -> Result<()> {
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

    let mut mcp = McpProcess::new_without_managed_config(&codex_home).await?;
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

    let _task_started: JSONRPCNotification = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/started"),
    )
    .await??;

    let steer_req = mcp
        .send_turn_steer_request(TurnSteerParams {
            thread_id: thread.id.clone(),
            input: vec![V2UserInput::Text {
                text: "steer".to_string(),
                text_elements: Vec::new(),
            }],
            responsesapi_client_metadata: None,
            expected_turn_id: turn.id.clone(),
        })
        .await?;
    let steer_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(steer_req)),
    )
    .await??;
    let steer: TurnSteerResponse = to_response::<TurnSteerResponse>(steer_resp)?;
    assert_eq!(steer.turn_id, turn.id);

    let event =
        wait_for_analytics_event(&server, DEFAULT_READ_TIMEOUT, "codex_turn_steer_event").await?;
    assert_eq!(event["event_params"]["thread_id"], thread.id);
    assert_eq!(event["event_params"]["result"], "accepted");
    assert_eq!(event["event_params"]["num_input_images"], 0);
    assert_eq!(event["event_params"]["expected_turn_id"], turn.id);
    assert_eq!(event["event_params"]["accepted_turn_id"], turn.id);
    assert_eq!(
        event["event_params"]["rejection_reason"],
        serde_json::Value::Null
    );

    mcp.interrupt_turn_and_wait_for_aborted(thread.id, steer.turn_id, DEFAULT_READ_TIMEOUT)
        .await?;

    Ok(())
}
