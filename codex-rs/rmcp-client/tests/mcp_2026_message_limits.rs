use std::collections::HashMap;
use std::ffi::OsString;
use std::sync::Arc;
use std::time::Duration;

use codex_config::types::AuthKeyringBackendKind;
use codex_config::types::OAuthCredentialsStoreMode;
use codex_exec_server::Environment;
use codex_rmcp_client::ElicitationAction;
use codex_rmcp_client::ElicitationResponse;
use codex_rmcp_client::ExecutorStdioServerLauncher;
use codex_rmcp_client::LocalStdioServerLauncher;
use codex_rmcp_client::McpProtocolMode;
use codex_rmcp_client::RmcpClient;
use codex_rmcp_client::StdioServerLauncher;
use futures::FutureExt;
use rmcp::model::ClientCapabilities;
use rmcp::model::Implementation;
use rmcp::model::InitializeRequestParams;
use rmcp::model::ProtocolVersion;
use serde_json::Value;
use serde_json::json;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::Request;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

const MAX_MCP_MESSAGE_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OversizedHttpResponse {
    DiscoveryJson,
    ToolJson,
    ToolError,
    ToolSseEvent,
}

fn initialize_params() -> InitializeRequestParams {
    InitializeRequestParams::new(
        ClientCapabilities::default(),
        Implementation::new("codex-message-limit-test", "0.0.0-test"),
    )
    .with_protocol_version(ProtocolVersion::V_2025_06_18)
}

async fn initialize(client: &RmcpClient) -> anyhow::Result<()> {
    client
        .initialize(
            initialize_params(),
            Some(Duration::from_secs(15)),
            Box::new(|_, _| {
                async {
                    Ok(ElicitationResponse {
                        action: ElicitationAction::Accept,
                        content: Some(json!({})),
                        meta: None,
                    })
                }
                .boxed()
            }),
        )
        .await?;
    Ok(())
}

fn discovery_response(request: &Value, padding: Option<&str>) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": request["id"],
        "result": {
            "resultType": "complete",
            "supportedVersions": ["2026-07-28"],
            "capabilities": {"tools": {}},
            "instructions": padding.unwrap_or_default(),
            "_meta": {
                "io.modelcontextprotocol/serverInfo": {
                    "name": "message-limit-test",
                    "version": "1.0.0",
                },
            },
            "ttlMs": 0,
            "cacheScope": "private",
        },
    })
}

fn tools_response(request: &Value, padding: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": request["id"],
        "result": {
            "resultType": "complete",
            "tools": [{
                "name": "oversized_tool",
                "description": padding,
                "inputSchema": {"type": "object"},
            }],
        },
    })
}

async fn http_client(server: &MockServer, mode: McpProtocolMode) -> anyhow::Result<RmcpClient> {
    RmcpClient::new_streamable_http_client_with_protocol_mode(
        "message-limits",
        &format!("{}/mcp", server.uri()),
        /*bearer_token*/ None,
        /*http_headers*/ None,
        /*env_http_headers*/ None,
        OAuthCredentialsStoreMode::File,
        AuthKeyringBackendKind::default(),
        Environment::default_for_tests().get_http_client(),
        /*auth_provider*/ None,
        mode,
    )
    .await
}

#[tokio::test]
async fn modern_http_rejects_oversized_json_error_and_sse_bodies() -> anyhow::Result<()> {
    for oversized in [
        OversizedHttpResponse::DiscoveryJson,
        OversizedHttpResponse::ToolJson,
        OversizedHttpResponse::ToolError,
        OversizedHttpResponse::ToolSseEvent,
    ] {
        let server = MockServer::start().await;
        let padding = Arc::new("x".repeat(MAX_MCP_MESSAGE_BYTES + 1));
        Mock::given(method("POST"))
            .and(path("/mcp"))
            .respond_with(move |request: &Request| {
                let body: Value = request.body_json().expect("JSON-RPC request");
                match body["method"].as_str() {
                    Some("server/discover") => {
                        ResponseTemplate::new(200).set_body_json(discovery_response(
                            &body,
                            (oversized == OversizedHttpResponse::DiscoveryJson)
                                .then_some(padding.as_str()),
                        ))
                    }
                    Some("tools/list") if oversized == OversizedHttpResponse::ToolError => {
                        ResponseTemplate::new(500).set_body_json(json!({
                            "jsonrpc": "2.0",
                            "id": body["id"],
                            "error": {"code": -32603, "message": padding.as_str()},
                        }))
                    }
                    Some("tools/list") if oversized == OversizedHttpResponse::ToolSseEvent => {
                        ResponseTemplate::new(200).set_body_raw(
                            format!(
                                "event: message\ndata: {}\n\n",
                                tools_response(&body, &padding),
                            ),
                            "text/event-stream; charset=utf-8",
                        )
                    }
                    Some("tools/list") => {
                        ResponseTemplate::new(200).set_body_json(tools_response(&body, &padding))
                    }
                    other => panic!("unexpected MCP method {other:?}"),
                }
            })
            .mount(&server)
            .await;

        let client = http_client(&server, McpProtocolMode::V20260728).await?;
        if oversized == OversizedHttpResponse::DiscoveryJson {
            let error = initialize(&client)
                .await
                .expect_err("oversized discovery must be rejected");
            assert!(format!("{error:#}").contains("8388608 bytes"));
        } else {
            initialize(&client).await?;
            let error = client
                .list_tools(/*params*/ None, Some(Duration::from_secs(5)))
                .await
                .expect_err("oversized tools response must be rejected");
            let error = format!("{error:#}");
            assert!(
                error.contains("8388608 bytes")
                    || (oversized == OversizedHttpResponse::ToolSseEvent
                        && (error.contains("timed out") || error.contains("Transport closed"))),
                "unexpected {oversized:?} error: {error}"
            );
        }
        client.shutdown().await;
    }
    Ok(())
}

