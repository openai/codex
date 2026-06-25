mod streamable_http_test_support;

use std::convert::Infallible;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;

use anyhow::Context;
use axum::Json;
use axum::Router;
use axum::body::Body;
use axum::extract::State;
use axum::http::HeaderValue;
use axum::http::Response;
use axum::http::StatusCode;
use axum::http::header::CONTENT_LENGTH;
use axum::http::header::CONTENT_TYPE;
use axum::routing::post;
use bytes::Bytes;
use codex_config::types::AuthKeyringBackendKind;
use codex_config::types::OAuthCredentialsStoreMode;
use codex_exec_server::Environment;
use codex_rmcp_client::RmcpClient;
use futures::StreamExt;
use futures::stream;
use serde_json::Value;
use serde_json::json;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

use streamable_http_test_support::initialize_client;

const POST_RESPONSE_LIMIT: usize = 1_024;

#[derive(Clone, Copy)]
enum ResponseEncoding {
    Json,
    Sse,
}

#[derive(Clone, Copy)]
struct ServerState {
    list_response_bytes: usize,
    response_encoding: ResponseEncoding,
}

struct TestServer {
    url: String,
    task: JoinHandle<()>,
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

#[tokio::test]
async fn bounded_streamable_http_accepts_json_at_the_byte_limit() -> anyhow::Result<()> {
    let server = spawn_server(POST_RESPONSE_LIMIT, ResponseEncoding::Json).await?;
    let client = bounded_client(&server.url).await?;

    let tools = client.list_tools(/*params*/ None, /*timeout*/ None).await?;
    assert!(tools.tools.is_empty());
    client.shutdown().await;

    Ok(())
}

#[tokio::test]
async fn bounded_streamable_http_rejects_chunked_json_over_the_byte_limit() -> anyhow::Result<()> {
    let server = spawn_server(POST_RESPONSE_LIMIT + 1, ResponseEncoding::Json).await?;
    let client = bounded_client(&server.url).await?;

    let error = client
        .list_tools(/*params*/ None, /*timeout*/ None)
        .await
        .expect_err("chunked tools/list response should exceed the byte limit");
    let message = format!("{error:#}");
    assert!(
        message.contains("exceeds the 1024-byte limit"),
        "unexpected response-limit error: {message}"
    );
    client.shutdown().await;

    Ok(())
}

#[tokio::test]
async fn bounded_streamable_http_sse_overflow_resolves_the_pending_request() -> anyhow::Result<()> {
    let server = spawn_server(POST_RESPONSE_LIMIT + 1, ResponseEncoding::Sse).await?;
    let client = bounded_client(&server.url).await?;

    let error = tokio::time::timeout(
        Duration::from_secs(2),
        client.list_tools(/*params*/ None, /*timeout*/ None),
    )
    .await
    .context("oversized SSE tools/list response left the request pending")?
    .expect_err("chunked SSE tools/list response should exceed the byte limit");
    let message = format!("{error:#}");
    assert!(
        message.contains("exceeds the 1024-byte limit"),
        "unexpected SSE response-limit error: {message}"
    );
    client.shutdown().await;

    Ok(())
}

#[tokio::test]
async fn existing_streamable_http_constructor_remains_unbounded() -> anyhow::Result<()> {
    let server = spawn_server(POST_RESPONSE_LIMIT + 1, ResponseEncoding::Json).await?;
    let client = unbounded_client(&server.url).await?;

    let tools = client.list_tools(/*params*/ None, /*timeout*/ None).await?;
    assert!(tools.tools.is_empty());
    client.shutdown().await;

    Ok(())
}

#[tokio::test]
async fn bounded_initialize_rejects_declared_oversize_without_retry() -> anyhow::Result<()> {
    let (server, initialize_requests) = spawn_declared_oversize_initialize_server().await?;
    let client = bounded_uninitialized_client(&server.url).await?;

    let error = tokio::time::timeout(Duration::from_secs(1), initialize_client(&client))
        .await
        .context("declared oversized initialize response body was read")?
        .expect_err("declared oversized initialize response should be rejected");
    let message = format!("{error:#}");
    assert!(
        message.contains("exceeds the 1024-byte limit"),
        "unexpected initialize response-limit error: {message}"
    );
    assert_eq!(
        initialize_requests.load(Ordering::Acquire),
        1,
        "response-limit failures must not retry initialize"
    );
    client.shutdown().await;

    Ok(())
}

async fn bounded_client(url: &str) -> anyhow::Result<RmcpClient> {
    let client = bounded_uninitialized_client(url).await?;
    initialize_client(&client).await?;
    Ok(client)
}

async fn bounded_uninitialized_client(url: &str) -> anyhow::Result<RmcpClient> {
    let client = RmcpClient::new_streamable_http_client_with_post_response_body_limit(
        "bounded-streamable-http-test",
        url,
        Some("test-bearer".to_string()),
        /*http_headers*/ None,
        /*env_http_headers*/ None,
        OAuthCredentialsStoreMode::File,
        AuthKeyringBackendKind::default(),
        Environment::default_for_tests().get_http_client(),
        /*auth_provider*/ None,
        NonZeroUsize::new(POST_RESPONSE_LIMIT).context("test limit must be non-zero")?,
    )
    .await?;
    Ok(client)
}

async fn unbounded_client(url: &str) -> anyhow::Result<RmcpClient> {
    let client = RmcpClient::new_streamable_http_client(
        "unbounded-streamable-http-test",
        url,
        Some("test-bearer".to_string()),
        /*http_headers*/ None,
        /*env_http_headers*/ None,
        OAuthCredentialsStoreMode::File,
        AuthKeyringBackendKind::default(),
        Environment::default_for_tests().get_http_client(),
        /*auth_provider*/ None,
    )
    .await?;
    initialize_client(&client).await?;
    Ok(client)
}

async fn spawn_server(
    list_response_bytes: usize,
    response_encoding: ResponseEncoding,
) -> anyhow::Result<TestServer> {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
    let addr = listener.local_addr()?;
    let router = Router::new()
        .route("/mcp", post(handle_mcp_post))
        .with_state(ServerState {
            list_response_bytes,
            response_encoding,
        });
    let task = tokio::spawn(async move {
        if let Err(error) = axum::serve(listener, router).await {
            panic!("bounded Streamable HTTP test server failed: {error}");
        }
    });
    Ok(TestServer {
        url: format!("http://{addr}/mcp"),
        task,
    })
}

async fn spawn_declared_oversize_initialize_server()
-> anyhow::Result<(TestServer, Arc<AtomicUsize>)> {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
    let addr = listener.local_addr()?;
    let initialize_requests = Arc::new(AtomicUsize::new(0));
    let router = Router::new().route(
        "/mcp",
        post({
            let initialize_requests = Arc::clone(&initialize_requests);
            move |Json(request): Json<Value>| {
                let initialize_requests = Arc::clone(&initialize_requests);
                async move {
                    assert_eq!(
                        request.get("method").and_then(Value::as_str),
                        Some("initialize")
                    );
                    initialize_requests.fetch_add(1, Ordering::AcqRel);
                    let mut response = Response::new(Body::from_stream(stream::pending::<
                        Result<Bytes, Infallible>,
                    >()));
                    response
                        .headers_mut()
                        .insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
                    response
                        .headers_mut()
                        .insert(CONTENT_LENGTH, HeaderValue::from(POST_RESPONSE_LIMIT + 1));
                    response
                }
            }
        }),
    );
    let task = tokio::spawn(async move {
        if let Err(error) = axum::serve(listener, router).await {
            panic!("declared oversized initialize test server failed: {error}");
        }
    });
    Ok((
        TestServer {
            url: format!("http://{addr}/mcp"),
            task,
        },
        initialize_requests,
    ))
}

async fn handle_mcp_post(
    State(state): State<ServerState>,
    Json(request): Json<Value>,
) -> Response<Body> {
    let method = request.get("method").and_then(Value::as_str);
    match method {
        Some("initialize") => json_response(json!({
            "jsonrpc": "2.0",
            "id": request.get("id").cloned().unwrap_or(Value::Null),
            "result": {
                "protocolVersion": "2025-06-18",
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "response-limit-test", "version": "1" }
            }
        })),
        Some("notifications/initialized") => response(StatusCode::ACCEPTED, Body::empty()),
        Some("tools/list") => {
            let response = json!({
                "jsonrpc": "2.0",
                "id": request.get("id").cloned().unwrap_or(Value::Null),
                "result": { "tools": [] }
            });
            chunked_response(response, state)
        }
        _ => response(StatusCode::BAD_REQUEST, Body::from("unexpected MCP method")),
    }
}

