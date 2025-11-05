//! Blueprint WebSocket Handler
//!
//! Provides real-time execution progress updates via WebSocket.

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, State,
    },
    response::Response,
    routing::get,
    Router,
};
use codex_core::blueprint::{ExecutionEvent};
use futures::{sink::SinkExt, stream::StreamExt};
use serde_json;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{debug, error, info, warn};

/// WebSocket state
pub struct BlueprintWsState {
    /// Event broadcaster
    event_tx: broadcast::Sender<ExecutionEvent>,
}

impl BlueprintWsState {
    /// Create new WebSocket state
    pub fn new() -> Self {
        let (event_tx, _) = broadcast::channel(100);
        Self { event_tx }
    }
    
    /// Subscribe to execution events
    pub fn subscribe(&self) -> broadcast::Receiver<ExecutionEvent> {
        self.event_tx.subscribe()
    }
    
    /// Broadcast an event
    pub fn broadcast(&self, event: ExecutionEvent) -> Result<usize, broadcast::error::SendError<ExecutionEvent>> {
        self.event_tx.send(event)
    }
}

/// Create Blueprint WebSocket router
pub fn create_blueprint_ws_router(state: Arc<BlueprintWsState>) -> Router {
    Router::new()
        .route("/api/blueprint/ws/:blueprint_id", get(blueprint_ws_handler))
        .with_state(state)
}

/// WebSocket upgrade handler
async fn blueprint_ws_handler(
    ws: WebSocketUpgrade,
    Path(blueprint_id): Path<String>,
    State(state): State<Arc<BlueprintWsState>>,
) -> Response {
    info!("WebSocket connection request for blueprint: {}", blueprint_id);
    
    ws.on_upgrade(move |socket| handle_socket(socket, blueprint_id, state))
}

/// Handle WebSocket connection
async fn handle_socket(socket: WebSocket, blueprint_id: String, state: Arc<BlueprintWsState>) {
    let (mut sender, mut receiver) = socket.split();
    
    // Subscribe to execution events
    let mut event_rx = state.subscribe();
    
    info!("WebSocket connected for blueprint: {}", blueprint_id);
    
    // Send initial connection confirmation
    let init_message = serde_json::json!({
        "type": "connected",
        "blueprint_id": blueprint_id,
        "timestamp": chrono::Utc::now().to_rfc3339(),
    });
    
    if let Ok(json) = serde_json::to_string(&init_message) {
        if sender.send(Message::Text(json)).await.is_err() {
            error!("Failed to send initial message");
            return;
        }
    }
    
    // Spawn task to forward events to WebSocket
    let mut send_task = tokio::spawn(async move {
        while let Ok(event) = event_rx.recv().await {
            // Filter events for this blueprint only
            let event_blueprint_id = match &event {
                ExecutionEvent::Started { blueprint_id, .. } => blueprint_id.clone(),
                ExecutionEvent::StepCompleted { .. } => "".to_string(), // Need to track
                ExecutionEvent::FileChanged { .. } => "".to_string(),
                ExecutionEvent::TestPassed { .. } => "".to_string(),
                ExecutionEvent::TestFailed { .. } => "".to_string(),
                ExecutionEvent::Progress { .. } => "".to_string(),
                ExecutionEvent::Completed { .. } => "".to_string(),
                ExecutionEvent::Failed { .. } => "".to_string(),
            };
            
            // For now, broadcast all events (TODO: filter by blueprint_id)
            if let Ok(json) = serde_json::to_string(&event) {
                if sender.send(Message::Text(json)).await.is_err() {
                    warn!("Client disconnected");
                    break;
                }
            }
        }
    });
    
    // Receive task (handle client messages)
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                Message::Text(text) => {
                    debug!("Received WebSocket message: {}", text);
                    
                    // Handle ping/pong
                    if text == "ping" {
                        // Ping response handled by framework
                        debug!("Received ping");
                    }
                }
                Message::Close(_) => {
                    info!("WebSocket close message received");
                    break;
                }
                Message::Ping(_) | Message::Pong(_) => {
                    // Handled automatically by axum
                }
                _ => {}
            }
        }
    });
    
    // Wait for either task to finish
    tokio::select! {
        _ = (&mut send_task) => {
            debug!("Send task completed");
            recv_task.abort();
        }
        _ = (&mut recv_task) => {
            debug!("Receive task completed");
            send_task.abort();
        }
    }
    
    info!("WebSocket disconnected for blueprint: {}", blueprint_id);
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_blueprint_ws_state_creation() {
        let state = BlueprintWsState::new();
        let _rx = state.subscribe();
        // State should be created successfully
    }
}
