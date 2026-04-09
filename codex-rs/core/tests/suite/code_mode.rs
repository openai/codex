#![allow(clippy::expect_used, clippy::unwrap_used)]

use anyhow::Result;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use codex_config::types::McpServerConfig;
use codex_config::types::McpServerTransportConfig;
use codex_features::Feature;
use codex_protocol::dynamic_tools::DynamicToolCallOutputContentItem;
use codex_protocol::dynamic_tools::DynamicToolResponse;
use codex_protocol::dynamic_tools::DynamicToolSpec;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::user_input::UserInput;
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
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_match;
use pretty_assertions::assert_eq;
use serde_json::Value;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::time::Duration;
use std::time::Instant;
use wiremock::MockServer;

fn custom_tool_output_items(req: &ResponsesRequest, call_id: &str) -> Vec<Value> {
    match req.custom_tool_call_output(call_id).get("output") {
        Some(Value::Array(items)) => items.clone(),
        Some(Value::String(text)) => {
            vec![serde_json::json!({ "type": "input_text", "text": text })]
        }
        _ => panic!("custom tool output should be serialized as text or content items"),
    }
}

fn tool_names(body: &Value) -> Vec<String> {
    body.get("tools")
        .and_then(Value::as_array)
        .map(|tools| {
            tools
                .iter()
                .filter_map(|tool| {
                    tool.get("name")
                        .or_else(|| tool.get("type"))
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
                .collect()
        })
        .unwrap_or_default()
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

fn extract_running_cell_id(text: &str) -> String {
    text.strip_prefix("Script running with cell ID ")
        .and_then(|rest| rest.split('\n').next())
        .expect("running header should contain a cell ID")
        .to_string()
}

fn wait_for_file_source(path: &Path) -> Result<String> {
    let quoted_path = shlex::try_join([path.to_string_lossy().as_ref()])?;
    let command = format!("if [ -f {quoted_path} ]; then printf ready; fi");
    Ok(format!(
        r#"while ((await tools.exec_command({{ cmd: {command:?} }})).output !== "ready") {{
}}"#
    ))
}

fn custom_tool_output_body_and_success(
    req: &ResponsesRequest,
    call_id: &str,
) -> (String, Option<bool>) {
    let (content, success) = req
        .custom_tool_call_output_content_and_success(call_id)
        .expect("custom tool output should be present");
    let items = custom_tool_output_items(req, call_id);
    let text_items = items
        .iter()
        .filter_map(|item| item.get("text").and_then(Value::as_str))
        .collect::<Vec<_>>();
    let output = match text_items.as_slice() {
        [] => content.unwrap_or_default(),
        [only] => (*only).to_string(),
        [_, rest @ ..] => rest.concat(),
    };
    (output, success)
}

fn custom_tool_output_last_non_empty_text(req: &ResponsesRequest, call_id: &str) -> Option<String> {
    match req.custom_tool_call_output(call_id).get("output") {
        Some(Value::String(text)) if !text.trim().is_empty() => Some(text.clone()),
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| item.get("text").and_then(Value::as_str))
            .rfind(|text| !text.trim().is_empty())
            .map(str::to_string),
        Some(Value::String(_))
        | Some(Value::Object(_))
        | Some(Value::Number(_))
        | Some(Value::Bool(_))
        | Some(Value::Null)
        | None => None,
    }
}

async fn run_code_mode_turn(
    server: &MockServer,
    prompt: &str,
    code: &str,
    include_apply_patch: bool,
) -> Result<(TestCodex, ResponseMock)> {
    let mut builder = test_codex()
        .with_model("test-gpt-5.1-codex")
        .with_config(move |config| {
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
    let mut builder = test_codex()
        .with_model("test-gpt-5.1-codex")
        .with_config(move |config| {
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
                    tools: HashMap::new(),
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
text(JSON.stringify(await tools.exec_command({ cmd: "printf code_mode_exec_marker" })));
"#,
        /*include_apply_patch*/ false,
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
        text_item(&items, /*index*/ 0),
    );
    let parsed: Value = serde_json::from_str(text_item(&items, /*index*/ 1))?;
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_only_restricts_prompt_tools() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let resp_mock = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-1"),
        ]),
    )
    .await;

    let mut builder = test_codex().with_config(|config| {
        let _ = config.features.enable(Feature::CodeModeOnly);
    });
    let test = builder.build(&server).await?;
    test.submit_turn("list tools in code mode only").await?;

    let first_body = resp_mock.single_request().body_json();
    assert_eq!(
        tool_names(&first_body),
        vec!["exec".to_string(), "wait".to_string()]
    );

    Ok(())
}

#[cfg_attr(windows, ignore = "no exec_command on Windows")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_only_can_call_nested_tools() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_custom_tool_call(
                "call-1",
                "exec",
                r#"
