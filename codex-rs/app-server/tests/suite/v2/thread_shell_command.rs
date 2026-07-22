use anyhow::Result;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use app_test_support::create_final_assistant_message_sse_response;
use app_test_support::create_mock_responses_server_sequence;
use app_test_support::create_shell_command_sse_response;
use app_test_support::format_with_current_shell_display;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::CommandExecutionApprovalDecision;
use codex_app_server_protocol::CommandExecutionOutputDeltaNotification;
use codex_app_server_protocol::CommandExecutionRequestApprovalResponse;
use codex_app_server_protocol::CommandExecutionSource;
use codex_app_server_protocol::CommandExecutionStatus;
use codex_app_server_protocol::ItemCompletedNotification;
use codex_app_server_protocol::ItemStartedNotification;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::SortDirection;
use codex_app_server_protocol::ThreadForkParams;
use codex_app_server_protocol::ThreadForkResponse;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadReadParams;
use codex_app_server_protocol::ThreadReadResponse;
use codex_app_server_protocol::ThreadShellCommandParams;
use codex_app_server_protocol::ThreadShellCommandResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::ThreadTurnsListParams;
use codex_app_server_protocol::ThreadTurnsListResponse;
use codex_app_server_protocol::TurnCompletedNotification;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::UserInput as V2UserInput;
use codex_core::shell::default_user_shell;
use codex_exec_server::CODEX_EXEC_SERVER_URL_ENV_VAR;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[tokio::test]
async fn thread_shell_command_history_responses_exclude_persisted_command_executions() -> Result<()>
{
    let tmp = TempDir::new()?;
    let codex_home = tmp.path().join("codex_home");
    std::fs::create_dir(&codex_home)?;
    let workspace = tmp.path().join("workspace");
    std::fs::create_dir(&workspace)?;

    let server = create_mock_responses_server_sequence(vec![]).await;
    MockResponsesConfig::new(&server.uri()).write(&codex_home)?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.as_path())
        // thread/shellCommand intentionally executes on the app-server host.
        .without_auto_env()
        .build_initialized()
        .await?;
    let ThreadStartResponse { thread, .. } = mcp
        .request(|request_id| ClientRequest::ThreadStart {
            request_id,
            params: ThreadStartParams::default(),
        })
        .await?;
    let (shell_command, expected_output) = current_shell_output_command("hello from bang")?;

    let _: ThreadShellCommandResponse = mcp
        .request(|request_id| ClientRequest::ThreadShellCommand {
            request_id,
            params: ThreadShellCommandParams {
                thread_id: thread.id.clone(),
                command: shell_command,
            },
        })
        .await?;

    let started = wait_for_command_execution_started(&mut mcp, /*expected_id*/ None).await?;
    let ThreadItem::CommandExecution {
        id, source, status, ..
    } = &started.item
    else {
        unreachable!("helper returns command execution item");
    };
    let command_id = id.clone();
    assert_eq!(source, &CommandExecutionSource::UserShell);
    assert_eq!(status, &CommandExecutionStatus::InProgress);

    let delta = wait_for_command_execution_output_delta(&mut mcp, &command_id).await?;
    assert_eq!(
        delta.delta.trim_end_matches(['\r', '\n']),
        expected_output.trim_end_matches(['\r', '\n'])
    );

    let completed = wait_for_command_execution_completed(&mut mcp, Some(&command_id)).await?;
    let ThreadItem::CommandExecution {
        id,
        source,
        status,
        aggregated_output,
        exit_code,
        ..
    } = &completed.item
    else {
        unreachable!("helper returns command execution item");
    };
    assert_eq!(id, &command_id);
    assert_eq!(source, &CommandExecutionSource::UserShell);
    assert_eq!(status, &CommandExecutionStatus::Completed);
    assert_eq!(aggregated_output.as_deref(), Some(expected_output.as_str()));
    assert_eq!(*exit_code, Some(0));

    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let ThreadReadResponse { thread, .. } = mcp
        .request(|request_id| ClientRequest::ThreadRead {
            request_id,
            params: ThreadReadParams {
                thread_id: thread.id.clone(),
                include_turns: true,
            },
        })
        .await?;
    assert_eq!(thread.turns.len(), 1);
    assert_no_command_executions(&thread.turns[0].items, "thread/read");

    let ThreadTurnsListResponse { data, .. } = mcp
        .request(|request_id| ClientRequest::ThreadTurnsList {
            request_id,
            params: ThreadTurnsListParams {
                thread_id: thread.id.clone(),
                cursor: None,
                limit: None,
                sort_direction: Some(SortDirection::Asc),
                items_view: None,
            },
        })
        .await?;
    assert_eq!(data.len(), 1);
    assert_no_command_executions(&data[0].items, "thread/turns/list");

    let ThreadForkResponse { thread, .. } = mcp
        .request(|request_id| ClientRequest::ThreadFork {
            request_id,
            params: ThreadForkParams {
                thread_id: thread.id,
                ..Default::default()
            },
        })
        .await?;
    assert_eq!(thread.turns.len(), 1);
    assert_no_command_executions(&thread.turns[0].items, "thread/fork");

    Ok(())
}

