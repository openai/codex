use std::collections::HashMap;
use std::ffi::OsStr;
use std::ffi::OsString;
use std::fs;
use std::net::TcpListener;
use std::path::Path;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use codex_core::config::types::McpServerConfig;
use codex_core::config::types::McpServerTransportConfig;

use codex_core::protocol::AskForApproval;
use codex_core::protocol::EventMsg;
use codex_core::protocol::McpInvocation;
use codex_core::protocol::McpToolCallBeginEvent;
use codex_core::protocol::Op;
use codex_core::protocol::SandboxPolicy;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::user_input::UserInput;
use codex_utils_cargo_bin::cargo_bin;
use core_test_support::responses;
use core_test_support::responses::mount_sse_once;
use core_test_support::skip_if_no_network;
use core_test_support::stdio_server_bin;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use serde_json::Value;
use serde_json::json;
use serial_test::serial;
use tempfile::tempdir;
use tokio::net::TcpStream;
use tokio::process::Child;
use tokio::process::Command;
use tokio::time::Instant;
use tokio::time::sleep;

static OPENAI_PNG: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAD0AAAA9CAYAAAAeYmHpAAAE6klEQVR4Aeyau44UVxCGx1fZsmRLlm3Zoe0XcGQ5cUiCCIgJeS9CHgAhMkISQnIuGQgJEkBcxLW+nqnZ6uqqc+nuWRC7q/P3qetf9e+MtOwyX25O4Nep6JPyop++0qev9HrfgZ+F6r2DuB/vHOrt/UIkqdDHYvujOW6fO7h/CNEI+a5jc+pBR8uy0jVFsziYu5HtfSUk+Io34q921hLNctFSX0gwww+S8wce8K1LfCU+cYW4888aov8NxqvQILUPPReLOrm6zyLxa4i+6VZuFbJo8d1MOHZm+7VUtB/aIvhPWc/3SWg49JcwFLlHxuXKjtyloo+YNhuW3VS+WPBuUEMvCFKjEDVgFBQHXrnazpqiSxNZCkQ1kYiozsbm9Oz7l4i2Il7vGccGNWAc3XosDrZe/9P3ZnMmzHNEQw4smf8RQ87XEAMsC7Az0Au+dgXerfH4+sHvEc0SYGic8WBBUGqFH2gN7yDrazy7m2pbRTeRmU3+MjZmr1h6LJgPbGy23SI6GlYT0brQ71IY8Us4PNQCm+zepSbaD2BY9xCaAsD9IIj/IzFmKMSdHHonwdZATbTnYREf6/VZGER98N9yCWIvXQwXDoDdhZJoT8jwLnJXDB9w4Sb3e6nK5ndzlkTLnP3JBu4LKkbrYrU69gCVceV0JvpyuW1xlsUVngzhwMetn/XamtTORF9IO5YnWNiyeF9zCAfqR3fUW+vZZKLtgP+ts8BmQRBREAdRDhH3o8QuRh/YucNFz2BEjxbRN6LGzphfKmvP6v6QhqIQyZ8XNJ0W0X83MR1PEcJBNO2KC2Z1TW/v244scp9FwRViZxIOBF0Lctk7ZVSavdLvRlV1hz/ysUi9sr8CIcB3nvWBwA93ykTz18eAYxQ6N/K2DkPA1lv3iXCwmDUT7YkjIby9siXueIJj9H+pzSqJ9oIuJWTUgSSt4WO7o/9GGg0viR4VinNRUDoIj34xoCd6pxD3aK3zfdbnx5v1J3ZNNEJsE0sBG7N27ReDrJc4sFxz7dI/ZAbOmmiKvHBitQXpAdR6+F7v+/ol/tOouUV01EeMZQF2BoQDn6dP4XNr+j9GZEtEK1/L8pFw7bd3a53tsTa7WD+054jOFmPg1XBKPQgnqFfmFcy32ZRvjmiIIQTYFvyDxQ8nH8WIwwGwlyDjDznnilYyFr6njrlZwsKkBpO59A7OwgdzPEWRm+G+oeb7IfyNuzjEEVLrOVxJsxvxwF8kmCM6I2QYmJunz4u4TrADpfl7mlbRTWQ7VmrBzh3+C9f6Grc3YoGN9dg/SXFthpRsT6vobfXRs2VBlgBHXVMLHjDNbIZv1sZ9+X3hB09cXdH1JKViyG0+W9bWZDa/r2f9zAFR71sTzGpMSWz2iI4YssWjWo3REy1MDGjdwe5e0dFSiAC1JakBvu4/CUS8Eh6dqHdU0Or0ioY3W5ClSqDXAy7/6SRfgw8vt4I+tbvvNtFT2kVDhY5+IGb1rCqYaXNF08vSALsXCPmt0kQNqJT1p5eI1mkIV/BxCY1z85lOzeFbPBQHURkkPTlwTYK9gTVE25l84IbFFN+YJDHjdpn0gq6mrHht0dkcjbM4UL9283O5p77GN+SPW/QwVB4IUYg7Or+Kp7naR6qktP98LNF2UxWo9yObPIT9KYg+hK4i56no4rfnM0qeyFf6AwAAAP//trwR3wAAAAZJREFUAwBZ0sR75itw5gAAAABJRU5ErkJggg==";

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[serial(mcp_test_value)]
async fn stdio_server_round_trip() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;

    let call_id = "call-123";
    let server_name = "rmcp";
    let tool_name = format!("mcp__{server_name}__echo");

    mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_response_created("resp-1"),
            responses::ev_function_call(call_id, &tool_name, "{\"message\":\"ping\"}"),
            responses::ev_completed("resp-1"),
        ]),
    )
    .await;
    mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_assistant_message("msg-1", "rmcp echo tool completed successfully."),
            responses::ev_completed("resp-2"),
        ]),
    )
    .await;

    let expected_env_value = "propagated-env";
    let rmcp_test_server_bin = stdio_server_bin()?;

    let fixture = test_codex()
        .with_config(move |config| {
            let mut servers = config.mcp_servers.get().clone();
            servers.insert(
                server_name.to_string(),
                McpServerConfig {
                    transport: McpServerTransportConfig::Stdio {
                        command: rmcp_test_server_bin,
                        args: Vec::new(),
                        env: Some(HashMap::from([(
                            "MCP_TEST_VALUE".to_string(),
                            expected_env_value.to_string(),
                        )])),
                        env_vars: Vec::new(),
                        cwd: None,
                    },
                    enabled: true,
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
        })
        .build(&server)
        .await?;
    let session_model = fixture.session_configured.model.clone();

    fixture
        .codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "call the rmcp echo tool".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: fixture.cwd.path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::ReadOnly,
            model: session_model,
            effort: None,
            summary: ReasoningSummary::Auto,
            collaboration_mode: None,
            personality: None,
        })
        .await?;

    let begin_event = wait_for_event(&fixture.codex, |ev| {
        matches!(ev, EventMsg::McpToolCallBegin(_))
    })
    .await;

    let EventMsg::McpToolCallBegin(begin) = begin_event else {
        unreachable!("event guard guarantees McpToolCallBegin");
    };
    assert_eq!(begin.invocation.server, server_name);
    assert_eq!(begin.invocation.tool, "echo");

    let end_event = wait_for_event(&fixture.codex, |ev| {
        matches!(ev, EventMsg::McpToolCallEnd(_))
    })
    .await;
    let EventMsg::McpToolCallEnd(end) = end_event else {
        unreachable!("event guard guarantees McpToolCallEnd");
    };

    let result = end
        .result
        .as_ref()
        .expect("rmcp echo tool should return success");
    assert_eq!(result.is_error, Some(false));
    assert!(
        result.content.is_empty(),
        "content should default to an empty array"
    );

    let structured = result
        .structured_content
        .as_ref()
        .expect("structured content");
    let Value::Object(map) = structured else {
        panic!("structured content should be an object: {structured:?}");
    };
    let echo_value = map
        .get("echo")
        .and_then(Value::as_str)
        .expect("echo payload present");
    assert_eq!(echo_value, "ECHOING: ping");
    let env_value = map
        .get("env")
        .and_then(Value::as_str)
        .expect("env snapshot inserted");
    assert_eq!(env_value, expected_env_value);

    wait_for_event(&fixture.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    server.verify().await;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[serial(mcp_test_value)]
async fn stdio_image_responses_round_trip() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;

    let call_id = "img-1";
    let server_name = "rmcp";
    let tool_name = format!("mcp__{server_name}__image");

    // First stream: model decides to call the image tool.
    mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_response_created("resp-1"),
            responses::ev_function_call(call_id, &tool_name, "{}"),
            responses::ev_completed("resp-1"),
        ]),
    )
    .await;
    // Second stream: after tool execution, assistant emits a message and completes.
    let final_mock = mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_assistant_message("msg-1", "rmcp image tool completed successfully."),
            responses::ev_completed("resp-2"),
        ]),
    )
    .await;

    // Build the stdio rmcp server and pass the image as data URL so it can construct ImageContent.
    let rmcp_test_server_bin = stdio_server_bin()?;

    let fixture = test_codex()
        .with_config(move |config| {
            let mut servers = config.mcp_servers.get().clone();
            servers.insert(
                server_name.to_string(),
                McpServerConfig {
                    transport: McpServerTransportConfig::Stdio {
                        command: rmcp_test_server_bin,
                        args: Vec::new(),
                        env: Some(HashMap::from([(
                            "MCP_TEST_IMAGE_DATA_URL".to_string(),
                            OPENAI_PNG.to_string(),
                        )])),
                        env_vars: Vec::new(),
                        cwd: None,
                    },
                    enabled: true,
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
        })
        .build(&server)
        .await?;
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
            sandbox_policy: SandboxPolicy::ReadOnly,
            model: session_model,
            effort: None,
            summary: ReasoningSummary::Auto,
            collaboration_mode: None,
            personality: None,
        })
        .await?;

    // Wait for tool begin/end and final completion.
    let begin_event = wait_for_event(&fixture.codex, |ev| {
        matches!(ev, EventMsg::McpToolCallBegin(_))
    })
    .await;
    let EventMsg::McpToolCallBegin(begin) = begin_event else {
        unreachable!("begin");
    };
    assert_eq!(
        begin,
        McpToolCallBeginEvent {
            call_id: call_id.to_string(),
            invocation: McpInvocation {
                server: server_name.to_string(),
                tool: "image".to_string(),
                arguments: Some(json!({})),
            },
        },
    );

    let end_event = wait_for_event(&fixture.codex, |ev| {
        matches!(ev, EventMsg::McpToolCallEnd(_))
    })
    .await;
    let EventMsg::McpToolCallEnd(end) = end_event else {
        unreachable!("end");
    };
    assert_eq!(end.call_id, call_id);
    assert_eq!(
        end.invocation,
        McpInvocation {
            server: server_name.to_string(),
            tool: "image".to_string(),
            arguments: Some(json!({})),
        }
    );
    let result = end.result.expect("rmcp image tool should return success");
    assert_eq!(result.is_error, Some(false));
    assert_eq!(result.content.len(), 1);
    let base64_only = OPENAI_PNG
        .strip_prefix("data:image/png;base64,")
        .expect("data url prefix");
    let entry = result.content[0].as_object().expect("content object");
    assert_eq!(entry.get("type"), Some(&json!("image")));
    assert_eq!(entry.get("mimeType"), Some(&json!("image/png")));
    assert_eq!(entry.get("data"), Some(&json!(base64_only)));

    wait_for_event(&fixture.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let output_item = final_mock.single_request().function_call_output(call_id);
    assert_eq!(
        output_item,
        json!({
            "type": "function_call_output",
            "call_id": call_id,
            "output": [{
                "type": "input_image",
                "image_url": OPENAI_PNG
            }]
        })
    );
    server.verify().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[serial(mcp_test_value)]
async fn stdio_server_propagates_whitelisted_env_vars() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;

    let call_id = "call-1234";
    let server_name = "rmcp_whitelist";
    let tool_name = format!("mcp__{server_name}__echo");

    mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_response_created("resp-1"),
            responses::ev_function_call(call_id, &tool_name, "{\"message\":\"ping\"}"),
            responses::ev_completed("resp-1"),
        ]),
    )
    .await;
    mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_assistant_message("msg-1", "rmcp echo tool completed successfully."),
            responses::ev_completed("resp-2"),
        ]),
    )
    .await;

    let expected_env_value = "propagated-env-from-whitelist";
    let _guard = EnvVarGuard::set("MCP_TEST_VALUE", OsStr::new(expected_env_value));
    let rmcp_test_server_bin = stdio_server_bin()?;

    let fixture = test_codex()
        .with_config(move |config| {
            let mut servers = config.mcp_servers.get().clone();
            servers.insert(
                server_name.to_string(),
                McpServerConfig {
                    transport: McpServerTransportConfig::Stdio {
                        command: rmcp_test_server_bin,
                        args: Vec::new(),
                        env: None,
                        env_vars: vec!["MCP_TEST_VALUE".to_string()],
                        cwd: None,
                    },
                    enabled: true,
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
        })
        .build(&server)
        .await?;
    let session_model = fixture.session_configured.model.clone();

    fixture
        .codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "call the rmcp echo tool".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: fixture.cwd.path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::ReadOnly,
            model: session_model,
            effort: None,
            summary: ReasoningSummary::Auto,
            collaboration_mode: None,
            personality: None,
        })
        .await?;

    let begin_event = wait_for_event(&fixture.codex, |ev| {
        matches!(ev, EventMsg::McpToolCallBegin(_))
    })
    .await;

    let EventMsg::McpToolCallBegin(begin) = begin_event else {
        unreachable!("event guard guarantees McpToolCallBegin");
    };
    assert_eq!(begin.invocation.server, server_name);
    assert_eq!(begin.invocation.tool, "echo");

    let end_event = wait_for_event(&fixture.codex, |ev| {
        matches!(ev, EventMsg::McpToolCallEnd(_))
    })
    .await;
    let EventMsg::McpToolCallEnd(end) = end_event else {
        unreachable!("event guard guarantees McpToolCallEnd");
    };

    let result = end
        .result
        .as_ref()
        .expect("rmcp echo tool should return success");
    assert_eq!(result.is_error, Some(false));
    assert!(
        result.content.is_empty(),
        "content should default to an empty array"
    );

    let structured = result
        .structured_content
        .as_ref()
        .expect("structured content");
    let Value::Object(map) = structured else {
        panic!("structured content should be an object: {structured:?}");
    };
    let echo_value = map
        .get("echo")
        .and_then(Value::as_str)
        .expect("echo payload present");
    assert_eq!(echo_value, "ECHOING: ping");
    let env_value = map
        .get("env")
        .and_then(Value::as_str)
        .expect("env snapshot inserted");
    assert_eq!(env_value, expected_env_value);

    wait_for_event(&fixture.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    server.verify().await;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn streamable_http_tool_call_round_trip() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;

    let call_id = "call-456";
    let server_name = "rmcp_http";
    let tool_name = format!("mcp__{server_name}__echo");

    mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_response_created("resp-1"),
            responses::ev_function_call(call_id, &tool_name, "{\"message\":\"ping\"}"),
            responses::ev_completed("resp-1"),
        ]),
    )
    .await;
    mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_assistant_message(
                "msg-1",
                "rmcp streamable http echo tool completed successfully.",
            ),
            responses::ev_completed("resp-2"),
        ]),
    )
    .await;

    let expected_env_value = "propagated-env-http";
    let rmcp_http_server_bin = match cargo_bin("test_streamable_http_server") {
        Ok(path) => path,
        Err(err) => {
            eprintln!("test_streamable_http_server binary not available, skipping test: {err}");
            return Ok(());
        }
    };

    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    drop(listener);
    let bind_addr = format!("127.0.0.1:{port}");
    let server_url = format!("http://{bind_addr}/mcp");

    let mut http_server_child = Command::new(&rmcp_http_server_bin)
        .kill_on_drop(true)
        .env("MCP_STREAMABLE_HTTP_BIND_ADDR", &bind_addr)
        .env("MCP_TEST_VALUE", expected_env_value)
        .spawn()?;

    wait_for_streamable_http_server(&mut http_server_child, &bind_addr, Duration::from_secs(5))
        .await?;

    let fixture = test_codex()
        .with_config(move |config| {
            let mut servers = config.mcp_servers.get().clone();
            servers.insert(
                server_name.to_string(),
                McpServerConfig {
                    transport: McpServerTransportConfig::StreamableHttp {
                        url: server_url,
                        bearer_token_env_var: None,
                        http_headers: None,
                        env_http_headers: None,
                    },
                    enabled: true,
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
        })
        .build(&server)
        .await?;
    let session_model = fixture.session_configured.model.clone();

    fixture
        .codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "call the rmcp streamable http echo tool".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: fixture.cwd.path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::ReadOnly,
            model: session_model,
            effort: None,
            summary: ReasoningSummary::Auto,
            collaboration_mode: None,
            personality: None,
        })
        .await?;

    let begin_event = wait_for_event(&fixture.codex, |ev| {
        matches!(ev, EventMsg::McpToolCallBegin(_))
    })
    .await;

    let EventMsg::McpToolCallBegin(begin) = begin_event else {
        unreachable!("event guard guarantees McpToolCallBegin");
    };
    assert_eq!(begin.invocation.server, server_name);
    assert_eq!(begin.invocation.tool, "echo");

    let end_event = wait_for_event(&fixture.codex, |ev| {
        matches!(ev, EventMsg::McpToolCallEnd(_))
    })
    .await;
    let EventMsg::McpToolCallEnd(end) = end_event else {
        unreachable!("event guard guarantees McpToolCallEnd");
    };

    let result = end
        .result
        .as_ref()
        .expect("rmcp echo tool should return success");
    assert_eq!(result.is_error, Some(false));
    assert!(
        result.content.is_empty(),
        "content should default to an empty array"
    );

    let structured = result
        .structured_content
        .as_ref()
        .expect("structured content");
    let Value::Object(map) = structured else {
        panic!("structured content should be an object: {structured:?}");
    };
    let echo_value = map
        .get("echo")
        .and_then(Value::as_str)
        .expect("echo payload present");
    assert_eq!(echo_value, "ECHOING: ping");
    let env_value = map
        .get("env")
        .and_then(Value::as_str)
        .expect("env snapshot inserted");
    assert_eq!(env_value, expected_env_value);

    wait_for_event(&fixture.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    server.verify().await;

    match http_server_child.try_wait() {
        Ok(Some(_)) => {}
        Ok(None) => {
            let _ = http_server_child.kill().await;
        }
        Err(error) => {
            eprintln!("failed to check streamable http server status: {error}");
            let _ = http_server_child.kill().await;
        }
    }
    if let Err(error) = http_server_child.wait().await {
        eprintln!("failed to await streamable http server shutdown: {error}");
    }

    Ok(())
}

