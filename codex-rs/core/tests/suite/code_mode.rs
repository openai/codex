#![allow(clippy::expect_used, clippy::unwrap_used)]

use anyhow::Result;
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
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use serde_json::Value;
use std::fs;
use wiremock::MockServer;

fn custom_tool_output_items(req: &ResponsesRequest, call_id: &str) -> Vec<Value> {
    req.custom_tool_call_output(call_id)
        .get("output")
        .and_then(Value::as_array)
        .expect("custom tool output should be serialized as content items")
        .clone()
}

fn text_item(items: &[Value], index: usize) -> &str {
    items[index]
        .get("text")
        .and_then(Value::as_str)
        .expect("content item should be input_text")
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
            ev_custom_tool_call("call-1", "code_mode", code),
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
        "use code_mode to run exec_command",
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
        r"(?s)\AScript completed\nWall time \d+\.\d seconds\nOutput:\n\z",
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
        "use code_mode to truncate the final result",
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
        r"(?s)\AScript completed\nWall time \d+\.\d seconds\nOutput:\n\z",
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
        r"(?s)\AScript failed\nWall time \d+\.\d seconds\nOutput:\n\z",
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
        run_code_mode_turn(&server, "use code_mode to run apply_patch", &code, true).await?;

    let req = second_mock.single_request();
    let items = custom_tool_output_items(&req, "call-1");
    assert_eq!(items.len(), 2);
    assert_regex_match(
        r"(?s)\AScript completed\nWall time \d+\.\d seconds\nOutput:\n\z",
        text_item(&items, 0),
    );

    let file_path = test.cwd_path().join(file_name);
    assert_eq!(fs::read_to_string(&file_path)?, "hello from code_mode\n");

    Ok(())
}
