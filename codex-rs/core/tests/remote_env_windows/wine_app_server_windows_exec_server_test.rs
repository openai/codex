#![cfg(target_os = "linux")]

#[path = "wine_exec_server_harness.rs"]
mod wine_exec_server_harness;

use std::fs;
use std::path::Path;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use app_test_support::TestAppServer;
use app_test_support::to_response;
use codex_app_server_protocol::AskForApproval;
use codex_app_server_protocol::CommandExecutionStatus;
use codex_app_server_protocol::ItemCompletedNotification;
use codex_app_server_protocol::ItemStartedNotification;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::SandboxPolicy;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadReadParams;
use codex_app_server_protocol::ThreadReadResponse;
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
use codex_utils_path_uri::ApiPathString;
use core_test_support::responses;
use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;
use tokio::time::timeout;
use wine_exec_server_harness::POWERSHELL_PATH;
use wine_exec_server_harness::POWERSHELL_VERSION;
use wine_exec_server_harness::WINDOWS_WORKSPACE;
use wine_exec_server_harness::WineExecServer;

const APP_SERVER_TIMEOUT: Duration = Duration::from_secs(90);
const TEST_TIMEOUT: Duration = Duration::from_secs(240);
const FIRST_EXEC_CALL_ID: &str = "wine-app-server-pwsh-default";
const STICKY_EXEC_CALL_ID: &str = "wine-app-server-pwsh-explicit";
const FIRST_OUTPUT_MARKER: &str = "WINE_APP_SERVER_DEFAULT";
const STICKY_OUTPUT_MARKER: &str = "WINE_APP_SERVER_STICKY";
const RELATIVE_WORKDIR: &str = "relative";
const FIRST_WINDOWS_CWD: &str = r"C:\workspace\relative";
const ABSOLUTE_WINDOWS_CWD: &str = r"C:\workspace\absolute";

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn app_server_executes_sticky_windows_powershell_turns_under_wine() -> Result<()> {
    timeout(TEST_TIMEOUT, async {
        let (server, websocket_url) = WineExecServer::start().await?;
        let test_result = exercise_app_server(websocket_url).await;
        let shutdown_result = server.shutdown().await;
        test_result?;
        shutdown_result
    })
    .await
    .context("Wine app-server end-to-end test timed out")?
}

