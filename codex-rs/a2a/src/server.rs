//! A2A RC v1 HTTP server using Axum.
//!
//! Mirrors `a2a-js/src/server/express/` and `a2a-js/src/server/request_handler/`.
//!
//! Uses [`AgentExecutor`] for agent logic, [`TaskStore`] for persistence,
//! and [`EventBus`] for streaming events.

use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Path, State},
    routing::{get, post},
};
use crate::error::A2AError;
use crate::event::{EventBus, ExecutionEvent};
use crate::executor::{AgentExecutor, RequestContext};
use crate::store::TaskStore;
use crate::types::*;

// ============================================================
// Server state
// ============================================================

/// Shared state for the A2A server.
pub struct A2AServerState<E: AgentExecutor, S: TaskStore> {
    pub executor: Arc<E>,
    pub store: Arc<S>,
    pub base_url: String,
}

impl<E: AgentExecutor, S: TaskStore> Clone for A2AServerState<E, S> {
    fn clone(&self) -> Self {
        Self {
            executor: Arc::clone(&self.executor),
            store: Arc::clone(&self.store),
            base_url: self.base_url.clone(),
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
        };

        Router::new()
            .route("/.well-known/agent.json", get(handle_agent_card::<E, S>))
            .route("/message:send", post(handle_send_message::<E, S>))
            .route("/tasks/{id}", get(handle_get_task::<E, S>))
            .route("/tasks/{id}:cancel", post(handle_cancel_task::<E, S>))
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

    // Execute in background.
    let executor = Arc::clone(&state.executor);
    tokio::spawn(async move {
        if let Err(e) = executor.execute(context, &event_bus).await {
            tracing::error!("AgentExecutor error: {e}");
        }
    });

    // Wait for the first event (the result).
    match rx.recv().await {
        Ok(ExecutionEvent::Task(task)) => {
            // Save to store.
            state.store.save(task.clone()).await?;
            Ok(Json(SendMessageResponse::Task(task)))
        }
        Ok(ExecutionEvent::Message(message)) => {
            Ok(Json(SendMessageResponse::Message(message)))
        }
        Ok(ExecutionEvent::StatusUpdate(_) | ExecutionEvent::ArtifactUpdate(_)) => {
            Err(A2AError::internal_error("Unexpected streaming event in non-streaming mode"))
        }
        Err(e) => Err(A2AError::internal_error(format!("Event bus error: {e}"))),
    }
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

/// `POST /tasks/{id}:cancel`
async fn handle_cancel_task<E: AgentExecutor, S: TaskStore>(
    State(state): State<A2AServerState<E, S>>,
    Path(task_id): Path<String>,
) -> Result<Json<Task>, A2AError> {
    let event_bus = EventBus::new(16);
    state.executor.cancel(&task_id, &event_bus).await?;

    match state.store.load(&task_id).await? {
        Some(task) => Ok(Json(task)),
        None => Err(A2AError::task_not_found(&task_id)),
    }
}
