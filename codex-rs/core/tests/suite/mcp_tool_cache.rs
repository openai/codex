use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use codex_core::NewThread;
use codex_exec_server::ExecutorFileSystem;
use codex_exec_server::RemoveOptions;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::user_input::UserInput;
use codex_utils_path_uri::PathUri;
use core_test_support::responses;
use core_test_support::responses::ResponseMock;
use core_test_support::responses::mount_sse_once;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_wine_exec;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;

use super::rmcp_client::remote_aware_environment_id;
use super::rmcp_client::remote_aware_stdio_server_bin;

const SERVER_NAME: &str = "cached_rmcp";
const NAMESPACE: &str = "mcp__cached_rmcp";

fn user_turn(prompt: &str) -> Op {
    Op::UserInput {
        items: vec![UserInput::Text {
            text: prompt.to_string(),
            text_elements: Vec::new(),
        }],
        final_output_json_schema: None,
        responsesapi_client_metadata: None,
        additional_context: Default::default(),
        thread_settings: codex_protocol::protocol::ThreadSettingsOverrides {
            approval_policy: Some(AskForApproval::Never),
            permission_profile: Some(PermissionProfile::Disabled),
            ..Default::default()
        },
    }
}

fn process_label(pid: &str) -> String {
    format!("rmcp-test-process-{pid}")
}

fn assert_definition(response: &ResponseMock, namespace_description: &str, tool_description: &str) {
    let body = response.single_request().body_json();
    let namespace = body
        .get("tools")
        .and_then(Value::as_array)
        .and_then(|tools| {
            tools
                .iter()
                .find(|tool| tool.get("name").and_then(Value::as_str) == Some(NAMESPACE))
        })
        .expect("request should contain the MCP namespace");
    assert_eq!(
        namespace.get("description").and_then(Value::as_str),
        Some(namespace_description)
    );
    assert_eq!(
        responses::namespace_child_tool(&body, NAMESPACE, "echo")
            .and_then(|tool| tool.get("description"))
            .and_then(Value::as_str),
        Some(tool_description)
    );
}

