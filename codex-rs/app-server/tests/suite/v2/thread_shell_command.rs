use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::create_final_assistant_message_sse_response;
use app_test_support::create_mock_responses_server_sequence;
use app_test_support::create_shell_command_sse_response;
use app_test_support::format_with_current_shell_display;
use app_test_support::to_response;
use codex_app_server_protocol::CommandExecutionApprovalDecision;
use codex_app_server_protocol::CommandExecutionOutputDeltaNotification;
use codex_app_server_protocol::CommandExecutionRequestApprovalResponse;
use codex_app_server_protocol::CommandExecutionSource;
use codex_app_server_protocol::CommandExecutionStatus;
use codex_app_server_protocol::ItemCompletedNotification;
use codex_app_server_protocol::ItemStartedNotification;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadReadParams;
use codex_app_server_protocol::ThreadReadResponse;
use codex_app_server_protocol::ThreadShellCommandParams;
use codex_app_server_protocol::ThreadShellCommandResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnCompletedNotification;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::UserInput as V2UserInput;
use codex_core::shell::default_user_shell;
use codex_features::FEATURES;
use codex_features::Feature;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use std::path::Path;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[tokio::test]
async fn thread_shell_command_runs_as_standalone_turn_and_persists_history() -> Result<()> {
    let tmp = TempDir::new()?;
    let codex_home = tmp.path().join("codex_home");
    std::fs::create_dir(&codex_home)?;
    let workspace = tmp.path().join("workspace");
    std::fs::create_dir(&workspace)?;

    let server = create_mock_responses_server_sequence(vec![]).await;
    create_config_toml(
        codex_home.as_path(),
        &server.uri(),
        "never",
        &BTreeMap::default(),
    )?;

    let mut mcp = McpProcess::new(codex_home.as_path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let start_id = mcp
        .send_thread_start_request(ThreadStartParams {
            persist_extended_history: true,
            ..Default::default()
        })
        .await?;
    let start_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(start_id)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response::<ThreadStartResponse>(start_resp)?;
    let (shell_command, expected_output) = current_shell_output_command("hello from bang")?;

    let shell_id = mcp
        .send_thread_shell_command_request(ThreadShellCommandParams {
            thread_id: thread.id.clone(),
            command: shell_command,
        })
        .await?;
    let shell_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(shell_id)),
    )
    .await??;
    let _: ThreadShellCommandResponse = to_response::<ThreadShellCommandResponse>(shell_resp)?;

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

    let read_id = mcp
        .send_thread_read_request(ThreadReadParams {
            thread_id: thread.id,
            include_turns: true,
        })
        .await?;
    let read_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(read_id)),
    )
    .await??;
    let ThreadReadResponse { thread } = to_response::<ThreadReadResponse>(read_resp)?;
    assert_eq!(thread.turns.len(), 1);
    let ThreadItem::CommandExecution {
        source,
        status,
        aggregated_output,
        ..
    } = thread.turns[0]
        .items
        .iter()
        .find(|item| matches!(item, ThreadItem::CommandExecution { .. }))
        .expect("expected persisted command execution item")
    else {
        unreachable!("matched command execution item");
    };
    assert_eq!(source, &CommandExecutionSource::UserShell);
    assert_eq!(status, &CommandExecutionStatus::Completed);
    assert_eq!(aggregated_output.as_deref(), Some(expected_output.as_str()));

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
            Some(10_000),
            "call-approve",
        )?,
        create_final_assistant_message_sse_response("done")?,
    ];
    let server = create_mock_responses_server_sequence(responses).await;
    create_config_toml(
        codex_home.as_path(),
        &server.uri(),
        "untrusted",
        &BTreeMap::default(),
    )?;

    let mut mcp = McpProcess::new(codex_home.as_path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let start_id = mcp
        .send_thread_start_request(ThreadStartParams {
            persist_extended_history: true,
            ..Default::default()
        })
        .await?;
    let start_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(start_id)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response::<ThreadStartResponse>(start_resp)?;
    let (shell_command, expected_output) = current_shell_output_command("active turn bang")?;

    let turn_id = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![V2UserInput::Text {
                text: "run python".to_string(),
                text_elements: Vec::new(),
            }],
            cwd: Some(workspace.clone()),
            ..Default::default()
        })
        .await?;
    let turn_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_id)),
    )
    .await??;
    let TurnStartResponse { turn } = to_response::<TurnStartResponse>(turn_resp)?;

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

    let shell_id = mcp
        .send_thread_shell_command_request(ThreadShellCommandParams {
            thread_id: thread.id.clone(),
            command: shell_command,
        })
        .await?;
    let shell_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(shell_id)),
    )
    .await??;
    let _: ThreadShellCommandResponse = to_response::<ThreadShellCommandResponse>(shell_resp)?;

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
    let _: TurnCompletedNotification = serde_json::from_value(
        timeout(
            DEFAULT_READ_TIMEOUT,
            mcp.read_stream_until_notification_message("turn/completed"),
        )
        .await??
        .params
        .expect("turn/completed params"),
    )?;

    let read_id = mcp
        .send_thread_read_request(ThreadReadParams {
            thread_id: thread.id,
            include_turns: true,
        })
        .await?;
    let read_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(read_id)),
    )
    .await??;
    let ThreadReadResponse { thread } = to_response::<ThreadReadResponse>(read_resp)?;
    assert_eq!(thread.turns.len(), 1);
    assert!(
        thread.turns[0].items.iter().any(|item| {
            matches!(
                item,
                ThreadItem::CommandExecution {
                    source: CommandExecutionSource::UserShell,
                    aggregated_output,
                    ..
                } if aggregated_output.as_deref() == Some(expected_output.as_str())
            )
        }),
        "expected active-turn shell command to be persisted on the existing turn"
    );

    Ok(())
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
    mcp: &mut McpProcess,
    expected_id: Option<&str>,
) -> Result<ItemStartedNotification> {
    loop {
        let notif = mcp
            .read_stream_until_notification_message("item/started")
            .await?;
        let started: ItemStartedNotification = serde_json::from_value(
            notif
                .params
                .ok_or_else(|| anyhow::anyhow!("missing item/started params"))?,
        )?;
        let ThreadItem::CommandExecution { id, .. } = &started.item else {
            continue;
        };
        if expected_id.is_none() || expected_id == Some(id.as_str()) {
            return Ok(started);
        }
    }
}

