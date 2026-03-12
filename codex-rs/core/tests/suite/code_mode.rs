#![allow(clippy::expect_used, clippy::unwrap_used)]

use anyhow::Result;
use codex_core::config::types::McpServerConfig;
use codex_core::config::types::McpServerTransportConfig;
use codex_core::features::Feature;
use core_test_support::assert_regex_match;
use core_test_support::responses;
use core_test_support::responses::ResponseMock;
use core_test_support::responses::ResponsesRequest;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_custom_tool_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::sse;
use core_test_support::skip_if_no_network;
use core_test_support::stdio_server_bin;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::Duration;
use wiremock::MockServer;

fn custom_tool_output_items(req: &ResponsesRequest, call_id: &str) -> Vec<Value> {
    req.custom_tool_call_output(call_id)
        .get("output")
        .and_then(Value::as_array)
        .expect("custom tool output should be serialized as content items")
        .clone()
}

fn function_tool_output_items(req: &ResponsesRequest, call_id: &str) -> Vec<Value> {
    match req.function_call_output(call_id).get("output") {
        Some(Value::Array(items)) => items.clone(),
        Some(Value::String(text)) => {
            vec![serde_json::json!({ "type": "input_text", "text": text })]
        }
        _ => panic!("function tool output should be serialized as text or content items"),
    }
}

fn text_item(items: &[Value], index: usize) -> &str {
    items[index]
        .get("text")
        .and_then(Value::as_str)
        .expect("content item should be input_text")
}

fn extract_running_session_id(text: &str) -> i32 {
    text.strip_prefix("Script running with session ID ")
        .and_then(|rest| rest.split('\n').next())
        .expect("running header should contain a session ID")
        .parse()
        .expect("session ID should parse as i32")
}

fn wait_for_file_source(path: &Path) -> Result<String> {
    let quoted_path = shlex::try_join([path.to_string_lossy().as_ref()])?;
    let command = format!("if [ -f {quoted_path} ]; then printf ready; fi");
    Ok(format!(
        r#"while ((await exec_command({{ cmd: {command:?} }})).output !== "ready") {{
}}"#
    ))
}

fn custom_tool_output_body_and_success(
    req: &ResponsesRequest,
    call_id: &str,
) -> (String, Option<bool>) {
    let (_, success) = req
        .custom_tool_call_output_content_and_success(call_id)
        .expect("custom tool output should be present");
    let items = custom_tool_output_items(req, call_id);
    let output = items
        .iter()
        .skip(1)
        .filter_map(|item| item.get("text").and_then(Value::as_str))
        .collect();
    (output, success)
}

async fn run_code_mode_turn(
    server: &MockServer,
    prompt: &str,
    code: &str,
    include_apply_patch: bool,
) -> Result<(TestCodex, ResponseMock)> {
    let mut builder = test_codex().with_config(move |config| {
        let _ = config.features.enable(Feature::CodeMode);
        config.include_apply_patch_tool = include_apply_patch;
    });
    let test = builder.build(server).await?;

    responses::mount_sse_once(
        server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_custom_tool_call("call-1", "exec", code),
            ev_completed("resp-1"),
        ]),
    )
    .await;

    let second_mock = responses::mount_sse_once(
        server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    test.submit_turn(prompt).await?;
    Ok((test, second_mock))
}

async fn run_code_mode_turn_with_rmcp(
    server: &MockServer,
    prompt: &str,
    code: &str,
) -> Result<(TestCodex, ResponseMock)> {
    let rmcp_test_server_bin = stdio_server_bin()?;
    let mut builder = test_codex().with_config(move |config| {
        let _ = config.features.enable(Feature::CodeMode);

        let mut servers = config.mcp_servers.get().clone();
        servers.insert(
            "rmcp".to_string(),
            McpServerConfig {
                transport: McpServerTransportConfig::Stdio {
                    command: rmcp_test_server_bin,
                    args: Vec::new(),
                    env: Some(HashMap::from([(
                        "MCP_TEST_VALUE".to_string(),
                        "propagated-env".to_string(),
                    )])),
                    env_vars: Vec::new(),
                    cwd: None,
                },
                enabled: true,
                required: false,
                disabled_reason: None,
                startup_timeout_sec: Some(Duration::from_secs(10)),
                tool_timeout_sec: None,
                enabled_tools: None,
                disabled_tools: None,
                scopes: None,
                oauth_resource: None,
            },
        );
        config
            .mcp_servers
            .set(servers)
            .expect("test mcp servers should accept any configuration");
    });
    let test = builder.build(server).await?;

    responses::mount_sse_once(
        server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_custom_tool_call("call-1", "exec", code),
            ev_completed("resp-1"),
        ]),
    )
    .await;

    let second_mock = responses::mount_sse_once(
        server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    test.submit_turn(prompt).await?;
    Ok((test, second_mock))
}

