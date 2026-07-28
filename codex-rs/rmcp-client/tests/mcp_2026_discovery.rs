use std::collections::HashMap;
use std::ffi::OsString;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use codex_config::types::AuthKeyringBackendKind;
use codex_config::types::OAuthCredentialsStoreMode;
use codex_exec_server::Environment;
use codex_rmcp_client::ElicitationAction;
use codex_rmcp_client::ElicitationResponse;
use codex_rmcp_client::LocalStdioServerLauncher;
use codex_rmcp_client::McpProtocolMode;
use codex_rmcp_client::RmcpClient;
use futures::FutureExt;
use pretty_assertions::assert_eq;
use rmcp::model::ClientCapabilities;
use rmcp::model::Implementation;
use rmcp::model::InitializeRequestParams;
use rmcp::model::InitializeResult;
use rmcp::model::ProtocolVersion;
use serde_json::Value;
use serde_json::json;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::Request;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

const MODERN_VERSION: &str = "2026-07-28";
const LEGACY_VERSION: &str = "2025-06-18";
const MAX_MCP_MESSAGE_BYTES: usize = 8 * 1024 * 1024;

fn initialize_params() -> InitializeRequestParams {
    InitializeRequestParams::new(
        ClientCapabilities::default(),
        Implementation::new("codex-discovery-test", "0.0.0"),
    )
    .with_protocol_version(ProtocolVersion::V_2025_06_18)
}

async fn create_client(server: &MockServer, mode: McpProtocolMode) -> anyhow::Result<RmcpClient> {
    RmcpClient::new_streamable_http_client_with_protocol_mode(
        "discovery-test",
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

async fn initialize_client(client: &RmcpClient) -> anyhow::Result<InitializeResult> {
    client
        .initialize(
            initialize_params(),
            Some(Duration::from_secs(5)),
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
        .await
}

fn modern_discover_response(request: &Value) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(modern_discover_result(request))
}

fn modern_discover_result(request: &Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": request["id"],
        "result": {
            "resultType": "complete",
            "supportedVersions": [MODERN_VERSION],
            "capabilities": {"tools": {}},
            "serverInfo": {"name": "modern-test", "version": "1.0.0"},
            "ttlMs": 0,
            "cacheScope": "private",
        },
    })
}

fn legacy_initialize_response(request: &Value) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(json!({
        "jsonrpc": "2.0",
        "id": request["id"],
        "result": {
            "protocolVersion": LEGACY_VERSION,
            "capabilities": {"tools": {}},
            "serverInfo": {"name": "legacy-test", "version": "1.0.0"},
        },
    }))
}

