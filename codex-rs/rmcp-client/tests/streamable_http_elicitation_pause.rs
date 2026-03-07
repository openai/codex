use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use axum::Router;
use axum::body::Body;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::http::StatusCode;
use axum::http::header::CONTENT_TYPE;
use axum::response::IntoResponse;
use axum::response::Response;
use axum::routing::post;
use codex_rmcp_client::ElicitationAction;
use codex_rmcp_client::ElicitationResponse;
use codex_rmcp_client::OAuthCredentialsStoreMode;
use codex_rmcp_client::RmcpClient;
use futures::FutureExt as _;
use futures::StreamExt as _;
use futures::stream;
use pretty_assertions::assert_eq;
use rmcp::model::ClientCapabilities;
use rmcp::model::CreateElicitationRequestParams;
use rmcp::model::ElicitationCapability;
use rmcp::model::FormElicitationCapability;
use rmcp::model::Implementation;
use rmcp::model::InitializeRequestParams;
use rmcp::model::NumberOrString;
use rmcp::model::ProtocolVersion;
use serde_json::Value;
use serde_json::json;
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tokio::time::sleep;

const SESSION_ID: &str = "session-1";
const ELICITATION_ID: &str = "elicitation-1";
const SSE_EVENT_ID: &str = "approval-event-1";

#[derive(Debug, Clone, PartialEq, Eq)]
struct GetRequestRecord {
    session_id: Option<String>,
    last_event_id: Option<String>,
}

#[derive(Debug, Clone)]
struct PostRequestRecord {
    session_id: Option<String>,
    body: Value,
}

#[derive(Default)]
struct ApprovalFlowState {
    get_requests: Mutex<Vec<GetRequestRecord>>,
    post_requests: Mutex<Vec<PostRequestRecord>>,
    observed_elicitation: Mutex<Option<(NumberOrString, CreateElicitationRequestParams)>>,
}

fn init_params() -> InitializeRequestParams {
    InitializeRequestParams {
        meta: None,
        capabilities: ClientCapabilities {
            experimental: None,
            extensions: None,
            roots: None,
            sampling: None,
            elicitation: Some(ElicitationCapability {
                form: Some(FormElicitationCapability {
                    schema_validation: None,
                }),
                url: None,
            }),
            tasks: None,
        },
        client_info: Implementation {
            name: "codex-test".into(),
            version: "0.0.0-test".into(),
            title: Some("Codex rmcp elicitation pause test".into()),
            description: None,
            icons: None,
            website_url: None,
        },
        protocol_version: ProtocolVersion::V_2025_06_18,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn streamable_http_approval_elicitation_pause_flow_records_current_behavior() -> Result<()> {
    let state = Arc::new(ApprovalFlowState::default());
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;

    let router = Router::new()
        .route("/mcp", post(mcp_post).get(mcp_get).delete(mcp_delete))
        .with_state(Arc::clone(&state));

    let server = tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });

    let client = Arc::new(
        RmcpClient::new_streamable_http_client(
            "approval-test",
            &format!("http://{addr}/mcp"),
            Some("test-bearer".to_string()),
            None,
            None,
            OAuthCredentialsStoreMode::File,
        )
        .await?,
    );

    let state_for_elicitation = Arc::clone(&state);
    client
        .initialize(
            init_params(),
            Some(Duration::from_secs(2)),
            Box::new(move |request_id, request| {
                let state_for_elicitation = Arc::clone(&state_for_elicitation);
                async move {
                    {
                        let mut observed = state_for_elicitation.observed_elicitation.lock().await;
                        *observed = Some((request_id, request));
                    }
                    Ok(ElicitationResponse {
                        action: ElicitationAction::Accept,
                        content: Some(json!({
                            "approved": true,
                        })),
                        meta: None,
                    })
                }
                .boxed()
            }),
        )
        .await?;

    let client_for_call = Arc::clone(&client);
    let call_task = tokio::spawn(async move {
        client_for_call
            .call_tool(
                "approval_required".to_string(),
                Some(json!({})),
                Some(Duration::from_millis(250)),
            )
            .await
    });

    wait_until(Duration::from_secs(1), || {
        let state = Arc::clone(&state);
        async move {
            state.observed_elicitation.lock().await.is_some()
                && state
                    .post_requests
                    .lock()
                    .await
                    .iter()
                    .any(|record| is_elicitation_response(&record.body))
        }
    })
    .await?;

    sleep(Duration::from_millis(100)).await;

    let call_error = call_task
        .await?
        .expect_err("tools/call should still be awaiting a result");
    assert!(
        call_error
            .to_string()
            .contains("timed out awaiting tools/call"),
        "expected tools/call timeout after accepted elicitation, got: {call_error:#}",
    );

    let observed_elicitation = state.observed_elicitation.lock().await.clone();
    assert_eq!(
        observed_elicitation,
        Some((
            NumberOrString::String(ELICITATION_ID.to_string().into()),
            CreateElicitationRequestParams::FormElicitationParams {
                meta: None,
                message: "Approve this tool call?".to_string(),
                requested_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "approved": { "type": "boolean" }
                    },
                    "required": ["approved"],
                    "additionalProperties": false
                }))?,
            }
        ))
    );

    let post_requests = state.post_requests.lock().await.clone();
    let elicitation_response = post_requests
        .iter()
        .find(|record| is_elicitation_response(&record.body))
        .expect("elicitation response POST");
    assert_eq!(elicitation_response.session_id.as_deref(), Some(SESSION_ID));
    assert_eq!(
        elicitation_response.body,
        json!({
            "jsonrpc": "2.0",
            "id": ELICITATION_ID,
            "result": {
                "action": "accept",
                "content": {
                    "approved": true
                }
            }
        })
    );

    let get_requests = state.get_requests.lock().await.clone();
    assert_eq!(
        get_requests.first(),
        Some(&GetRequestRecord {
            session_id: Some(SESSION_ID.to_string()),
            last_event_id: None,
        })
    );
    assert!(
        get_requests
            .iter()
            .any(|record| { record.last_event_id.as_deref() == Some(SSE_EVENT_ID) }),
        "expected clean close to trigger a reconnect with Last-Event-ID",
    );

    server.abort();
    let _ = server.await;
    Ok(())
}

