#![allow(clippy::expect_used, clippy::unwrap_used)]

use anyhow::Result;
use codex_core::features::Feature;
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
use regex_lite::Regex;
use std::fs;
use wiremock::MockServer;

fn custom_tool_output_text_and_success(
    req: &ResponsesRequest,
    call_id: &str,
) -> (String, Option<bool>) {
    let (output, success) = req
        .custom_tool_call_output_content_and_success(call_id)
        .expect("custom tool output should be present");
    (output.unwrap_or_default(), success)
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

add_content(await exec_command({ cmd: "printf code_mode_exec_marker" }));
"#,
        false,
    )
    .await?;

    let req = second_mock.single_request();
    let (output, success) = custom_tool_output_text_and_success(&req, "call-1");
    assert_ne!(
        success,
        Some(false),
        "code_mode call failed unexpectedly: {output}"
    );
    let regex = Regex::new(
        r#"(?ms)^Chunk ID: [[:xdigit:]]+
Wall time: [0-9]+(?:\.[0-9]+)? seconds
Process exited with code 0
Original token count: [0-9]+
Output:
code_mode_exec_marker
?$"#,
    )?;
    assert!(
        regex.is_match(&output),
        "expected exec_command output envelope to match regex, got: {output}"
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
    let (output, success) = custom_tool_output_text_and_success(&req, "call-1");
    assert_ne!(
        success,
        Some(false),
        "code_mode apply_patch call failed unexpectedly: {output}"
    );

    let file_path = test.cwd_path().join(file_name);
    assert_eq!(fs::read_to_string(&file_path)?, "hello from code_mode\n");

    Ok(())
}
