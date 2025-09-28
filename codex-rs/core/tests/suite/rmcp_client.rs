use std::collections::HashMap;
use std::net::TcpListener;
use std::time::Duration;

use codex_core::config_types::McpServerConfig;
use codex_core::config_types::McpServerTransportConfig;

use codex_core::protocol::AskForApproval;
use codex_core::protocol::EventMsg;
use codex_core::protocol::InputItem;
use codex_core::protocol::Op;
use codex_core::protocol::SandboxPolicy;
use codex_protocol::config_types::ReasoningSummary;
use core_test_support::responses;
use core_test_support::responses::mount_sse_once;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_with_timeout;
use escargot::CargoBuild;
use serde_json::Value;
use tokio::net::TcpStream;
use tokio::process::Child;
use tokio::process::Command;
use tokio::time::Instant;
use tokio::time::sleep;
use wiremock::matchers::any;

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn stdio_server_round_trip() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;

    let call_id = "call-123";
    let server_name = "rmcp";
    let tool_name = format!("{server_name}__echo");

    mount_sse_once(
        &server,
        any(),
        responses::sse(vec![
            serde_json::json!({
                "type": "response.created",
                "response": {"id": "resp-1"}
            }),
            responses::ev_function_call(call_id, &tool_name, "{\"message\":\"ping\"}"),
            responses::ev_completed("resp-1"),
        ]),
    )
    .await;
    mount_sse_once(
        &server,
        any(),
        responses::sse(vec![
            responses::ev_assistant_message("msg-1", "rmcp echo tool completed successfully."),
            responses::ev_completed("resp-2"),
        ]),
    )
    .await;

    let expected_env_value = "propagated-env";
    let rmcp_test_server_bin = CargoBuild::new()
        .package("codex-rmcp-client")
        .bin("test_stdio_server")
        .run()?
        .path()
        .to_string_lossy()
        .into_owned();

    let fixture = test_codex()
        .with_config(move |config| {
            config.use_experimental_use_rmcp_client = true;
            config.mcp_servers.insert(
                server_name.to_string(),
                McpServerConfig {
                    transport: McpServerTransportConfig::Stdio {
                        command: rmcp_test_server_bin.clone(),
                        args: Vec::new(),
                        env: Some(HashMap::from([(
                            "MCP_TEST_VALUE".to_string(),
                            expected_env_value.to_string(),
                        )])),
                    },
                    startup_timeout_sec: Some(Duration::from_secs(10)),
                    tool_timeout_sec: None,
                },
            );
        })
        .build(&server)
        .await?;
    let session_model = fixture.session_configured.model.clone();

    fixture
        .codex
        .submit(Op::UserTurn {
            items: vec![InputItem::Text {
                text: "call the rmcp echo tool".into(),
            }],
            final_output_json_schema: None,
            cwd: fixture.cwd.path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: session_model,
            effort: None,
            summary: ReasoningSummary::Auto,
        })
        .await?;

    let begin_event = wait_for_event_with_timeout(
        &fixture.codex,
        |ev| matches!(ev, EventMsg::McpToolCallBegin(_)),
        Duration::from_secs(10),
    )
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

    wait_for_event(&fixture.codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    server.verify().await;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn streamable_http_tool_call_round_trip() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;

    let call_id = "call-456";
    let server_name = "rmcp_http";
    let tool_name = format!("{server_name}__echo");

    mount_sse_once(
        &server,
        any(),
        responses::sse(vec![
            serde_json::json!({
                "type": "response.created",
                "response": {"id": "resp-1"}
            }),
            responses::ev_function_call(call_id, &tool_name, "{\"message\":\"ping\"}"),
            responses::ev_completed("resp-1"),
        ]),
    )
    .await;
    mount_sse_once(
        &server,
        any(),
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
    let rmcp_http_server_bin = CargoBuild::new()
        .package("codex-rmcp-client")
        .bin("test_streamable_http_server")
        .run()?
        .path()
        .to_string_lossy()
        .into_owned();

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
            config.use_experimental_use_rmcp_client = true;
            config.mcp_servers.insert(
                server_name.to_string(),
                McpServerConfig {
                    transport: McpServerTransportConfig::StreamableHttp {
                        url: server_url,
                        bearer_token: None,
                    },
                    startup_timeout_sec: Some(Duration::from_secs(10)),
                    tool_timeout_sec: None,
                },
            );
        })
        .build(&server)
        .await?;
    let session_model = fixture.session_configured.model.clone();

    fixture
        .codex
        .submit(Op::UserTurn {
            items: vec![InputItem::Text {
                text: "call the rmcp streamable http echo tool".into(),
            }],
            final_output_json_schema: None,
            cwd: fixture.cwd.path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: session_model,
            effort: None,
            summary: ReasoningSummary::Auto,
        })
        .await?;

    let begin_event = wait_for_event_with_timeout(
        &fixture.codex,
        |ev| matches!(ev, EventMsg::McpToolCallBegin(_)),
        Duration::from_secs(10),
    )
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

    wait_for_event(&fixture.codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

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
