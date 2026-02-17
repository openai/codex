//! A2A RC v1 HTTP server using Axum.
//!
//! Mirrors `a2a-js/src/server/express/` and `a2a-js/src/server/request_handler/`.
//!
//! Uses [`AgentExecutor`] for agent logic, [`TaskStore`] for persistence,
//! and [`EventBus`] for streaming events.

use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::Arc;

use crate::error::A2AError;
use crate::event::{EventBus, ExecutionEvent};
use crate::executor::{AgentExecutor, RequestContext};
use crate::store::TaskStore;
use crate::types::*;
use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::sse::{Event, Sse},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use futures::stream::{self, Stream};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::Mutex;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::BroadcastStream;

// ============================================================
// Server state
// ============================================================

/// Shared state for the A2A server.
pub struct A2AServerState<E: AgentExecutor, S: TaskStore> {
    pub executor: Arc<E>,
    pub store: Arc<S>,
    pub base_url: String,
    /// Active task cancellation tokens, keyed by task ID.
    pub cancel_tokens: Arc<Mutex<HashMap<String, tokio::sync::watch::Sender<bool>>>>,
}

impl<E: AgentExecutor, S: TaskStore> Clone for A2AServerState<E, S> {
    fn clone(&self) -> Self {
        Self {
            executor: Arc::clone(&self.executor),
            store: Arc::clone(&self.store),
            base_url: self.base_url.clone(),
            cancel_tokens: Arc::clone(&self.cancel_tokens),
        }
    }
}

// ============================================================
// A2AServer builder
// ============================================================

/// Builder for the A2A HTTP server.
pub struct A2AServer<E: AgentExecutor, S: TaskStore> {
    executor: Arc<E>,
    store: Arc<S>,
    addr: String,
    base_url: Option<String>,
}

impl<E: AgentExecutor, S: TaskStore> A2AServer<E, S> {
    /// Create a new server with the given executor and store.
    pub fn new(executor: E, store: S) -> Self {
        Self {
            executor: Arc::new(executor),
            store: Arc::new(store),
            addr: "0.0.0.0:5000".to_string(),
            base_url: None,
        }
    }

    /// Set the bind address (default: `0.0.0.0:5000`).
    pub fn bind(mut self, addr: impl Into<String>) -> Self {
        self.addr = addr.into();
        self
    }

    /// Set the base URL for the agent card (default: derived from addr).
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = Some(url.into());
        self
    }

    /// Build the Axum router without starting the server.
    pub fn router(&self) -> Router {
        let base_url = self
            .base_url
            .clone()
            .unwrap_or_else(|| format!("http://{}", self.addr));

        let state = A2AServerState {
            executor: Arc::clone(&self.executor),
            store: Arc::clone(&self.store),
            base_url,
            cancel_tokens: Arc::new(Mutex::new(HashMap::new())),
        };

        Router::new()
            .route("/", post(handle_jsonrpc::<E, S>))
            .route(
                "/.well-known/agent-card.json",
                get(handle_agent_card_v03::<E, S>),
            )
            .route("/.well-known/agent.json", get(handle_agent_card::<E, S>))
            .route("/message:send", post(handle_send_message::<E, S>))
            .route("/message:stream", post(handle_stream_message::<E, S>))
            .route(
                "/tasks/{id}",
                get(handle_get_task::<E, S>).post(handle_cancel_task_compat::<E, S>),
            )
            .route("/tasks/{id}/cancel", post(handle_cancel_task::<E, S>))
            .with_state(state)
    }

    /// Run the server (blocks until shutdown).
    pub async fn run(self) -> Result<(), Box<dyn std::error::Error>> {
        let router = self.router();
        let listener = tokio::net::TcpListener::bind(&self.addr).await?;
        tracing::info!("A2A server listening on {}", self.addr);
        axum::serve(listener, router).await?;
        Ok(())
    }
}

// ============================================================
// Route handlers
// ============================================================