#[cfg_attr(windows, ignore = "no exec_command on Windows")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_can_return_exec_command_output() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let (_test, second_mock) = run_code_mode_turn(
        &server,
        "use exec to run exec_command",
        r#"
import { exec_command } from "tools.js";

add_content(JSON.stringify(await exec_command({ cmd: "printf code_mode_exec_marker" })));
"#,
        false,
    )
    .await?;

    let req = second_mock.single_request();
    let items = custom_tool_output_items(&req, "call-1");
    assert_eq!(items.len(), 2);
    assert_regex_match(
        concat!(
            r"(?s)\A",
            r"Script completed\nWall time \d+\.\d seconds\nOutput:\n\z"
        ),
        text_item(&items, 0),
    );
    let parsed: Value = serde_json::from_str(text_item(&items, 1))?;
    assert!(
        parsed
            .get("chunk_id")
            .and_then(Value::as_str)
            .is_some_and(|chunk_id| !chunk_id.is_empty())
    );
    assert_eq!(
        parsed.get("output").and_then(Value::as_str),
        Some("code_mode_exec_marker"),
    );
    assert_eq!(parsed.get("exit_code").and_then(Value::as_i64), Some(0));
    assert!(parsed.get("wall_time_seconds").is_some());
    assert!(parsed.get("session_id").is_none());

    Ok(())
}

#[cfg_attr(windows, ignore = "no exec_command on Windows")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_can_truncate_final_result_with_configured_budget() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let (_test, second_mock) = run_code_mode_turn(
        &server,
        "use exec to truncate the final result",
        r#"
import { exec_command } from "tools.js";
import { set_max_output_tokens_per_exec_call } from "@openai/code_mode";

set_max_output_tokens_per_exec_call(6);

add_content(JSON.stringify(await exec_command({
  cmd: "printf 'token one token two token three token four token five token six token seven'",
  max_output_tokens: 100
})));
"#,
        false,
    )
    .await?;

    let req = second_mock.single_request();
    let items = custom_tool_output_items(&req, "call-1");
    assert_eq!(items.len(), 2);
    assert_regex_match(
        concat!(
            r"(?s)\A",
            r"Script completed\nWall time \d+\.\d seconds\nOutput:\n\z"
        ),
        text_item(&items, 0),
    );
    let expected_pattern = r#"(?sx)
\A
Total\ output\ lines:\ 1\n
\n
.*…\d+\ tokens\ truncated….*
\z
"#;
    assert_regex_match(expected_pattern, text_item(&items, 1));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_returns_accumulated_output_when_script_fails() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let (_test, second_mock) = run_code_mode_turn(
        &server,
        "use code_mode to surface script failures",
        r#"
add_content("before crash");
add_content("still before crash");
throw new Error("boom");
"#,
        false,
    )
    .await?;

    let req = second_mock.single_request();
    let items = custom_tool_output_items(&req, "call-1");
    assert_eq!(items.len(), 4);
    assert_regex_match(
        concat!(
            r"(?s)\A",
            r"Script failed\nWall time \d+\.\d seconds\nOutput:\n\z"
        ),
        text_item(&items, 0),
    );
    assert_eq!(text_item(&items, 1), "before crash");
    assert_eq!(text_item(&items, 2), "still before crash");
    assert_regex_match(
        r#"(?sx)
\A
Script\ error:\n
Error:\ boom\n
(?:\s+at\ .+\n?)+
\z
"#,
        text_item(&items, 3),
    );

    Ok(())
}

