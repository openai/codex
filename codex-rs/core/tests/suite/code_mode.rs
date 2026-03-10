#![allow(clippy::expect_used, clippy::unwrap_used)]

use anyhow::Result;
use codex_core::config::types::McpServerConfig;
use codex_core::config::types::McpServerTransportConfig;
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
use core_test_support::stdio_server_bin;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::time::Duration;
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
    let (output, success) = custom_tool_output_text_and_success(&req, "call-1");
    assert_ne!(
        success,
        Some(false),
        "code_mode call failed unexpectedly: {output}"
    );
    let parsed: Value = serde_json::from_str(&output)?;
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
        run_code_mode_turn_with_rmcp(&server, "use code_mode to run the rmcp echo tool", code)
            .await?;

    let req = second_mock.single_request();
    let (output, success) = custom_tool_output_text_and_success(&req, "call-1");
    assert_ne!(
        success,
        Some(false),
        "code_mode rmcp echo call failed unexpectedly: {output}"
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
async fn code_mode_can_access_namespaced_mcp_tool_from_flat_tools_namespace() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let code = r#"
import { tools } from "tools.js";

const { structuredContent, isError } = await tools["mcp__rmcp__echo"]({
  message: "ping",
});
add_content(
  `echo=${structuredContent?.echo ?? "missing"}\n` +
    `env=${structuredContent?.env ?? "missing"}\n` +
    `isError=${String(isError)}`
);
"#;

    let (_test, second_mock) =
        run_code_mode_turn_with_rmcp(&server, "use code_mode to run the rmcp echo tool", code)
            .await?;

    let req = second_mock.single_request();
    let (output, success) = custom_tool_output_text_and_success(&req, "call-1");
    assert_ne!(
        success,
        Some(false),
        "code_mode rmcp echo call failed unexpectedly: {output}"
    );
    assert_eq!(
        output,
        "echo=ECHOING: ping
env=propagated-env
isError=false"
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
        "use code_mode to run the rmcp image scenario tool",
        code,
    )
    .await?;

    let req = second_mock.single_request();
    let (output, success) = custom_tool_output_text_and_success(&req, "call-1");
    assert_ne!(
        success,
        Some(false),
        "code_mode rmcp image scenario call failed unexpectedly: {output}"
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
        run_code_mode_turn_with_rmcp(&server, "use code_mode to call rmcp echo badly", code)
            .await?;

    let req = second_mock.single_request();
    let (output, success) = custom_tool_output_text_and_success(&req, "call-1");
    assert_ne!(
        success,
        Some(false),
        "code_mode rmcp error call failed unexpectedly: {output}"
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
