use std::collections::HashMap;
use std::sync::Arc;

use axum::Json;
use axum::Router;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::post;
use codex_exec_server::Environment;
use pretty_assertions::assert_eq;
use reqwest::header::HeaderMap;
use rmcp::model::ClientJsonRpcMessage;
use rmcp::model::ClientRequest;
use rmcp::model::ErrorData;
use rmcp::model::JsonRpcError;
use rmcp::model::PingRequest;
use rmcp::model::RequestId;
use rmcp::model::ServerJsonRpcMessage;
use rmcp::transport::streamable_http_client::StreamableHttpClient;
use rmcp::transport::streamable_http_client::StreamableHttpError;
use rmcp::transport::streamable_http_client::StreamableHttpPostResponse;
use serde_json::json;
use tokio::net::TcpListener;

use super::*;

#[tokio::test]
async fn post_message_parses_json_error_body_before_retryable_status() -> anyhow::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let app = Router::new().route("/", post(json_error_response));
    let server = tokio::spawn(async move { axum::serve(listener, app).await });

    let adapter = StreamableHttpClientAdapter::new(
        Environment::default_for_tests().get_http_client(),
        HeaderMap::new(),
        /*auth_provider*/ None,
    );
    let request = ClientJsonRpcMessage::request(
        ClientRequest::PingRequest(PingRequest::default()),
        RequestId::Number(1),
    );

    let response = adapter
        .post_message(
            Arc::from(format!("http://{address}/")),
            request,
            /*session_id*/ None,
            /*auth_token*/ None,
            HashMap::new(),
        )
        .await?;

    server.abort();

    let StreamableHttpPostResponse::Json(message, _session_id) = response else {
        panic!("expected JSON response");
    };
    let ServerJsonRpcMessage::Error(error) = message else {
        panic!("expected JSON-RPC error");
    };
    assert_eq!(
        error,
        JsonRpcError::new(
            /*id*/ Some(RequestId::Number(1)),
            ErrorData::internal_error("transient json error", /*data*/ None),
        )
    );

    Ok(())
}

#[tokio::test]
async fn post_message_retries_non_json_rpc_json_error_body() -> anyhow::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let app = Router::new().route("/", post(non_json_rpc_json_error_response));
    let server = tokio::spawn(async move { axum::serve(listener, app).await });

    let adapter = StreamableHttpClientAdapter::new(
        Environment::default_for_tests().get_http_client(),
        HeaderMap::new(),
        /*auth_provider*/ None,
    );
    let request = ClientJsonRpcMessage::request(
        ClientRequest::PingRequest(PingRequest::default()),
        RequestId::Number(1),
    );

    let result = adapter
        .post_message(
            Arc::from(format!("http://{address}/")),
            request,
            /*session_id*/ None,
            /*auth_token*/ None,
            HashMap::new(),
        )
        .await;

    server.abort();

    let Err(StreamableHttpError::Client(StreamableHttpClientAdapterError::RetryableHttpStatus(
        status,
    ))) = result
    else {
        panic!("expected retryable HTTP status error");
    };
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE.as_u16());

    Ok(())
}

async fn json_error_response() -> impl IntoResponse {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        [(CONTENT_TYPE, JSON_MIME_TYPE)],
        Json(json!({
            "jsonrpc": "2.0",
            "id": 1,
            "error": {
                "code": -32603,
                "message": "transient json error",
            },
        })),
    )
}

async fn non_json_rpc_json_error_response() -> impl IntoResponse {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        [(CONTENT_TYPE, JSON_MIME_TYPE)],
        Json(json!({
            "error": "service temporarily unavailable",
        })),
    )
}