fn response(status: StatusCode, body: Body) -> Response<Body> {
    let mut response = Response::new(body);
    *response.status_mut() = status;
    response
}

fn json_response(value: Value) -> Response<Body> {
    let mut response = Response::new(Body::from(value.to_string()));
    response
        .headers_mut()
        .insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    response
}

fn chunked_response(value: Value, state: ServerState) -> Response<Body> {
    let json = value.to_string().into_bytes();
    let (content_type, mut body) = match state.response_encoding {
        ResponseEncoding::Json => ("application/json", json),
        ResponseEncoding::Sse => {
            let event = [b"event: message\ndata: ".as_slice(), &json, b"\n\n"].concat();
            let padding_len = state.list_response_bytes - event.len();
            let mut body = Vec::with_capacity(state.list_response_bytes);
            body.push(b':');
            body.resize(padding_len - 1, b' ');
            body.push(b'\n');
            body.extend(event);
            ("text/event-stream", body)
        }
    };
    assert!(body.len() <= state.list_response_bytes);
    body.resize(state.list_response_bytes, b' ');

    let split_at = POST_RESPONSE_LIMIT - 16;
    let chunks = vec![
        Ok::<_, Infallible>(Bytes::copy_from_slice(&body[..split_at])),
        Ok(Bytes::copy_from_slice(&body[split_at..])),
    ];
    let mut response = Response::new(Body::from_stream(stream::iter(chunks).then(
        |chunk| async {
            tokio::time::sleep(Duration::from_millis(10)).await;
            chunk
        },
    )));
    response
        .headers_mut()
        .insert(CONTENT_TYPE, HeaderValue::from_static(content_type));
    response
}
