//! HTTP transport for the Codex MCP server.
//!
//! Provides a Streamable HTTP endpoint that can run alongside—or instead
//! of—the default stdin/stdout transport.  The core [`MessageProcessor`]
//! is reused unchanged; only the I/O layer differs.

use std::collections::HashMap;
use std::sync::Arc;

use axum::Json;
use axum::Router;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::Response;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::routing::{get, post};
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, mpsc, oneshot};
use tokio_stream::wrappers::ReceiverStream;
use tracing::info;

use crate::outgoing_message::OutgoingJsonRpcMessage;
use crate::outgoing_message::OutgoingMessage;

/// Shared state across all HTTP request handlers.
#[derive(Clone)]
pub struct HttpState {
    /// Sender half of the channel that feeds incoming JSON-RPC messages into
    /// the [`MessageProcessor`].
    pub incoming_tx: mpsc::Sender<crate::IncomingMessage>,
    /// Map of pending request IDs → oneshot senders that will receive the
    /// JSON-RPC response from the processor.
    pub pending:
        Arc<Mutex<HashMap<String, oneshot::Sender<OutgoingJsonRpcMessage>>>>,
    /// Sender for SSE event broadcast.
    pub sse_tx: tokio::sync::broadcast::Sender<String>,
}

/// Build the axum [`Router`] for the MCP-over-HTTP transport.
pub fn build_router(state: HttpState) -> Router {
    Router::new()
        .route("/mcp", post(handle_mcp_post))
        .route("/mcp", get(handle_mcp_sse))
        .route("/health", get(handle_health))
        .with_state(state)
}