/// This test writes to a fallback credentials file in CODEX_HOME.
/// Ideally, we wouldn't need to serialize the test but it's much more cumbersome to wire CODEX_HOME through the code.
#[serial(codex_home)]
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn streamable_http_with_oauth_round_trip() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;

    let call_id = "call-789";
    let server_name = "rmcp_http_oauth";
    let tool_name = format!("mcp__{server_name}__echo");

    mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_response_created("resp-1"),
            responses::ev_function_call(call_id, &tool_name, "{\"message\":\"ping\"}"),
            responses::ev_completed("resp-1"),
        ]),
    )
    .await;
    mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_assistant_message(
                "msg-1",
                "rmcp streamable http oauth echo tool completed successfully.",
            ),
            responses::ev_completed("resp-2"),
        ]),
    )
    .await;

    let expected_env_value = "propagated-env-http-oauth";
    let expected_token = "initial-access-token";
    let client_id = "test-client-id";
    let refresh_token = "initial-refresh-token";
    let rmcp_http_server_bin = match cargo_bin("test_streamable_http_server") {
        Ok(path) => path,
        Err(err) => {
            eprintln!("test_streamable_http_server binary not available, skipping test: {err}");
            return Ok(());
        }
    };

    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    drop(listener);
    let bind_addr = format!("127.0.0.1:{port}");
    let server_url = format!("http://{bind_addr}/mcp");

    let mut http_server_child = Command::new(&rmcp_http_server_bin)
        .kill_on_drop(true)
        .env("MCP_STREAMABLE_HTTP_BIND_ADDR", &bind_addr)
        .env("MCP_EXPECT_BEARER", expected_token)
        .env("MCP_TEST_VALUE", expected_env_value)
        .spawn()?;

    wait_for_streamable_http_server(&mut http_server_child, &bind_addr, Duration::from_secs(5))
        .await?;

    let temp_home = tempdir()?;
    let _guard = EnvVarGuard::set("CODEX_HOME", temp_home.path().as_os_str());
    write_fallback_oauth_tokens(
        temp_home.path(),
        server_name,
        &server_url,
        client_id,
        expected_token,
        refresh_token,
    )?;

    let fixture = test_codex()
        .with_config(move |config| {
            let mut servers = config.mcp_servers.get().clone();
            servers.insert(
                server_name.to_string(),
                McpServerConfig {
                    transport: McpServerTransportConfig::StreamableHttp {
                        url: server_url,
                        bearer_token_env_var: None,
                        http_headers: None,
                        env_http_headers: None,
                    },
                    enabled: true,
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
        })
        .build(&server)
        .await?;
    let session_model = fixture.session_configured.model.clone();

    fixture
        .codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "call the rmcp streamable http oauth echo tool".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: fixture.cwd.path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::ReadOnly,
            model: session_model,
            effort: None,
            summary: ReasoningSummary::Auto,
            collaboration_mode: None,
            personality: None,
        })
        .await?;

    let begin_event = wait_for_event(&fixture.codex, |ev| {
        matches!(ev, EventMsg::McpToolCallBegin(_))
    })
    .await;

    let EventMsg::McpToolCallBegin(begin) = begin_event else {
        unreachable!("event guard guarantees McpToolCallBegin");
    };
    assert_eq!(begin.invocation.server, server_name);
    assert_eq!(begin.invocation.tool, "echo");

    let end_event = wait_for_event(&fixture.codex, |ev| {
        matches!(ev, EventMsg::McpToolCallEnd(_))
    })
    .await;
    let EventMsg::McpToolCallEnd(end) = end_event else {
        unreachable!("event guard guarantees McpToolCallEnd");
    };

    let result = end
        .result
        .as_ref()
        .expect("rmcp echo tool should return success");
    assert_eq!(result.is_error, Some(false));
    assert!(
        result.content.is_empty(),
        "content should default to an empty array"
    );

    let structured = result
        .structured_content
        .as_ref()
        .expect("structured content");
    let Value::Object(map) = structured else {
        panic!("structured content should be an object: {structured:?}");
    };
    let echo_value = map
        .get("echo")
        .and_then(Value::as_str)
        .expect("echo payload present");
    assert_eq!(echo_value, "ECHOING: ping");
    let env_value = map
        .get("env")
        .and_then(Value::as_str)
        .expect("env snapshot inserted");
    assert_eq!(env_value, expected_env_value);

    wait_for_event(&fixture.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    server.verify().await;

    match http_server_child.try_wait() {
        Ok(Some(_)) => {}
        Ok(None) => {
            let _ = http_server_child.kill().await;
        }
        Err(error) => {
            eprintln!("failed to check streamable http oauth server status: {error}");
            let _ = http_server_child.kill().await;
        }
    }
    if let Err(error) = http_server_child.wait().await {
        eprintln!("failed to await streamable http oauth server shutdown: {error}");
    }

    Ok(())
}

