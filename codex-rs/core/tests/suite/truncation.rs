#![cfg(not(target_os = "windows"))]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use anyhow::Context;
use anyhow::Result;
use codex_core::config::types::McpServerConfig;
use codex_core::config::types::McpServerTransportConfig;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::user_input::UserInput;
use core_test_support::assert_regex_match;
use core_test_support::responses;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::stdio_server_bin;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use serde_json::Value;
use serde_json::json;
use std::collections::HashMap;
use std::time::Duration;

// Verifies that a standard tool call (shell_command) exceeding the model formatting
// limits is truncated before being sent back to the model.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tool_call_output_configured_limit_chars_type() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    // Use a model that exposes the shell_command tool.
    let mut builder = test_codex().with_model("gpt-5.1").with_config(|config| {
        config.tool_output_token_limit = Some(100_000);
    });

    let fixture = builder.build(&server).await?;

    let call_id = "shell-too-large";
    let command = if cfg!(windows) {
        "for ($i=1; $i -le 100000; $i++) { Write-Output $i }"
    } else {
        "seq 1 100000"
    };
    let args = serde_json::json!({
        "command": command,
        "timeout_ms": 5_000,
    });

    // First response: model tells us to run the tool; second: complete the turn.
    mount_sse_once(
        &server,
        sse(vec![
            responses::ev_response_created("resp-1"),
            responses::ev_function_call(call_id, "shell_command", &serde_json::to_string(&args)?),
            responses::ev_completed("resp-1"),
        ]),
    )
    .await;
    let mock2 = mount_sse_once(
        &server,
        sse(vec![
            responses::ev_assistant_message("msg-1", "done"),
            responses::ev_completed("resp-2"),
        ]),
    )
    .await;

    fixture
        .submit_turn_with_policy("trigger big shell output", SandboxPolicy::DangerFullAccess)
        .await?;

    // Inspect what we sent back to the model; it should contain a truncated
    // function_call_output for the shell call.
    let output = mock2
        .single_request()
        .function_call_output_text(call_id)
        .context("function_call_output present for shell call")?;
    let output = output.replace("\r\n", "\n");

    // Expect plain text (not JSON) containing the entire shell output.
    assert!(
        serde_json::from_str::<Value>(&output).is_err(),
        "expected truncated shell output to be plain text"
    );

    assert!(
        (400000..=401000).contains(&output.len()),
        "we should be almost 100k tokens"
    );

    assert!(
        !output.contains("tokens truncated"),
        "shell output should not contain tokens truncated marker: {output}"
    );

    Ok(())
}

// Verifies that a standard tool call (shell_command) exceeding the model formatting
// limits is truncated before being sent back to the model.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tool_call_output_exceeds_limit_truncated_chars_limit() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    // Use a model that exposes the shell_command tool.
    let mut builder = test_codex().with_model("gpt-5.1");

    let fixture = builder.build(&server).await?;

    let call_id = "shell-too-large";
    let command = if cfg!(windows) {
        "for ($i=1; $i -le 100000; $i++) { Write-Output $i }"
    } else {
        "seq 1 100000"
    };
    let args = serde_json::json!({
        "command": command,
        "timeout_ms": 5_000,
    });

    // First response: model tells us to run the tool; second: complete the turn.
    mount_sse_once(
        &server,
        sse(vec![
            responses::ev_response_created("resp-1"),
            responses::ev_function_call(call_id, "shell_command", &serde_json::to_string(&args)?),
            responses::ev_completed("resp-1"),
        ]),
    )
    .await;
    let mock2 = mount_sse_once(
        &server,
        sse(vec![
            responses::ev_assistant_message("msg-1", "done"),
            responses::ev_completed("resp-2"),
        ]),
    )
    .await;

    fixture
        .submit_turn_with_policy("trigger big shell output", SandboxPolicy::DangerFullAccess)
        .await?;

    // Inspect what we sent back to the model; it should contain a truncated
    // function_call_output for the shell call.
    let output = mock2
        .single_request()
        .function_call_output_text(call_id)
        .context("function_call_output present for shell call")?;
    let output = output.replace("\r\n", "\n");

    // Expect plain text (not JSON) containing the entire shell output.
    assert!(
        serde_json::from_str::<Value>(&output).is_err(),
        "expected truncated shell output to be plain text"
    );

    let truncated_pattern = r#"(?s)^Exit code: 0\nWall time: [0-9]+(?:\.[0-9]+)? seconds\nTotal output lines: 100000\nOutput:\n.*?…\d+ chars truncated….*$"#;

    assert_regex_match(truncated_pattern, &output);

    let len = output.len();
    assert!(
        (9_900..=10_100).contains(&len),
        "expected ~10k chars after truncation, got {len}"
    );

    Ok(())
}