#[cfg_attr(windows, ignore = "no exec_command on Windows")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_can_yield_and_resume_with_exec_wait() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let mut builder = test_codex().with_config(move |config| {
        let _ = config.features.enable(Feature::CodeMode);
    });
    let test = builder.build(&server).await?;
    let phase_2_gate = test.workspace_path("code-mode-phase-2.ready");
    let phase_3_gate = test.workspace_path("code-mode-phase-3.ready");
    let phase_2_wait = wait_for_file_source(&phase_2_gate)?;
    let phase_3_wait = wait_for_file_source(&phase_3_gate)?;

    let code = format!(
        r#"
import {{ output_text, set_yield_time }} from "@openai/code_mode";
import {{ exec_command }} from "tools.js";

output_text("phase 1");
set_yield_time(10);
{phase_2_wait}
output_text("phase 2");
{phase_3_wait}
output_text("phase 3");
"#
    );

    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_custom_tool_call("call-1", "exec", &code),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let first_completion = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "waiting"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    test.submit_turn("start the long exec").await?;

    let first_request = first_completion.single_request();
    let first_items = custom_tool_output_items(&first_request, "call-1");
    assert_eq!(first_items.len(), 2);
    assert_regex_match(
        concat!(
            r"(?s)\A",
            r"Script running with session ID \d+\nWall time \d+\.\d seconds\nOutput:\n\z"
        ),
        text_item(&first_items, 0),
    );
    assert_eq!(text_item(&first_items, 1), "phase 1");
    let session_id = extract_running_session_id(text_item(&first_items, 0));

    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-3"),
            responses::ev_function_call(
                "call-2",
                "exec_wait",
                &serde_json::to_string(&serde_json::json!({
                    "session_id": session_id,
                    "yield_time_ms": 1_000,
                }))?,
            ),
            ev_completed("resp-3"),
        ]),
    )
    .await;
    let second_completion = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-2", "still waiting"),
            ev_completed("resp-4"),
        ]),
    )
    .await;

    fs::write(&phase_2_gate, "ready")?;
    test.submit_turn("wait again").await?;

    let second_request = second_completion.single_request();
    let second_items = function_tool_output_items(&second_request, "call-2");
    assert_eq!(second_items.len(), 2);
    assert_regex_match(
        concat!(
            r"(?s)\A",
            r"Script running with session ID \d+\nWall time \d+\.\d seconds\nOutput:\n\z"
        ),
        text_item(&second_items, 0),
    );
    assert_eq!(
        extract_running_session_id(text_item(&second_items, 0)),
        session_id
    );
    assert_eq!(text_item(&second_items, 1), "phase 2");

    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-5"),
            responses::ev_function_call(
                "call-3",
                "exec_wait",
                &serde_json::to_string(&serde_json::json!({
                    "session_id": session_id,
                    "yield_time_ms": 1_000,
                }))?,
            ),
            ev_completed("resp-5"),
        ]),
    )
    .await;
    let third_completion = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-3", "done"),
            ev_completed("resp-6"),
        ]),
    )
    .await;

    fs::write(&phase_3_gate, "ready")?;
    test.submit_turn("wait for completion").await?;

    let third_request = third_completion.single_request();
    let third_items = function_tool_output_items(&third_request, "call-3");
    assert_eq!(third_items.len(), 2);
    assert_regex_match(
        concat!(
            r"(?s)\A",
            r"Script completed\nWall time \d+\.\d seconds\nOutput:\n\z"
        ),
        text_item(&third_items, 0),
    );
    assert_eq!(text_item(&third_items, 1), "phase 3");

    Ok(())
}

#[cfg_attr(windows, ignore = "no exec_command on Windows")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_can_run_multiple_yielded_sessions() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let mut builder = test_codex().with_config(move |config| {
        let _ = config.features.enable(Feature::CodeMode);
    });
    let test = builder.build(&server).await?;
    let session_a_gate = test.workspace_path("code-mode-session-a.ready");
    let session_b_gate = test.workspace_path("code-mode-session-b.ready");
    let session_a_wait = wait_for_file_source(&session_a_gate)?;
    let session_b_wait = wait_for_file_source(&session_b_gate)?;

    let session_a_code = format!(
        r#"
import {{ output_text, set_yield_time }} from "@openai/code_mode";
import {{ exec_command }} from "tools.js";

output_text("session a start");
set_yield_time(10);
{session_a_wait}
output_text("session a done");
"#
    );
    let session_b_code = format!(
        r#"
import {{ output_text, set_yield_time }} from "@openai/code_mode";
import {{ exec_command }} from "tools.js";

output_text("session b start");
set_yield_time(10);
{session_b_wait}
output_text("session b done");
"#
    );

    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_custom_tool_call("call-1", "exec", &session_a_code),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let first_completion = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "session a waiting"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    test.submit_turn("start session a").await?;

    let first_request = first_completion.single_request();
    let first_items = custom_tool_output_items(&first_request, "call-1");
    assert_eq!(first_items.len(), 2);
    let session_a_id = extract_running_session_id(text_item(&first_items, 0));
    assert_eq!(text_item(&first_items, 1), "session a start");

    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-3"),
            ev_custom_tool_call("call-2", "exec", &session_b_code),
            ev_completed("resp-3"),
        ]),
    )
    .await;
    let second_completion = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-2", "session b waiting"),
            ev_completed("resp-4"),
        ]),
    )
    .await;

    test.submit_turn("start session b").await?;

    let second_request = second_completion.single_request();
    let second_items = custom_tool_output_items(&second_request, "call-2");
    assert_eq!(second_items.len(), 2);
    let session_b_id = extract_running_session_id(text_item(&second_items, 0));
    assert_eq!(text_item(&second_items, 1), "session b start");
    assert_ne!(session_a_id, session_b_id);

    fs::write(&session_a_gate, "ready")?;
    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-5"),
            responses::ev_function_call(
                "call-3",
                "exec_wait",
                &serde_json::to_string(&serde_json::json!({
                    "session_id": session_a_id,
                    "yield_time_ms": 1_000,
                }))?,
            ),
            ev_completed("resp-5"),
        ]),
    )
    .await;
    let third_completion = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-3", "session a done"),
            ev_completed("resp-6"),
        ]),
    )
    .await;

    test.submit_turn("wait session a").await?;

    let third_request = third_completion.single_request();
    let third_items = function_tool_output_items(&third_request, "call-3");
    assert_eq!(third_items.len(), 2);
    assert_regex_match(
        concat!(
            r"(?s)\A",
            r"Script completed\nWall time \d+\.\d seconds\nOutput:\n\z"
        ),
        text_item(&third_items, 0),
    );
    assert_eq!(text_item(&third_items, 1), "session a done");

    fs::write(&session_b_gate, "ready")?;
    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-7"),
            responses::ev_function_call(
                "call-4",
                "exec_wait",
                &serde_json::to_string(&serde_json::json!({
                    "session_id": session_b_id,
                    "yield_time_ms": 1_000,
                }))?,
            ),
            ev_completed("resp-7"),
        ]),
    )
    .await;
    let fourth_completion = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-4", "session b done"),
            ev_completed("resp-8"),
        ]),
    )
    .await;

    test.submit_turn("wait session b").await?;

    let fourth_request = fourth_completion.single_request();
    let fourth_items = function_tool_output_items(&fourth_request, "call-4");
    assert_eq!(fourth_items.len(), 2);
    assert_regex_match(
        concat!(
            r"(?s)\A",
            r"Script completed\nWall time \d+\.\d seconds\nOutput:\n\z"
        ),
        text_item(&fourth_items, 0),
    );
    assert_eq!(text_item(&fourth_items, 1), "session b done");

    Ok(())
}