async fn wait_for_streamable_http_server(
    server_child: &mut Child,
    address: &str,
    timeout: Duration,
) -> anyhow::Result<()> {
    let deadline = Instant::now() + timeout;

    loop {
        if let Some(status) = server_child.try_wait()? {
            return Err(anyhow::anyhow!(
                "streamable HTTP server exited early with status {status}"
            ));
        }

        let remaining = deadline.saturating_duration_since(Instant::now());

        if remaining.is_zero() {
            return Err(anyhow::anyhow!(
                "timed out waiting for streamable HTTP server at {address}: deadline reached"
            ));
        }

        match tokio::time::timeout(remaining, TcpStream::connect(address)).await {
            Ok(Ok(_)) => return Ok(()),
            Ok(Err(error)) => {
                if Instant::now() >= deadline {
                    return Err(anyhow::anyhow!(
                        "timed out waiting for streamable HTTP server at {address}: {error}"
                    ));
                }
            }
            Err(_) => {
                return Err(anyhow::anyhow!(
                    "timed out waiting for streamable HTTP server at {address}: connect call timed out"
                ));
            }
        }

        sleep(Duration::from_millis(50)).await;
    }
}

fn write_fallback_oauth_tokens(
    home: &Path,
    server_name: &str,
    server_url: &str,
    client_id: &str,
    access_token: &str,
    refresh_token: &str,
) -> anyhow::Result<()> {
    let expires_at = SystemTime::now()
        .checked_add(Duration::from_secs(3600))
        .ok_or_else(|| anyhow::anyhow!("failed to compute expiry time"))?
        .duration_since(UNIX_EPOCH)?
        .as_millis() as u64;

    let store = serde_json::json!({
        "stub": {
            "server_name": server_name,
            "server_url": server_url,
            "client_id": client_id,
            "access_token": access_token,
            "expires_at": expires_at,
            "refresh_token": refresh_token,
            "scopes": ["profile"],
        }
    });

    let file_path = home.join(".credentials.json");
    fs::write(&file_path, serde_json::to_vec(&store)?)?;
    Ok(())
}

struct EnvVarGuard {
    key: &'static str,
    original: Option<OsString>,
}

impl EnvVarGuard {
    fn set(key: &'static str, value: &std::ffi::OsStr) -> Self {
        let original = std::env::var_os(key);
        unsafe {
            std::env::set_var(key, value);
        }
        Self { key, original }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        unsafe {
            match &self.original {
                Some(value) => std::env::set_var(self.key, value),
                None => std::env::remove_var(self.key),
            }
        }
    }
}