// Verifies that a standard tool call (shell_command) exceeding the model formatting
// limits is truncated before being sent back to the model.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tool_call_output_exceeds_limit_truncated_for_model() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    // Use a model that exposes the shell_command tool.
    let mut builder = test_codex().with_model("gpt-5.1-codex");
    let fixture = builder.build(&server).await?;

    let call_id = "shell-too-large";
    let command = if cfg!(windows) {
        "for ($i=1; $i -le 100000; $i++) { Write-Output $i }"
    } else {
        "seq 1 100000"
    };
    let args = serde_json::json!({
        "command": command,
        "timeout_ms": 5_000,
    });

    // First response: model tells us to run the tool; second: complete the turn.
    mount_sse_once(
        &server,
        sse(vec![
            responses::ev_response_created("resp-1"),
            responses::ev_function_call(call_id, "shell_command", &serde_json::to_string(&args)?),
            responses::ev_completed("resp-1"),
        ]),
    )
    .await;
    let mock2 = mount_sse_once(
        &server,
        sse(vec![
            responses::ev_assistant_message("msg-1", "done"),
            responses::ev_completed("resp-2"),
        ]),
    )
    .await;

    fixture
        .submit_turn_with_policy("trigger big shell output", SandboxPolicy::DangerFullAccess)
        .await?;

    // Inspect what we sent back to the model; it should contain a truncated
    // function_call_output for the shell call.
    let output = mock2
        .single_request()
        .function_call_output_text(call_id)
        .context("function_call_output present for shell call")?;
    let output = output.replace("\r\n", "\n");

    // Expect plain text (not JSON) containing the entire shell output.
    assert!(
        serde_json::from_str::<Value>(&output).is_err(),
        "expected truncated shell output to be plain text"
    );
    let truncated_pattern = r#"(?s)^Exit code: 0
Wall time: [0-9]+(?:\.[0-9]+)? seconds
Total output lines: 100000
Output:
1
2
3
4
5
6
.*…137224 tokens truncated.*
99999
100000
$"#;
    assert_regex_match(truncated_pattern, &output);

    Ok(())
}

// Ensures shell_command outputs that exceed the line limit are truncated only once.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tool_call_output_truncated_only_once() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let mut builder = test_codex().with_model("gpt-5.1-codex");
    let fixture = builder.build(&server).await?;
    let call_id = "shell-single-truncation";
    let command = if cfg!(windows) {
        "for ($i=1; $i -le 10000; $i++) { Write-Output $i }"
    } else {
        "seq 1 10000"
    };
    let args = serde_json::json!({
        "command": command,
        "timeout_ms": 5_000,
    });

    mount_sse_once(
        &server,
        sse(vec![
            responses::ev_response_created("resp-1"),
            responses::ev_function_call(call_id, "shell_command", &serde_json::to_string(&args)?),
            responses::ev_completed("resp-1"),
        ]),
    )
    .await;
    let mock2 = mount_sse_once(
        &server,
        sse(vec![
            responses::ev_assistant_message("msg-1", "done"),
            responses::ev_completed("resp-2"),
        ]),
    )
    .await;

    fixture
        .submit_turn_with_policy("trigger big shell output", SandboxPolicy::DangerFullAccess)
        .await?;

    let output = mock2
        .single_request()
        .function_call_output_text(call_id)
        .context("function_call_output present for shell call")?;

    let truncation_markers = output.matches("tokens truncated").count();

    assert_eq!(
        truncation_markers, 1,
        "shell output should carry only one truncation marker: {output}"
    );

    Ok(())
}