#[cfg_attr(windows, ignore = "no exec_command on Windows")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_exec_wait_can_terminate_and_continue() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let mut builder = test_codex().with_config(move |config| {
        let _ = config.features.enable(Feature::CodeMode);
    });
    let test = builder.build(&server).await?;
    let termination_gate = test.workspace_path("code-mode-terminate.ready");
    let termination_wait = wait_for_file_source(&termination_gate)?;

    let code = format!(
        r#"
import {{ output_text, set_yield_time }} from "@openai/code_mode";
import {{ exec_command }} from "tools.js";

output_text("phase 1");
set_yield_time(10);
{termination_wait}
output_text("phase 2");
"#
    );

    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_custom_tool_call("call-1", "exec", &code),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let first_completion = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "waiting"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    test.submit_turn("start the long exec").await?;

    let first_request = first_completion.single_request();
    let first_items = custom_tool_output_items(&first_request, "call-1");
    assert_eq!(first_items.len(), 2);
    let session_id = extract_running_session_id(text_item(&first_items, 0));
    assert_eq!(text_item(&first_items, 1), "phase 1");

    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-3"),
            responses::ev_function_call(
                "call-2",
                "exec_wait",
                &serde_json::to_string(&serde_json::json!({
                    "session_id": session_id,
                    "terminate": true,
                }))?,
            ),
            ev_completed("resp-3"),
        ]),
    )
    .await;
    let second_completion = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-2", "terminated"),
            ev_completed("resp-4"),
        ]),
    )
    .await;

    test.submit_turn("terminate it").await?;

    let second_request = second_completion.single_request();
    let second_items = function_tool_output_items(&second_request, "call-2");
    assert_eq!(second_items.len(), 1);
    assert_regex_match(
        concat!(
            r"(?s)\A",
            r"Script terminated\nWall time \d+\.\d seconds\nOutput:\n\z"
        ),
        text_item(&second_items, 0),
    );

    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-5"),
            ev_custom_tool_call(
                "call-3",
                "exec",
                r#"
import { output_text } from "@openai/code_mode";

output_text("after terminate");
"#,
            ),
            ev_completed("resp-5"),
        ]),
    )
    .await;
    let third_completion = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-3", "done"),
            ev_completed("resp-6"),
        ]),
    )
    .await;

    test.submit_turn("run another exec").await?;

    let third_request = third_completion.single_request();
    let third_items = custom_tool_output_items(&third_request, "call-3");
    assert_eq!(third_items.len(), 2);
    assert_regex_match(
        concat!(
            r"(?s)\A",
            r"Script completed\nWall time \d+\.\d seconds\nOutput:\n\z"
        ),
        text_item(&third_items, 0),
    );
    assert_eq!(text_item(&third_items, 1), "after terminate");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_exec_wait_returns_error_for_unknown_session() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let mut builder = test_codex().with_config(move |config| {
        let _ = config.features.enable(Feature::CodeMode);
    });
    let test = builder.build(&server).await?;

    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            responses::ev_function_call(
                "call-1",
                "exec_wait",
                &serde_json::to_string(&serde_json::json!({
                    "session_id": 999_999,
                    "yield_time_ms": 1_000,
                }))?,
            ),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let completion = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    test.submit_turn("wait on an unknown exec session").await?;

    let request = completion.single_request();
    let (_, success) = request
        .function_call_output_content_and_success("call-1")
        .expect("function tool output should be present");
    assert_ne!(success, Some(true));

    let items = function_tool_output_items(&request, "call-1");
    assert_eq!(items.len(), 2);
    assert_regex_match(
        concat!(
            r"(?s)\A",
            r"Script failed\nWall time \d+\.\d seconds\nOutput:\n\z"
        ),
        text_item(&items, 0),
    );
    assert_eq!(
        text_item(&items, 1),
        "Script error:\nexec session 999999 not found"
    );

    Ok(())
}