async fn wait_for_new_pid(
    fs: &dyn ExecutorFileSystem,
    path: &PathUri,
    previous_pid: Option<&str>,
) -> anyhow::Result<String> {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Ok(contents) = fs.read_file_text(path, /*sandbox*/ None).await {
                let pid = contents.trim();
                if !pid.is_empty() && Some(pid) != previous_pid {
                    return pid.to_string();
                }
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .context("timed out waiting for a new MCP server process")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn regular_mcp_definition_cache_preserves_live_session_state() -> anyhow::Result<()> {
    skip_if_wine_exec!(
        Ok(()),
        "requires a Windows test_stdio_server in the Wine-exec environment"
    );
    skip_if_no_network!(Ok(()));

    let responses_server = responses::start_mock_server().await;
    let command = remote_aware_stdio_server_bin()?;
    let environment_id = remote_aware_environment_id();
    let fixture = test_codex()
        .with_model_info_override("gpt-5.4", |model| model.supports_search_tool = false)
        .with_config(move |config| {
            let app_only_cwd_marker_file = config.cwd.join("cwd-app-only");
            let barrier_file = config.cwd.join("allow-initialize");
            let pid_file = config.cwd.join("mcp.pid");
            let mut servers = config.mcp_servers.get().clone();
            servers.insert(
                SERVER_NAME.to_string(),
                serde_json::from_value(json!({
                    "command": command,
                    "environment_id": environment_id,
                    "env": {
                        "MCP_TEST_APP_ONLY_CWD_MARKER_FILE": app_only_cwd_marker_file,
                        "MCP_TEST_INITIALIZE_BARRIER_FILE": barrier_file,
                        "MCP_TEST_DYNAMIC_SERVER_METADATA": "1",
                        "MCP_TEST_PID_FILE": pid_file,
                    },
                    "enabled_tools": ["cwd", "echo"],
                    "startup_timeout_sec": 10,
                }))
                .expect("test MCP server configuration"),
            );
            config
                .mcp_servers
                .set(servers)
                .expect("test MCP server configuration");
        })
        .build_with_auto_env(&responses_server)
        .await?;
    let fs = fixture.fs();
    let app_only_cwd_marker_file =
        PathUri::from_host_native_path(fixture.config.cwd.join("cwd-app-only"))?;
    let barrier_file = PathUri::from_host_native_path(fixture.config.cwd.join("allow-initialize"))?;
    let pid_file = PathUri::from_host_native_path(fixture.config.cwd.join("mcp.pid"))?;

    let cold_response = mount_sse_once(
        &responses_server,
        responses::sse(vec![
            responses::ev_response_created("cold"),
            responses::ev_assistant_message("cold-message", "done"),
            responses::ev_completed("cold"),
        ]),
    )
    .await;
    fixture.codex.submit(user_turn("use the echo tool")).await?;
    let first_pid = wait_for_new_pid(fs.as_ref(), &pid_file, /*previous_pid*/ None).await?;
    fs.write_file(&barrier_file, b"ready".to_vec(), /*sandbox*/ None)
        .await?;
    wait_for_event(&fixture.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    let first_process = process_label(&first_pid);
    assert_definition(
        &cold_response,
        &format!("Use the tools from {first_process}."),
        &format!("Echo from {first_process}."),
    );

    fs.remove(
        &barrier_file,
        RemoveOptions {
            recursive: false,
            force: false,
        },
        /*sandbox*/ None,
    )
    .await?;
    fs.write_file(
        &app_only_cwd_marker_file,
        b"app-only".to_vec(),
        /*sandbox*/ None,
    )
    .await?;
    let NewThread {
        thread: second_thread,
        ..
    } = fixture
        .thread_manager
        .start_thread(fixture.config.clone())
        .await?;
    let second_pid = wait_for_new_pid(fs.as_ref(), &pid_file, Some(&first_pid)).await?;
    let second_process = process_label(&second_pid);

    let app_only_call_id = "cached-app-only-call";
    let cached_response = mount_sse_once(
        &responses_server,
        responses::sse(vec![
            responses::ev_response_created("cached-call"),
            responses::ev_function_call_with_namespace(
                "cached-call",
                NAMESPACE,
                "echo",
                r#"{"message":"hello"}"#,
            ),
            responses::ev_function_call_with_namespace(app_only_call_id, NAMESPACE, "cwd", "{}"),
            responses::ev_completed("cached-call"),
        ]),
    )
    .await;
    let cached_done_response = mount_sse_once(
        &responses_server,
        responses::sse(vec![
            responses::ev_response_created("cached-done"),
            responses::ev_assistant_message("cached-message", "done"),
            responses::ev_completed("cached-done"),
        ]),
    )
    .await;
    let second_for_turn = Arc::clone(&second_thread);
    let cached_turn = tokio::spawn(async move {
        second_for_turn
            .submit(user_turn("call the echo and cwd tools"))
            .await?;
        let end = wait_for_event(&second_for_turn, |event| {
            matches!(
                event,
                EventMsg::McpToolCallEnd(end) if end.call_id == "cached-call"
            )
        })
        .await;
        let EventMsg::McpToolCallEnd(end) = end else {
            unreachable!("event predicate guarantees an MCP tool result");
        };
        let called_process = end
            .result
            .expect("echo call should succeed")
            .structured_content
            .and_then(|content| content.get("echo").cloned())
            .and_then(|echo| echo.as_str().map(ToString::to_string))
            .expect("echo result should identify its live server process");
        wait_for_event(&second_for_turn, |event| {
            matches!(event, EventMsg::TurnComplete(_))
        })
        .await;
        anyhow::Ok(called_process)
    });
    tokio::time::timeout(Duration::from_secs(2), async {
        while cached_response.requests().is_empty() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .context("cached MCP definitions should reach inference before initialization")?;
    assert_definition(
        &cached_response,
        &format!("Tools in the {NAMESPACE} namespace."),
        &format!("Echo from {first_process}."),
    );

    fixture.codex.shutdown_and_wait().await?;
    fs.write_file(&barrier_file, b"ready".to_vec(), /*sandbox*/ None)
        .await?;
    let expected_error = format!("MCP tool `{SERVER_NAME}/cwd` is not available to the model");
    assert_eq!(cached_turn.await??, second_process);
    let output = cached_done_response
        .single_request()
        .function_call_output_text(app_only_call_id)
        .expect("app-only tool error should be returned to the model");
    assert!(
        output.contains(&expected_error),
        "model-visible tool output should contain the live visibility error: {output}"
    );
    let output = cached_done_response
        .single_request()
        .function_call_output_text("cached-call")
        .expect("successful tool output should be returned to the model");
    assert!(
        output.contains(&second_process),
        "model-visible tool output should come from the live server: {output}"
    );

    second_thread.shutdown_and_wait().await?;
    responses_server.verify().await;
    Ok(())
}
