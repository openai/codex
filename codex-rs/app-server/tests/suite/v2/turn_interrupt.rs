#![cfg(unix)]

use anyhow::Result;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use app_test_support::create_final_assistant_message_sse_response;
use app_test_support::create_mock_responses_server_sequence;
use app_test_support::create_mock_responses_server_sequence_unchecked;
use app_test_support::create_shell_command_sse_response;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::JSONRPCError;
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
use core_test_support::skip_if_remote;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
const INVALID_REQUEST_ERROR_CODE: i64 = -32600;

#[tokio::test]
async fn turn_interrupt_aborts_running_turn() -> Result<()> {
    // TODO(anp): Remove after the long-running command fixture can run in the selected remote environment.
    skip_if_remote!(
        Ok(()),
        "uses a host-local command and cwd fixture unavailable to remote executors"
    );

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
    MockResponsesConfig::new(&server.uri())
        .with_sandbox_mode("workspace-write")
        .with_root_config(r#"approvals_reviewer = "user""#)
        .write(&codex_home)?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(&codex_home)
        .build_initialized()
        .await?;

    // Start a v2 thread and capture its id.
    let ThreadStartResponse { thread, .. } = mcp
        .start_thread(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;

    // Start a turn that triggers a long-running command.
    let TurnStartResponse { turn } = mcp
        .request(|request_id| ClientRequest::TurnStart {
            request_id,
            params: TurnStartParams {
                thread_id: thread.id.clone(),
                client_user_message_id: None,
                input: vec![V2UserInput::Text {
                    text: "run sleep".to_string(),
                    text_elements: Vec::new(),
                }],
                cwd: Some(working_directory.clone()),
                ..Default::default()
            },
        })
        .await?;
    let turn_id = turn.id.clone();

    // Give the command a brief moment to start.
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    let thread_id = thread.id.clone();
    // Interrupt the in-progress turn by id (v2 API).
    let _: TurnInterruptResponse = mcp
        .request(|request_id| ClientRequest::TurnInterrupt {
            request_id,
            params: TurnInterruptParams {
                thread_id: thread_id.clone(),
                turn_id: turn_id.clone(),
            },
        })
        .await?;

    let completed: TurnCompletedNotification = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_notification("turn/completed"),
    )
    .await??;
    assert_eq!(completed.thread_id, thread_id);
    assert_eq!(completed.turn.status, TurnStatus::Interrupted);

    Ok(())
}

#[tokio::test]
async fn turn_interrupt_rejects_completed_turn() -> Result<()> {
    let tmp = TempDir::new()?;
    let codex_home = tmp.path().join("codex_home");
    std::fs::create_dir(&codex_home)?;

    let server = create_mock_responses_server_sequence_unchecked(vec![
        create_final_assistant_message_sse_response("done")?,
    ])
    .await;
    MockResponsesConfig::new(&server.uri())
        .with_sandbox_mode("workspace-write")
        .with_root_config(r#"approvals_reviewer = "user""#)
        .write(&codex_home)?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(&codex_home)
        .build_initialized()
        .await?;

    let ThreadStartResponse { thread, .. } = mcp
        .start_thread(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;

    let TurnStartResponse { turn } = mcp
        .request(|request_id| ClientRequest::TurnStart {
            request_id,
            params: TurnStartParams {
                thread_id: thread.id.clone(),
                client_user_message_id: None,
                input: vec![V2UserInput::Text {
                    text: "say done".to_string(),
                    text_elements: Vec::new(),
                }],
                ..Default::default()
            },
        })
        .await?;

    let completed: TurnCompletedNotification = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_notification("turn/completed"),
    )
    .await??;
    assert_eq!(completed.thread_id, thread.id);
    assert_eq!(completed.turn.id, turn.id);
    assert_eq!(completed.turn.status, TurnStatus::Completed);

    let interrupt_id = mcp
        .send_turn_interrupt_request(TurnInterruptParams {
            thread_id: thread.id,
            turn_id: turn.id,
        })
        .await?;

    let interrupt_err: JSONRPCError = timeout(
        std::time::Duration::from_millis(500),
        mcp.read_stream_until_error_message(RequestId::Integer(interrupt_id)),
    )
    .await??;
    assert_eq!(interrupt_err.error.code, INVALID_REQUEST_ERROR_CODE);

    Ok(())
}

#[tokio::test]
async fn turn_interrupt_resolves_pending_command_approval_request() -> Result<()> {
    // TODO(anp): Remove after the approval command fixture can run in the selected remote environment.
    skip_if_remote!(
        Ok(()),
        "uses a host-local command and cwd fixture unavailable to remote executors"
    );

    #[cfg(target_os = "windows")]
    let shell_command = vec![
        "powershell".to_string(),
        "-Command".to_string(),
        "Start-Sleep -Seconds 10".to_string(),
    ];
    #[cfg(not(target_os = "windows"))]
    let shell_command = vec![
        "python3".to_string(),
        "-c".to_string(),
        "import time; time.sleep(10)".to_string(),
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
        "call_sleep_approval",
    )?])
    .await;
    MockResponsesConfig::new(&server.uri())
        .with_approval_policy("untrusted")
        .with_root_config(r#"approvals_reviewer = "user""#)
        .write(&codex_home)?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(&codex_home)
        .build_initialized()
        .await?;

    let ThreadStartResponse { thread, .. } = mcp
        .start_thread(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;

    let TurnStartResponse { turn } = mcp
        .request(|request_id| ClientRequest::TurnStart {
            request_id,
            params: TurnStartParams {
                thread_id: thread.id.clone(),
                client_user_message_id: None,
                input: vec![V2UserInput::Text {
                    text: "run python".to_string(),
                    text_elements: Vec::new(),
                }],
                cwd: Some(working_directory),
                approval_policy: Some(codex_app_server_protocol::AskForApproval::UnlessTrusted),
                ..Default::default()
            },
        })
        .await?;

    let request = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_request_message(),
    )
    .await??;
    let ServerRequest::CommandExecutionRequestApproval { request_id, params } = request else {
        panic!("expected CommandExecutionRequestApproval request");
    };
    assert_eq!(params.item_id, "call_sleep_approval");
    assert_eq!(params.thread_id, thread.id);
    assert_eq!(params.turn_id, turn.id);

    let _: TurnInterruptResponse = mcp
        .request(|request_id| ClientRequest::TurnInterrupt {
            request_id,
            params: TurnInterruptParams {
                thread_id: thread.id.clone(),
                turn_id: turn.id.clone(),
            },
        })
        .await?;

    let resolved: ServerRequestResolvedNotification = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_notification("serverRequest/resolved"),
    )
    .await??;
    assert_eq!(resolved.thread_id, thread.id);
    assert_eq!(resolved.request_id, request_id);

    let completed: TurnCompletedNotification = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_notification("turn/completed"),
    )
    .await??;
    assert_eq!(completed.thread_id, thread.id);
    assert_eq!(completed.turn.status, TurnStatus::Interrupted);

    Ok(())
}