#[cfg_attr(windows, ignore = "no exec_command on Windows")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_exec_wait_terminate_returns_completed_session_if_it_finished_in_background()
-> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let mut builder = test_codex().with_config(move |config| {
        let _ = config.features.enable(Feature::CodeMode);
    });
    let test = builder.build(&server).await?;
    let session_a_gate = test.workspace_path("code-mode-session-a-finished.ready");
    let session_b_gate = test.workspace_path("code-mode-session-b-blocked.ready");
    let session_a_wait = wait_for_file_source(&session_a_gate)?;
    let session_b_wait = wait_for_file_source(&session_b_gate)?;

    let session_a_code = format!(
        r#"
import {{ output_text, set_yield_time }} from "@openai/code_mode";
import {{ exec_command }} from "tools.js";

output_text("session a start");
set_yield_time(10);
{session_a_wait}
output_text("session a done");
"#
    );
    let session_b_code = format!(
        r#"
import {{ output_text, set_yield_time }} from "@openai/code_mode";
import {{ exec_command }} from "tools.js";

output_text("session b start");
set_yield_time(10);
{session_b_wait}
output_text("session b done");
"#
    );

    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_custom_tool_call("call-1", "exec", &session_a_code),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let first_completion = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "session a waiting"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    test.submit_turn("start session a").await?;

    let first_request = first_completion.single_request();
    let first_items = custom_tool_output_items(&first_request, "call-1");
    assert_eq!(first_items.len(), 2);
    let session_a_id = extract_running_session_id(text_item(&first_items, 0));
    assert_eq!(text_item(&first_items, 1), "session a start");

    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-3"),
            ev_custom_tool_call("call-2", "exec", &session_b_code),
            ev_completed("resp-3"),
        ]),
    )
    .await;
    let second_completion = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-2", "session b waiting"),
            ev_completed("resp-4"),
        ]),
    )
    .await;

    test.submit_turn("start session b").await?;

    let second_request = second_completion.single_request();
    let second_items = custom_tool_output_items(&second_request, "call-2");
    assert_eq!(second_items.len(), 2);
    let session_b_id = extract_running_session_id(text_item(&second_items, 0));
    assert_eq!(text_item(&second_items, 1), "session b start");

    fs::write(&session_a_gate, "ready")?;
    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-5"),
            responses::ev_function_call(
                "call-3",
                "exec_wait",
                &serde_json::to_string(&serde_json::json!({
                    "session_id": session_b_id,
                    "yield_time_ms": 1_000,
                }))?,
            ),
            ev_completed("resp-5"),
        ]),
    )
    .await;
    let third_completion = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-3", "session b still waiting"),
            ev_completed("resp-6"),
        ]),
    )
    .await;

    test.submit_turn("wait session b").await?;

    let third_request = third_completion.single_request();
    let third_items = function_tool_output_items(&third_request, "call-3");
    assert_eq!(third_items.len(), 1);
    assert_regex_match(
        concat!(
            r"(?s)\A",
            r"Script running with session ID \d+\nWall time \d+\.\d seconds\nOutput:\n\z"
        ),
        text_item(&third_items, 0),
    );
    assert_eq!(
        extract_running_session_id(text_item(&third_items, 0)),
        session_b_id
    );

    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-7"),
            responses::ev_function_call(
                "call-4",
                "exec_wait",
                &serde_json::to_string(&serde_json::json!({
                    "session_id": session_a_id,
                    "terminate": true,
                }))?,
            ),
            ev_completed("resp-7"),
        ]),
    )
    .await;
    let fourth_completion = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-4", "session a already done"),
            ev_completed("resp-8"),
        ]),
    )
    .await;

    test.submit_turn("terminate session a").await?;

    let fourth_request = fourth_completion.single_request();
    let fourth_items = function_tool_output_items(&fourth_request, "call-4");
    assert_eq!(fourth_items.len(), 1);
    assert_regex_match(
        concat!(
            r"(?s)\A",
            r"Script terminated\nWall time \d+\.\d seconds\nOutput:\n\z"
        ),
        text_item(&fourth_items, 0),
    );

    Ok(())
}