async fn exercise_app_server(websocket_url: String) -> Result<()> {
    let model_server = responses::start_mock_server().await;
    let first_exec_arguments = json!({
        "cmd": powershell_probe(FIRST_OUTPUT_MARKER),
        "workdir": RELATIVE_WORKDIR,
        "tty": false,
        "yield_time_ms": 30_000,
        "max_output_tokens": 2_000,
    })
    .to_string();
    let sticky_exec_arguments = json!({
        "cmd": powershell_probe(STICKY_OUTPUT_MARKER),
        "shell": "powershell.exe",
        "workdir": ABSOLUTE_WINDOWS_CWD,
        "tty": false,
        "yield_time_ms": 30_000,
        "max_output_tokens": 2_000,
    })
    .to_string();
    let response_mock = responses::mount_sse_sequence(
        &model_server,
        vec![
            responses::sse(vec![
                responses::ev_response_created("wine-app-server-response-1"),
                responses::ev_function_call(
                    FIRST_EXEC_CALL_ID,
                    "exec_command",
                    &first_exec_arguments,
                ),
                responses::ev_completed("wine-app-server-response-1"),
            ]),
            responses::sse(vec![
                responses::ev_response_created("wine-app-server-response-2"),
                responses::ev_assistant_message("wine-app-server-message-1", "first done"),
                responses::ev_completed("wine-app-server-response-2"),
            ]),
            responses::sse(vec![
                responses::ev_response_created("wine-app-server-response-3"),
                responses::ev_function_call(
                    STICKY_EXEC_CALL_ID,
                    "exec_command",
                    &sticky_exec_arguments,
                ),
                responses::ev_completed("wine-app-server-response-3"),
            ]),
            responses::sse(vec![
                responses::ev_response_created("wine-app-server-response-4"),
                responses::ev_assistant_message("wine-app-server-message-2", "sticky done"),
                responses::ev_completed("wine-app-server-response-4"),
            ]),
        ],
    )
    .await;

    let codex_home = TempDir::new().context("create app-server CODEX_HOME")?;
    write_config(codex_home.path(), &model_server.uri())?;
    let app_server_program =
        codex_utils_cargo_bin::find_resource!("../../../app-server/codex-app-server")?;
    let mut app_server = TestAppServer::new_with_program_and_env(
        codex_home.path(),
        &app_server_program,
        &[(
            CODEX_EXEC_SERVER_URL_ENV_VAR,
            Some(websocket_url.as_str()),
        )],
    )
    .await?;
    timeout(APP_SERVER_TIMEOUT, app_server.initialize()).await??;

    let environment = remote_windows_environment();
    let thread_request_id = app_server
        .send_thread_start_request(ThreadStartParams {
            model: Some("mock-model".to_string()),
            environments: Some(vec![environment.clone()]),
            ..Default::default()
        })
        .await?;
    let thread_response: JSONRPCResponse = timeout(
        APP_SERVER_TIMEOUT,
        app_server.read_stream_until_response_message(RequestId::Integer(thread_request_id)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response(thread_response)?;
    let thread_id = thread.id;

    let first_items = submit_turn_and_collect_command(
        &mut app_server,
        TurnStartParams {
            thread_id: thread_id.clone(),
            input: vec![UserInput::Text {
                text: "Run the default Windows shell in a relative workdir.".to_string(),
                text_elements: Vec::new(),
            }],
            environments: Some(vec![environment]),
            approval_policy: Some(AskForApproval::Never),
            sandbox_policy: Some(SandboxPolicy::DangerFullAccess),
            model: Some("mock-model".to_string()),
            ..Default::default()
        },
        FIRST_EXEC_CALL_ID,
    )
    .await?;
    assert_command_items(
        &first_items,
        FIRST_EXEC_CALL_ID,
        FIRST_WINDOWS_CWD,
        FIRST_OUTPUT_MARKER,
        POWERSHELL_PATH,
    );

    let sticky_items = submit_turn_and_collect_command(
        &mut app_server,
        TurnStartParams {
            thread_id: thread_id.clone(),
            input: vec![UserInput::Text {
                text: "Use explicit powershell.exe in an absolute workdir.".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        },
        STICKY_EXEC_CALL_ID,
    )
    .await?;
    assert_command_items(
        &sticky_items,
        STICKY_EXEC_CALL_ID,
        ABSOLUTE_WINDOWS_CWD,
        STICKY_OUTPUT_MARKER,
        "powershell.exe",
    );

    drop(app_server);
    let mut app_server = TestAppServer::new_with_program_and_env(
        codex_home.path(),
        &app_server_program,
        &[(
            CODEX_EXEC_SERVER_URL_ENV_VAR,
            Some(websocket_url.as_str()),
        )],
    )
    .await?;
    timeout(APP_SERVER_TIMEOUT, app_server.initialize()).await??;
    let read_request_id = app_server
        .send_thread_read_request(ThreadReadParams {
            thread_id,
            include_turns: true,
        })
        .await?;
    let read_response: JSONRPCResponse = timeout(
        APP_SERVER_TIMEOUT,
        app_server.read_stream_until_response_message(RequestId::Integer(read_request_id)),
    )
    .await??;
    let ThreadReadResponse {
        thread: persisted_thread,
    } = to_response(read_response)?;
    let persisted_command_ids = persisted_thread
        .turns
        .iter()
        .map(|turn| {
            turn.items
                .iter()
                .filter_map(|item| match item {
                    ThreadItem::CommandExecution { id, .. } => Some(id.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        persisted_command_ids,
        vec![vec![FIRST_EXEC_CALL_ID], vec![STICKY_EXEC_CALL_ID]]
    );
    let persisted_first = persisted_thread.turns[0]
        .items
        .iter()
        .find(|item| matches!(item, ThreadItem::CommandExecution { id, .. } if id == FIRST_EXEC_CALL_ID))
        .context("persisted first command should be present")?;
    assert_completed_command_item(
        persisted_first,
        FIRST_EXEC_CALL_ID,
        FIRST_WINDOWS_CWD,
        FIRST_OUTPUT_MARKER,
    );
    let persisted_sticky = persisted_thread.turns[1]
        .items
        .iter()
        .find(|item| matches!(item, ThreadItem::CommandExecution { id, .. } if id == STICKY_EXEC_CALL_ID))
        .context("persisted sticky command should be present")?;
    assert_completed_command_item(
        persisted_sticky,
        STICKY_EXEC_CALL_ID,
        ABSOLUTE_WINDOWS_CWD,
        STICKY_OUTPUT_MARKER,
    );

    let requests = response_mock.requests();
    assert_eq!(requests.len(), 4);
    let first_environment_context = requests[0]
        .message_input_texts("user")
        .into_iter()
        .find(|text| text.starts_with("<environment_context>"))
        .context("first model request should contain environment context")?;
    assert_windows_environment_context(&first_environment_context);

    assert_function_output(
        &requests[1],
        FIRST_EXEC_CALL_ID,
        FIRST_OUTPUT_MARKER,
        FIRST_WINDOWS_CWD,
    )?;
    assert_function_output(
        &requests[3],
        STICKY_EXEC_CALL_ID,
        STICKY_OUTPUT_MARKER,
        ABSOLUTE_WINDOWS_CWD,
    )?;

    let sticky_contexts = requests[2]
        .message_input_texts("user")
        .into_iter()
        .filter(|text| text.starts_with("<environment_context>") && text.contains("<cwd>"))
        .collect::<Vec<_>>();
    assert_eq!(
        sticky_contexts.len(),
        1,
        "sticky turn should retain one environment context without redundant updates"
    );
    for environment_context in sticky_contexts {
        assert_windows_environment_context(&environment_context);
    }

    let linux_cwd = codex_home.path().to_string_lossy();
    for request in requests {
        assert_no_linux_execution_artifacts(&request.body_json().to_string(), &linux_cwd);
    }
    assert_no_linux_execution_artifacts(&format!("{first_items:?}{sticky_items:?}"), &linux_cwd);
    assert_no_linux_execution_artifacts(&format!("{persisted_thread:?}"), &linux_cwd);

    Ok(())
}

#[derive(Debug)]
struct CommandItems {
    started: ThreadItem,
    completed: ThreadItem,
}

async fn submit_turn_and_collect_command(
    app_server: &mut TestAppServer,
    params: TurnStartParams,
    call_id: &str,
) -> Result<CommandItems> {
    let turn_request_id = app_server.send_turn_start_request(params).await?;
    let turn_response: JSONRPCResponse = timeout(
        APP_SERVER_TIMEOUT,
        app_server.read_stream_until_response_message(RequestId::Integer(turn_request_id)),
    )
    .await??;
    let TurnStartResponse { turn } = to_response(turn_response)?;

    let started = read_command_item_started(app_server, call_id).await?;
    let completed = read_command_item_completed(app_server, call_id).await?;
    let completed_notification = timeout(
        APP_SERVER_TIMEOUT,
        app_server.read_stream_until_notification_message("turn/completed"),
    )
    .await??;
    let completed_turn: TurnCompletedNotification = serde_json::from_value(
        completed_notification
            .params
            .context("turn/completed should include params")?,
    )?;
    assert_eq!(completed_turn.turn.id, turn.id);
    assert_eq!(completed_turn.turn.status, TurnStatus::Completed);

    Ok(CommandItems { started, completed })
}

async fn read_command_item_started(
    app_server: &mut TestAppServer,
    call_id: &str,
) -> Result<ThreadItem> {
    timeout(APP_SERVER_TIMEOUT, async {
        loop {
            let notification = app_server
                .read_stream_until_notification_message("item/started")
                .await?;
            let started: ItemStartedNotification = serde_json::from_value(
                notification
                    .params
                    .context("item/started should include params")?,
            )?;
            if matches!(&started.item, ThreadItem::CommandExecution { id, .. } if id == call_id) {
                return Ok::<_, anyhow::Error>(started.item);
            }
        }
    })
    .await
    .context("timed out waiting for command item/started")?
}

async fn read_command_item_completed(
    app_server: &mut TestAppServer,
    call_id: &str,
) -> Result<ThreadItem> {
    timeout(APP_SERVER_TIMEOUT, async {
        loop {
            let notification = app_server
                .read_stream_until_notification_message("item/completed")
                .await?;
            let completed: ItemCompletedNotification = serde_json::from_value(
                notification
                    .params
                    .context("item/completed should include params")?,
            )?;
            if matches!(&completed.item, ThreadItem::CommandExecution { id, .. } if id == call_id) {
                return Ok::<_, anyhow::Error>(completed.item);
            }
        }
    })
    .await
    .context("timed out waiting for command item/completed")?
}

fn assert_command_items(
    items: &CommandItems,
    call_id: &str,
    cwd: &str,
    output_marker: &str,
    expected_shell: &str,
) {
    let ThreadItem::CommandExecution {
        id,
        command,
        cwd: started_cwd,
        status: started_status,
        exit_code: started_exit_code,
        ..
    } = &items.started
    else {
        panic!("expected started command execution item")
    };
    assert_eq!(id, call_id);
    let expected_shell_name = expected_shell.rsplit(['/', '\\']).next().unwrap_or(expected_shell);
    assert!(
        command.contains(expected_shell_name),
        "expected {expected_shell_name:?} in started command {command:?}"
    );
    assert_eq!(started_cwd, &ApiPathString::new(cwd));
    assert_eq!(started_status, &CommandExecutionStatus::InProgress);
    assert_eq!(started_exit_code, &None);

    assert_completed_command_item(&items.completed, call_id, cwd, output_marker);
}

fn assert_completed_command_item(
    item: &ThreadItem,
    call_id: &str,
    cwd: &str,
    output_marker: &str,
) {
    let ThreadItem::CommandExecution {
        id,
        cwd: completed_cwd,
        status: completed_status,
        aggregated_output,
        exit_code,
        ..
    } = item
    else {
        panic!("expected completed command execution item")
    };
    assert_eq!(id, call_id);
    assert_eq!(completed_cwd, &ApiPathString::new(cwd));
    assert_eq!(completed_status, &CommandExecutionStatus::Completed);
    assert_eq!(exit_code, &Some(0));
    assert!(
        aggregated_output
            .as_deref()
            .is_some_and(|output| output.contains(&expected_probe(output_marker, cwd))),
        "unexpected completed command output: {aggregated_output:?}"
    );
}

fn assert_function_output(
    request: &responses::ResponsesRequest,
    call_id: &str,
    marker: &str,
    cwd: &str,
) -> Result<()> {
    let output = request
        .function_call_output_text(call_id)
        .with_context(|| format!("missing function_call_output for {call_id}"))?;
    assert!(
        output.contains(&expected_probe(marker, cwd)),
        "unexpected function_call_output: {output:?}"
    );
    Ok(())
}

fn remote_windows_environment() -> TurnEnvironmentParams {
    TurnEnvironmentParams {
        environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
        cwd: ApiPathString::new(WINDOWS_WORKSPACE),
    }
}

fn powershell_probe(marker: &str) -> String {
    format!(
        "$ErrorActionPreference = 'Stop'; \
         [Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false); \
         $separatorCode = [int]([System.IO.Path]::DirectorySeparatorChar); \
         Write-Output ('{marker}|' + $PSVersionTable.PSVersion.ToString() + '|' + \
         $PSVersionTable.PSEdition + '|' + $IsWindows.ToString().ToLowerInvariant() + '|' + \
         (Get-Location).ProviderPath + '|' + $separatorCode)"
    )
}

fn expected_probe(marker: &str, cwd: &str) -> String {
    format!("{marker}|{POWERSHELL_VERSION}|Core|true|{cwd}|92")
}

fn assert_windows_environment_context(environment_context: &str) {
    assert!(
        environment_context.contains(&format!("<cwd>{WINDOWS_WORKSPACE}</cwd>")),
        "unexpected environment context: {environment_context:?}"
    );
    assert!(
        environment_context.contains("<shell>powershell</shell>"),
        "unexpected environment context: {environment_context:?}"
    );
    assert!(
        environment_context.contains("<permission_profile type=\"disabled\">"),
        "unexpected environment context: {environment_context:?}"
    );
}

fn assert_no_linux_execution_artifacts(value: &str, linux_cwd: &str) {
    for unexpected in [
        "/bin/bash",
        "/bin/sh",
        "codex-linux-sandbox",
        "codex-execve-wrapper",
    ] {
        assert!(
            !value.contains(unexpected),
            "Linux execution artifact {unexpected:?} leaked into {value:?}"
        );
    }
    let linux_cwd_element = format!("<cwd>{linux_cwd}</cwd>");
    assert!(
        !value.contains(&linux_cwd_element),
        "Linux execution cwd leaked into {value:?}"
    );
}

fn write_config(codex_home: &Path, server_uri: &str) -> Result<()> {
    fs::write(
        codex_home.join("config.toml"),
        format!(
            r#"
model = "mock-model"
approval_policy = "never"
sandbox_mode = "danger-full-access"
model_provider = "mock_provider"

[shell_environment_policy]
inherit = "core"

[shell_environment_policy.set]
DOTNET_CLI_TELEMETRY_OPTOUT = "1"
DOTNET_NOLOGO = "1"
POWERSHELL_TELEMETRY_OPTOUT = "1"
POWERSHELL_UPDATECHECK = "Off"

[features]
unified_exec = true

[model_providers.mock_provider]
name = "Mock provider for Wine app-server test"
base_url = "{server_uri}/v1"
wire_api = "responses"
request_max_retries = 0
stream_max_retries = 0
supports_websockets = false
"#
        ),
    )
    .context("write app-server config")
}