const output = await tools.exec_command({ cmd: "printf code_mode_only_nested_tool_marker" });
text(output.output);
"#,
            ),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let follow_up_mock = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    let mut builder = test_codex().with_config(|config| {
        let _ = config.features.enable(Feature::CodeModeOnly);
    });
    let test = builder.build(&server).await?;
    test.submit_turn("use exec to run nested tool in code mode only")
        .await?;

    let request = follow_up_mock.single_request();
    let (output, success) = custom_tool_output_body_and_success(&request, "call-1");
    assert_ne!(
        success,
        Some(false),
        "code_mode_only nested tool call failed unexpectedly: {output}"
    );
    assert_eq!(output, "code_mode_only_nested_tool_marker");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_update_plan_nested_tool_result_is_empty_object() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let (_test, second_mock) = run_code_mode_turn(
        &server,
        "use exec to run update_plan",
        r#"
const result = await tools.update_plan({
  plan: [{ step: "Run update_plan from code mode", status: "in_progress" }],
});
text(JSON.stringify(result));
"#,
        /*include_apply_patch*/ false,
    )
    .await?;

    let req = second_mock.single_request();
    let (output, success) = custom_tool_output_body_and_success(&req, "call-1");
    assert_ne!(
        success,
        Some(false),
        "exec update_plan call failed unexpectedly: {output}"
    );

    let parsed: Value = serde_json::from_str(&output)?;
    assert_eq!(parsed, serde_json::json!({}));

    Ok(())
}

#[cfg_attr(windows, ignore = "flaky on windows")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_nested_tool_calls_can_run_in_parallel() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let mut builder = test_codex()
        .with_model("test-gpt-5.1-codex")
        .with_config(move |config| {
            let _ = config.features.enable(Feature::CodeMode);
        });
    let test = builder.build(&server).await?;

    let warmup_code = r#"
const args = {
  sleep_after_ms: 10,
  barrier: {
    id: "code-mode-parallel-tools-warmup",
    participants: 2,
    timeout_ms: 1_000,
  },
};

await Promise.all([
  tools.test_sync_tool(args),
  tools.test_sync_tool(args),
]);
"#;
    let code = r#"
const args = {
  sleep_after_ms: 300,
  barrier: {
    id: "code-mode-parallel-tools",
    participants: 2,
    timeout_ms: 1_000,
  },
};

const results = await Promise.all([
  tools.test_sync_tool(args),
  tools.test_sync_tool(args),
]);

text(JSON.stringify(results));
"#;

    let response_mock = responses::mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-warm-1"),
                ev_custom_tool_call("call-warm-1", "exec", warmup_code),
                ev_completed("resp-warm-1"),
            ]),
            sse(vec![
                ev_assistant_message("msg-warm-1", "warmup done"),
                ev_completed("resp-warm-2"),
            ]),
            sse(vec![
                ev_response_created("resp-1"),
                ev_custom_tool_call("call-1", "exec", code),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_assistant_message("msg-1", "done"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    test.submit_turn("warm up nested tools in parallel").await?;

    let start = Instant::now();
    test.submit_turn("run nested tools in parallel").await?;
    let duration = start.elapsed();

    assert!(
        duration < Duration::from_millis(1_600),
        "expected nested tools to finish in parallel, got {duration:?}",
    );

    let req = response_mock
        .last_request()
        .expect("parallel code mode run should send a completion request");
    let items = custom_tool_output_items(&req, "call-1");
    assert_eq!(items.len(), 2);
    assert_eq!(text_item(&items, /*index*/ 1), "[\"ok\",\"ok\"]");

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
        r#"// @exec: {"max_output_tokens": 6}
text(JSON.stringify(await tools.exec_command({
  cmd: "printf 'token one token two token three token four token five token six token seven'",
  max_output_tokens: 100
})));
"#,
        /*include_apply_patch*/ false,
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
        text_item(&items, /*index*/ 0),
    );
    let expected_pattern = r#"(?sx)
\A
Total\ output\ lines:\ 1\n
\n
.*…\d+\ tokens\ truncated….*
\z
"#;
    assert_regex_match(expected_pattern, text_item(&items, /*index*/ 1));

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
text("before crash");
text("still before crash");
throw new Error("boom");
"#,
        /*include_apply_patch*/ false,
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
        text_item(&items, /*index*/ 0),
    );
    assert_eq!(text_item(&items, /*index*/ 1), "before crash");
    assert_eq!(text_item(&items, /*index*/ 2), "still before crash");
    assert_regex_match(
        r#"(?sx)
\A
Script\ error:\n
Error:\ boom\n
(?:\s+at\ .+\n?)+
\z
"#,
        text_item(&items, /*index*/ 3),
    );

    Ok(())
}

#[cfg_attr(windows, ignore = "no exec_command on Windows")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_exec_surfaces_handler_errors_as_exceptions() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let (_test, second_mock) = run_code_mode_turn(
        &server,
        "surface nested tool handler failures as script exceptions",
        r#"
try {
  await tools.exec_command({});
  text("no-exception");
} catch (error) {
  text(`caught:${error?.message ?? String(error)}`);
}
"#,
        /*include_apply_patch*/ false,
    )
    .await?;

    let request = second_mock.single_request();
    let (output, success) = custom_tool_output_body_and_success(&request, "call-1");
    assert_ne!(
        success,
        Some(false),
        "script should catch the nested tool error: {output}"
    );
    assert!(
        output.contains("caught:"),
        "expected caught exception text in output: {output}"
    );
    assert!(
        !output.contains("no-exception"),
        "nested tool error should not allow success path: {output}"
    );

    Ok(())
}