#[cfg_attr(windows, ignore = "no exec_command on Windows")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_exec_wait_uses_its_own_max_tokens_budget() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let mut builder = test_codex().with_config(move |config| {
        let _ = config.features.enable(Feature::CodeMode);
    });
    let test = builder.build(&server).await?;
    let completion_gate = test.workspace_path("code-mode-max-tokens.ready");
    let completion_wait = wait_for_file_source(&completion_gate)?;

    let code = format!(
        r#"
import {{ output_text, set_max_output_tokens_per_exec_call, set_yield_time }} from "@openai/code_mode";
import {{ exec_command }} from "tools.js";

output_text("phase 1");
set_max_output_tokens_per_exec_call(100);
set_yield_time(10);
{completion_wait}
output_text("token one token two token three token four token five token six token seven");
"#
    );

    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_custom_tool_call("call-1", "exec", &code),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let first_completion = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "waiting"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    test.submit_turn("start the long exec").await?;

    let first_request = first_completion.single_request();
    let first_items = custom_tool_output_items(&first_request, "call-1");
    assert_eq!(first_items.len(), 2);
    assert_eq!(text_item(&first_items, 1), "phase 1");
    let session_id = extract_running_session_id(text_item(&first_items, 0));

    fs::write(&completion_gate, "ready")?;
    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-3"),
            responses::ev_function_call(
                "call-2",
                "exec_wait",
                &serde_json::to_string(&serde_json::json!({
                    "session_id": session_id,
                    "yield_time_ms": 1_000,
                    "max_tokens": 6,
                }))?,
            ),
            ev_completed("resp-3"),
        ]),
    )
    .await;
    let second_completion = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-2", "done"),
            ev_completed("resp-4"),
        ]),
    )
    .await;

    test.submit_turn("wait for completion").await?;

    let second_request = second_completion.single_request();
    let second_items = function_tool_output_items(&second_request, "call-2");
    assert_eq!(second_items.len(), 2);
    assert_regex_match(
        concat!(
            r"(?s)\A",
            r"Script completed\nWall time \d+\.\d seconds\nOutput:\n\z"
        ),
        text_item(&second_items, 0),
    );
    let expected_pattern = r#"(?sx)