/// `GET /.well-known/agent.json`
async fn handle_agent_card<E: AgentExecutor, S: TaskStore>(
    State(state): State<A2AServerState<E, S>>,
) -> Json<AgentCard> {
    Json(state.executor.agent_card(&state.base_url))
}

/// Compatibility endpoint for A2A 0.3 JSON-RPC stacks.
///
/// Returns a v0.3-style card shape at `/.well-known/agent-card.json`.
async fn handle_agent_card_v03<E: AgentExecutor, S: TaskStore>(
    State(state): State<A2AServerState<E, S>>,
) -> Json<Value> {
    let card = state.executor.agent_card(&state.base_url);
    let url = card
        .supported_interfaces
        .first()
        .map(|iface| iface.url.clone())
        .unwrap_or_else(|| format!("{}/", state.base_url.trim_end_matches('/')));
    Json(json!({
        "name": card.name,
        "description": card.description,
        "url": url,
        "provider": card.provider,
        "version": card.version,
        "protocolVersion": "0.3.0",
        "capabilities": {
            "streaming": card.capabilities.streaming.unwrap_or(false),
            "pushNotifications": card.capabilities.push_notifications.unwrap_or(false),
            "stateTransitionHistory": false
        },
        "defaultInputModes": card.default_input_modes,
        "defaultOutputModes": card.default_output_modes,
        "skills": card.skills,
        "supportsAuthenticatedExtendedCard": card.capabilities.extended_agent_card.unwrap_or(false)
    }))
}

#[derive(Debug, Deserialize)]
struct JsonRpcEnvelope {
    jsonrpc: String,
    method: String,
    #[serde(default)]
    params: Value,
    #[serde(default = "jsonrpc_null_id")]
    id: Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JsonRpcTaskQueryParams {
    id: String,
    #[allow(dead_code)]
    history_length: Option<i32>,
}

#[derive(Debug, Deserialize)]
struct JsonRpcTaskIdParams {
    id: String,
}

fn jsonrpc_null_id() -> Value {
    Value::Null
}

fn jsonrpc_ok<T: serde::Serialize>(id: Value, result: T) -> Response {
    Json(json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    }))
    .into_response()
}

fn jsonrpc_err(id: Value, err: A2AError, status: StatusCode) -> Response {
    (
        status,
        Json(json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": err.to_jsonrpc_error()
        })),
    )
        .into_response()
}

fn jsonrpc_sse_single(
    id: Value,
    result: Value,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let payload = json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    })
    .to_string();
    Sse::new(stream::once(
        async move { Ok(Event::default().data(payload)) },
    ))
}