async fn wait_for_command_execution_started_by_source(
    mcp: &mut McpProcess,
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
    mcp: &mut McpProcess,
    expected_id: Option<&str>,
) -> Result<ItemCompletedNotification> {
    loop {
        let notif = mcp
            .read_stream_until_notification_message("item/completed")
            .await?;
        let completed: ItemCompletedNotification = serde_json::from_value(
            notif
                .params
                .ok_or_else(|| anyhow::anyhow!("missing item/completed params"))?,
        )?;
        let ThreadItem::CommandExecution { id, .. } = &completed.item else {
            continue;
        };
        if expected_id.is_none() || expected_id == Some(id.as_str()) {
            return Ok(completed);
        }
    }
}

async fn wait_for_command_execution_output_delta(
    mcp: &mut McpProcess,
    item_id: &str,
) -> Result<CommandExecutionOutputDeltaNotification> {
    loop {
        let notif = mcp
            .read_stream_until_notification_message("item/commandExecution/outputDelta")
            .await?;
        let delta: CommandExecutionOutputDeltaNotification = serde_json::from_value(
            notif
                .params
                .ok_or_else(|| anyhow::anyhow!("missing output delta params"))?,
        )?;
        if delta.item_id == item_id {
            return Ok(delta);
        }
    }
}

fn create_config_toml(
    codex_home: &Path,
    server_uri: &str,
    approval_policy: &str,
    feature_flags: &BTreeMap<Feature, bool>,
) -> std::io::Result<()> {
    let mut features = BTreeMap::new();
    features.insert(Feature::Apps, false);
    features.insert(Feature::Plugins, false);
    for (feature, enabled) in feature_flags {
        features.insert(*feature, *enabled);
    }
    let feature_entries = features
        .iter()
        .map(|(feature, enabled)| {
            let key = FEATURES
                .iter()
                .find(|spec| spec.id == *feature)
                .map(|spec| spec.key)
                .unwrap_or_else(|| panic!("missing feature key for {feature:?}"));
            format!("{key} = {enabled}")
        })
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(
        codex_home.join("config.toml"),
        format!(
            r#"
model = "mock-model"
approval_policy = "{approval_policy}"
sandbox_mode = "read-only"

model_provider = "mock_provider"

[features]
{feature_entries}

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