#[tokio::test]
async fn legacy_http_keeps_existing_large_json_response_behavior() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    let padding = Arc::new("x".repeat(MAX_MCP_MESSAGE_BYTES + 1));
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .respond_with(move |request: &Request| {
            let body: Value = request.body_json().expect("JSON-RPC request");
            match body["method"].as_str() {
                Some("initialize") => ResponseTemplate::new(200).set_body_json(json!({
                    "jsonrpc": "2.0",
                    "id": body["id"],
                    "result": {
                        "protocolVersion": "2025-06-18",
                        "capabilities": {"tools": {}},
                        "serverInfo": {"name": "legacy-large-response", "version": "1.0.0"},
                    },
                })),
                Some("notifications/initialized") => ResponseTemplate::new(202),
                Some("tools/list") => {
                    ResponseTemplate::new(200).set_body_json(tools_response(&body, &padding))
                }
                other => panic!("unexpected legacy MCP method {other:?}"),
            }
        })
        .mount(&server)
        .await;

    let client = http_client(&server, McpProtocolMode::Legacy).await?;
    initialize(&client).await?;
    let tools = client
        .list_tools(/*params*/ None, Some(Duration::from_secs(15)))
        .await?;
    assert_eq!(
        tools.tools[0].description.as_deref().map(str::len),
        Some(MAX_MCP_MESSAGE_BYTES + 1)
    );
    client.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn modern_local_and_executor_stdio_reject_oversized_lines() -> anyhow::Result<()> {
    let server = codex_utils_cargo_bin::cargo_bin("test_mcp_2026_stdio_server")?;

    for (modern, executor) in [(false, false), (false, true), (true, false), (true, true)] {
        let mode = if modern {
            McpProtocolMode::V20260728
        } else {
            McpProtocolMode::Legacy
        };
        let env = modern.then(|| {
            HashMap::from([(
                OsString::from("CODEX_MCP_PROTOCOL_VERSION"),
                OsString::from("2026-07-28"),
            )])
        });
        let launcher: Arc<dyn StdioServerLauncher> = if executor {
            Arc::new(ExecutorStdioServerLauncher::new(
                Environment::default_for_tests().get_exec_backend(),
            ))
        } else {
            Arc::new(LocalStdioServerLauncher::new(std::env::current_dir()?))
        };
        let fixture_mode = if modern {
            "oversized-stdout"
        } else {
            "oversized-stdout-legacy"
        };

        let client = RmcpClient::new_stdio_client_with_protocol_mode(
            server.clone().into(),
            vec![OsString::from(fixture_mode)],
            env,
            &[],
            Some(std::env::current_dir()?.to_string_lossy().into_owned()),
            launcher,
            mode,
        )
        .await?;
        initialize(&client).await?;
        let result = client
            .list_tools(/*params*/ None, Some(Duration::from_secs(10)))
            .await;
        if !modern && !executor {
            let tools = result?;
            assert_eq!(
                tools.tools[0].description.as_deref().map(str::len),
                Some(MAX_MCP_MESSAGE_BYTES + 1),
                "legacy local stdio must preserve the existing unbounded native codec"
            );
        } else {
            assert!(
                result.is_err(),
                "oversized stdio line must be rejected (modern={modern}, executor={executor})"
            );
        }
        client.shutdown().await;
    }
    Ok(())
}
