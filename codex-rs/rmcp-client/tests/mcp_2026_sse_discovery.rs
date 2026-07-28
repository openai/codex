use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use codex_config::types::AuthKeyringBackendKind;
use codex_config::types::OAuthCredentialsStoreMode;
use codex_exec_server::Environment;
use codex_rmcp_client::ElicitationAction;
use codex_rmcp_client::ElicitationResponse;
use codex_rmcp_client::McpProtocolMode;
use codex_rmcp_client::RmcpClient;
use futures::FutureExt;
use pretty_assertions::assert_eq;
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

const MODERN_VERSION: &str = "2026-07-28";
const LEGACY_VERSION: &str = "2025-06-18";

async fn initialize_modern_client(server: &MockServer) -> anyhow::Result<RmcpClient> {
    let client = RmcpClient::new_streamable_http_client_with_protocol_mode(
        "sse-discovery-test",
        &format!("{}/mcp", server.uri()),
        /*bearer_token*/ None,
        /*http_headers*/ None,
        /*env_http_headers*/ None,
        OAuthCredentialsStoreMode::File,
        AuthKeyringBackendKind::default(),
        Environment::default_for_tests().get_http_client(),
        /*auth_provider*/ None,
        McpProtocolMode::V20260728,
    )
    .await?;

    let params = InitializeRequestParams::new(
        ClientCapabilities::default(),
        Implementation::new("codex-sse-discovery-test", "0.0.0"),
    )
    .with_protocol_version(ProtocolVersion::V_2025_06_18);

    client
        .initialize(
            params,
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
        .await?;

    Ok(client)
}

fn sse_response(message: Value) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_raw(
        format!("event: message\ndata: {message}\n\n"),
        "text/event-stream",
    )
}

#[tokio::test]
async fn modern_sse_discovery_accepts_metadata_namespaced_server_identity() -> anyhow::Result<()> {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/mcp"))
        .respond_with(|request: &Request| {
            let body: Value = request.body_json().expect("valid JSON-RPC request");
            assert_eq!(body["method"], "server/discover");

            sse_response(json!({
                "jsonrpc": "2.0",
                "id": body["id"],
                "result": {
                    "resultType": "complete",
                    "supportedVersions": [MODERN_VERSION],
                    "capabilities": {"tools": {}},
                    "_meta": {
                        "io.modelcontextprotocol/serverInfo": {
                            "name": "modern-sse-test",
                            "version": "1.0.0",
                        },
                    },
                    "ttlMs": 0,
                    "cacheScope": "private",
                },
            }))
        })
        .expect(1)
        .mount(&server)
        .await;

    let client = initialize_modern_client(&server).await?;
    client.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn modern_sse_discovery_falls_back_for_correlated_method_not_found() -> anyhow::Result<()> {
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
                "server/discover" => sse_response(json!({
                    "jsonrpc": "2.0",
                    "id": body["id"],
                    "error": {"code": -32601, "message": "method not found"},
                })),
                "initialize" => {
                    assert_eq!(body["params"]["protocolVersion"], LEGACY_VERSION);
                    ResponseTemplate::new(200).set_body_json(json!({
                        "jsonrpc": "2.0",
                        "id": body["id"],
                        "result": {
                            "protocolVersion": LEGACY_VERSION,
                            "capabilities": {"tools": {}},
                            "serverInfo": {
                                "name": "legacy-sse-test",
                                "version": "1.0.0",
                            },
                        },
                    }))
                }
                "notifications/initialized" => ResponseTemplate::new(202),
                other => panic!("unexpected legacy SSE fallback request: {other}"),
            }
        })
        .expect(3)
        .mount(&server)
        .await;

    let client = initialize_modern_client(&server).await?;
    assert_eq!(
        *observed.lock().expect("requests lock"),
        vec!["server/discover", "initialize", "notifications/initialized"]
    );
    client.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn modern_sse_discovery_rejects_uncorrelated_method_not_found() -> anyhow::Result<()> {
    for (case, rejected_id) in [("null", json!(null)), ("mismatched", json!("unrelated"))] {
        let server = MockServer::start().await;
        let observed = Arc::new(Mutex::new(Vec::<String>::new()));
        let recorded = Arc::clone(&observed);

        Mock::given(method("POST"))
            .and(path("/mcp"))
            .respond_with(move |request: &Request| {
                let body: Value = request.body_json().expect("valid JSON-RPC request");
                let method = body["method"].as_str().expect("JSON-RPC method");
                recorded.lock().expect("requests lock").push(method.into());
                assert_eq!(method, "server/discover");

                sse_response(json!({
                    "jsonrpc": "2.0",
                    "id": rejected_id.clone(),
                    "error": {"code": -32601, "message": "method not found"},
                }))
            })
            .expect(1)
            .mount(&server)
            .await;

        assert!(
            initialize_modern_client(&server).await.is_err(),
            "{case} SSE discovery error must not downgrade to legacy"
        );
        assert_eq!(
            *observed.lock().expect("requests lock"),
            vec!["server/discover"],
            "{case} SSE discovery error must not initialize a legacy session"
        );
    }

    Ok(())
}