#[tokio::test]
async fn thread_shell_command_returns_error_when_local_environment_is_disabled() -> Result<()> {
    let tmp = TempDir::new()?;
    let codex_home = tmp.path().join("codex_home");
    std::fs::create_dir(&codex_home)?;
    let server = create_mock_responses_server_sequence(vec![]).await;
    MockResponsesConfig::new(&server.uri()).write(&codex_home)?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.as_path())
        // This test intentionally exercises thread/shellCommand without a local host environment.
        .without_auto_env()
        .with_env_overrides(&[(CODEX_EXEC_SERVER_URL_ENV_VAR, Some("none"))])
        .build_initialized()
        .await?;
    let ThreadStartResponse { thread, .. } = mcp
        .request(|request_id| ClientRequest::ThreadStart {
            request_id,
            params: ThreadStartParams::default(),
        })
        .await?;
    let shell_id = mcp
        .send_thread_shell_command_request(ThreadShellCommandParams {
            thread_id: thread.id,
            command: "pwd".to_string(),
        })
        .await?;
    let error = mcp
        .read_stream_until_error_message(RequestId::Integer(shell_id))
        .await?;
    assert_eq!(error.error.message, "local environment is not configured");

    Ok(())
}

#[tokio::test]
async fn thread_shell_command_uses_existing_active_turn() -> Result<()> {
    let tmp = TempDir::new()?;
    let codex_home = tmp.path().join("codex_home");
    std::fs::create_dir(&codex_home)?;
    let workspace = tmp.path().join("workspace");
    std::fs::create_dir(&workspace)?;

    let responses = vec![
        create_shell_command_sse_response(
            vec![
                "python3".to_string(),
                "-c".to_string(),
                "print(42)".to_string(),
            ],
            /*workdir*/ None,
            Some(5000),
            "call-approve",
        )?,
        create_final_assistant_message_sse_response("done")?,
    ];
    let server = create_mock_responses_server_sequence(responses).await;
    MockResponsesConfig::new(&server.uri())
        .with_approval_policy("untrusted")
        .write(&codex_home)?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.as_path())
        // thread/shellCommand intentionally joins the app-server's host-local active turn.
        .without_auto_env()
        .build_initialized()
        .await?;
    let ThreadStartResponse { thread, .. } = mcp
        .request(|request_id| ClientRequest::ThreadStart {
            request_id,
            params: ThreadStartParams::default(),
        })
        .await?;
    let (shell_command, expected_output) = current_shell_output_command("active turn bang")?;

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
                cwd: Some(workspace.clone()),
                ..Default::default()
            },
        })
        .await?;

    let agent_started = wait_for_command_execution_started(&mut mcp, Some("call-approve")).await?;
    let ThreadItem::CommandExecution {
        command, source, ..
    } = &agent_started.item
    else {
        unreachable!("helper returns command execution item");
    };
    assert_eq!(source, &CommandExecutionSource::Agent);
    assert_eq!(
        command,
        &format_with_current_shell_display("python3 -c 'print(42)'")
    );

    let server_req = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_request_message(),
    )
    .await??;
    let ServerRequest::CommandExecutionRequestApproval { request_id, .. } = server_req else {
        panic!("expected approval request");
    };

    let _: ThreadShellCommandResponse = mcp
        .request(|request_id| ClientRequest::ThreadShellCommand {
            request_id,
            params: ThreadShellCommandParams {
                thread_id: thread.id.clone(),
                command: shell_command,
            },
        })
        .await?;

    let started =
        wait_for_command_execution_started_by_source(&mut mcp, CommandExecutionSource::UserShell)
            .await?;
    assert_eq!(started.turn_id, turn.id);
    let command_id = match &started.item {
        ThreadItem::CommandExecution { id, .. } => id.clone(),
        _ => unreachable!("helper returns command execution item"),
    };
    let completed = wait_for_command_execution_completed(&mut mcp, Some(&command_id)).await?;
    assert_eq!(completed.turn_id, turn.id);
    let ThreadItem::CommandExecution {
        source,
        aggregated_output,
        ..
    } = &completed.item
    else {
        unreachable!("helper returns command execution item");
    };
    assert_eq!(source, &CommandExecutionSource::UserShell);
    assert_eq!(aggregated_output.as_deref(), Some(expected_output.as_str()));

    mcp.send_response(
        request_id,
        serde_json::to_value(CommandExecutionRequestApprovalResponse {
            decision: CommandExecutionApprovalDecision::Decline,
        })?,
    )
    .await?;
    let _: TurnCompletedNotification = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_notification("turn/completed"),
    )
    .await??;
    let ThreadReadResponse { thread, .. } = mcp
        .request(|request_id| ClientRequest::ThreadRead {
            request_id,
            params: ThreadReadParams {
                thread_id: thread.id,
                include_turns: true,
            },
        })
        .await?;
    assert_eq!(thread.turns.len(), 1);
    assert_no_command_executions(&thread.turns[0].items, "thread/read");

    Ok(())
}