/// `POST /` JSON-RPC compatibility endpoint for A2A 0.3 clients.
async fn handle_jsonrpc<E: AgentExecutor, S: TaskStore>(
    State(state): State<A2AServerState<E, S>>,
    Json(body): Json<Value>,
) -> Response {
    let fallback_id = body.get("id").cloned().unwrap_or(Value::Null);
    let envelope: JsonRpcEnvelope = match serde_json::from_value(body) {
        Ok(envelope) => envelope,
        Err(e) => {
            return jsonrpc_err(
                fallback_id,
                A2AError::parse_error(format!("Failed to parse JSON-RPC request: {e}")),
                StatusCode::BAD_REQUEST,
            );
        }
    };

    if envelope.jsonrpc != "2.0" {
        return jsonrpc_err(
            envelope.id,
            A2AError::invalid_request("jsonrpc must be '2.0'"),
            StatusCode::BAD_REQUEST,
        );
    }

    match envelope.method.as_str() {
        "message/send" => {
            let params: SendMessageRequest = match serde_json::from_value(envelope.params) {
                Ok(v) => v,
                Err(e) => {
                    return jsonrpc_err(
                        envelope.id,
                        A2AError::invalid_params(format!("Invalid message/send params: {e}")),
                        StatusCode::BAD_REQUEST,
                    );
                }
            };
            match handle_send_message::<E, S>(State(state), Json(params)).await {
                Ok(Json(result)) => jsonrpc_ok(envelope.id, result),
                Err(err) => jsonrpc_err(envelope.id, err, StatusCode::OK),
            }
        }
        "tasks/get" => {
            let params: JsonRpcTaskQueryParams = match serde_json::from_value(envelope.params) {
                Ok(v) => v,
                Err(e) => {
                    return jsonrpc_err(
                        envelope.id,
                        A2AError::invalid_params(format!("Invalid tasks/get params: {e}")),
                        StatusCode::BAD_REQUEST,
                    );
                }
            };
            match handle_get_task::<E, S>(State(state), Path(params.id)).await {
                Ok(Json(task)) => jsonrpc_ok(envelope.id, task),
                Err(err) => jsonrpc_err(envelope.id, err, StatusCode::OK),
            }
        }
        "tasks/cancel" => {
            let params: JsonRpcTaskIdParams = match serde_json::from_value(envelope.params) {
                Ok(v) => v,
                Err(e) => {
                    return jsonrpc_err(
                        envelope.id,
                        A2AError::invalid_params(format!("Invalid tasks/cancel params: {e}")),
                        StatusCode::BAD_REQUEST,
                    );
                }
            };
            match handle_cancel_task::<E, S>(State(state), Path(params.id)).await {
                Ok(Json(task)) => jsonrpc_ok(envelope.id, task),
                Err(err) => jsonrpc_err(envelope.id, err, StatusCode::OK),
            }
        }
        "message/stream" => {
            let params: SendMessageRequest = match serde_json::from_value(envelope.params) {
                Ok(v) => v,
                Err(e) => {
                    return jsonrpc_err(
                        envelope.id,
                        A2AError::invalid_params(format!("Invalid message/stream params: {e}")),
                        StatusCode::BAD_REQUEST,
                    );
                }
            };
            handle_jsonrpc_stream_message::<E, S>(state, params, envelope.id)
                .await
                .into_response()
        }
        "tasks/resubscribe" => {
            let params: JsonRpcTaskIdParams = match serde_json::from_value(envelope.params) {
                Ok(v) => v,
                Err(e) => {
                    return jsonrpc_err(
                        envelope.id,
                        A2AError::invalid_params(format!("Invalid tasks/resubscribe params: {e}")),
                        StatusCode::BAD_REQUEST,
                    );
                }
            };
            match state.store.load(&params.id).await {
                Ok(Some(task)) => jsonrpc_sse_single(envelope.id, json!(task)).into_response(),
                Ok(None) => jsonrpc_err(
                    envelope.id,
                    A2AError::task_not_found(&params.id),
                    StatusCode::NOT_FOUND,
                ),
                Err(err) => jsonrpc_err(envelope.id, err, StatusCode::INTERNAL_SERVER_ERROR),
            }
        }
        "agent/getAuthenticatedExtendedCard" => jsonrpc_err(
            envelope.id,
            A2AError::unsupported_operation("agent/getAuthenticatedExtendedCard"),
            StatusCode::NOT_IMPLEMENTED,
        ),
        method => jsonrpc_err(
            envelope.id,
            A2AError::method_not_found(method),
            StatusCode::NOT_FOUND,
        ),
    }
}

