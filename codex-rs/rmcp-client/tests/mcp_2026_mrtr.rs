use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use codex_config::types::AuthKeyringBackendKind;
use codex_config::types::OAuthCredentialsStoreMode;
use codex_exec_server::Environment;
use codex_rmcp_client::Elicitation;
use codex_rmcp_client::ElicitationAction;
use codex_rmcp_client::ElicitationResponse;
use codex_rmcp_client::McpProtocolMode;
use codex_rmcp_client::RmcpClient;
use futures::FutureExt;
use pretty_assertions::assert_eq;
use rmcp::model::ClientCapabilities;
use rmcp::model::ElicitRequestParams;
use rmcp::model::ElicitationCapability;
use rmcp::model::FormElicitationCapability;
use rmcp::model::Implementation;
use rmcp::model::InitializeRequestParams;
use rmcp::model::ProtocolVersion;
use rmcp::model::ReadResourceRequestParams;
use rmcp::model::ServerResult;
use rmcp::model::UrlElicitationCapability;
use serde_json::Value;
use serde_json::json;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::Request;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

const MODERN_VERSION: &str = "2026-07-28";
const OPAQUE_STATE: &str = " opaque/\u{2603}/=?base64?literal?=\n";

fn discover_response(body: &Value) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(json!({
        "jsonrpc": "2.0",
        "id": body["id"],
        "result": {
            "resultType": "complete",
            "supportedVersions": [MODERN_VERSION],
            "capabilities": {"tools": {}, "resources": {}},
            "_meta": {
                "io.modelcontextprotocol/serverInfo": {
                    "name": "mrtr-test",
                    "version": "1.0.0",
                },
            },
            "ttlMs": 0,
            "cacheScope": "private",
        },
    }))
}

fn result_response(body: &Value, result: Value) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(json!({
        "jsonrpc": "2.0",
        "id": body["id"],
        "result": result,
    }))
}

fn sse_result_response(body: &Value, result: Value) -> ResponseTemplate {
    let message = json!({
        "jsonrpc": "2.0",
        "id": body["id"],
        "result": result,
    });
    ResponseTemplate::new(200).set_body_raw(
        format!("event: message\ndata: {message}\n\n"),
        "text/event-stream; charset=utf-8",
    )
}

fn elicitation_request(mode: &str) -> Value {
    let params = match mode {
        "form" => json!({
            "mode": "form",
            "message": "Confirm the MCP request.",
            "requestedSchema": {
                "type": "object",
                "properties": {"confirmed": {"type": "boolean"}},
                "required": ["confirmed"],
            },
            "_meta": {"inputContext": "server-context"},
        }),
        "url" => json!({
            "mode": "url",
            "message": "Approve the MCP request.",
            "url": "https://example.test/approve",
            "elicitationId": "approval-123",
            "_meta": {"inputContext": "server-context"},
        }),
        other => panic!("unexpected elicitation mode: {other}"),
    };
    json!({"method": "elicitation/create", "params": params})
}