fn assert_no_command_executions(items: &[ThreadItem], context: &str) {
    assert!(
        items
            .iter()
            .all(|item| !matches!(item, ThreadItem::CommandExecution { .. })),
        "{context} should always exclude command executions from returned turns"
    );
}

fn current_shell_output_command(text: &str) -> Result<(String, String)> {
    let command_and_output = match default_user_shell().name() {
        "powershell" => {
            let escaped_text = text.replace('\'', "''");
            (
                format!("Write-Output '{escaped_text}'"),
                format!("{text}\r\n"),
            )
        }
        "cmd" => (format!("echo {text}"), format!("{text}\r\n")),
        _ => {
            let quoted_text = shlex::try_quote(text)?;
            (format!("printf '%s\\n' {quoted_text}"), format!("{text}\n"))
        }
    };
    Ok(command_and_output)
}

async fn wait_for_command_execution_started(
    mcp: &mut TestAppServer,
    expected_id: Option<&str>,
) -> Result<ItemStartedNotification> {
    loop {
        let started: ItemStartedNotification = mcp.read_notification("item/started").await?;
        let ThreadItem::CommandExecution { id, .. } = &started.item else {
            continue;
        };
        if expected_id.is_none() || expected_id == Some(id.as_str()) {
            return Ok(started);
        }
    }
}

async fn wait_for_command_execution_started_by_source(
    mcp: &mut TestAppServer,
    expected_source: CommandExecutionSource,
) -> Result<ItemStartedNotification> {
    loop {
        let started = wait_for_command_execution_started(mcp, /*expected_id*/ None).await?;
        let ThreadItem::CommandExecution { source, .. } = &started.item else {
            continue;
        };
        if source == &expected_source {
            return Ok(started);
        }
    }
}

async fn wait_for_command_execution_completed(
    mcp: &mut TestAppServer,
    expected_id: Option<&str>,
) -> Result<ItemCompletedNotification> {
    loop {
        let completed: ItemCompletedNotification = mcp.read_notification("item/completed").await?;
        let ThreadItem::CommandExecution { id, .. } = &completed.item else {
            continue;
        };
        if expected_id.is_none() || expected_id == Some(id.as_str()) {
            return Ok(completed);
        }
    }
}

async fn wait_for_command_execution_output_delta(
    mcp: &mut TestAppServer,
    item_id: &str,
) -> Result<CommandExecutionOutputDeltaNotification> {
    loop {
        let delta: CommandExecutionOutputDeltaNotification = mcp
            .read_notification("item/commandExecution/outputDelta")
            .await?;
        if delta.item_id == item_id {
            return Ok(delta);
        }
    }
}