/// `POST /message:send`
async fn handle_send_message<E: AgentExecutor, S: TaskStore>(
    State(state): State<A2AServerState<E, S>>,
    Json(request): Json<SendMessageRequest>,
) -> Result<Json<SendMessageResponse>, A2AError> {
    let context_id = request
        .message
        .context_id
        .clone()
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    let task_id = uuid::Uuid::new_v4().to_string();
    let event_bus = EventBus::new(16);
    let mut rx = event_bus.subscribe();

    let context = RequestContext {
        request,
        task_id: Some(task_id.clone()),
        context_id,
    };

    // Register cancel token
    let (cancel_tx, _cancel_rx) = tokio::sync::watch::channel(false);
    state
        .cancel_tokens
        .lock()
        .await
        .insert(task_id.clone(), cancel_tx);

    // Execute in background.
    let executor = Arc::clone(&state.executor);
    let cancel_tokens = Arc::clone(&state.cancel_tokens);
    let task_id_clone = task_id.clone();
    tokio::spawn(async move {
        if let Err(e) = executor.execute(context, &event_bus).await {
            tracing::error!("AgentExecutor error: {e}");
        }
        // Cleanup cancel token
        cancel_tokens.lock().await.remove(&task_id_clone);
    });

    // Non-streaming mode still may receive intermediate streaming events first.
    // Keep consuming until a terminal Task or Message is received.
    loop {
        match rx.recv().await {
            Ok(ExecutionEvent::Task(task)) => {
                // Save to store.
                state.store.save(task.clone()).await?;
                return Ok(Json(SendMessageResponse::Task(task)));
            }
            Ok(ExecutionEvent::Message(message)) => {
                return Ok(Json(SendMessageResponse::Message(message)));
            }
            Ok(ExecutionEvent::StatusUpdate(_) | ExecutionEvent::ArtifactUpdate(_)) => {
                continue;
            }
            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                continue;
            }
            Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                return Err(A2AError::internal_error(
                    "Executor finished without terminal event",
                ));
            }
        }
    }
}

/// `POST /message:stream` — SSE streaming of task events.
async fn handle_stream_message<E: AgentExecutor, S: TaskStore>(
    State(state): State<A2AServerState<E, S>>,
    Json(request): Json<SendMessageRequest>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let context_id = request
        .message
        .context_id
        .clone()
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    let task_id = uuid::Uuid::new_v4().to_string();
    let event_bus = EventBus::new(64);
    let rx = event_bus.subscribe();

    let context = RequestContext {
        request,
        task_id: Some(task_id.clone()),
        context_id,
    };

    // Register cancel token
    let (cancel_tx, _cancel_rx) = tokio::sync::watch::channel(false);
    state
        .cancel_tokens
        .lock()
        .await
        .insert(task_id.clone(), cancel_tx);

    // Execute in background.
    let executor = Arc::clone(&state.executor);
    let _store = Arc::clone(&state.store);
    let cancel_tokens = Arc::clone(&state.cancel_tokens);
    let task_id_clone = task_id.clone();
    tokio::spawn(async move {
        if let Err(e) = executor.execute(context, &event_bus).await {
            tracing::error!("AgentExecutor error: {e}");
        }
        cancel_tokens.lock().await.remove(&task_id_clone);
    });

    // Convert broadcast receiver into SSE stream.
    let stream = BroadcastStream::new(rx).filter_map(move |result| {
        match result {
            Ok(event) => {
                let sse_event = match &event {
                    ExecutionEvent::Task(task) => {
                        Event::default()
                            .event("task")
                            .json_data(task)
                            .ok()
                    }
                    ExecutionEvent::Message(msg) => {
                        Event::default()
                            .event("message")
                            .json_data(msg)
                            .ok()
                    }
                    ExecutionEvent::StatusUpdate(update) => {
                        Event::default()
                            .event("status")
                            .json_data(update)
                            .ok()
                    }
                    ExecutionEvent::ArtifactUpdate(update) => {
                        Event::default()
                            .event("artifact")
                            .json_data(update)
                            .ok()
                    }
                };
                // If this is a terminal event, we'll close after sending.
                let is_terminal = matches!(&event,
                    ExecutionEvent::Task(t) if matches!(t.status.state, TaskState::Completed | TaskState::Failed | TaskState::Canceled)
                );
                match sse_event {
                    Some(e) => {
                        if is_terminal {
                            // Send the event — stream will end naturally after broadcast closes.
                            Some(Ok(e))
                        } else {
                            Some(Ok(e))
                        }
                    }
                    None => None,
                }
            }
            Err(_) => None, // Stream ended
        }
    });

    Sse::new(stream)
}

