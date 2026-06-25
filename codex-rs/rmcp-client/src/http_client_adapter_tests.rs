use std::collections::HashMap;
use std::convert::Infallible;
use std::num::NonZeroUsize;
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::Response;
use axum::http::StatusCode;
use axum::http::header::CONTENT_TYPE;
use axum::routing::post;
use bytes::Bytes;
use codex_exec_server::HttpClient;
use codex_exec_server::ReqwestHttpClient;
use futures::StreamExt;
use futures::stream;
use reqwest::header::HeaderMap;
use rmcp::model::ClientJsonRpcMessage;
use rmcp::model::JsonRpcMessage;
use rmcp::model::ServerJsonRpcMessage;
use rmcp::transport::streamable_http_client::StreamableHttpClient;
use rmcp::transport::streamable_http_client::StreamableHttpPostResponse;
use serde_json::json;
use tokio::net::TcpListener;

use super::StreamableHttpClientAdapter;

const RESPONSE_LIMIT: usize = 1_024;

#[tokio::test]
async fn post_sse_response_limit_is_inclusive_and_precedes_deserialization() -> anyhow::Result<()> {
    let at_limit_url = spawn_chunked_sse_server(RESPONSE_LIMIT).await?;
    let mut at_limit = post_tools_list(&at_limit_url).await?;
    let event = at_limit.next().await.expect("one tools/list SSE event")?;
    assert!(event.data.is_some());

    let over_limit_url = spawn_chunked_sse_server(RESPONSE_LIMIT + 1).await?;
    let mut over_limit = post_tools_list(&over_limit_url).await?;
    let event = over_limit
        .next()
        .await
        .expect("oversized SSE stream response")?;
    let message: ServerJsonRpcMessage = serde_json::from_str(
        event
            .data
            .as_deref()
            .expect("synthetic response-limit error data"),
    )?;
    let JsonRpcMessage::Error(error) = message else {
        anyhow::bail!("expected synthetic response-limit error, got {message:?}")
    };
    assert!(error.error.message.contains("exceeds the 1024-byte limit"));

    Ok(())
}

async fn post_tools_list(
    url: &str,
) -> anyhow::Result<futures::stream::BoxStream<'static, Result<sse_stream::Sse, sse_stream::Error>>>
{
    let adapter = StreamableHttpClientAdapter::new_with_post_response_body_limit(
        Arc::new(ReqwestHttpClient) as Arc<dyn HttpClient>,
        HeaderMap::new(),
        /*auth_provider*/ None,
        NonZeroUsize::new(RESPONSE_LIMIT).expect("test limit is non-zero"),
    );
    let message: ClientJsonRpcMessage = serde_json::from_value(json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/list",
        "params": {}
    }))?;
    match adapter
        .post_message(
            Arc::from(url),
            message,
            /*session_id*/ None,
            /*auth_token*/ None,
            HashMap::new(),
        )
        .await?
    {
        StreamableHttpPostResponse::Sse(stream, _) => Ok(stream),
        response => anyhow::bail!("expected SSE response, got {response:?}"),
    }
}

async fn spawn_chunked_sse_server(response_bytes: usize) -> anyhow::Result<String> {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
    let addr = listener.local_addr()?;
    let router = Router::new().route(
        "/mcp",
        post(move || async move { chunked_sse_response(response_bytes) }),
    );
    tokio::spawn(async move {
        if let Err(error) = axum::serve(listener, router).await {
            panic!("SSE response-limit test server failed: {error}");
        }
    });
    Ok(format!("http://{addr}/mcp"))
}

fn chunked_sse_response(response_bytes: usize) -> Response<Body> {
    let event =
        b"event: message\ndata: {\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"tools\":[]}}\n\n";
    let padding_len = response_bytes - event.len();
    let mut body = Vec::with_capacity(response_bytes);
    body.push(b':');
    body.resize(padding_len - 1, b' ');
    body.push(b'\n');
    body.extend(event);
    assert_eq!(body.len(), response_bytes);

    let split_at = RESPONSE_LIMIT - 16;
    let chunks = vec![
        Ok::<_, Infallible>(Bytes::copy_from_slice(&body[..split_at])),
        Ok(Bytes::copy_from_slice(&body[split_at..])),
    ];
    Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, "text/event-stream")
        .body(Body::from_stream(stream::iter(chunks).then(
            |chunk| async {
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                chunk
            },
        )))
        .expect("valid chunked SSE response")
}