// Verifies that an MCP tool call result exceeding the model formatting limits
// is truncated before being sent back to the model.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn mcp_tool_call_output_exceeds_limit_truncated_for_model() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let call_id = "rmcp-truncated";
    let server_name = "rmcp";
    let tool_name = format!("mcp__{server_name}__echo");

    // Build a very large message to exceed 10KiB once serialized.
    let large_msg = "long-message-with-newlines-".repeat(6000);
    let args_json = serde_json::json!({ "message": large_msg });

    mount_sse_once(
        &server,
        sse(vec![
            responses::ev_response_created("resp-1"),
            responses::ev_function_call(call_id, &tool_name, &args_json.to_string()),
            responses::ev_completed("resp-1"),
        ]),
    )
    .await;
    let mock2 = mount_sse_once(
        &server,
        sse(vec![
            responses::ev_assistant_message("msg-1", "rmcp echo tool completed."),
            responses::ev_completed("resp-2"),
        ]),
    )
    .await;

    // Compile the rmcp stdio test server and configure it.
    let rmcp_test_server_bin = stdio_server_bin()?;

    let mut builder = test_codex().with_config(move |config| {
        let mut servers = config.mcp_servers.get().clone();
        servers.insert(
            server_name.to_string(),
            codex_core::config::types::McpServerConfig {
                transport: codex_core::config::types::McpServerTransportConfig::Stdio {
                    command: rmcp_test_server_bin,
                    args: Vec::new(),
                    env: None,
                    env_vars: Vec::new(),
                    cwd: None,
                },
                enabled: true,
                required: false,
                disabled_reason: None,
                startup_timeout_sec: Some(std::time::Duration::from_secs(10)),
                tool_timeout_sec: None,
                enabled_tools: None,
                disabled_tools: None,
                scopes: None,
            },
        );
        config
            .mcp_servers
            .set(servers)
            .expect("test mcp servers should accept any configuration");
        config.tool_output_token_limit = Some(500);
    });
    let fixture = builder.build(&server).await?;

    fixture
        .submit_turn_with_policy(
            "call the rmcp echo tool with a very large message",
            SandboxPolicy::new_read_only_policy(),
        )
        .await?;

    // The MCP tool call output is converted to a function_call_output for the model.
    let output = mock2
        .single_request()
        .function_call_output_text(call_id)
        .context("function_call_output present for rmcp call")?;

    assert!(
        !output.contains("Total output lines:"),
        "MCP output should not include line-based truncation header: {output}"
    );

    let truncated_pattern = r#"(?s)^\{"echo":\s*"ECHOING: long-message-with-newlines-.*tokens truncated.*long-message-with-newlines-.*$"#;
    assert_regex_match(truncated_pattern, &output);
    assert!(output.len() < 2500, "{}", output.len());

    Ok(())
}