/// JSON-RPC variant of `message/stream` that wraps each SSE chunk into
/// `{ jsonrpc, id, result }`.
async fn handle_jsonrpc_stream_message<E: AgentExecutor, S: TaskStore>(
    state: A2AServerState<E, S>,
    request: SendMessageRequest,
    request_id: Value,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let context_id = request
        .message
        .context_id
        .clone()
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    let task_id = uuid::Uuid::new_v4().to_string();
    let event_bus = EventBus::new(64);
    let rx = event_bus.subscribe();

    let context = RequestContext {
        request,
        task_id: Some(task_id.clone()),
        context_id,
    };

    // Register cancel token
    let (cancel_tx, _cancel_rx) = tokio::sync::watch::channel(false);
    state
        .cancel_tokens
        .lock()
        .await
        .insert(task_id.clone(), cancel_tx);

    // Execute in background.
    let executor = Arc::clone(&state.executor);
    let cancel_tokens = Arc::clone(&state.cancel_tokens);
    let task_id_clone = task_id.clone();
    tokio::spawn(async move {
        if let Err(e) = executor.execute(context, &event_bus).await {
            tracing::error!("AgentExecutor error: {e}");
        }
        cancel_tokens.lock().await.remove(&task_id_clone);
    });

    let stream = BroadcastStream::new(rx).filter_map(move |result| {
        let request_id = request_id.clone();
        match result {
            Ok(event) => {
                let result_value = match event {
                    ExecutionEvent::Task(task) => json!(task),
                    ExecutionEvent::Message(msg) => json!(msg),
                    ExecutionEvent::StatusUpdate(update) => json!(update),
                    ExecutionEvent::ArtifactUpdate(update) => json!(update),
                };
                let payload = json!({
                    "jsonrpc": "2.0",
                    "id": request_id,
                    "result": result_value
                })
                .to_string();
                Some(Ok(Event::default().data(payload)))
            }
            Err(_) => None,
        }
    });

    Sse::new(stream)
}

/// `GET /tasks/{id}`
async fn handle_get_task<E: AgentExecutor, S: TaskStore>(
    State(state): State<A2AServerState<E, S>>,
    Path(task_id): Path<String>,
) -> Result<Json<Task>, A2AError> {
    match state.store.load(&task_id).await? {
        Some(task) => Ok(Json(task)),
        None => Err(A2AError::task_not_found(&task_id)),
    }
}

/// Compatibility handler for `POST /tasks/{id}:cancel`.
///
/// Axum path params cannot include both a param and literal text in one segment,
/// so `/tasks/{id}:cancel` is matched as `/tasks/{id}` and parsed here.
async fn handle_cancel_task_compat<E: AgentExecutor, S: TaskStore>(
    State(state): State<A2AServerState<E, S>>,
    Path(task_segment): Path<String>,
) -> Result<Json<Task>, A2AError> {
    let Some(task_id) = task_segment.strip_suffix(":cancel") else {
        return Err(A2AError::invalid_params(
            "Expected path format /tasks/{id}:cancel",
        ));
    };
    cancel_task_by_id(state, task_id).await
}

/// `POST /tasks/{id}:cancel`
async fn handle_cancel_task<E: AgentExecutor, S: TaskStore>(
    State(state): State<A2AServerState<E, S>>,
    Path(task_id): Path<String>,
) -> Result<Json<Task>, A2AError> {
    cancel_task_by_id(state, &task_id).await
}

