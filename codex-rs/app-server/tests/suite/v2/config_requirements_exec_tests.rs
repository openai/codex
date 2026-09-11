//! Managed unified-exec restrictions must retain approved, completion-only command execution.

use anyhow::Context;
use anyhow::Result;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use codex_app_server_protocol::CommandExecutionApprovalDecision;
use codex_app_server_protocol::CommandExecutionRequestApprovalResponse;
use codex_app_server_protocol::CommandExecutionStatus;
use codex_app_server_protocol::ItemCompletedNotification;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::TurnCompletedNotification;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::TurnStatus;
use codex_app_server_protocol::UserInput;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_wine_exec;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::timeout;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn managed_unified_exec_disabled_preserves_approved_command_execution() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_wine_exec!(
        Ok(()),
        "non-TTY PowerShell execution and command approval routing are unsupported under Wine"
    );

    let read_timeout = Duration::from_secs(/*secs*/ 60);
    let server = responses::start_mock_server().await;
    let home = TempDir::new()?;
    std::fs::write(
        home.path().join("requirements.toml"),
        r#"
[features]
unified_exec = false
shell_tool = true

[[rules.prefix_rules]]
pattern = [{ token = "echo" }]
decision = "prompt"
"#,
    )?;
    MockResponsesConfig::new(&server.uri())
        .with_approval_policy("on-request")
        .with_sandbox_mode("workspace-write")
        .write(home.path())?;

    let call_id = "managed-exec";
    let command = "echo managed-exec-ok";
    let command_response = responses::mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_response_created("command"),
            responses::ev_function_call(
                call_id,
                "exec_command",
                &json!({"cmd": command, "timeout_ms": 10_000}).to_string(),
            ),
            responses::ev_completed("command"),
        ]),
    )
    .await;
    let continuation = responses::mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_assistant_message("done", "Command completed"),
            responses::ev_completed("completed"),
        ]),
    )
    .await;

    let mut app = TestAppServer::builder()
        .with_codex_home(home.path())
        .build_initialized_with_timeout(read_timeout)
        .await?;
    let environment_id = app.auto_env_params()?.environment_id;
    let thread = app.start_thread(ThreadStartParams::default()).await?.thread;
    let request_id = app
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![UserInput::Text {
                text: "Run the command".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    let _: TurnStartResponse = timeout(read_timeout, app.read_response(request_id)).await??;

    let request = timeout(read_timeout, app.read_stream_until_request_message()).await??;
    let ServerRequest::CommandExecutionRequestApproval { request_id, params } = request else {
        anyhow::bail!("expected command approval, got {request:?}");
    };
    assert_eq!(
        (params.thread_id, params.item_id, params.environment_id),
        (thread.id.clone(), call_id.to_string(), Some(environment_id)),
    );

    let body = command_response.single_request().body_json();
    let command_tools = body["tools"]
        .as_array()
        .context("model request must advertise tools")?
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        .filter(|name| matches!(*name, "exec_command" | "write_stdin"))
        .collect::<Vec<_>>();
    assert_eq!(command_tools, vec!["exec_command"]);

    app.send_response(
        request_id,
        serde_json::to_value(CommandExecutionRequestApprovalResponse {
            decision: CommandExecutionApprovalDecision::Accept,
        })?,
    )
    .await?;
    timeout(read_timeout, async {
        loop {
            let completed: ItemCompletedNotification =
                app.read_notification("item/completed").await?;
            if let ThreadItem::CommandExecution {
                id,
                status,
                exit_code,
                ..
            } = completed.item
                && id == call_id
            {
                assert_eq!(
                    (status, exit_code),
                    (CommandExecutionStatus::Completed, Some(0))
                );
                break;
            }
        }
        anyhow::Ok(())
    })
    .await??;
    let completed: TurnCompletedNotification =
        timeout(read_timeout, app.read_notification("turn/completed")).await??;
    assert_eq!(completed.turn.status, TurnStatus::Completed);

    // Check the model continuation because early stdout can precede event subscription.
    let output = continuation
        .single_request()
        .function_call_output_text(call_id)
        .context("model must receive command output")?;
    assert!(output.contains("managed-exec-ok"), "{output}");
    assert!(
        !output.contains("Process running with session ID"),
        "{output}"
    );
    app.shutdown_gracefully().await?;
    Ok(())
}
