use anyhow::Result;
use app_test_support::TestAppServer;
use app_test_support::create_final_assistant_message_sse_response;
use app_test_support::create_mock_responses_server_sequence;
use app_test_support::create_shell_command_sse_response;
use app_test_support::to_response;
use app_test_support::write_mock_responses_config_toml;
use codex_app_server_protocol::CommandExecutionStatus;
use codex_app_server_protocol::ItemCompletedNotification;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::UserInput as V2UserInput;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(10);

#[tokio::test]
async fn thread_start_with_auto_env_uses_fixture_cwd() -> Result<()> {
    let responses = vec![
        create_shell_command_sse_response(
            vec!["echo".to_string(), "auto-env-ok".to_string()],
            /*workdir*/ None,
            /*timeout_ms*/ None,
            "cwd-call",
        )?,
        create_final_assistant_message_sse_response("done")?,
    ];
    let server = create_mock_responses_server_sequence(responses).await;
    let codex_home = TempDir::new()?;
    write_mock_responses_config_toml(
        codex_home.path(),
        &server.uri(),
        &BTreeMap::new(),
        /*auto_compact_limit*/ 100_000,
        /*requires_openai_auth*/ None,
        "mock_provider",
        "compact",
    )?;

    let mut mcp = TestAppServer::new_with_auto_env(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;
    let expected_environment = mcp.auto_env_params()?;

    let err = mcp
        .send_thread_start_request_with_auto_env(ThreadStartParams {
            environments: Some(Vec::new()),
            ..Default::default()
        })
        .await
        .expect_err("the auto-env helper should reject caller-supplied environments");
    assert_eq!(
        err.to_string(),
        "send_thread_start_request_with_auto_env requires params.environments to be omitted"
    );

    let request_id = mcp
        .send_thread_start_request_with_auto_env(ThreadStartParams::default())
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response::<ThreadStartResponse>(response)?;

    let request_id = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id,
            input: vec![V2UserInput::Text {
                text: "report the current directory".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    let _: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;

    let command = timeout(DEFAULT_READ_TIMEOUT, async {
        loop {
            let notification = mcp
                .read_stream_until_notification_message("item/completed")
                .await?;
            let completed: ItemCompletedNotification = serde_json::from_value(
                notification
                    .params
                    .expect("item/completed params must be present"),
            )?;
            if let ThreadItem::CommandExecution { .. } = completed.item {
                return Ok::<ThreadItem, anyhow::Error>(completed.item);
            }
        }
    })
    .await??;
    let ThreadItem::CommandExecution {
        cwd,
        status,
        exit_code,
        ..
    } = command
    else {
        unreachable!("loop returns only command execution items");
    };
    assert_eq!(
        (cwd, status, exit_code),
        (
            expected_environment.cwd,
            CommandExecutionStatus::Completed,
            Some(0)
        )
    );

    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    Ok(())
}

#[tokio::test]
async fn auto_env_rejects_explicit_environment_config() -> Result<()> {
    let codex_home = TempDir::new()?;
    std::fs::write(codex_home.path().join("environments.toml"), "")?;

    let result = TestAppServer::new_with_auto_env(codex_home.path()).await;
    let Err(err) = result else {
        anyhow::bail!("auto-env construction unexpectedly succeeded");
    };
    assert_eq!(
        err.to_string(),
        format!(
            "new_with_auto_env cannot be used when {} exists",
            codex_home.path().join("environments.toml").display()
        )
    );

    Ok(())
}
