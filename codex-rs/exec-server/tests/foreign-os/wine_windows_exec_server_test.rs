#[cfg(not(target_os = "linux"))]
compile_error!("the Wine exec-server test can only run on Linux");

use std::collections::BTreeMap;

use anyhow::Context;
use anyhow::Result;
use app_test_support::TestAppServer;
use app_test_support::create_final_assistant_message_sse_response;
use app_test_support::create_mock_responses_server_sequence;
use app_test_support::to_response;
use app_test_support::write_mock_responses_config_toml;
use codex_app_server_protocol::CommandExecutionStatus;
use codex_app_server_protocol::ItemCompletedNotification;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::SandboxMode;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnCompletedNotification;
use codex_app_server_protocol::TurnEnvironmentParams;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::TurnStatus;
use codex_app_server_protocol::UserInput;
use codex_exec_server::CODEX_EXEC_SERVER_URL_ENV_VAR;
use codex_exec_server::REMOTE_ENVIRONMENT_ID;
use codex_features::Feature;
use core_test_support::responses;
use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tokio::process::ChildStdout;
use wine_test_support::WineTestCommand;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn app_server_records_host_shell_mismatch_for_windows_exec_server_under_wine() -> Result<()> {
    let executable = codex_utils_cargo_bin::cargo_bin("wine-windows-exec-server")?;
    let mut server = WineTestCommand::new(executable)
        .env("CODEX_HOME", r"C:\codex-home")
        .spawn()?;
    let stdout = server.take_stdout();

    server.scope(exercise_through_app_server(stdout)).await
}

async fn exercise_through_app_server(stdout: ChildStdout) -> Result<()> {
    let mut lines = BufReader::new(stdout).lines();
    let websocket_url = loop {
        let line = lines
            .next_line()
            .await?
            .context("Wine exec-server exited before reporting its URL")?;
        if line.starts_with("ws://") {
            break line;
        }
    };

    let responses_server = create_mock_responses_server_sequence(vec![
        exec_command_response("wine-cmd-smoke")?,
        create_final_assistant_message_sse_response("done")?,
    ])
    .await;
    let codex_home = TempDir::new()?;
    write_mock_responses_config_toml(
        codex_home.path(),
        &responses_server.uri(),
        &BTreeMap::from([(Feature::UnifiedExec, true)]),
        /*auto_compact_limit*/ 1_000_000,
        /*requires_openai_auth*/ None,
        "mock_provider",
        "compact",
    )?;

    let app_server_program =
        codex_utils_cargo_bin::find_resource!("../../../app-server/codex-app-server")?;
    let mut app_server = TestAppServer::new_with_program_and_env(
        codex_home.path(),
        &app_server_program,
        &[(CODEX_EXEC_SERVER_URL_ENV_VAR, Some(&websocket_url))],
    )
    .await?;
    app_server.initialize().await?;

    let remote_environment = TurnEnvironmentParams {
        environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
        cwd: codex_home.path().to_path_buf().try_into()?,
    };
    let remote_cwd = remote_environment.cwd.clone();
    let thread_request_id = app_server
        .send_thread_start_request(ThreadStartParams {
            model: Some("mock-model".to_string()),
            sandbox: Some(SandboxMode::DangerFullAccess),
            environments: Some(vec![remote_environment]),
            ..Default::default()
        })
        .await?;
    let thread_response: JSONRPCResponse = app_server
        .read_stream_until_response_message(RequestId::Integer(thread_request_id))
        .await?;
    let ThreadStartResponse { thread, .. } = to_response(thread_response)?;

    let turn_request_id = app_server
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id,
            input: vec![UserInput::Text {
                text: "run the Windows smoke command".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    let turn_response: JSONRPCResponse = app_server
        .read_stream_until_response_message(RequestId::Integer(turn_request_id))
        .await?;
    let TurnStartResponse { turn } = to_response(turn_response)?;

    let command_item = loop {
        let notification = app_server
            .read_stream_until_notification_message("item/completed")
            .await?;
        let completed: ItemCompletedNotification = serde_json::from_value(
            notification
                .params
                .context("item/completed notification should include params")?,
        )?;
        if matches!(completed.item, ThreadItem::CommandExecution { .. }) {
            break completed.item;
        }
    };
    let ThreadItem::CommandExecution {
        command,
        cwd,
        status,
        aggregated_output,
        exit_code,
        ..
    } = command_item
    else {
        unreachable!("loop exits only for a command execution item");
    };
    assert_eq!(cwd, remote_cwd);
    // This intentionally records the current cross-OS failure mode: the Linux
    // orchestrator resolves its own shell before sending the command to the
    // Windows exec-server, where that Unix shell cannot start.
    assert!(
        command.starts_with("/bin/bash -c") || command.starts_with("/bin/sh -c"),
        "unexpected command: {command:?}"
    );
    assert_eq!(status, CommandExecutionStatus::Failed);
    assert_eq!(aggregated_output, None);
    assert_eq!(exit_code, Some(-1));

    let completed_notification = app_server
        .read_stream_until_notification_message("turn/completed")
        .await?;
    let completed: TurnCompletedNotification = serde_json::from_value(
        completed_notification
            .params
            .context("turn/completed notification should include params")?,
    )?;
    assert_eq!(completed.turn.id, turn.id);
    assert_eq!(completed.turn.status, TurnStatus::Completed);

    Ok(())
}

fn exec_command_response(call_id: &str) -> Result<String> {
    let arguments = serde_json::to_string(&json!({
        "cmd": "echo WINE_BAZEL_OK&&cd",
        "login": false,
        "yield_time_ms": 5_000,
    }))?;
    Ok(responses::sse(vec![
        responses::ev_response_created("resp-1"),
        responses::ev_function_call(call_id, "exec_command", &arguments),
        responses::ev_completed("resp-1"),
    ]))
}