#[cfg_attr(windows, ignore = "no exec_command on Windows")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_can_yield_and_resume_with_wait() -> Result<()> {
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
text("phase 1");
yield_control();
{phase_2_wait}
text("phase 2");
{phase_3_wait}
text("phase 3");
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
            r"Script running with cell ID \d+\nWall time \d+\.\d seconds\nOutput:\n\z"
        ),
        text_item(&first_items, /*index*/ 0),
    );
    assert_eq!(text_item(&first_items, /*index*/ 1), "phase 1");
    let cell_id = extract_running_cell_id(text_item(&first_items, /*index*/ 0));

    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-3"),
            responses::ev_function_call(
                "call-2",
                "wait",
                &serde_json::to_string(&serde_json::json!({
                    "cell_id": cell_id.clone(),
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
            r"Script running with cell ID \d+\nWall time \d+\.\d seconds\nOutput:\n\z"
        ),
        text_item(&second_items, /*index*/ 0),
    );
    assert_eq!(
        extract_running_cell_id(text_item(&second_items, /*index*/ 0)),
        cell_id
    );
    assert_eq!(text_item(&second_items, /*index*/ 1), "phase 2");

    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-5"),
            responses::ev_function_call(
                "call-3",
                "wait",
                &serde_json::to_string(&serde_json::json!({
                    "cell_id": cell_id.clone(),
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
        text_item(&third_items, /*index*/ 0),
    );
    assert_eq!(text_item(&third_items, /*index*/ 1), "phase 3");

    Ok(())
}

#[cfg_attr(windows, ignore = "no exec_command on Windows")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_yield_timeout_works_for_busy_loop() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let mut builder = test_codex().with_config(move |config| {
        let _ = config.features.enable(Feature::CodeMode);
    });
    let test = builder.build(&server).await?;

    let code = r#"// @exec: {"yield_time_ms": 100}
text("phase 1");
while (true) {}
"#;

    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_custom_tool_call("call-1", "exec", code),
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

    tokio::time::timeout(
        Duration::from_secs(5),
        test.submit_turn("start the busy loop"),
    )
    .await??;

    let first_request = first_completion.single_request();
    let first_items = custom_tool_output_items(&first_request, "call-1");
    assert_eq!(first_items.len(), 2);
    assert_regex_match(
        concat!(
            r"(?s)\A",
            r"Script running with cell ID \d+\nWall time \d+\.\d seconds\nOutput:\n\z"
        ),
        text_item(&first_items, /*index*/ 0),
    );
    assert_eq!(text_item(&first_items, /*index*/ 1), "phase 1");
    let cell_id = extract_running_cell_id(text_item(&first_items, /*index*/ 0));

    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-3"),
            responses::ev_function_call(
                "call-2",
                "wait",
                &serde_json::to_string(&serde_json::json!({
                    "cell_id": cell_id.clone(),
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
        text_item(&second_items, /*index*/ 0),
    );

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
text("session a start");
yield_control();
{session_a_wait}
text("session a done");
"#
    );
    let session_b_code = format!(
        r#"
text("session b start");
yield_control();
{session_b_wait}
text("session b done");
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
    let session_a_id = extract_running_cell_id(text_item(&first_items, /*index*/ 0));
    assert_eq!(text_item(&first_items, /*index*/ 1), "session a start");

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
    let session_b_id = extract_running_cell_id(text_item(&second_items, /*index*/ 0));
    assert_eq!(text_item(&second_items, /*index*/ 1), "session b start");
    assert_ne!(session_a_id, session_b_id);

    fs::write(&session_a_gate, "ready")?;
    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-5"),
            responses::ev_function_call(
                "call-3",
                "wait",
                &serde_json::to_string(&serde_json::json!({
                    "cell_id": session_a_id.clone(),
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
        text_item(&third_items, /*index*/ 0),
    );
    assert_eq!(text_item(&third_items, /*index*/ 1), "session a done");

    fs::write(&session_b_gate, "ready")?;
    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-7"),
            responses::ev_function_call(
                "call-4",
                "wait",
                &serde_json::to_string(&serde_json::json!({
                    "cell_id": session_b_id.clone(),
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
        text_item(&fourth_items, /*index*/ 0),
    );
    assert_eq!(text_item(&fourth_items, /*index*/ 1), "session b done");

    Ok(())
}

#[cfg_attr(windows, ignore = "no exec_command on Windows")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_wait_can_terminate_and_continue() -> Result<()> {
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
text("phase 1");
yield_control();
{termination_wait}
text("phase 2");
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
    let cell_id = extract_running_cell_id(text_item(&first_items, /*index*/ 0));
    assert_eq!(text_item(&first_items, /*index*/ 1), "phase 1");

    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-3"),
            responses::ev_function_call(
                "call-2",
                "wait",
                &serde_json::to_string(&serde_json::json!({
                    "cell_id": cell_id.clone(),
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
        text_item(&second_items, /*index*/ 0),
    );

    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-5"),
            ev_custom_tool_call(
                "call-3",
                "exec",
                r#"
text("after terminate");
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
        text_item(&third_items, /*index*/ 0),
    );
    assert_eq!(text_item(&third_items, /*index*/ 1), "after terminate");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_wait_returns_error_for_unknown_session() -> Result<()> {
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
                "wait",
                &serde_json::to_string(&serde_json::json!({
                    "cell_id": "999999",
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

    test.submit_turn("wait on an unknown exec cell").await?;

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
        text_item(&items, /*index*/ 0),
    );
    assert_eq!(
        text_item(&items, /*index*/ 1),
        "Script error:\nexec cell 999999 not found"
    );

    Ok(())
}

#[cfg_attr(windows, ignore = "no exec_command on Windows")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_wait_terminate_returns_completed_session_if_it_finished_after_yield_control()
-> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let mut builder = test_codex().with_config(move |config| {
        let _ = config.features.enable(Feature::CodeMode);
    });
    let test = builder.build(&server).await?;
    let session_a_gate = test.workspace_path("code-mode-session-a-finished.ready");
    let session_b_gate = test.workspace_path("code-mode-session-b-blocked.ready");
    let session_a_done_marker = test.workspace_path("code-mode-session-a-done.txt");
    let session_a_wait = wait_for_file_source(&session_a_gate)?;
    let session_b_wait = wait_for_file_source(&session_b_gate)?;
    let session_a_done_marker_quoted =
        shlex::try_join([session_a_done_marker.to_string_lossy().as_ref()])?;
    let session_a_done_command = format!("printf done > {session_a_done_marker_quoted}");

    let session_a_code = format!(
        r#"
text("session a start");
yield_control();
{session_a_wait}
text("session a done");
await tools.exec_command({{ cmd: {session_a_done_command:?} }});
"#
    );
    let session_b_code = format!(
        r#"
text("session b start");
yield_control();
{session_b_wait}
text("session b done");
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
    let session_a_id = extract_running_cell_id(text_item(&first_items, /*index*/ 0));
    assert_eq!(text_item(&first_items, /*index*/ 1), "session a start");

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
    let session_b_id = extract_running_cell_id(text_item(&second_items, /*index*/ 0));
    assert_eq!(text_item(&second_items, /*index*/ 1), "session b start");

    fs::write(&session_a_gate, "ready")?;
    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-5"),
            responses::ev_function_call(
                "call-3",
                "wait",
                &serde_json::to_string(&serde_json::json!({
                    "cell_id": session_b_id.clone(),
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
            r"Script running with cell ID \d+\nWall time \d+\.\d seconds\nOutput:\n\z"
        ),
        text_item(&third_items, /*index*/ 0),
    );
    assert_eq!(
        extract_running_cell_id(text_item(&third_items, /*index*/ 0)),
        session_b_id
    );

    for _ in 0..100 {
        if session_a_done_marker.exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(session_a_done_marker.exists());

    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-7"),
            responses::ev_function_call(
                "call-4",
                "wait",
                &serde_json::to_string(&serde_json::json!({
                    "cell_id": session_a_id.clone(),
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
    match fourth_items.len() {
        1 => {
            assert_regex_match(
                concat!(
                    r"(?s)\A",
                    r"Script terminated\nWall time \d+\.\d seconds\nOutput:\n\z"
                ),
                text_item(&fourth_items, /*index*/ 0),
            );
        }
        2 => {
            assert_regex_match(
                concat!(
                    r"(?s)\A",
                    r"Script (?:completed|terminated)\nWall time \d+\.\d seconds\nOutput:\n\z"
                ),
                text_item(&fourth_items, /*index*/ 0),
            );
            assert_eq!(text_item(&fourth_items, /*index*/ 1), "session a done");
        }
        other => panic!("unexpected number of content items: {other}"),
    }

    Ok(())
}

#[cfg_attr(windows, ignore = "no exec_command on Windows")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_background_keeps_running_on_later_turn_without_wait() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let mut builder = test_codex().with_config(move |config| {
        let _ = config.features.enable(Feature::CodeMode);
    });
    let test = builder.build(&server).await?;
    let resumed_file = test.workspace_path("code-mode-yield-resumed.txt");
    let resumed_file_quoted = shlex::try_join([resumed_file.to_string_lossy().as_ref()])?;
    let write_file_command = format!("printf resumed > {resumed_file_quoted}");
    let wait_for_file_command =
        format!("while [ ! -f {resumed_file_quoted} ]; do sleep 0.01; done; printf ready");
    let code = format!(
        r#"
text("before yield");
yield_control();
await tools.exec_command({{ cmd: {write_file_command:?} }});
text("after yield");
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
            ev_assistant_message("msg-1", "exec yielded"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    test.submit_turn("start yielded exec").await?;

    let first_request = first_completion.single_request();
    let first_items = custom_tool_output_items(&first_request, "call-1");
    assert_eq!(first_items.len(), 2);
    assert_regex_match(
        concat!(
            r"(?s)\A",
            r"Script running with cell ID \d+\nWall time \d+\.\d seconds\nOutput:\n\z"
        ),
        text_item(&first_items, /*index*/ 0),
    );
    assert_eq!(text_item(&first_items, /*index*/ 1), "before yield");

    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-3"),
            responses::ev_function_call(
                "call-2",
                "exec_command",
                &serde_json::to_string(&serde_json::json!({
                    "cmd": wait_for_file_command,
                }))?,
            ),
            ev_completed("resp-3"),
        ]),
    )
    .await;
    let second_completion = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-2", "file appeared"),
            ev_completed("resp-4"),
        ]),
    )
    .await;

    test.submit_turn("wait for resumed file").await?;

    let second_request = second_completion.single_request();
    assert!(
        second_request
            .function_call_output_text("call-2")
            .is_some_and(|output| output.ends_with("ready"))
    );
    assert_eq!(fs::read_to_string(&resumed_file)?, "resumed");

    Ok(())
}

#[cfg_attr(windows, ignore = "no exec_command on Windows")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_wait_uses_its_own_max_tokens_budget() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let mut builder = test_codex().with_config(move |config| {
        let _ = config.features.enable(Feature::CodeMode);
    });
    let test = builder.build(&server).await?;
    let completion_gate = test.workspace_path("code-mode-max-tokens.ready");
    let completion_wait = wait_for_file_source(&completion_gate)?;

    let code = format!(
        r#"// @exec: {{"max_output_tokens": 100}}
text("phase 1");
yield_control();
{completion_wait}
text("token one token two token three token four token five token six token seven");
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
    assert_eq!(text_item(&first_items, /*index*/ 1), "phase 1");
    let cell_id = extract_running_cell_id(text_item(&first_items, /*index*/ 0));

    fs::write(&completion_gate, "ready")?;
    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-3"),
            responses::ev_function_call(
                "call-2",
                "wait",
                &serde_json::to_string(&serde_json::json!({
                    "cell_id": cell_id.clone(),
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
        text_item(&second_items, /*index*/ 0),
    );
    let expected_pattern = r#"(?sx)
\A
Total\ output\ lines:\ 1\n
\n
.*…\d+\ tokens\ truncated….*
\z
"#;
    assert_regex_match(expected_pattern, text_item(&second_items, /*index*/ 1));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_can_output_serialized_text_via_global_helper() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let (_test, second_mock) = run_code_mode_turn(
        &server,
        "use exec to return structured text",
        r#"
text({ json: true });
"#,
        /*include_apply_patch*/ false,
    )
    .await?;

    let req = second_mock.single_request();
    let (output, success) = custom_tool_output_body_and_success(&req, "call-1");
    eprintln!(
        "hidden dynamic tool raw output: {}",
        req.custom_tool_call_output("call-1")
    );
    assert_ne!(
        success,
        Some(false),
        "exec call failed unexpectedly: {output}"
    );
    assert_eq!(output, r#"{"json":true}"#);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_can_resume_after_set_timeout() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let (_test, second_mock) = run_code_mode_turn(
        &server,
        "use exec to wait for a timeout",
        r#"
await new Promise((resolve) => setTimeout(resolve, 10));
text("timer done");
"#,
        /*include_apply_patch*/ false,
    )
    .await?;

    let req = second_mock.single_request();
    let (output, success) = custom_tool_output_body_and_success(&req, "call-1");
    assert_ne!(
        success,
        Some(false),
        "exec setTimeout call failed unexpectedly: {output}"
    );
    assert_eq!(output, "timer done");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_notify_injects_additional_exec_tool_output_into_active_context() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let (_test, second_mock) = run_code_mode_turn(
        &server,
        "use exec notify helper",
        r#"
notify("code_mode_notify_marker");
await tools.test_sync_tool({});
text("done");
"#,
        /*include_apply_patch*/ false,
    )
    .await?;

    let req = second_mock.single_request();
    let has_notify_output = req
        .inputs_of_type("custom_tool_call_output")
        .iter()
        .any(|item| {
            item.get("call_id").and_then(serde_json::Value::as_str) == Some("call-1")
                && item
                    .get("output")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|text| text.contains("code_mode_notify_marker"))
                && item.get("name").and_then(serde_json::Value::as_str) == Some("exec")
        });
    assert!(
        has_notify_output,
        "expected notify marker in custom_tool_call_output item: {:?}",
        req.input()
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_exit_stops_script_immediately() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let (_test, second_mock) = run_code_mode_turn(
        &server,
        "use exec to stop script early with exit helper",
        r#"
text("before");
exit();
text("after");
"#,
        /*include_apply_patch*/ false,
    )
    .await?;

    let req = second_mock.single_request();
    let items = custom_tool_output_items(&req, "call-1");
    let (output, success) = custom_tool_output_body_and_success(&req, "call-1");
    assert_ne!(
        success,
        Some(false),
        "exec exit helper call failed unexpectedly: {output}"
    );
    assert_eq!(items.len(), 2);
    assert_regex_match(
        concat!(
            r"(?s)\A",
            r"Script completed\nWall time \d+\.\d seconds\nOutput:\n\z"
        ),
        text_item(&items, /*index*/ 0),
    );
    assert_eq!(text_item(&items, /*index*/ 1), "before");
    assert_eq!(output, "before");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_surfaces_text_stringify_errors() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let (_test, second_mock) = run_code_mode_turn(
        &server,
        "use exec to return circular text",
        r#"
const circular = {};
circular.self = circular;
text(circular);
"#,
        /*include_apply_patch*/ false,
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
        text_item(&items, /*index*/ 0),
    );
    assert!(text_item(&items, /*index*/ 1).contains("Script error:"));
    assert!(text_item(&items, /*index*/ 1).contains("Converting circular structure to JSON"));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_can_output_images_via_global_helper() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let (_test, second_mock) = run_code_mode_turn(
        &server,
        "use exec to return images",
        r#"
image("https://example.com/image.jpg");
image("data:image/png;base64,AAA");
"#,
        /*include_apply_patch*/ false,
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
        text_item(&items, /*index*/ 0),
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
async fn code_mode_can_use_view_image_result_with_image_helper() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let mut builder = test_codex()
        .with_model("gpt-5.3-codex")
        .with_config(move |config| {
            let _ = config.features.enable(Feature::CodeMode);
            let _ = config.features.enable(Feature::ImageDetailOriginal);
        });
    let test = builder.build(&server).await?;

    let image_bytes = BASE64_STANDARD.decode(
        "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==",
    )?;
    let image_path = test.cwd_path().join("code_mode_view_image.png");
    fs::write(&image_path, image_bytes)?;

    let image_path_json = serde_json::to_string(&image_path.to_string_lossy().to_string())?;
    let code = format!(
        r#"
const out = await tools.view_image({{ path: {image_path_json}, detail: "original" }});
image(out);
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

    let second_mock = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    test.submit_turn("use exec to call view_image and emit its image output")
        .await?;

    let req = second_mock.single_request();
    let items = custom_tool_output_items(&req, "call-1");
    let (_, success) = custom_tool_output_body_and_success(&req, "call-1");
    assert_ne!(
        success,
        Some(false),
        "code_mode view_image call failed unexpectedly"
    );
    assert_eq!(items.len(), 2);
    assert_regex_match(
        concat!(
            r"(?s)\A",
            r"Script completed\nWall time \d+\.\d seconds\nOutput:\n\z"
        ),
        text_item(&items, /*index*/ 0),
    );

    assert_eq!(
        items[1].get("type").and_then(Value::as_str),
        Some("input_image")
    );

    let emitted_image_url = items[1]
        .get("image_url")
        .and_then(Value::as_str)
        .expect("image helper should emit an input_image item with image_url");
    assert!(emitted_image_url.starts_with("data:image/png;base64,"));
    assert_eq!(
        items[1].get("detail").and_then(Value::as_str),
        Some("original")
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
    let code = format!("text(await tools.apply_patch({patch:?}));\n");

    let (test, second_mock) = run_code_mode_turn(
        &server,
        "use exec to run apply_patch",
        &code,
        /*include_apply_patch*/ true,
    )
    .await?;

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
        text_item(&items, /*index*/ 0),
    );
    assert_eq!(text_item(&items, /*index*/ 1), "{}");

    let file_path = test.cwd_path().join(file_name);
    assert_eq!(fs::read_to_string(&file_path)?, "hello from code_mode\n");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_can_print_structured_mcp_tool_result_fields() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let code = r#"
const { content, structuredContent, isError } = await tools.mcp__rmcp__echo({
  message: "ping",
});
text(
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
async fn code_mode_exposes_mcp_tools_on_global_tools_object() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let code = r#"
const { content, structuredContent, isError } = await tools.mcp__rmcp__echo({
  message: "ping",
});
text(
  `hasEcho=${String(Object.keys(tools).includes("mcp__rmcp__echo"))}\n` +
    `echoType=${typeof tools.mcp__rmcp__echo}\n` +
    `echo=${structuredContent?.echo ?? "missing"}\n` +
    `isError=${String(isError)}\n` +
    `contentLength=${content.length}`
);
"#;

    let (_test, second_mock) =
        run_code_mode_turn_with_rmcp(&server, "use exec to inspect the global tools object", code)
            .await?;

    let req = second_mock.single_request();
    let (output, success) = custom_tool_output_body_and_success(&req, "call-1");
    assert_ne!(
        success,
        Some(false),
        "exec global rmcp access failed unexpectedly: {output}"
    );
    assert_eq!(
        output,
        "hasEcho=true
echoType=function
echo=ECHOING: ping
isError=false
contentLength=0"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_exposes_namespaced_mcp_tools_on_global_tools_object() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let code = r#"
text(JSON.stringify({
  hasExecCommand: typeof tools.exec_command === "function",
  hasNamespacedEcho: typeof tools.mcp__rmcp__echo === "function",
}));
"#;

    let (_test, second_mock) =
        run_code_mode_turn_with_rmcp(&server, "use exec to inspect the global tools object", code)
            .await?;

    let req = second_mock.single_request();
    let (output, success) = custom_tool_output_body_and_success(&req, "call-1");
    assert_ne!(
        success,
        Some(false),
        "exec global tools inspection failed unexpectedly: {output}"
    );

    let parsed: Value = serde_json::from_str(&output)?;
    assert_eq!(
        parsed,
        serde_json::json!({
            "hasExecCommand": !cfg!(windows),
            "hasNamespacedEcho": true,
        })
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_exposes_normalized_illegal_mcp_tool_names() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let code = r#"
const result = await tools.mcp__rmcp__echo_tool({ message: "ping" });
text(`echo=${result.structuredContent.echo}`);
"#;

    let (_test, second_mock) = run_code_mode_turn_with_rmcp(
        &server,
        "use exec to call a normalized rmcp tool name",
        code,
    )
    .await?;

    let req = second_mock.single_request();
    let (output, success) = custom_tool_output_body_and_success(&req, "call-1");
    assert_ne!(
        success,
        Some(false),
        "exec normalized rmcp tool call failed unexpectedly: {output}"
    );
    assert_eq!(output, "echo=ECHOING: ping");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_lists_global_scope_items() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let code = r#"
text(JSON.stringify(Object.getOwnPropertyNames(globalThis).sort()));
"#;

    let (_test, second_mock) =
        run_code_mode_turn_with_rmcp(&server, "use exec to inspect global scope", code).await?;

    let req = second_mock.single_request();
    let (output, success) = custom_tool_output_body_and_success(&req, "call-1");
    assert_ne!(
        success,
        Some(false),
        "exec global scope inspection failed unexpectedly: {output}"
    );
    let globals = serde_json::from_str::<Vec<String>>(&output)?;
    let globals = globals.into_iter().collect::<HashSet<_>>();
    let expected = [
        "AggregateError",
        "ALL_TOOLS",
        "Array",
        "ArrayBuffer",
        "AsyncDisposableStack",
        "Atomics",
        "BigInt",
        "BigInt64Array",
        "BigUint64Array",
        "Boolean",
        "clearTimeout",
        "DataView",
        "Date",
        "DisposableStack",
        "Error",
        "EvalError",
        "FinalizationRegistry",
        "Float16Array",
        "Float32Array",
        "Float64Array",
        "Function",
        "Infinity",
        "Int16Array",
        "Int32Array",
        "Int8Array",
        "Intl",
        "Iterator",
        "JSON",
        "Map",
        "Math",
        "NaN",
        "Number",
        "Object",
        "Promise",
        "Proxy",
        "RangeError",
        "ReferenceError",
        "Reflect",
        "RegExp",
        "Set",
        "SharedArrayBuffer",
        "String",
        "SuppressedError",
        "Symbol",
        "SyntaxError",
        "Temporal",
        "TypeError",
        "URIError",
        "Uint16Array",
        "Uint32Array",
        "Uint8Array",
        "Uint8ClampedArray",
        "WeakMap",
        "WeakRef",
        "WeakSet",
        "WebAssembly",
        "__codexContentItems",
        "add_content",
        "decodeURI",
        "decodeURIComponent",
        "encodeURI",
        "encodeURIComponent",
        "escape",
        "exit",
        "eval",
        "globalThis",
        "image",
        "isFinite",
        "isNaN",
        "load",
        "notify",
        "parseFloat",
        "parseInt",
        "setTimeout",
        "store",
        "text",
        "tools",
        "undefined",
        "unescape",
        "yield_control",
    ];
    for g in &globals {
        assert!(
            expected.contains(&g.as_str()),
            "unexpected global {g} in {globals:?}"
        );
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_exports_all_tools_metadata_for_builtin_tools() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let code = r#"
const tool = ALL_TOOLS.find(({ name }) => name === "view_image");
text(JSON.stringify(tool));
"#;

    let (_test, second_mock) = run_code_mode_turn(
        &server,
        "use exec to inspect ALL_TOOLS",
        code,
        /*include_apply_patch*/ false,
    )
    .await?;

    let req = second_mock.single_request();
    let (output, success) = custom_tool_output_body_and_success(&req, "call-1");
    assert_ne!(
        success,
        Some(false),
        "exec ALL_TOOLS lookup failed unexpectedly: {output}"
    );

    let parsed: Value = serde_json::from_str(
        &custom_tool_output_last_non_empty_text(&req, "call-1")
            .expect("exec ALL_TOOLS lookup should emit JSON"),
    )?;
    assert_eq!(
        parsed,
        serde_json::json!({
            "name": "view_image",
            "description": "View a local image from the filesystem (only use if given a full filepath by the user, and the image isn't already attached to the thread context within <image ...> tags).\n\nexec tool declaration:\n```ts\ndeclare const tools: { view_image(args: {\n  // Local filesystem path to an image file\n  path: string;\n}): Promise<{\n  // Image detail hint returned by view_image. Returns `original` when original resolution is preserved, otherwise `null`.\n  detail: string | null;\n  // Data URL for the loaded image.\n  image_url: string;\n}>; };\n```",
        })
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_exports_all_tools_metadata_for_namespaced_mcp_tools() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let code = r#"
const tool = ALL_TOOLS.find(
  ({ name }) => name === "mcp__rmcp__echo"
);
text(JSON.stringify(tool));
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

    let parsed: Value = serde_json::from_str(
        &custom_tool_output_last_non_empty_text(&req, "call-1")
            .expect("exec ALL_TOOLS MCP lookup should emit JSON"),
    )?;
    assert_eq!(
        parsed,
        serde_json::json!({
            "name": "mcp__rmcp__echo",
            "description": concat!(
                "Echo back the provided message and include environment data.\n\n",
                "exec tool declaration:\n",
                "```ts\n",
                "declare const tools: { mcp__rmcp__echo(args: { env_var?: string; message: string; }): ",
                "Promise<mcp_result<{ echo: string; env: string | null; }>>; };\n",
                "```",
            ),
        })
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_can_call_hidden_dynamic_tools() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let mut builder = test_codex().with_config(move |config| {
        let _ = config.features.enable(Feature::CodeMode);
    });
    let base_test = builder.build(&server).await?;
    let new_thread = base_test
        .thread_manager
        .start_thread_with_tools(
            base_test.config.clone(),
            vec![DynamicToolSpec {
                name: "hidden_dynamic_tool".to_string(),
                description: "A hidden dynamic tool.".to_string(),
                input_schema: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "city": { "type": "string" }
                        },
                    "required": ["city"],
                    "additionalProperties": false,
                }),
                defer_loading: true,
            }],
            /*persist_extended_history*/ false,
        )
        .await?;
    let mut test = base_test;
    test.codex = new_thread.thread;
    test.session_configured = new_thread.session_configured;

    let code = r#"
const tool = ALL_TOOLS.find(({ name }) => name === "hidden_dynamic_tool");
const out = await tools.hidden_dynamic_tool({ city: "Paris" });
text(
  JSON.stringify({
    name: tool?.name ?? null,
    description: tool?.description ?? null,
    out,
  })
);
"#;

    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_custom_tool_call("call-1", "exec", code),
            ev_completed("resp-1"),
        ]),
    )
    .await;

    let second_mock = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    test.codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "use exec to inspect and call hidden tools".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: test.cwd.path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            approvals_reviewer: None,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: test.session_configured.model.clone(),
            effort: None,
            summary: None,
            service_tier: None,
            collaboration_mode: None,
            personality: None,
        })
        .await?;

    let turn_id = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::TurnStarted(event) => Some(event.turn_id.clone()),
        _ => None,
    })
    .await;
    let request = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::DynamicToolCallRequest(request) => Some(request.clone()),
        _ => None,
    })
    .await;
    assert_eq!(request.tool, "hidden_dynamic_tool");
    assert_eq!(request.arguments, serde_json::json!({ "city": "Paris" }));
    test.codex
        .submit(Op::DynamicToolResponse {
            id: request.call_id,
            response: DynamicToolResponse {
                content_items: vec![DynamicToolCallOutputContentItem::InputText {
                    text: "hidden-ok".to_string(),
                }],
                success: true,
            },
        })
        .await?;
    wait_for_event(&test.codex, |event| match event {
        EventMsg::TurnComplete(event) => event.turn_id == turn_id,
        _ => false,
    })
    .await;

    let req = second_mock.single_request();
    let (output, success) = custom_tool_output_body_and_success(&req, "call-1");
    assert_ne!(
        success,
        Some(false),
        "exec hidden dynamic tool call failed unexpectedly: {output}"
    );

    let parsed: Value = serde_json::from_str(
        &custom_tool_output_last_non_empty_text(&req, "call-1")
            .expect("exec hidden dynamic tool lookup should emit JSON"),
    )?;
    assert_eq!(
        parsed.get("name"),
        Some(&Value::String("hidden_dynamic_tool".to_string()))
    );
    assert_eq!(
        parsed.get("out"),
        Some(&Value::String("hidden-ok".to_string()))
    );
    assert!(
        parsed
            .get("description")
            .and_then(Value::as_str)
            .is_some_and(|description| {
                description.contains("A hidden dynamic tool.")
                    && description.contains("declare const tools:")
                    && description.contains("hidden_dynamic_tool(args:")
            })
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_can_print_content_only_mcp_tool_result_fields() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let code = r#"
const { content, structuredContent, isError } = await tools.mcp__rmcp__image_scenario({
  scenario: "text_only",
  caption: "caption from mcp",
});
text(
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
const { content, structuredContent, isError } = await tools.mcp__rmcp__echo({});
const firstText = content[0]?.text ?? "";
const mentionsMissingMessage =
  firstText.includes("missing field") && firstText.includes("message");
text(
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
store("nb", { title: "Notebook", items: [1, true, null] });
text("stored");
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
text(JSON.stringify(load("nb")));
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
    let loaded: Value = serde_json::from_str(
        &custom_tool_output_last_non_empty_text(&second_request, "call-2")
            .expect("exec load call should emit JSON"),
    )?;
    assert_eq!(
        loaded,
        serde_json::json!({ "title": "Notebook", "items": [1, true, null] })
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_can_compare_elapsed_time_around_set_timeout() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let (_test, second_mock) = run_code_mode_turn(
        &server,
        "measure elapsed time around setTimeout",
        r#"
const start_ms = Date.now();
await new Promise((resolve) => setTimeout(resolve, 100));
const end_ms = Date.now();
text(JSON.stringify({
  start_ms,
  end_ms,
  elapsed_ms: end_ms - start_ms,
  waited_long_enough: end_ms - start_ms >= 100,
}));
"#,
        /*include_apply_patch*/ false,
    )
    .await?;

    let second_request = second_mock.single_request();
    let (second_output, second_success) =
        custom_tool_output_body_and_success(&second_request, "call-1");
    assert_ne!(
        second_success,
        Some(false),
        "exec compare time call failed unexpectedly: {second_output}"
    );
    let compared: Value = serde_json::from_str(
        &custom_tool_output_last_non_empty_text(&second_request, "call-1")
            .expect("exec compare time call should emit JSON"),
    )?;
    let elapsed_ms = compared
        .get("elapsed_ms")
        .and_then(Value::as_i64)
        .expect("elapsed_ms should be an integer");
    assert!(
        elapsed_ms >= 100,
        "expected elapsed_ms >= 100, got {elapsed_ms}"
    );
    assert_eq!(compared.get("waited_long_enough"), Some(&Value::Bool(true)));

    Ok(())
}