// Verifies that an MCP image tool output is serialized as content_items array with
// the image preserved and no truncation summary appended (since there are no text items).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn mcp_image_output_preserves_image_and_no_text_summary() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let call_id = "rmcp-image-no-trunc";
    let server_name = "rmcp";
    let tool_name = format!("mcp__{server_name}__image");

    mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call(call_id, &tool_name, "{}"),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let final_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    // Build the stdio rmcp server and pass a tiny PNG via data URL so it can construct ImageContent.
    let rmcp_test_server_bin = stdio_server_bin()?;

    // 1x1 PNG data URL
    let openai_png = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMB/ee9bQAAAABJRU5ErkJggg==";

    let mut builder = test_codex().with_config(move |config| {
        let mut servers = config.mcp_servers.get().clone();
        servers.insert(
            server_name.to_string(),
            McpServerConfig {
                transport: McpServerTransportConfig::Stdio {
                    command: rmcp_test_server_bin,
                    args: Vec::new(),
                    env: Some(HashMap::from([(
                        "MCP_TEST_IMAGE_DATA_URL".to_string(),
                        openai_png.to_string(),
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
            },
        );
        config
            .mcp_servers
            .set(servers)
            .expect("test mcp servers should accept any configuration");
    });
    let fixture = builder.build(&server).await?;
    let session_model = fixture.session_configured.model.clone();

    fixture
        .codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "call the rmcp image tool".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: fixture.cwd.path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::new_read_only_policy(),
            model: session_model,
            effort: None,
            summary: ReasoningSummary::Auto,
            collaboration_mode: None,
            personality: None,
        })
        .await?;

    // Wait for completion to ensure the outbound request is captured.
    wait_for_event(&fixture.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;
    let output_item = final_mock.single_request().function_call_output(call_id);
    // Expect exactly one array element: the image item; and no trailing summary text.
    let output = output_item.get("output").expect("output");
    assert!(output.is_array(), "expected array output");
    let arr = output.as_array().unwrap();
    assert_eq!(arr.len(), 1, "no truncation summary should be appended");
    assert_eq!(
        arr[0],
        json!({"type": "input_image", "image_url": openai_png})
    );

    Ok(())
}

// Token-based policy should report token counts even when truncation is byte-estimated.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn token_policy_marker_reports_tokens() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex()
        .with_model("gpt-5.1-codex")
        .with_config(|config| {
            config.tool_output_token_limit = Some(50); // small budget to force truncation
        });
    let fixture = builder.build(&server).await?;

    let call_id = "shell-token-marker";
    let args = json!({
        "command": "seq 1 150",
        "timeout_ms": 5_000,
    });

    mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call(call_id, "shell_command", &serde_json::to_string(&args)?),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let done_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    fixture
        .submit_turn_with_policy("run the shell tool", SandboxPolicy::DangerFullAccess)
        .await?;

    let output = done_mock
        .single_request()
        .function_call_output_text(call_id)
        .context("shell output present")?;

    let pattern = r"(?s)^Exit code: 0\nWall time: [0-9]+(?:\.[0-9]+)? seconds\nTotal output lines: 150\nOutput:\n1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n14\n15\n16\n17\n18\n19.*tokens truncated.*129\n130\n131\n132\n133\n134\n135\n136\n137\n138\n139\n140\n141\n142\n143\n144\n145\n146\n147\n148\n149\n150\n$";

    assert_regex_match(pattern, &output);

    Ok(())
}

// Byte-based policy should report bytes removed.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn byte_policy_marker_reports_bytes() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_model("gpt-5.1").with_config(|config| {
        config.tool_output_token_limit = Some(50); // ~200 byte cap
    });
    let fixture = builder.build(&server).await?;

    let call_id = "shell-byte-marker";
    let args = json!({
        "command": "seq 1 150",
        "timeout_ms": 5_000,
    });

    mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call(call_id, "shell_command", &serde_json::to_string(&args)?),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let done_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    fixture
        .submit_turn_with_policy("run the shell tool", SandboxPolicy::DangerFullAccess)
        .await?;

    let output = done_mock
        .single_request()
        .function_call_output_text(call_id)
        .context("shell output present")?;

    let pattern = r"(?s)^Exit code: 0\nWall time: [0-9]+(?:\.[0-9]+)? seconds\nTotal output lines: 150\nOutput:\n1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n14\n15\n16\n17\n18\n19.*chars truncated.*129\n130\n131\n132\n133\n134\n135\n136\n137\n138\n139\n140\n141\n142\n143\n144\n145\n146\n147\n148\n149\n150\n$";

    assert_regex_match(pattern, &output);

    Ok(())
}