#[tokio::test]
async fn modern_mode_uses_sdk_discovery_and_self_contained_request_metadata() -> anyhow::Result<()>
{
    let server = MockServer::start().await;
    let observed = Arc::new(Mutex::new(Vec::<Value>::new()));
    let recorded = Arc::clone(&observed);

    Mock::given(method("POST"))
        .and(path("/mcp"))
        .respond_with(move |request: &Request| {
            let body: Value = request.body_json().expect("valid JSON-RPC request");
            recorded.lock().expect("requests lock").push(body.clone());
            match body["method"].as_str() {
                Some("server/discover") => {
                    assert_eq!(
                        body.pointer("/params/_meta/io.modelcontextprotocol~1protocolVersion"),
                        Some(&json!(MODERN_VERSION))
                    );
                    assert_eq!(
                        body.pointer("/params/_meta/io.modelcontextprotocol~1clientInfo/name"),
                        Some(&json!("codex-discovery-test"))
                    );
                    modern_discover_response(&body)
                }
                Some("tools/list") => {
                    assert_eq!(
                        body.pointer("/params/_meta/io.modelcontextprotocol~1protocolVersion"),
                        Some(&json!(MODERN_VERSION))
                    );
                    assert_eq!(
                        body.pointer("/params/_meta/io.modelcontextprotocol~1clientInfo/name"),
                        Some(&json!("codex-discovery-test"))
                    );
                    ResponseTemplate::new(200).set_body_json(json!({
                        "jsonrpc": "2.0",
                        "id": body["id"],
                        "result": {"resultType": "complete", "tools": []},
                    }))
                }
                other => panic!("unexpected modern lifecycle request: {other:?}"),
            }
        })
        .expect(2)
        .mount(&server)
        .await;

    let client = create_client(&server, McpProtocolMode::V20260728).await?;
    initialize_client(&client).await?;
    let tools = client
        .list_tools(/*params*/ None, Some(Duration::from_secs(5)))
        .await?;
    assert!(tools.tools.is_empty());

    let methods = observed
        .lock()
        .expect("requests lock")
        .iter()
        .map(|request| request["method"].as_str().unwrap_or_default().to_owned())
        .collect::<Vec<_>>();
    assert_eq!(methods, vec!["server/discover", "tools/list"]);
    client.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn modern_discovery_does_not_follow_redirects_with_sensitive_headers() -> anyhow::Result<()> {
    let redirect_target = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/forwarded"))
        .and(header("x-api-key", "sensitive-key"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&redirect_target)
        .await;

    let resource_server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(header("x-api-key", "sensitive-key"))
        .respond_with(
            ResponseTemplate::new(307)
                .insert_header("location", format!("{}/forwarded", redirect_target.uri())),
        )
        .expect(1)
        .mount(&resource_server)
        .await;

    let client = RmcpClient::new_streamable_http_client_with_protocol_mode(
        "discovery-redirect-test",
        &format!("{}/mcp", resource_server.uri()),
        /*bearer_token*/ None,
        Some(HashMap::from([(
            "x-api-key".to_string(),
            "sensitive-key".to_string(),
        )])),
        /*env_http_headers*/ None,
        OAuthCredentialsStoreMode::File,
        AuthKeyringBackendKind::default(),
        Environment::default_for_tests().get_http_client(),
        /*auth_provider*/ None,
        McpProtocolMode::V20260728,
    )
    .await?;
    let error = initialize_client(&client)
        .await
        .expect_err("modern MCP discovery must not follow redirects");
    assert!(
        error.to_string().contains("307"),
        "redirect rejection should report its HTTP status: {error:#}"
    );
    redirect_target.verify().await;
    resource_server.verify().await;
    client.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn legacy_mcp_requests_preserve_existing_redirect_behavior() -> anyhow::Result<()> {
    let redirect_target = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/forwarded"))
        .and(header("x-api-key", "sensitive-key"))
        .respond_with(|request: &Request| {
            let body: Value = request.body_json().expect("valid JSON-RPC request");
            match body["method"].as_str() {
                Some("initialize") => legacy_initialize_response(&body),
                Some("notifications/initialized") => ResponseTemplate::new(202),
                other => panic!("unexpected redirected legacy method: {other:?}"),
            }
        })
        .expect(2)
        .mount(&redirect_target)
        .await;

    let resource_server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .respond_with(
            ResponseTemplate::new(307)
                .insert_header("location", format!("{}/forwarded", redirect_target.uri())),
        )
        .expect(2)
        .mount(&resource_server)
        .await;

    let client = RmcpClient::new_streamable_http_client_with_protocol_mode(
        "legacy-redirect-test",
        &format!("{}/mcp", resource_server.uri()),
        /*bearer_token*/ None,
        Some(HashMap::from([(
            "x-api-key".to_string(),
            "sensitive-key".to_string(),
        )])),
        /*env_http_headers*/ None,
        OAuthCredentialsStoreMode::File,
        AuthKeyringBackendKind::default(),
        Environment::default_for_tests().get_http_client(),
        /*auth_provider*/ None,
        McpProtocolMode::Legacy,
    )
    .await?;
    initialize_client(&client).await?;
    redirect_target.verify().await;
    resource_server.verify().await;
    client.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn modern_discovery_accepts_metadata_namespaced_server_identity() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .respond_with(|request: &Request| {
            let body: Value = request.body_json().expect("valid JSON-RPC request");
            assert_eq!(body["method"], "server/discover");
            let mut response = modern_discover_result(&body);
            let server_info = response["result"]
                .as_object_mut()
                .expect("discovery object")
                .remove("serverInfo")
                .expect("top-level identity");
            response["result"]["_meta"] = json!({
                "io.modelcontextprotocol/serverInfo": server_info,
            });
            ResponseTemplate::new(200).set_body_json(response)
        })
        .expect(1)
        .mount(&server)
        .await;

    let client = create_client(&server, McpProtocolMode::V20260728).await?;
    initialize_client(&client).await?;
    client.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn modern_discovery_accepts_native_sse_responses() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .respond_with(|request: &Request| {
            let body: Value = request.body_json().expect("valid JSON-RPC request");
            assert_eq!(body["method"], "server/discover");
            let response = modern_discover_result(&body);
            ResponseTemplate::new(200).set_body_raw(
                format!(": keepalive\n\nevent: message\ndata: {response}\n\n"),
                "text/event-stream; charset=utf-8",
            )
        })
        .expect(1)
        .mount(&server)
        .await;

    let client = create_client(&server, McpProtocolMode::V20260728).await?;
    initialize_client(&client).await?;
    client.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn modern_discovery_accepts_metadata_namespaced_server_identity_over_sse()
-> anyhow::Result<()> {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .respond_with(|request: &Request| {
            let body: Value = request.body_json().expect("valid JSON-RPC request");
            assert_eq!(body["method"], "server/discover");
            let mut response = modern_discover_result(&body);
            let server_info = response["result"]
                .as_object_mut()
                .expect("discovery object")
                .remove("serverInfo")
                .expect("top-level identity");
            response["result"]["_meta"] = json!({
                "io.modelcontextprotocol/serverInfo": server_info,
            });
            ResponseTemplate::new(200).set_body_raw(
                format!("event: message\ndata: {response}\n\n"),
                "text/event-stream; charset=utf-8",
            )
        })
        .expect(1)
        .mount(&server)
        .await;

    let client = create_client(&server, McpProtocolMode::V20260728).await?;
    initialize_client(&client).await?;
    client.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn modern_http_rejects_oversized_discovery_and_tool_json_responses() -> anyhow::Result<()> {
    for oversized_method in ["server/discover", "tools/list", "tools/list:error"] {
        let server = MockServer::start().await;
        let padding = Arc::new("x".repeat(MAX_MCP_MESSAGE_BYTES + 1));
        Mock::given(method("POST"))
            .and(path("/mcp"))
            .respond_with(move |request: &Request| {
                let body: Value = request.body_json().expect("valid JSON-RPC request");
                match body["method"].as_str() {
                    Some("server/discover") => {
                        let mut response = modern_discover_result(&body);
                        if oversized_method == "server/discover" {
                            response["result"]["instructions"] = json!(padding.as_str());
                        }
                        ResponseTemplate::new(200).set_body_json(response)
                    }
                    Some("tools/list") if oversized_method == "tools/list:error" => {
                        ResponseTemplate::new(500).set_body_json(json!({
                            "jsonrpc": "2.0",
                            "id": body["id"],
                            "error": {"code": -32603, "message": padding.as_str()},
                        }))
                    }
                    Some("tools/list") => ResponseTemplate::new(200).set_body_json(json!({
                        "jsonrpc": "2.0",
                        "id": body["id"],
                        "result": {
                            "resultType": "complete",
                            "tools": [{
                                "name": "oversized_tool",
                                "description": padding.as_str(),
                                "inputSchema": {"type": "object"},
                            }],
                        },
                    })),
                    other => panic!("unexpected MCP method {other:?}"),
                }
            })
            .mount(&server)
            .await;

        let client = create_client(&server, McpProtocolMode::V20260728).await?;
        let error = if oversized_method == "server/discover" {
            initialize_client(&client)
                .await
                .expect_err("oversized discovery response must be rejected")
        } else {
            initialize_client(&client).await?;
            client
                .list_tools(/*params*/ None, Some(Duration::from_secs(5)))
                .await
                .expect_err("oversized tools response must be rejected")
        };
        assert!(
            format!("{error:#}").contains("8388608 bytes"),
            "expected bounded {oversized_method} response, got {error:#}"
        );
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
            let body: Value = request.body_json().expect("valid JSON-RPC request");
            match body["method"].as_str() {
                Some("initialize") => legacy_initialize_response(&body),
                Some("notifications/initialized") => ResponseTemplate::new(202),
                Some("tools/list") => ResponseTemplate::new(200).set_body_json(json!({
                    "jsonrpc": "2.0",
                    "id": body["id"],
                    "result": {
                        "tools": [{
                            "name": "legacy_large_tool",
                            "description": padding.as_str(),
                            "inputSchema": {"type": "object"},
                        }],
                    },
                })),
                other => panic!("unexpected legacy MCP method {other:?}"),
            }
        })
        .mount(&server)
        .await;

    let client = create_client(&server, McpProtocolMode::Legacy).await?;
    initialize_client(&client).await?;
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

#[tokio::test]
async fn modern_discovery_retries_transient_service_unavailable() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    let observed = Arc::new(Mutex::new(Vec::<Value>::new()));
    let recorded = Arc::clone(&observed);

    Mock::given(method("POST"))
        .and(path("/mcp"))
        .respond_with(move |request: &Request| {
            let body: Value = request.body_json().expect("valid JSON-RPC request");
            assert_eq!(body["method"], "server/discover");
            let attempt = {
                let mut requests = recorded.lock().expect("requests lock");
                requests.push(body.clone());
                requests.len()
            };
            if attempt == 1 {
                ResponseTemplate::new(503).set_body_json(json!({
                    "jsonrpc": "2.0",
                    "id": body["id"],
                    "error": {
                        "code": -32000,
                        "message": "service unavailable",
                    },
                }))
            } else {
                modern_discover_response(&body)
            }
        })
        .expect(2)
        .mount(&server)
        .await;

    let client = create_client(&server, McpProtocolMode::V20260728).await?;
    initialize_client(&client).await?;
    {
        let requests = observed.lock().expect("requests lock");
        assert_eq!(
            requests
                .iter()
                .map(|request| request["method"].as_str())
                .collect::<Vec<_>>(),
            vec![Some("server/discover"), Some("server/discover")],
            "transient discovery failures must retry without falling back to legacy initialization"
        );
    }
    client.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn modern_discovery_retries_json_unsupported_protocol_errors() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    let observed = Arc::new(Mutex::new(Vec::<Value>::new()));
    let recorded = Arc::clone(&observed);

    Mock::given(method("POST"))
        .and(path("/mcp"))
        .respond_with(move |request: &Request| {
            let body: Value = request.body_json().expect("valid JSON-RPC request");
            assert_eq!(body["method"], "server/discover");
            let attempt = {
                let mut requests = recorded.lock().expect("requests lock");
                requests.push(body.clone());
                requests.len()
            };
            if attempt == 1 {
                ResponseTemplate::new(200).set_body_json(json!({
                    "jsonrpc": "2.0",
                    "id": body["id"],
                    "error": {
                        "code": -32022,
                        "message": "unsupported protocol version",
                        "data": {"supported": [LEGACY_VERSION, MODERN_VERSION]},
                    },
                }))
            } else {
                modern_discover_response(&body)
            }
        })
        .expect(2)
        .mount(&server)
        .await;

    let client = create_client(&server, McpProtocolMode::V20260728).await?;
    initialize_client(&client).await?;
    {
        let requests = observed.lock().expect("requests lock");
        assert_eq!(requests.len(), 2);
        assert_ne!(
            requests[0]["id"], requests[1]["id"],
            "each SDK discovery attempt must use a distinct JSON-RPC request ID"
        );
    }
    client.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn modern_sdk_emits_standard_headers_and_rejects_invalid_header_schemas() -> anyhow::Result<()>
{
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/mcp"))
        .respond_with(move |request: &Request| {
            let body: Value = request.body_json().expect("valid JSON-RPC request");
            let method = body["method"].as_str().expect("JSON-RPC method");
            assert_eq!(
                request
                    .headers
                    .get("mcp-protocol-version")
                    .and_then(|value| value.to_str().ok()),
                Some(MODERN_VERSION)
            );
            assert_eq!(
                request
                    .headers
                    .get("mcp-method")
                    .and_then(|value| value.to_str().ok()),
                Some(method)
            );

            match method {
                "server/discover" => modern_discover_response(&body),
                "tools/list" => ResponseTemplate::new(200).set_body_json(json!({
                    "jsonrpc": "2.0",
                    "id": body["id"],
                    "result": {
                        "resultType": "complete",
                        "tools": [
                            {
                                "name": "echo",
                                "inputSchema": {
                                    "type": "object",
                                    "properties": {
                                        "tenant": {"type": "string", "x-mcp-header": "Tenant"},
                                    },
                                },
                            },
                            {
                                "name": "invalid-header-schema",
                                "inputSchema": {
                                    "type": "object",
                                    "properties": {
                                        "tenant": {"type": "string", "x-mcp-header": true},
                                    },
                                },
                            },
                        ],
                    },
                })),
                "tools/call" => {
                    assert_eq!(
                        request
                            .headers
                            .get("mcp-name")
                            .and_then(|value| value.to_str().ok()),
                        Some("echo")
                    );
                    assert_eq!(
                        request
                            .headers
                            .get("mcp-param-tenant")
                            .and_then(|value| value.to_str().ok()),
                        Some("acme")
                    );
                    ResponseTemplate::new(200).set_body_json(json!({
                        "jsonrpc": "2.0",
                        "id": body["id"],
                        "result": {
                            "resultType": "complete",
                            "content": [{"type": "text", "text": "headers work"}],
                        },
                    }))
                }
                other => panic!("unexpected modern request: {other}"),
            }
        })
        .expect(3)
        .mount(&server)
        .await;

    let client = create_client(&server, McpProtocolMode::V20260728).await?;
    initialize_client(&client).await?;
    let tools = client
        .list_tools(/*params*/ None, Some(Duration::from_secs(5)))
        .await?;
    assert_eq!(tools.tools.len(), 1);
    assert_eq!(tools.tools[0].name.as_ref(), "echo");

    let result = client
        .call_tool(
            "echo".to_string(),
            Some(json!({"tenant": "acme"})),
            /*meta*/ None,
            Some(Duration::from_secs(5)),
        )
        .await?;
    assert_eq!(
        result.content[0].as_text().map(|text| text.text.as_str()),
        Some("headers work")
    );
    client.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn legacy_mode_never_probes_server_discovery() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    let observed = Arc::new(Mutex::new(Vec::<String>::new()));
    let recorded = Arc::clone(&observed);

    Mock::given(method("POST"))
        .and(path("/mcp"))
        .respond_with(move |request: &Request| {
            let body: Value = request.body_json().expect("valid JSON-RPC request");
            let method = body["method"].as_str().expect("JSON-RPC method");
            recorded.lock().expect("requests lock").push(method.into());
            match method {
                "initialize" => {
                    assert_eq!(body["params"]["protocolVersion"], LEGACY_VERSION);
                    legacy_initialize_response(&body)
                }
                "notifications/initialized" => ResponseTemplate::new(202),
                other => panic!("legacy mode must not use modern lifecycle: {other}"),
            }
        })
        .expect(2)
        .mount(&server)
        .await;

    let client = create_client(&server, McpProtocolMode::Legacy).await?;
    initialize_client(&client).await?;
    assert_eq!(
        *observed.lock().expect("requests lock"),
        vec!["initialize", "notifications/initialized"]
    );
    client.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn modern_mode_falls_back_only_for_a_json_rpc_method_not_found() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    let observed = Arc::new(Mutex::new(Vec::<String>::new()));
    let recorded = Arc::clone(&observed);

    Mock::given(method("POST"))
        .and(path("/mcp"))
        .respond_with(move |request: &Request| {
            let body: Value = request.body_json().expect("valid JSON-RPC request");
            let method = body["method"].as_str().expect("JSON-RPC method");
            recorded.lock().expect("requests lock").push(method.into());
            match method {
                "server/discover" => ResponseTemplate::new(200).set_body_json(json!({
                    "jsonrpc": "2.0",
                    "id": body["id"],
                    "error": {"code": -32601, "message": "method not found"},
                })),
                "initialize" => {
                    assert_eq!(body["params"]["protocolVersion"], LEGACY_VERSION);
                    legacy_initialize_response(&body)
                }
                "notifications/initialized" => ResponseTemplate::new(202),
                other => panic!("unexpected legacy fallback request: {other}"),
            }
        })
        .expect(3)
        .mount(&server)
        .await;

    let client = create_client(&server, McpProtocolMode::V20260728).await?;
    initialize_client(&client).await?;
    assert_eq!(
        *observed.lock().expect("requests lock"),
        vec!["server/discover", "initialize", "notifications/initialized"]
    );
    client.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn modern_mode_falls_back_when_discovery_only_advertises_legacy_versions()
-> anyhow::Result<()> {
    for legacy_response in ["discover-result", "unsupported-protocol"] {
        let server = MockServer::start().await;
        let observed = Arc::new(Mutex::new(Vec::<String>::new()));
        let recorded = Arc::clone(&observed);

        Mock::given(method("POST"))
            .and(path("/mcp"))
            .respond_with(move |request: &Request| {
                let body: Value = request.body_json().expect("valid JSON-RPC request");
                let method = body["method"].as_str().expect("JSON-RPC method");
                recorded.lock().expect("requests lock").push(method.into());
                match method {
                    "server/discover" if legacy_response == "discover-result" => {
                        let mut response = modern_discover_result(&body);
                        response["result"]["supportedVersions"] = json!(["2025-11-25"]);
                        ResponseTemplate::new(200).set_body_json(response)
                    }
                    "server/discover" => ResponseTemplate::new(400).set_body_json(json!({
                        "jsonrpc": "2.0",
                        "id": body["id"],
                        "error": {
                            "code": -32022,
                            "message": "only legacy MCP protocols are supported",
                            "data": {"supported": [LEGACY_VERSION, "2025-11-25"]},
                        },
                    })),
                    "initialize" => {
                        assert_eq!(body["params"]["protocolVersion"], LEGACY_VERSION);
                        legacy_initialize_response(&body)
                    }
                    "notifications/initialized" => ResponseTemplate::new(202),
                    other => {
                        panic!("unexpected {legacy_response} fallback request: {other}")
                    }
                }
            })
            .expect(3)
            .mount(&server)
            .await;

        let client = create_client(&server, McpProtocolMode::V20260728).await?;
        initialize_client(&client).await?;
        assert_eq!(
            *observed.lock().expect("requests lock"),
            vec!["server/discover", "initialize", "notifications/initialized"],
            "{legacy_response} should permit legacy initialization fallback"
        );
        client.shutdown().await;
    }
    Ok(())
}

#[tokio::test]
async fn modern_discovery_never_downgrades_without_valid_legacy_only_version_evidence()
-> anyhow::Result<()> {
    for (case, supported) in [
        ("missing-versions", None),
        ("empty-versions", Some(json!([]))),
        (
            "mixed-unknown-version",
            Some(json!([LEGACY_VERSION, "1999-01-01"])),
        ),
        ("mixed-invalid-version", Some(json!([LEGACY_VERSION, 42]))),
    ] {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/mcp"))
            .respond_with(move |request: &Request| {
                let body: Value = request.body_json().expect("valid JSON-RPC request");
                assert_eq!(body["method"], "server/discover");
                let mut error = json!({
                    "code": -32022,
                    "message": "unsupported protocol version",
                    "data": {},
                });
                if let Some(supported) = &supported {
                    error["data"]["supported"] = supported.clone();
                }
                ResponseTemplate::new(400).set_body_json(json!({
                    "jsonrpc": "2.0",
                    "id": body["id"],
                    "error": error,
                }))
            })
            .expect(1)
            .mount(&server)
            .await;

        let client = create_client(&server, McpProtocolMode::V20260728).await?;
        assert!(
            initialize_client(&client).await.is_err(),
            "{case} must not silently downgrade to legacy"
        );
    }
    Ok(())
}

#[tokio::test]
async fn modern_mode_falls_back_when_legacy_server_explicitly_rejects_modern_version()
-> anyhow::Result<()> {
    for error_code in [-32022, -32600, -32602] {
        let server = MockServer::start().await;
        let observed = Arc::new(Mutex::new(Vec::<String>::new()));
        let recorded = Arc::clone(&observed);

        Mock::given(method("POST"))
            .and(path("/mcp"))
            .respond_with(move |request: &Request| {
                let body: Value = request.body_json().expect("valid JSON-RPC request");
                let method = body["method"].as_str().expect("JSON-RPC method");
                recorded.lock().expect("requests lock").push(method.into());
                match method {
                    "server/discover" => ResponseTemplate::new(400).set_body_json(json!({
                        "jsonrpc": "2.0",
                        "id": body["id"],
                        "error": {
                            "code": error_code,
                            "message": "Unsupported protocol version: 2026-07-28",
                        },
                    })),
                    "initialize" => legacy_initialize_response(&body),
                    "notifications/initialized" => ResponseTemplate::new(202),
                    other => panic!("unexpected legacy fallback method: {other}"),
                }
            })
            .expect(3)
            .mount(&server)
            .await;

        let client = create_client(&server, McpProtocolMode::V20260728).await?;
        initialize_client(&client).await?;
        assert_eq!(
            *observed.lock().expect("requests lock"),
            vec!["server/discover", "initialize", "notifications/initialized"]
        );
        client.shutdown().await;
    }
    Ok(())
}

#[tokio::test]
async fn modern_discovery_rejects_uncorrelated_legacy_version_rejection() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .respond_with(|request: &Request| {
            let body: Value = request.body_json().expect("valid JSON-RPC request");
            assert_eq!(body["method"], "server/discover");
            ResponseTemplate::new(400).set_body_json(json!({
                "jsonrpc": "2.0",
                "id": "unrelated-request",
                "error": {
                    "code": -32602,
                    "message": "Unsupported protocol version: 2026-07-28",
                },
            }))
        })
        .expect(1)
        .mount(&server)
        .await;

    let client = create_client(&server, McpProtocolMode::V20260728).await?;
    assert!(
        initialize_client(&client).await.is_err(),
        "an unrelated response ID must never authorize protocol downgrade"
    );
    client.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn modern_discovery_rejects_uncorrelated_method_not_found_errors() -> anyhow::Result<()> {
    for response_id in [Value::Null, json!("unrelated-request")] {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/mcp"))
            .respond_with(move |request: &Request| {
                let body: Value = request.body_json().expect("valid JSON-RPC request");
                assert_eq!(body["method"], "server/discover");
                ResponseTemplate::new(200).set_body_json(json!({
                    "jsonrpc": "2.0",
                    "id": response_id,
                    "error": {"code": -32601, "message": "method not found"},
                }))
            })
            .expect(1)
            .mount(&server)
            .await;

        let client = create_client(&server, McpProtocolMode::V20260728).await?;
        let error = initialize_client(&client)
            .await
            .expect_err("an unrelated error must not authorize protocol downgrade");
        assert!(
            error.to_string().contains("did not match its request ID"),
            "uncorrelated method-not-found error should explain its rejection: {error:#}"
        );
        client.shutdown().await;
    }
    Ok(())
}

#[tokio::test]
async fn modern_mode_falls_back_for_legacy_http_prevalidation_errors() -> anyhow::Result<()> {
    for (message, content_type) in [
        (
            "Bad Request: Unsupported protocol version: 2026-07-28 (supported versions: 2025-11-25, 2025-06-18, 2025-03-26, 2024-11-05, 2024-10-07)",
            "application/json",
        ),
        // tinymcp.dev omits the rejected version from its legacy error shape.
        (
            "Bad Request: Unsupported protocol version (supported versions: 2025-06-18, 2025-03-26, 2024-11-05, 2024-10-07)",
            "application/json",
        ),
        (
            "Bad Request: Unsupported protocol version (supported versions: 2025-06-18, 2025-03-26, 2024-11-05, 2024-10-07)",
            "text/plain",
        ),
        (
            "Bad Request: Unsupported protocol version (supported versions: 2025-06-18, 2025-03-26, 2024-11-05, 2024-10-07)",
            "application/octet-stream",
        ),
        (
            "Bad Request: No valid session ID provided",
            "application/json",
        ),
    ] {
        let server = MockServer::start().await;
        let observed = Arc::new(Mutex::new(Vec::<String>::new()));
        let recorded = Arc::clone(&observed);

        Mock::given(method("POST"))
            .and(path("/mcp"))
            .respond_with(move |request: &Request| {
                let body: Value = request.body_json().expect("valid JSON-RPC request");
                let method = body["method"].as_str().expect("JSON-RPC method");
                recorded.lock().expect("requests lock").push(method.into());
                match method {
                    "server/discover" => ResponseTemplate::new(400).set_body_raw(
                        serde_json::to_vec(&json!({
                            "jsonrpc": "2.0",
                            "id": null,
                            "error": {"code": -32000, "message": message},
                        }))
                        .expect("legacy discovery error serializes"),
                        content_type,
                    ),
                    "initialize" => legacy_initialize_response(&body),
                    "notifications/initialized" => ResponseTemplate::new(202),
                    other => panic!("unexpected legacy fallback method: {other}"),
                }
            })
            .expect(3)
            .mount(&server)
            .await;

        let client = create_client(&server, McpProtocolMode::V20260728).await?;
        initialize_client(&client).await?;
        assert_eq!(
            *observed.lock().expect("requests lock"),
            vec!["server/discover", "initialize", "notifications/initialized"]
        );
        client.shutdown().await;
    }
    Ok(())
}

#[tokio::test]
async fn modern_mode_negotiates_a_server_selected_legacy_protocol_version() -> anyhow::Result<()> {
    for (message, content_type, negotiated_version) in [
        (
            "Bad Request: Unsupported protocol version: 2026-07-28 (supported versions: 2025-11-25)",
            "application/json",
            "2025-11-25",
        ),
        (
            "Bad Request: Unsupported protocol version (supported versions: 2025-11-25)",
            "application/json",
            "2025-11-25",
        ),
        (
            "Bad Request: Unsupported protocol version (supported versions: 2025-11-25)",
            "text/plain",
            "2025-11-25",
        ),
        (
            "Bad Request: Unsupported protocol version (supported versions: 2025-03-26)",
            "application/json",
            "2025-03-26",
        ),
        (
            "Bad Request: Unsupported protocol version (supported versions: 2024-11-05)",
            "application/json",
            "2024-11-05",
        ),
    ] {
        let server = MockServer::start().await;
        let observed = Arc::new(Mutex::new(Vec::<String>::new()));
        let recorded = Arc::clone(&observed);

        Mock::given(method("POST"))
            .and(path("/mcp"))
            .respond_with(move |request: &Request| {
                let body: Value = request.body_json().expect("valid JSON-RPC request");
                let method = body["method"].as_str().expect("JSON-RPC method");
                recorded.lock().expect("requests lock").push(method.into());
                match method {
                    "server/discover" => ResponseTemplate::new(400).set_body_raw(
                        serde_json::to_vec(&json!({
                            "jsonrpc": "2.0",
                            "id": null,
                            "error": {"code": -32000, "message": message},
                        }))
                        .expect("legacy discovery error serializes"),
                        content_type,
                    ),
                    "initialize" => {
                        assert_eq!(body["params"]["protocolVersion"], LEGACY_VERSION);
                        ResponseTemplate::new(200).set_body_json(json!({
                            "jsonrpc": "2.0",
                            "id": body["id"],
                            "result": {
                                "protocolVersion": negotiated_version,
                                "capabilities": {"tools": {}},
                                "serverInfo": {"name": "legacy-test", "version": "1.0.0"},
                            },
                        }))
                    }
                    "notifications/initialized" => {
                        assert_eq!(
                            request
                                .headers
                                .get("mcp-protocol-version")
                                .and_then(|value| value.to_str().ok()),
                            Some(negotiated_version)
                        );
                        ResponseTemplate::new(202)
                    }
                    other => panic!("unexpected legacy fallback method: {other}"),
                }
            })
            .expect(3)
            .mount(&server)
            .await;

        let client = create_client(&server, McpProtocolMode::V20260728).await?;
        let initialized = initialize_client(&client).await?;
        assert_eq!(initialized.protocol_version.as_str(), negotiated_version);
        assert_eq!(
            *observed.lock().expect("requests lock"),
            vec!["server/discover", "initialize", "notifications/initialized"]
        );
        client.shutdown().await;
    }
    Ok(())
}

#[tokio::test]
async fn modern_discovery_rejects_unrecognized_or_modern_null_id_errors() -> anyhow::Result<()> {
    for (status, message, content_type) in [
        (400, "Bad Request: malformed request", "application/json"),
        (400, "Bad Request: malformed request", "text/plain"),
        (
            400,
            "Bad Request: Unsupported protocol version: 2026-07-28 (supported versions: 2025-06-18, 2099-01-01)",
            "application/json",
        ),
        (
            400,
            "Bad Request: Unsupported protocol version (supported versions: 2025-06-18, 2099-01-01)",
            "application/json",
        ),
        (
            400,
            "Bad Request: Unsupported protocol version (supported versions: 2025-06-18, 2099-01-01)",
            "text/plain",
        ),
        (
            400,
            "Bad Request: Unsupported protocol version (supported versions: 2024-10-07)",
            "application/json",
        ),
        (
            200,
            "Bad Request: No valid session ID provided",
            "application/json",
        ),
    ] {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/mcp"))
            .respond_with(move |request: &Request| {
                let body: Value = request.body_json().expect("valid JSON-RPC request");
                assert_eq!(body["method"], "server/discover");
                ResponseTemplate::new(status).set_body_raw(
                    serde_json::to_vec(&json!({
                        "jsonrpc": "2.0",
                        "id": null,
                        "error": {"code": -32000, "message": message},
                    }))
                    .expect("legacy discovery error serializes"),
                    content_type,
                )
            })
            .expect(1)
            .mount(&server)
            .await;

        let client = create_client(&server, McpProtocolMode::V20260728).await?;
        assert!(
            initialize_client(&client).await.is_err(),
            "unproven legacy downgrade must fail (HTTP {status}, {message})"
        );
        client.shutdown().await;
    }
    Ok(())
}

#[tokio::test]
async fn modern_discovery_rejects_non_legacy_http_prevalidation_errors() -> anyhow::Result<()> {
    for (status, error_code, response_id, content_type) in [
        (400, -32022, None, "application/json"),
        (400, -32021, None, "application/json"),
        (400, -32020, None, "application/json"),
        (400, -32601, None, "application/json"),
        (400, -32602, None, "application/json"),
        (400, -32020, None, "text/plain"),
        (400, -32000, Some("unrelated-request"), "application/json"),
        (401, -32000, None, "application/json"),
        (403, -32000, None, "application/json"),
        (200, -32000, None, "application/json"),
    ] {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/mcp"))
            .respond_with(move |request: &Request| {
                let body: Value = request.body_json().expect("valid JSON-RPC request");
                assert_eq!(body["method"], "server/discover");
                ResponseTemplate::new(status).set_body_raw(
                    serde_json::to_vec(&json!({
                        "jsonrpc": "2.0",
                        "id": response_id,
                        "error": {"code": error_code, "message": "discovery rejected"},
                    }))
                    .expect("discovery error serializes"),
                    content_type,
                )
            })
            .expect(1)
            .mount(&server)
            .await;

        let client = create_client(&server, McpProtocolMode::V20260728).await?;
        assert!(
            initialize_client(&client).await.is_err(),
            "non-legacy discovery error must not downgrade (HTTP {status}, error {error_code}, response ID {response_id:?})"
        );
        client.shutdown().await;
    }
    Ok(())
}

#[tokio::test]
async fn modern_mode_falls_back_for_plain_404_and_405_discovery_responses() -> anyhow::Result<()> {
    for status in [404, 405] {
        let server = MockServer::start().await;
        let observed = Arc::new(Mutex::new(Vec::<String>::new()));
        let recorded = Arc::clone(&observed);

        Mock::given(method("POST"))
            .and(path("/mcp"))
            .respond_with(move |request: &Request| {
                let body: Value = request.body_json().expect("valid JSON-RPC request");
                let method = body["method"].as_str().expect("JSON-RPC method");
                recorded.lock().expect("requests lock").push(method.into());
                match method {
                    "server/discover" => {
                        ResponseTemplate::new(status).set_body_string("legacy MCP endpoint")
                    }
                    "initialize" => legacy_initialize_response(&body),
                    "notifications/initialized" => ResponseTemplate::new(202),
                    other => panic!("unexpected HTTP {status} fallback request: {other}"),
                }
            })
            .expect(3)
            .mount(&server)
            .await;

        let client = create_client(&server, McpProtocolMode::V20260728).await?;
        initialize_client(&client).await?;
        assert_eq!(
            *observed.lock().expect("requests lock"),
            vec!["server/discover", "initialize", "notifications/initialized"],
            "HTTP {status} should permit legacy initialization fallback"
        );
        client.shutdown().await;
    }
    Ok(())
}

#[tokio::test]
async fn modern_discovery_rejection_does_not_downgrade_to_legacy() -> anyhow::Result<()> {
    for (status, error_code) in [(400, -32602), (403, -32021), (200, -32020)] {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/mcp"))
            .respond_with(move |request: &Request| {
                let body: Value = request.body_json().expect("valid JSON-RPC request");
                assert_eq!(body["method"], "server/discover");
                ResponseTemplate::new(status).set_body_json(json!({
                    "jsonrpc": "2.0",
                    "id": body["id"],
                    "error": {"code": error_code, "message": "modern discovery rejected"},
                }))
            })
            .expect(1)
            .mount(&server)
            .await;

        let client = create_client(&server, McpProtocolMode::V20260728).await?;
        let error = initialize_client(&client)
            .await
            .expect_err("modern discovery errors must not silently downgrade");
        assert!(
            error.to_string().contains("modern discovery rejected"),
            "unexpected discovery rejection for HTTP {status}: {error:#}"
        );
    }
    Ok(())
}

#[tokio::test]
async fn stdio_protocol_marker_rejects_unknown_versions_before_launch() -> anyhow::Result<()> {
    let env = HashMap::from([(
        OsString::from("CODEX_MCP_PROTOCOL_VERSION"),
        OsString::from("1999-01-01"),
    )]);
    let error = RmcpClient::new_stdio_client_with_protocol_mode(
        OsString::from("never-launch"),
        Vec::new(),
        Some(env),
        &[],
        /*cwd*/ None,
        Arc::new(LocalStdioServerLauncher::new(std::env::current_dir()?)),
        McpProtocolMode::V20260728,
    )
    .await
    .err()
    .expect("an unsupported stdio protocol must fail before launch");

    assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    assert!(error.to_string().contains("1999-01-01"));
    Ok(())
}