async fn cancel_task_by_id<E: AgentExecutor, S: TaskStore>(
    state: A2AServerState<E, S>,
    task_id: &str,
) -> Result<Json<Task>, A2AError> {
    // Signal cancellation via the watch channel.
    let cancelled = {
        let tokens = state.cancel_tokens.lock().await;
        if let Some(tx) = tokens.get(task_id) {
            let _ = tx.send(true);
            true
        } else {
            false
        }
    };

    if !cancelled {
        return Err(A2AError::task_not_found(&task_id));
    }

    // Also call executor cancel for cleanup.
    let event_bus = EventBus::new(16);
    let _ = state.executor.cancel(task_id, &event_bus).await;

    match state.store.load(&task_id).await? {
        Some(task) => Ok(Json(task)),
        None => Err(A2AError::task_not_found(&task_id)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::InMemoryTaskStore;

    struct StreamingFirstExecutor;

    impl AgentExecutor for StreamingFirstExecutor {
        fn execute(
            &self,
            context: RequestContext,
            event_bus: &EventBus,
        ) -> impl std::future::Future<Output = Result<(), A2AError>> + Send {
            async move {
                let task_id = context.task_id.unwrap_or_else(|| "task-1".to_string());
                let context_id = context.context_id;
                event_bus.publish_status_update(TaskStatusUpdateEvent {
                    task_id: task_id.clone(),
                    context_id: context_id.clone(),
                    status: TaskStatus {
                        state: TaskState::Working,
                        message: Some(Message {
                            message_id: "status-1".to_string(),
                            context_id: Some(context_id.clone()),
                            task_id: Some(task_id.clone()),
                            role: Role::Agent,
                            parts: vec![Part::text("working")],
                            metadata: None,
                            extensions: vec![],
                            reference_task_ids: None,
                        }),
                        timestamp: None,
                    },
                    metadata: None,
                });
                event_bus.publish(ExecutionEvent::Task(completed_task(
                    task_id, context_id, "done",
                )));
                Ok(())
            }
        }

        fn cancel(
            &self,
            _task_id: &str,
            _event_bus: &EventBus,
        ) -> impl std::future::Future<Output = Result<(), A2AError>> + Send {
            async move { Ok(()) }
        }

        fn agent_card(&self, base_url: &str) -> AgentCard {
            AgentCard {
                name: "test-agent".to_string(),
                description: "test".to_string(),
                supported_interfaces: vec![AgentInterface {
                    url: base_url.to_string(),
                    protocol_binding: "HTTP+JSON".to_string(),
                    tenant: None,
                    protocol_version: "1.0".to_string(),
                }],
                provider: None,
                version: "0.0.0".to_string(),
                documentation_url: None,
                capabilities: AgentCapabilities {
                    streaming: Some(true),
                    push_notifications: Some(false),
                    extended_agent_card: None,
                },
                default_input_modes: vec!["text/plain".to_string()],
                default_output_modes: vec!["text/plain".to_string()],
                skills: vec![],
                icon_url: None,
            }
        }
    }

    #[tokio::test]
    async fn send_message_ignores_intermediate_status_and_returns_task() {
        let state: A2AServerState<StreamingFirstExecutor, InMemoryTaskStore> = A2AServerState {
            executor: Arc::new(StreamingFirstExecutor),
            store: Arc::new(InMemoryTaskStore::new()),
            base_url: "http://localhost".to_string(),
            cancel_tokens: Arc::new(Mutex::new(HashMap::new())),
        };
        let state_for_assert = state.clone();

        let request = SendMessageRequest {
            message: Message {
                message_id: "msg-1".to_string(),
                context_id: Some("ctx-1".to_string()),
                task_id: None,
                role: Role::User,
                parts: vec![Part::text("hello")],
                metadata: None,
                extensions: vec![],
                reference_task_ids: None,
            },
            configuration: None,
            metadata: None,
        };

        let Json(response) = handle_send_message::<StreamingFirstExecutor, InMemoryTaskStore>(
            State(state),
            Json(request),
        )
        .await
        .expect("send should succeed");

        let SendMessageResponse::Task(task) = response else {
            panic!("expected task response");
        };
        assert_eq!(task.status.state, TaskState::Completed);

        let saved = state_for_assert
            .store
            .load(&task.id)
            .await
            .expect("store load should succeed");
        assert!(saved.is_some());
    }
}