async fn create_client(
    server: &MockServer,
    elicitation_modes: Arc<Mutex<Vec<String>>>,
) -> anyhow::Result<RmcpClient> {
    let client = RmcpClient::new_streamable_http_client_with_protocol_mode(
        "mrtr-test",
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

    let mut capabilities = ClientCapabilities::default();
    capabilities.elicitation = Some(
        ElicitationCapability::new()
            .with_form(FormElicitationCapability::new())
            .with_url(UrlElicitationCapability::new()),
    );
    client
        .initialize(
            InitializeRequestParams::new(
                capabilities,
                Implementation::new("codex-mrtr-test", "0.0.0"),
            )
            .with_protocol_version(ProtocolVersion::V_2025_06_18),
            Some(Duration::from_secs(5)),
            Box::new(move |request_id, request| {
                let elicitation_modes = Arc::clone(&elicitation_modes);
                async move {
                    assert_eq!(request_id.to_string(), "confirmation");
                    let Elicitation::Mcp(request) = request else {
                        anyhow::bail!("MRTR input must be a standard MCP elicitation");
                    };
                    let mode = match request {
                        ElicitRequestParams::FormElicitationParams { meta, .. } => {
                            assert_eq!(
                                meta.and_then(|meta| meta.get("inputContext").cloned()),
                                Some(json!("server-context"))
                            );
                            "form"
                        }
                        ElicitRequestParams::UrlElicitationParams { meta, .. } => {
                            assert_eq!(
                                meta.and_then(|meta| meta.get("inputContext").cloned()),
                                Some(json!("server-context"))
                            );
                            "url"
                        }
                        _ => anyhow::bail!("unsupported elicitation mode"),
                    };
                    elicitation_modes
                        .lock()
                        .map_err(|_| anyhow::anyhow!("elicitation lock was poisoned"))?
                        .push(mode.to_owned());
                    Ok(ElicitationResponse {
                        action: ElicitationAction::Accept,
                        content: Some(json!({"confirmed": true})),
                        meta: Some(json!({"clientContext": "preserved"})),
                    })
                }
                .boxed()
            }),
        )
        .await?;
    Ok(client)
}

#[tokio::test]
async fn modern_sse_input_required_preserves_response_metadata() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    let expected_result = json!({
        "resultType": "input_required",
        "requestState": OPAQUE_STATE,
        "_meta": {"responseContext": "preserved"},
    });
    let result = expected_result.clone();

    Mock::given(method("POST"))
        .and(path("/mcp"))
        .respond_with(move |request: &Request| {
            let body: Value = request.body_json().expect("valid JSON-RPC request");
            match body["method"].as_str() {
                Some("server/discover") => discover_response(&body),
                Some("tools/call") => sse_result_response(&body, result.clone()),
                other => panic!("unexpected MRTR request: {other:?}"),
            }
        })
        .expect(2)
        .mount(&server)
        .await;

    let client = create_client(&server, Arc::new(Mutex::new(Vec::new()))).await?;
    let result = client
        .send_custom_request_with_timeout(
            "tools/call",
            Some(json!({"name": "confirm", "arguments": {}})),
            Some(Duration::from_secs(5)),
        )
        .await?;
    let ServerResult::InputRequiredResult(result) = result else {
        anyhow::bail!("SSE response must remain an input_required result");
    };

    assert_eq!(serde_json::to_value(result)?, expected_result);

    client.shutdown().await;
    server.verify().await;

    Ok(())
}

#[tokio::test]
async fn modern_tool_mrtr_drives_form_and_url_elicitation_and_preserves_metadata()
-> anyhow::Result<()> {
    for mode in ["form", "url"] {
        let server = MockServer::start().await;
        let calls = Arc::new(Mutex::new(Vec::<Value>::new()));
        let recorded = Arc::clone(&calls);

        Mock::given(method("POST"))
            .and(path("/mcp"))
            .respond_with(move |request: &Request| {
                let body: Value = request.body_json().expect("valid JSON-RPC request");
                match body["method"].as_str() {
                    Some("server/discover") => discover_response(&body),
                    Some("tools/call") => {
                        recorded.lock().expect("requests lock").push(body.clone());
                        assert_eq!(
                            body.pointer("/params/_meta/requestContext"),
                            Some(&json!("caller-context"))
                        );
                        assert_eq!(
                            body.pointer("/params/_meta/io.modelcontextprotocol~1protocolVersion"),
                            Some(&json!(MODERN_VERSION))
                        );
                        if body.pointer("/params/inputResponses").is_none() {
                            result_response(
                                &body,
                                json!({
                                    "resultType": "input_required",
                                    "inputRequests": {
                                        "confirmation": elicitation_request(mode),
                                    },
                                    "requestState": OPAQUE_STATE,
                                    "_meta": {
                                        "io.modelcontextprotocol/serverInfo": {
                                            "name": "mrtr-test",
                                            "version": "1.0.0",
                                        },
                                        "responseContext": "preserved",
                                    },
                                }),
                            )
                        } else {
                            assert_eq!(
                                body.pointer("/params/requestState"),
                                Some(&json!(OPAQUE_STATE))
                            );
                            assert_eq!(
                                body.pointer("/params/inputResponses/confirmation"),
                                Some(&json!({
                                    "action": "accept",
                                    "content": {"confirmed": true},
                                    "_meta": {"clientContext": "preserved"},
                                }))
                            );
                            result_response(
                                &body,
                                json!({
                                    "resultType": "complete",
                                    "content": [{"type": "text", "text": "MRTR completed"}],
                                }),
                            )
                        }
                    }
                    other => panic!("unexpected MRTR request: {other:?}"),
                }
            })
            .expect(3)
            .mount(&server)
            .await;

        let elicitation_modes = Arc::new(Mutex::new(Vec::new()));
        let client = create_client(&server, Arc::clone(&elicitation_modes)).await?;
        let result = client
            .call_tool(
                "confirm".into(),
                Some(json!({"mode": mode})),
                Some(json!({"requestContext": "caller-context"})),
                Some(Duration::from_secs(5)),
            )
            .await?;
        assert_eq!(
            result.content[0].as_text().map(|text| text.text.as_str()),
            Some("MRTR completed")
        );
        assert_eq!(
            *elicitation_modes.lock().expect("elicitation lock"),
            vec![mode]
        );

        {
            let calls = calls.lock().expect("requests lock");
            assert_eq!(calls.len(), 2);
            assert_ne!(calls[0]["id"], calls[1]["id"]);
        }
        client.shutdown().await;
    }
    Ok(())
}