async fn mcp_post(
    State(state): State<Arc<ApprovalFlowState>>,
    headers: HeaderMap,
    body: String,
) -> Response {
    let json: Value = match serde_json::from_str(&body) {
        Ok(json) => json,
        Err(error) => panic!("invalid JSON-RPC request: {error}: {body}"),
    };
    let session_id = headers
        .get("mcp-session-id")
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);

    state.post_requests.lock().await.push(PostRequestRecord {
        session_id,
        body: json.clone(),
    });

    let method = json.get("method").and_then(Value::as_str);
    match method {
        Some("initialize") => json_response(
            json!({
                "jsonrpc": "2.0",
                "id": match json.get("id").cloned() {
                    Some(id) => id,
                    None => panic!("initialize request missing id: {json}"),
                },
                "result": {
                    "protocolVersion": "2025-06-18",
                    "capabilities": {
                        "tools": {},
                        "elicitation": {}
                    },
                    "serverInfo": {
                        "name": "approval-test-server",
                        "version": "0.0.0-test"
                    }
                }
            }),
            Some(SESSION_ID),
        ),
        Some("notifications/initialized") => StatusCode::ACCEPTED.into_response(),
        Some("tools/call") => match Response::builder()
            .status(StatusCode::OK)
            .header(CONTENT_TYPE, "text/event-stream")
            .header("mcp-session-id", SESSION_ID)
            .body(Body::from(format!(
                "id: {SSE_EVENT_ID}\nretry: 25\ndata: {}\n\n",
                json!({
                    "jsonrpc": "2.0",
                    "id": ELICITATION_ID,
                    "method": "elicitation/create",
                    "params": {
                        "message": "Approve this tool call?",
                        "requestedSchema": {
                            "type": "object",
                            "properties": {
                                "approved": { "type": "boolean" }
                            },
                            "required": ["approved"],
                            "additionalProperties": false
                        }
                    }
                })
            ))) {
            Ok(response) => response,
            Err(error) => panic!("failed to build SSE response: {error}"),
        },
        None if is_elicitation_response(&json) => StatusCode::ACCEPTED.into_response(),
        other => panic!("unexpected POST method: {other:?} body={json}"),
    }
}

async fn mcp_get(State(state): State<Arc<ApprovalFlowState>>, headers: HeaderMap) -> Response {
    state.get_requests.lock().await.push(GetRequestRecord {
        session_id: headers
            .get("mcp-session-id")
            .and_then(|value| value.to_str().ok())
            .map(str::to_string),
        last_event_id: headers
            .get("last-event-id")
            .and_then(|value| value.to_str().ok())
            .map(str::to_string),
    });

    let body_stream =
        stream::once(async { Ok::<Bytes, Infallible>(Bytes::from_static(b": keep-alive\n\n")) })
            .chain(stream::pending::<Result<Bytes, Infallible>>());

    match Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, "text/event-stream")
        .body(Body::from_stream(body_stream))
    {
        Ok(response) => response,
        Err(error) => panic!("failed to build GET SSE response: {error}"),
    }
}

async fn mcp_delete() -> Response {
    StatusCode::NO_CONTENT.into_response()
}

fn json_response(body: Value, session_id: Option<&str>) -> Response {
    let mut response = (StatusCode::OK, axum::Json(body)).into_response();
    if let Some(session_id) = session_id {
        let session_id = match session_id.parse() {
            Ok(session_id) => session_id,
            Err(error) => panic!("invalid session id header {session_id}: {error}"),
        };
        response.headers_mut().insert("mcp-session-id", session_id);
    }
    response
}

fn is_elicitation_response(body: &Value) -> bool {
    body.get("method").is_none()
        && body.get("id") == Some(&Value::String(ELICITATION_ID.to_string()))
        && body.get("result").is_some()
}

async fn wait_until<F, Fut>(timeout: Duration, predicate: F) -> Result<()>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    tokio::time::timeout(timeout, async {
        loop {
            if predicate().await {
                break;
            }
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await?;
    Ok(())
}
