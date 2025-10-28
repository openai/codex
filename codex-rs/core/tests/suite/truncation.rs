#![cfg(not(target_os = "windows"))]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use anyhow::Context;
use anyhow::Result;
use codex_core::features::Feature;
use codex_core::model_family::find_family_for_model;
use codex_core::protocol::SandboxPolicy;
use core_test_support::assert_regex_match;
use core_test_support::responses;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once_match;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use escargot::CargoBuild;
use regex_lite::Regex;
use serde_json::Value;
use serde_json::json;
use wiremock::matchers::any;

// Verifies byte-truncation formatting for function error output (RespondToModel errors)
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn truncate_function_error_trims_respond_to_model() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        // Use the test model that wires function tools like grep_files
        config.model = "test-gpt-5-codex".to_string();
        config.model_family =
            find_family_for_model("test-gpt-5-codex").expect("model family for test model");
    });
    let test = builder.build(&server).await?;

    // Construct a very long, non-existent path to force a RespondToModel error with a large message
    let long_path = "a".repeat(20_000);
    let call_id = "grep-huge-error";
    let args = json!({
        "pattern": "alpha",
        "path": long_path,
        "limit": 10
    });
    let responses = vec![
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call(call_id, "grep_files", &serde_json::to_string(&args)?),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    ];
    let mock = mount_sse_sequence(&server, responses).await;

    test.submit_turn_with_policy(
        "trigger grep_files with long path to test truncation",
        SandboxPolicy::DangerFullAccess,
    )
    .await?;

    let output = mock
        .function_call_output_text(call_id)
        .context("function error output present")?;

    tracing::debug!(output = %output, "truncated function error output");

    // Expect plaintext with byte-truncation marker and no omitted-lines marker
    assert!(
        serde_json::from_str::<serde_json::Value>(&output).is_err(),
        "expected error output to be plain text",
    );
    let truncated_pattern = r#"(?s)^Total output lines: 1\s+.*\[\.\.\. output truncated to fit 10240 bytes \.\.\.\]\s*$"#;
    assert_regex_match(truncated_pattern, &output);
    assert!(
        !output.contains("omitted"),
        "line omission marker should not appear when no lines were dropped: {output}"
    );

    Ok(())
}

// Verifies that a standard tool call (shell) exceeding the model formatting
// limits is truncated before being sent back to the model.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tool_call_output_exceeds_limit_truncated_for_model() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    // Use a model that exposes the generic shell tool.
    let mut builder = test_codex().with_config(|config| {
        config.model = "gpt-5-codex".to_string();
        config.model_family =
            find_family_for_model("gpt-5-codex").expect("gpt-5-codex is a model family");
    });
    let fixture = builder.build(&server).await?;

    let call_id = "shell-too-large";
    let args = serde_json::json!({
        "command": ["/bin/sh", "-c", "seq 1 400"],
        "timeout_ms": 5_000,
    });

    // First response: model tells us to run the tool; second: complete the turn.
    mount_sse_once_match(
        &server,
        any(),
        sse(vec![
            responses::ev_response_created("resp-1"),
            responses::ev_function_call(call_id, "shell", &serde_json::to_string(&args)?),
            responses::ev_completed("resp-1"),
        ]),
    )
    .await;
    let mock2 = mount_sse_once_match(
        &server,
        any(),
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

    // Expect plain text (not JSON) with truncation markers and line elision.
    assert!(
        serde_json::from_str::<Value>(&output).is_err(),
        "expected truncated shell output to be plain text"
    );
    let truncated_pattern = r#"(?s)^Exit code: 0
Wall time: .* seconds
Total output lines: 400
Output:
1
2
3
4
5
6
.*
\[\.{3} omitted 144 of 400 lines \.{3}\]

.*
396
397
398
399
400
$"#;
    assert_regex_match(truncated_pattern, &output);

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
    let large_msg = "long-message-with-newlines-".repeat(600);
    let args_json = serde_json::json!({ "message": large_msg });

    mount_sse_once_match(
        &server,
        any(),
        sse(vec![
            responses::ev_response_created("resp-1"),
            responses::ev_function_call(call_id, &tool_name, &args_json.to_string()),
            responses::ev_completed("resp-1"),
        ]),
    )
    .await;
    let mock2 = mount_sse_once_match(
        &server,
        any(),
        sse(vec![
            responses::ev_assistant_message("msg-1", "rmcp echo tool completed."),
            responses::ev_completed("resp-2"),
        ]),
    )
    .await;

    // Compile the rmcp stdio test server and configure it.
    let rmcp_test_server_bin = CargoBuild::new()
        .package("codex-rmcp-client")
        .bin("test_stdio_server")
        .run()?
        .path()
        .to_string_lossy()
        .into_owned();

    let mut builder = test_codex().with_config(move |config| {
        config.features.enable(Feature::RmcpClient);
        config.mcp_servers.insert(
            server_name.to_string(),
            codex_core::config_types::McpServerConfig {
                transport: codex_core::config_types::McpServerTransportConfig::Stdio {
                    command: rmcp_test_server_bin,
                    args: Vec::new(),
                    env: None,
                    env_vars: Vec::new(),
                    cwd: None,
                },
                enabled: true,
                startup_timeout_sec: Some(std::time::Duration::from_secs(10)),
                tool_timeout_sec: None,
                enabled_tools: None,
                disabled_tools: None,
            },
        );
    });
    let fixture = builder.build(&server).await?;

    fixture
        .submit_turn_with_policy(
            "call the rmcp echo tool with a very large message",
            SandboxPolicy::ReadOnly,
        )
        .await?;

    // The MCP tool call output is converted to a function_call_output for the model.
    let output = mock2
        .single_request()
        .function_call_output_text(call_id)
        .context("function_call_output present for rmcp call")?;

    // Expect plain text with byte-based truncation marker.
    assert!(
        serde_json::from_str::<Value>(&output).is_err(),
        "expected truncated MCP output to be plain text"
    );
    assert!(
        output.starts_with("Total output lines: 1\n\n{"),
        "expected total line header and JSON head, got: {output}"
    );
    let byte_marker = Regex::new(r"\[\.\.\. output truncated to fit 10240 bytes \.\.\.\]")
        .expect("compile regex");
    assert!(
        byte_marker.is_match(&output),
        "expected byte truncation marker, got: {output}"
    );

    Ok(())
}