// shell_command output should remain intact when the config opts into a large token budget.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shell_command_output_not_truncated_with_custom_limit() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex()
        .with_model("gpt-5.1-codex")
        .with_config(|config| {
            config.tool_output_token_limit = Some(50_000); // ample budget
        });
    let fixture = builder.build(&server).await?;

    let call_id = "shell-no-trunc";
    let args = json!({
        "command": "seq 1 1000",
        "timeout_ms": 5_000,
    });
    let expected_body: String = (1..=1000).map(|i| format!("{i}\n")).collect();

    mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call(call_id, "shell_command", &serde_json::to_string(&args)?),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let done_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    fixture
        .submit_turn_with_policy(
            "run big output without truncation",
            SandboxPolicy::DangerFullAccess,
        )
        .await?;

    let output = done_mock
        .single_request()
        .function_call_output_text(call_id)
        .context("shell output present")?;

    assert!(
        output.ends_with(&expected_body),
        "expected entire shell output when budget increased: {output}"
    );
    assert!(
        !output.contains("truncated"),
        "output should remain untruncated with ample budget"
    );

    Ok(())
}

// MCP server output should also remain intact when the config increases the token limit.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn mcp_tool_call_output_not_truncated_with_custom_limit() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let call_id = "rmcp-untruncated";
    let server_name = "rmcp";
    let tool_name = format!("mcp__{server_name}__echo");
    let large_msg = "a".repeat(80_000);
    let args_json = serde_json::json!({ "message": large_msg });

    mount_sse_once(
        &server,
        sse(vec![
            responses::ev_response_created("resp-1"),
            responses::ev_function_call(call_id, &tool_name, &args_json.to_string()),
            responses::ev_completed("resp-1"),
        ]),
    )
    .await;
    let mock2 = mount_sse_once(
        &server,
        sse(vec![
            responses::ev_assistant_message("msg-1", "rmcp echo tool completed."),
            responses::ev_completed("resp-2"),
        ]),
    )
    .await;

    let rmcp_test_server_bin = stdio_server_bin()?;

    let mut builder = test_codex().with_config(move |config| {
        config.tool_output_token_limit = Some(50_000);
        let mut servers = config.mcp_servers.get().clone();
        servers.insert(
            server_name.to_string(),
            codex_core::config::types::McpServerConfig {
                transport: codex_core::config::types::McpServerTransportConfig::Stdio {
                    command: rmcp_test_server_bin,
                    args: Vec::new(),
                    env: None,
                    env_vars: Vec::new(),
                    cwd: None,
                },
                enabled: true,
                required: false,
                disabled_reason: None,
                startup_timeout_sec: Some(std::time::Duration::from_secs(10)),
                tool_timeout_sec: None,
                enabled_tools: None,
                disabled_tools: None,
                scopes: None,
            },
        );
        config
            .mcp_servers
            .set(servers)
            .expect("test mcp servers should accept any configuration");
    });
    let fixture = builder.build(&server).await?;

    fixture
        .submit_turn_with_policy(
            "call the rmcp echo tool with a very large message",
            SandboxPolicy::new_read_only_policy(),
        )
        .await?;

    let output = mock2
        .single_request()
        .function_call_output_text(call_id)
        .context("function_call_output present for rmcp call")?;

    let parsed: Value = serde_json::from_str(&output)?;
    assert_eq!(
        output.len(),
        80031,
        "parsed MCP output should retain its serialized length"
    );
    let expected_echo = format!("ECHOING: {large_msg}");
    let echo_str = parsed["echo"]
        .as_str()
        .context("echo field should be a string in rmcp echo output")?;
    assert_eq!(
        echo_str.len(),
        expected_echo.len(),
        "echo length should match"
    );
    assert_eq!(echo_str, expected_echo);
    assert!(
        !output.contains("truncated"),
        "output should not include truncation markers when limit is raised: {output}"
    );

    Ok(())
}