\A
Total\ output\ lines:\ 1\n
\n
.*…\d+\ tokens\ truncated….*
\z
"#;
    assert_regex_match(expected_pattern, text_item(&second_items, 1));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_can_output_serialized_text_via_openai_code_mode_module() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let (_test, second_mock) = run_code_mode_turn(
        &server,
        "use exec to return structured text",
        r#"
import { output_text } from "@openai/code_mode";

output_text({ json: true });
"#,
        false,
    )
    .await?;

    let req = second_mock.single_request();
    let (output, success) = custom_tool_output_body_and_success(&req, "call-1");
    assert_ne!(
        success,
        Some(false),
        "exec call failed unexpectedly: {output}"
    );
    assert_eq!(output, r#"{"json":true}"#);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_surfaces_output_text_stringify_errors() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let (_test, second_mock) = run_code_mode_turn(
        &server,
        "use exec to return circular text",
        r#"
import { output_text } from "@openai/code_mode";

const circular = {};
circular.self = circular;
output_text(circular);
"#,
        false,
    )
    .await?;

    let req = second_mock.single_request();
    let items = custom_tool_output_items(&req, "call-1");
    let (_, success) = req
        .custom_tool_call_output_content_and_success("call-1")
        .expect("custom tool output should be present");
    assert_ne!(
        success,
        Some(true),
        "circular stringify unexpectedly succeeded"
    );
    assert_eq!(items.len(), 2);
    assert_regex_match(
        concat!(
            r"(?s)\A",
            r"Script failed\nWall time \d+\.\d seconds\nOutput:\n\z"
        ),
        text_item(&items, 0),
    );
    assert!(text_item(&items, 1).contains("Script error:"));
    assert!(text_item(&items, 1).contains("Converting circular structure to JSON"));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_can_output_images_via_openai_code_mode_module() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let (_test, second_mock) = run_code_mode_turn(
        &server,
        "use exec to return images",
        r#"
import { output_image } from "@openai/code_mode";

output_image("https://example.com/image.jpg");
output_image("data:image/png;base64,AAA");
"#,
        false,
    )
    .await?;

    let req = second_mock.single_request();
    let items = custom_tool_output_items(&req, "call-1");
    let (_, success) = custom_tool_output_body_and_success(&req, "call-1");
    assert_ne!(
        success,
        Some(false),
        "code_mode image output failed unexpectedly"
    );
    assert_eq!(items.len(), 3);
    assert_regex_match(
        concat!(
            r"(?s)\A",
            r"Script completed\nWall time \d+\.\d seconds\nOutput:\n\z"
        ),
        text_item(&items, 0),
    );
    assert_eq!(
        items[1],
        serde_json::json!({
            "type": "input_image",
            "image_url": "https://example.com/image.jpg"
        }),
    );
    assert_eq!(
        items[2],
        serde_json::json!({
            "type": "input_image",
            "image_url": "data:image/png;base64,AAA"
        }),
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_can_apply_patch_via_nested_tool() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let file_name = "code_mode_apply_patch.txt";
    let patch = format!(
        "*** Begin Patch\n*** Add File: {file_name}\n+hello from code_mode\n*** End Patch\n"
    );
    let code = format!(
        "import {{ apply_patch }} from \"tools.js\";\nconst items = await apply_patch({patch:?});\nadd_content(items);\n"
    );

    let (test, second_mock) =
        run_code_mode_turn(&server, "use exec to run apply_patch", &code, true).await?;

    let req = second_mock.single_request();
    let items = custom_tool_output_items(&req, "call-1");
    let (_, success) = req
        .custom_tool_call_output_content_and_success("call-1")
        .expect("custom tool output should be present");
    assert_ne!(
        success,
        Some(false),
        "exec apply_patch call failed unexpectedly: {items:?}"
    );
    assert_eq!(items.len(), 2);
    assert_regex_match(
        concat!(
            r"(?s)\A",
            r"Script completed\nWall time \d+\.\d seconds\nOutput:\n\z"
        ),
        text_item(&items, 0),
    );

    let file_path = test.cwd_path().join(file_name);
    assert_eq!(fs::read_to_string(&file_path)?, "hello from code_mode\n");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_can_print_structured_mcp_tool_result_fields() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let code = r#"
import { echo } from "tools/mcp/rmcp.js";

const { content, structuredContent, isError } = await echo({
  message: "ping",
});
add_content(
  `echo=${structuredContent?.echo ?? "missing"}\n` +
    `env=${structuredContent?.env ?? "missing"}\n` +
    `isError=${String(isError)}\n` +
    `contentLength=${content.length}`
);
"#;

    let (_test, second_mock) =
        run_code_mode_turn_with_rmcp(&server, "use exec to run the rmcp echo tool", code).await?;

    let req = second_mock.single_request();
    let (output, success) = custom_tool_output_body_and_success(&req, "call-1");
    assert_ne!(
        success,
        Some(false),
        "exec rmcp echo call failed unexpectedly: {output}"
    );
    assert_eq!(
        output,
        "echo=ECHOING: ping
env=propagated-env
isError=false
contentLength=0"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_exports_all_tools_metadata_for_builtin_tools() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let code = r#"
import { ALL_TOOLS } from "tools.js";

const tool = ALL_TOOLS.find(({ module, name }) => module === "tools.js" && name === "view_image");
add_content(JSON.stringify(tool));
"#;

    let (_test, second_mock) =
        run_code_mode_turn(&server, "use exec to inspect ALL_TOOLS", code, false).await?;

    let req = second_mock.single_request();
    let (output, success) = custom_tool_output_body_and_success(&req, "call-1");
    assert_ne!(
        success,
        Some(false),
        "exec ALL_TOOLS lookup failed unexpectedly: {output}"
    );

    let parsed: Value = serde_json::from_str(&output)?;
    assert_eq!(
        parsed,
        serde_json::json!({
            "module": "tools.js",
            "name": "view_image",
            "description": "View a local image from the filesystem (only use if given a full filepath by the user, and the image isn't already attached to the thread context within <image ...> tags).\n\nCode mode declaration:\n```ts\nimport { view_image } from \"tools.js\";\ndeclare function view_image(args: {\n  path: string;\n}): Promise<unknown>;\n```",
        })
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_exports_all_tools_metadata_for_namespaced_mcp_tools() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let code = r#"
import { ALL_TOOLS } from "tools.js";

const tool = ALL_TOOLS.find(
  ({ module, name }) => module === "tools/mcp/rmcp.js" && name === "echo"
);
add_content(JSON.stringify(tool));
"#;

    let (_test, second_mock) =
        run_code_mode_turn_with_rmcp(&server, "use exec to inspect ALL_TOOLS", code).await?;

    let req = second_mock.single_request();
    let (output, success) = custom_tool_output_body_and_success(&req, "call-1");
    assert_ne!(
        success,
        Some(false),
        "exec ALL_TOOLS MCP lookup failed unexpectedly: {output}"
    );

    let parsed: Value = serde_json::from_str(&output)?;
    assert_eq!(
        parsed,
        serde_json::json!({
            "module": "tools/mcp/rmcp.js",
            "name": "echo",
            "description": "Echo back the provided message and include environment data.\n\nCode mode declaration:\n```ts\nimport { echo } from \"tools/mcp/rmcp.js\";\ndeclare function echo(args: {\n  env_var?: string;\n  message: string;\n}): Promise<{\n  _meta?: unknown;\n  content: Array<unknown>;\n  isError?: boolean;\n  structuredContent?: unknown;\n}>;\n```",
        })
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_can_print_content_only_mcp_tool_result_fields() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let code = r#"
import { image_scenario } from "tools/mcp/rmcp.js";

const { content, structuredContent, isError } = await image_scenario({
  scenario: "text_only",
  caption: "caption from mcp",
});
add_content(
  `firstType=${content[0]?.type ?? "missing"}\n` +
    `firstText=${content[0]?.text ?? "missing"}\n` +
    `structuredContent=${String(structuredContent ?? null)}\n` +
    `isError=${String(isError)}`
);
"#;

    let (_test, second_mock) = run_code_mode_turn_with_rmcp(
        &server,
        "use exec to run the rmcp image scenario tool",
        code,
    )
    .await?;

    let req = second_mock.single_request();
    let (output, success) = custom_tool_output_body_and_success(&req, "call-1");
    assert_ne!(
        success,
        Some(false),
        "exec rmcp image scenario call failed unexpectedly: {output}"
    );
    assert_eq!(
        output,
        "firstType=text
firstText=caption from mcp
structuredContent=null
isError=false"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_can_print_error_mcp_tool_result_fields() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let code = r#"
import { echo } from "tools/mcp/rmcp.js";

const { content, structuredContent, isError } = await echo({});
const firstText = content[0]?.text ?? "";
const mentionsMissingMessage =
  firstText.includes("missing field") && firstText.includes("message");
add_content(
  `isError=${String(isError)}\n` +
    `contentLength=${content.length}\n` +
    `mentionsMissingMessage=${String(mentionsMissingMessage)}\n` +
    `structuredContent=${String(structuredContent ?? null)}`
);
"#;

    let (_test, second_mock) =
        run_code_mode_turn_with_rmcp(&server, "use exec to call rmcp echo badly", code).await?;

    let req = second_mock.single_request();
    let (output, success) = custom_tool_output_body_and_success(&req, "call-1");
    assert_ne!(
        success,
        Some(false),
        "exec rmcp error call failed unexpectedly: {output}"
    );
    assert_eq!(
        output,
        "isError=true
contentLength=1
mentionsMissingMessage=true
structuredContent=null"
    );

    Ok(())
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_can_store_and_load_values_across_turns() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let mut builder = test_codex().with_config(move |config| {
        let _ = config.features.enable(Feature::CodeMode);
    });
    let test = builder.build(&server).await?;

    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_custom_tool_call(
                "call-1",
                "exec",
                r#"
import { store } from "@openai/code_mode";

store("nb", { title: "Notebook", items: [1, true, null] });
add_content("stored");
"#,
            ),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let first_follow_up = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "stored"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    test.submit_turn("store value for later").await?;

    let first_request = first_follow_up.single_request();
    let (first_output, first_success) =
        custom_tool_output_body_and_success(&first_request, "call-1");
    assert_ne!(
        first_success,
        Some(false),
        "exec store call failed unexpectedly: {first_output}"
    );
    assert_eq!(first_output, "stored");

    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-3"),
            ev_custom_tool_call(
                "call-2",
                "exec",
                r#"
import { load } from "openai/code_mode";

add_content(JSON.stringify(load("nb")));
"#,
            ),
            ev_completed("resp-3"),
        ]),
    )
    .await;
    let second_follow_up = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-2", "loaded"),
            ev_completed("resp-4"),
        ]),
    )
    .await;

    test.submit_turn("load the stored value").await?;

    let second_request = second_follow_up.single_request();
    let (second_output, second_success) =
        custom_tool_output_body_and_success(&second_request, "call-2");
    assert_ne!(
        second_success,
        Some(false),
        "exec load call failed unexpectedly: {second_output}"
    );
    let loaded: Value = serde_json::from_str(&second_output)?;
    assert_eq!(
        loaded,
        serde_json::json!({ "title": "Notebook", "items": [1, true, null] })
    );

    Ok(())
}