#[tokio::test]
async fn modern_tool_mrtr_uses_recovered_protocol_after_legacy_session_expiry() -> anyhow::Result<()>
{
    let server = MockServer::start().await;
    let calls = Arc::new(Mutex::new(Vec::<Value>::new()));
    let sessions = Arc::new(Mutex::new(Vec::<String>::new()));
    let recorded_calls = Arc::clone(&calls);
    let recorded_sessions = Arc::clone(&sessions);

    Mock::given(method("GET"))
        .and(path("/mcp"))
        .respond_with(ResponseTemplate::new(405))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/mcp"))
        .respond_with(move |request: &Request| {
            let body: Value = request.body_json().expect("valid JSON-RPC request");
            match body["method"].as_str() {
                Some("server/discover") => {
                    let mut sessions = recorded_sessions.lock().expect("sessions lock");
                    if sessions.is_empty() {
                        ResponseTemplate::new(200).set_body_json(json!({
                            "jsonrpc": "2.0",
                            "id": body["id"],
                            "error": {"code": -32601, "message": "method not found"},
                        }))
                    } else {
                        sessions.push("modern-session".to_owned());
                        discover_response(&body).insert_header("mcp-session-id", "modern-session")
                    }
                }
                Some("initialize") => {
                    assert_eq!(
                        body.pointer("/params/protocolVersion"),
                        Some(&json!(ProtocolVersion::V_2025_06_18.as_str()))
                    );
                    recorded_sessions
                        .lock()
                        .expect("sessions lock")
                        .push("legacy-session".to_owned());
                    ResponseTemplate::new(200)
                        .set_body_json(json!({
                            "jsonrpc": "2.0",
                            "id": body["id"],
                            "result": {
                                "protocolVersion": ProtocolVersion::V_2025_06_18.as_str(),
                                "capabilities": {"tools": {}},
                                "serverInfo": {"name": "mrtr-test", "version": "1.0.0"},
                            },
                        }))
                        .insert_header("mcp-session-id", "legacy-session")
                }
                Some("notifications/initialized") => ResponseTemplate::new(202),
                Some("tools/call") => {
                    assert_eq!(
                        body.pointer("/params/_meta/requestContext"),
                        Some(&json!("caller-context"))
                    );
                    let attempt = {
                        let mut calls = recorded_calls.lock().expect("requests lock");
                        calls.push(body.clone());
                        calls.len()
                    };

                    match attempt {
                        1 => {
                            assert_eq!(
                                body.pointer(
                                    "/params/_meta/io.modelcontextprotocol~1protocolVersion"
                                ),
                                None
                            );
                            ResponseTemplate::new(404)
                        }
                        2 => {
                            assert_eq!(
                                body.pointer(
                                    "/params/_meta/io.modelcontextprotocol~1protocolVersion"
                                ),
                                Some(&json!(MODERN_VERSION))
                            );
                            result_response(
                                &body,
                                json!({
                                    "resultType": "input_required",
                                    "inputRequests": {
                                        "confirmation": elicitation_request("form"),
                                    },
                                    "requestState": OPAQUE_STATE,
                                }),
                            )
                        }
                        3 => {
                            assert_eq!(
                                body.pointer(
                                    "/params/_meta/io.modelcontextprotocol~1protocolVersion"
                                ),
                                Some(&json!(MODERN_VERSION))
                            );
                            assert_eq!(
                                body.pointer("/params/requestState"),
                                Some(&json!(OPAQUE_STATE))
                            );
                            assert_eq!(
                                body.pointer("/params/inputResponses/confirmation"),
                                Some(&json!({
                                    "action": "accept",
                                    "content": {"confirmed": true},
                                    "_meta": {"clientContext": "preserved"},
                                }))
                            );
                            result_response(
                                &body,
                                json!({
                                    "resultType": "complete",
                                    "content": [{"type": "text", "text": "recovered MRTR completed"}],
                                }),
                            )
                        }
                        other => panic!("unexpected recovered MRTR attempt: {other}"),
                    }
                }
                other => panic!("unexpected recovered MRTR request: {other:?}"),
            }
        })
        .mount(&server)
        .await;

    let elicitation_modes = Arc::new(Mutex::new(Vec::new()));
    let client = create_client(&server, Arc::clone(&elicitation_modes)).await?;
    let result = client
        .call_tool(
            "confirm".into(),
            Some(json!({})),
            Some(json!({"requestContext": "caller-context"})),
            Some(Duration::from_secs(5)),
        )
        .await?;

    assert_eq!(
        result.content[0].as_text().map(|text| text.text.as_str()),
        Some("recovered MRTR completed")
    );
    assert_eq!(
        *sessions.lock().expect("sessions lock"),
        vec!["legacy-session", "modern-session"]
    );
    assert_eq!(calls.lock().expect("requests lock").len(), 3);
    assert_eq!(
        *elicitation_modes.lock().expect("elicitation lock"),
        vec!["form"]
    );
    client.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn modern_resource_mrtr_retries_with_opaque_request_state() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    let calls = Arc::new(Mutex::new(Vec::<Value>::new()));
    let recorded = Arc::clone(&calls);

    Mock::given(method("POST"))
        .and(path("/mcp"))
        .respond_with(move |request: &Request| {
            let body: Value = request.body_json().expect("valid JSON-RPC request");
            match body["method"].as_str() {
                Some("server/discover") => discover_response(&body),
                Some("resources/read") => {
                    recorded.lock().expect("requests lock").push(body.clone());
                    if body.pointer("/params/inputResponses").is_none() {
                        result_response(
                            &body,
                            json!({
                                "resultType": "input_required",
                                "inputRequests": {
                                    "confirmation": elicitation_request("form"),
                                },
                                "requestState": OPAQUE_STATE,
                                "_meta": {
                                    "io.modelcontextprotocol/serverInfo": {
                                        "name": "mrtr-test",
                                        "version": "1.0.0",
                                    },
                                    "responseContext": "preserved",
                                },
                            }),
                        )
                    } else {
                        assert_eq!(
                            body.pointer("/params/requestState"),
                            Some(&json!(OPAQUE_STATE))
                        );
                        assert_eq!(
                            body.pointer("/params/inputResponses/confirmation/_meta/clientContext"),
                            Some(&json!("preserved"))
                        );
                        result_response(
                            &body,
                            json!({
                                "resultType": "complete",
                                "contents": [{
                                    "uri": "memo://requires-confirmation",
                                    "mimeType": "text/plain",
                                    "text": "approved resource",
                                }],
                            }),
                        )
                    }
                }
                other => panic!("unexpected resource MRTR request: {other:?}"),
            }
        })
        .expect(3)
        .mount(&server)
        .await;

    let elicitation_modes = Arc::new(Mutex::new(Vec::new()));
    let client = create_client(&server, Arc::clone(&elicitation_modes)).await?;
    let result = client
        .read_resource(
            ReadResourceRequestParams::new("memo://requires-confirmation"),
            Some(Duration::from_secs(5)),
        )
        .await?;
    assert_eq!(result.contents.len(), 1);
    assert_eq!(
        *elicitation_modes.lock().expect("elicitation lock"),
        vec!["form"]
    );

    {
        let calls = calls.lock().expect("requests lock");
        assert_eq!(calls.len(), 2);
        assert_ne!(calls[0]["id"], calls[1]["id"]);
    }
    client.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn modern_tool_mrtr_accepts_sse_discovery_and_metadata_bearing_rounds() -> anyhow::Result<()>
{
    let server = MockServer::start().await;
    let calls = Arc::new(Mutex::new(Vec::<Value>::new()));
    let recorded = Arc::clone(&calls);

    Mock::given(method("POST"))
        .and(path("/mcp"))
        .respond_with(move |request: &Request| {
            let body: Value = request.body_json().expect("valid JSON-RPC request");
            match body["method"].as_str() {
                Some("server/discover") => sse_result_response(
                    &body,
                    json!({
                        "resultType": "complete",
                        "supportedVersions": [MODERN_VERSION],
                        "capabilities": {"tools": {}},
                        "_meta": {
                            "io.modelcontextprotocol/serverInfo": {
                                "name": "sse-mrtr-test",
                                "version": "1.0.0",
                            },
                        },
                        "ttlMs": 0,
                        "cacheScope": "private",
                    }),
                ),
                Some("tools/call") => {
                    recorded.lock().expect("requests lock").push(body.clone());
                    if body.pointer("/params/inputResponses").is_none() {
                        sse_result_response(
                            &body,
                            json!({
                                "resultType": "input_required",
                                "inputRequests": {
                                    "confirmation": elicitation_request("form"),
                                },
                                "requestState": OPAQUE_STATE,
                                "_meta": {"responseContext": "sse-round"},
                            }),
                        )
                    } else {
                        assert_eq!(
                            body.pointer("/params/requestState"),
                            Some(&json!(OPAQUE_STATE))
                        );
                        assert_eq!(
                            body.pointer("/params/inputResponses/confirmation/action"),
                            Some(&json!("accept"))
                        );
                        sse_result_response(
                            &body,
                            json!({
                                "resultType": "complete",
                                "content": [{"type": "text", "text": "SSE MRTR completed"}],
                            }),
                        )
                    }
                }
                other => panic!("unexpected SSE MRTR request: {other:?}"),
            }
        })
        .expect(3)
        .mount(&server)
        .await;

    let elicitation_modes = Arc::new(Mutex::new(Vec::new()));
    let client = create_client(&server, Arc::clone(&elicitation_modes)).await?;
    let result = client
        .call_tool(
            "confirm".to_owned(),
            Some(json!({})),
            /*meta*/ None,
            Some(Duration::from_secs(5)),
        )
        .await?;

    assert_eq!(
        result.content[0].as_text().map(|text| text.text.as_str()),
        Some("SSE MRTR completed")
    );
    assert_eq!(
        *elicitation_modes.lock().expect("elicitation lock"),
        vec!["form"]
    );
    assert_eq!(calls.lock().expect("requests lock").len(), 2);
    client.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn modern_tool_mrtr_preserves_state_only_rounds_without_elicitation() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    let states = Arc::new(Mutex::new(Vec::<Option<String>>::new()));
    let recorded = Arc::clone(&states);

    Mock::given(method("POST"))
        .and(path("/mcp"))
        .respond_with(move |request: &Request| {
            let body: Value = request.body_json().expect("valid JSON-RPC request");
            match body["method"].as_str() {
                Some("server/discover") => discover_response(&body),
                Some("tools/call") => {
                    let state = body
                        .pointer("/params/requestState")
                        .and_then(Value::as_str)
                        .map(str::to_owned);
                    recorded.lock().expect("states lock").push(state.clone());
                    match state.as_deref() {
                        None => result_response(
                            &body,
                            json!({
                                "resultType": "input_required",
                                "requestState": "state-1",
                                "_meta": {"responseContext": "first-round"},
                            }),
                        ),
                        Some("state-1") => result_response(
                            &body,
                            json!({
                                "resultType": "input_required",
                                "requestState": "state-2",
                                "_meta": {"responseContext": "second-round"},
                            }),
                        ),
                        Some("state-2") => result_response(
                            &body,
                            json!({
                                "resultType": "complete",
                                "content": [{"type": "text", "text": "state completed"}],
                            }),
                        ),
                        Some(other) => panic!("unexpected opaque state: {other}"),
                    }
                }
                other => panic!("unexpected state-only MRTR request: {other:?}"),
            }
        })
        .expect(4)
        .mount(&server)
        .await;

    let elicitation_modes = Arc::new(Mutex::new(Vec::new()));
    let client = create_client(&server, Arc::clone(&elicitation_modes)).await?;
    let result = client
        .call_tool(
            "state-only".into(),
            Some(json!({})),
            /*meta*/ None,
            Some(Duration::from_secs(5)),
        )
        .await?;
    assert_eq!(
        result.content[0].as_text().map(|text| text.text.as_str()),
        Some("state completed")
    );
    assert!(
        elicitation_modes
            .lock()
            .expect("elicitation lock")
            .is_empty()
    );
    assert_eq!(
        *states.lock().expect("states lock"),
        vec![
            None,
            Some("state-1".to_string()),
            Some("state-2".to_string())
        ]
    );
    client.shutdown().await;
    Ok(())
}