/// `POST /mcp` — accepts a single JSON-RPC request body and returns the
/// response as `application/json`, plus streams notifications via SSE on the
/// same connection using `text/event-stream` if the client `Accept`s it.
async fn handle_mcp_post(
    State(state): State<HttpState>,
    headers: axum::http::HeaderMap,
    body: Bytes,
) -> Response {
    // Parse the incoming JSON-RPC message.
    let msg: crate::IncomingMessage = match serde_json::from_slice(&body) {
        Ok(m) => m,
        Err(e) => {
            let err = JsonRpcErrorResponse {
                jsonrpc: "2.0".into(),
                id: serde_json::Value::Null,
                error: JsonRpcError {
                    code: -32700,
                    message: format!("Parse error: {e}"),
                },
            };
            return (StatusCode::BAD_REQUEST, Json(err)).into_response();
        }
    };

    // Extract the request ID so we can match the response later.
    let request_id = extract_request_id(&msg);

    // Check if the client wants SSE streaming.
    let wants_sse = headers
        .get(axum::http::header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|s| s.contains("text/event-stream"));

    if let Some(ref id) = request_id {
        // Register a oneshot channel so the outgoing interceptor can route the
        // response back to us.
        let (tx, rx) = oneshot::channel();
        state.pending.lock().await.insert(id.clone(), tx);

        // Forward the message to the processor.
        if state.incoming_tx.send(msg).await.is_err() {
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }

        if wants_sse {
            // Return SSE stream: first the response, then ongoing notifications.
            let (sse_tx, sse_rx) =
                mpsc::channel::<Result<Event, std::convert::Infallible>>(64);

            // Wait for the response in a spawned task and push it as SSE event.
            let broadcast_rx = state.sse_tx.subscribe();
            tokio::spawn(async move {
                // 1. Send the response once ready.
                if let Ok(resp) = rx.await {
                    if let Ok(json) = serde_json::to_string(&resp) {
                        let _ = sse_tx
                            .send(Ok(Event::default().data(json)))
                            .await;
                    }
                }
                // 2. Forward broadcast notifications.
                let mut broadcast_rx = broadcast_rx;
                loop {
                    match broadcast_rx.recv().await {
                        Ok(data) => {
                            if sse_tx
                                .send(Ok(Event::default().data(data)))
                                .await
                                .is_err()
                            {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
            });

            return Sse::new(ReceiverStream::new(sse_rx))
                .keep_alive(KeepAlive::default())
                .into_response();
        }

        // Non-SSE: wait for the JSON-RPC response and return it as JSON.
        match tokio::time::timeout(std::time::Duration::from_secs(300), rx).await
        {
            Ok(Ok(resp)) => {
                return (
                    StatusCode::OK,
                    [("content-type", "application/json")],
                    serde_json::to_string(&resp).unwrap_or_default(),
                )
                    .into_response();
            }
            Ok(Err(_)) => {
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
            Err(_) => {
                return StatusCode::GATEWAY_TIMEOUT.into_response();
            }
        }
    }

    // Notification (no request ID) — fire-and-forget.
    if state.incoming_tx.send(msg).await.is_err() {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    StatusCode::ACCEPTED.into_response()
}

/// `GET /mcp` — SSE endpoint for server-initiated notifications.
async fn handle_mcp_sse(State(state): State<HttpState>) -> impl IntoResponse {
    let mut broadcast_rx = state.sse_tx.subscribe();
    let (tx, rx) = mpsc::channel::<Result<Event, std::convert::Infallible>>(64);

    tokio::spawn(async move {
        loop {
            match broadcast_rx.recv().await {
                Ok(data) => {
                    if tx
                        .send(Ok(Event::default().data(data)))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    Sse::new(ReceiverStream::new(rx)).keep_alive(KeepAlive::default())
}

/// `GET /health` — simple health check.
async fn handle_health() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "ok",
        "transport": ["stdio", "http"],
    }))
}

// ----------------------------------------------------------------
// Helpers
// ----------------------------------------------------------------

fn extract_request_id(msg: &crate::IncomingMessage) -> Option<String> {
    use rmcp::model::JsonRpcMessage;
    match msg {
        JsonRpcMessage::Request(r) => Some(format!("{}", r.id)),
        _ => None,
    }
}

/// Intercepts outgoing messages from the [`MessageProcessor`] and routes
/// JSON-RPC responses to their matching HTTP request handler via the pending
/// map.  Notifications are broadcast to all SSE listeners.
pub(crate) async fn outgoing_http_interceptor(
    mut outgoing_rx: mpsc::UnboundedReceiver<OutgoingMessage>,
    pending: Arc<Mutex<HashMap<String, oneshot::Sender<OutgoingJsonRpcMessage>>>>,
    sse_tx: tokio::sync::broadcast::Sender<String>,
    // Also write to stdout when running in dual mode.
    write_stdout: bool,
    // Optional A2A notification broadcast — forwards all notifications.
    a2a_notif_tx: Option<tokio::sync::broadcast::Sender<String>>,
) {
    let mut stdout = if write_stdout {
        Some(tokio::io::stdout())
    } else {
        None
    };

    while let Some(outgoing_message) = outgoing_rx.recv().await {
        let msg: OutgoingJsonRpcMessage = outgoing_message.into();

        // Try to match to a pending HTTP request.
        let id_str = extract_outgoing_id(&msg);
        let mut routed_to_http = false;

        if let Some(id) = id_str {
            let mut map = pending.lock().await;
            if let Some(tx) = map.remove(&id) {
                let _ = tx.send(msg.clone());
                routed_to_http = true;
            }
        }

        // Broadcast notifications to SSE and/or write to stdout.
        if let Ok(json) = serde_json::to_string(&msg) {
            if !routed_to_http {
                // Notification — broadcast to SSE listeners.
                let _ = sse_tx.send(json.clone());
            }

            // Forward to A2A broadcast channel if enabled.
            if let Some(ref a2a_tx) = a2a_notif_tx {
                let _ = a2a_tx.send(json.clone());
            }

            // Dual mode: also write to stdout.
            if let Some(ref mut out) = stdout {
                use tokio::io::AsyncWriteExt;
                let _ = out.write_all(json.as_bytes()).await;
                let _ = out.write_all(b"\n").await;
            }
        }
    }

    info!("HTTP outgoing interceptor exited");
}

fn extract_outgoing_id(msg: &OutgoingJsonRpcMessage) -> Option<String> {
    // OutgoingJsonRpcMessage is an enum; we need to match on it.
    // For now, serialize and extract "id" field.
    if let Ok(v) = serde_json::to_value(msg) {
        if let Some(id) = v.get("id") {
            if !id.is_null() {
                return Some(id.to_string());
            }
        }
    }
    None
}

// ----------------------------------------------------------------
// JSON-RPC error types for HTTP error responses.
// ----------------------------------------------------------------

#[derive(Serialize, Deserialize)]
struct JsonRpcErrorResponse {
    jsonrpc: String,
    id: serde_json::Value,
    error: JsonRpcError,
}

#[derive(Serialize, Deserialize)]
struct JsonRpcError {
    code: i32,
    message: String,
}
