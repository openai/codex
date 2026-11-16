#![cfg(unix)]

use anyhow::Result;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use app_test_support::create_command_execution_sse_response;
use app_test_support::create_final_assistant_message_sse_response;
use app_test_support::create_mock_responses_server_sequence;
use app_test_support::create_mock_responses_server_sequence_unchecked;
use app_test_support::write_models_cache_with_models;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::CommandExecutionStatus;
use codex_app_server_protocol::ItemCompletedNotification;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::ServerRequestResolvedNotification;
use codex_app_server_protocol::ThreadItemPayload;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnCompletedNotification;
use codex_app_server_protocol::TurnInterruptParams;
use codex_app_server_protocol::TurnInterruptResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::TurnStartedNotification;
use codex_app_server_protocol::TurnStatus;
use codex_app_server_protocol::UserInput as V2UserInput;
use codex_features::Feature;
use codex_models_manager::model_info::model_info_from_slug;
use core_test_support::responses;
use core_test_support::skip_if_remote;
use pretty_assertions::assert_eq;
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
    let server = create_mock_responses_server_sequence_unchecked(vec![
        create_command_execution_sse_response(
            shell_command.clone(),
            Some(&working_directory),
            Some(10_000),
            "call_sleep",
        )?,
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

    let started: TurnStartedNotification =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_notification("turn/started")).await??;
    assert_eq!(started.thread_id, thread.id);
    assert_eq!(started.turn.id, turn_id);

    let thread_id = thread.id.clone();
    let interrupt_id = mcp
        .send_turn_interrupt_request(TurnInterruptParams {
            thread_id: thread_id.clone(),
            turn_id: "wrong-turn".to_string(),
        })
        .await?;
    let interrupt_err = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(interrupt_id)),
    )
    .await??;
    assert_eq!(
        interrupt_err.error,
        JSONRPCErrorError {
            code: INVALID_REQUEST_ERROR_CODE,
            message: format!("expected active turn id wrong-turn but found {turn_id}"),
            data: None,
        }
    );

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

    let repeat_id = mcp
        .send_turn_interrupt_request(TurnInterruptParams { thread_id, turn_id })
        .await?;
    let repeat_err = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(repeat_id)),
    )
    .await??;
    assert_eq!(
        repeat_err.error,
        JSONRPCErrorError {
            code: INVALID_REQUEST_ERROR_CODE,
            message: "no active turn to interrupt".to_string(),
            data: None,
        }
    );

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
    let traces = TempDir::new()?;
    let codex_home = tmp.path().join("codex_home");
    std::fs::create_dir(&codex_home)?;
    let working_directory = tmp.path().join("workdir");
    std::fs::create_dir(&working_directory)?;

    let server =
        create_mock_responses_server_sequence(vec![create_command_execution_sse_response(
            shell_command.clone(),
            Some(&working_directory),
            Some(10_000),
            "call_sleep_approval",
        )?])
        .await;
    MockResponsesConfig::new(&server.uri())
        .with_approval_policy("on-request")
        .with_root_config(r#"approvals_reviewer = "user""#)
        .write(&codex_home)?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(&codex_home)
        .with_env_overrides(&[("CODEX_ROLLOUT_TRACE_ROOT", traces.path().to_str())])
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

    assert!(mcp.shutdown_gracefully().await?.success());
    let bundles = std::fs::read_dir(traces.path())?.collect::<std::io::Result<Vec<_>>>()?;
    let [bundle] = bundles.as_slice() else {
        anyhow::bail!("expected one trace bundle, got {}", bundles.len());
    };
    let trace = codex_rollout_trace::replay_bundle(bundle.path())?;
    let call = &trace.tool_calls["call_sleep_approval"];
    assert_eq!(
        call.execution.status,
        codex_rollout_trace::ExecutionStatus::Aborted
    );
    assert!(call.execution.ended_at_unix_ms >= Some(call.execution.started_at_unix_ms));

    Ok(())
}

#[tokio::test]
async fn turn_interrupt_closes_late_command_approval_from_completed_turn() -> Result<()> {
    let codex_home = TempDir::new()?;
    let server = responses::start_mock_server().await;
    MockResponsesConfig::new(&server.uri())
        .with_model("gpt-5.4")
        .with_approval_policy("on-request")
        .with_sandbox_mode("read-only")
        .with_root_config(r#"approvals_reviewer = "user""#)
        .enable_feature(Feature::CodeMode)
        .write(codex_home.path())?;
    let mut model = model_info_from_slug("gpt-5.4");
    model.experimental_supported_tools = vec!["test_sync_tool".to_string()];
    write_models_cache_with_models(codex_home.path(), vec![model]).await?;
    let mut app = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;
    let thread = app.start_thread(ThreadStartParams::default()).await?.thread;

    // Release the first turn's cell from the second turn, then hold the second turn open.
    let release = r#"{"barrier":{"id":"release-cell","participants":2,"timeout_ms":60000}}"#;
    let hold = r#"{"barrier":{"id":"hold-until-interrupt","participants":2,"timeout_ms":60000}}"#;
    let cell = format!(
        r#"yield_control();
await tools.test_sync_tool({release});
await tools.exec_command({{cmd: "echo late", sandbox_permissions: "require_escalated", justification: "test approval"}});"#
    );
    responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_custom_tool_call("cell", "exec", &cell),
                responses::ev_completed("yield"),
            ]),
            responses::sse_completed("first-turn-done"),
            responses::sse(vec![
                responses::ev_function_call("release", "test_sync_tool", release),
                responses::ev_function_call("hold", "test_sync_tool", hold),
                responses::ev_completed("second-turn"),
            ]),
        ],
    )
    .await;
    let input = |text: &str| TurnStartParams {
        thread_id: thread.id.clone(),
        input: vec![V2UserInput::Text {
            text: text.to_string(),
            text_elements: Vec::new(),
        }],
        ..Default::default()
    };
    let first = app
        .start_turn_and_wait_for_completion(input("start background cell"))
        .await?
        .turn;
    assert_eq!(first.status, TurnStatus::Completed);
    let TurnStartResponse { turn: second } = app
        .request(|request_id| ClientRequest::TurnStart {
            request_id,
            params: input("release cell"),
        })
        .await?;
    let ServerRequest::CommandExecutionRequestApproval { params, .. } = timeout(
        DEFAULT_READ_TIMEOUT,
        app.read_stream_until_request_message(),
    )
    .await??
    else {
        anyhow::bail!("expected command approval");
    };
    assert_eq!(params.turn_id, first.id);

    app.interrupt_turn_and_wait_for_aborted(thread.id, second.id, DEFAULT_READ_TIMEOUT)
        .await?;
    let completed = timeout(DEFAULT_READ_TIMEOUT, async {
        loop {
            let event: ItemCompletedNotification = app.read_notification("item/completed").await?;
            if event.item.id() == params.item_id {
                return anyhow::Ok(event);
            }
        }
    })
    .await??;
    assert_eq!(completed.turn_id, first.id);
    assert_eq!(completed.item.started_at_ms, Some(params.started_at_ms));
    let ThreadItemPayload::CommandExecution { status, .. } = completed.item.payload else {
        anyhow::bail!("expected command execution");
    };
    assert_eq!(status, CommandExecutionStatus::Declined);
    Ok(())
}
