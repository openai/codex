use crate::git::GitWatcher;
use axum::{
    extract::{
        ws::{Message, WebSocket},
        Query, WebSocketUpgrade,
    },
    response::Response,
};
use futures::{sink::SinkExt, stream::StreamExt};
use serde::Deserialize;
use std::env;
use tracing::{debug, error, info};

#[derive(Deserialize)]
pub struct WebSocketQuery {
    #[serde(default)]
    repo_path: Option<String>,
}

/// WebSocket handler for real-time updates
pub async fn handler(
    ws: WebSocketUpgrade,
    Query(params): Query<WebSocketQuery>,
) -> Response {
    let repo_path = params
        .repo_path
        .unwrap_or_else(|| env::current_dir().unwrap().to_string_lossy().to_string());

    ws.on_upgrade(move |socket| handle_socket(socket, repo_path))
}

async fn handle_socket(socket: WebSocket, repo_path: String) {
    info!("🔌 New WebSocket connection for repo: {}", repo_path);

    let (mut sender, mut receiver) = socket.split();

    // Create git watcher
    let (_watcher, mut event_rx) = match GitWatcher::new(&repo_path) {
        Ok((w, rx)) => (w, rx),
        Err(e) => {
            error!("Failed to create GitWatcher: {}", e);
            let _ = sender
                .send(Message::Text(
                    serde_json::json!({
                        "type": "error",
                        "message": format!("Failed to watch repository: {}", e)
                    })
                    .to_string(),
                ))
                .await;
            return;
        }
    };

    // Send initial connection success message
    let _ = sender
        .send(Message::Text(
            serde_json::json!({
                "type": "connected",
                "message": "Real-time updates enabled"
            })
            .to_string(),
        ))
        .await;

    // Spawn task to forward events to WebSocket
    let mut send_task = tokio::spawn(async move {
        while let Ok(event) = event_rx.recv().await {
            match serde_json::to_string(&event) {
                Ok(json) => {
                    if sender.send(Message::Text(json)).await.is_err() {
                        break;
                    }
                }
                Err(e) => {
                    error!("Failed to serialize event: {}", e);
                }
            }
        }
    });

    // Handle incoming messages (ping/pong)
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                Message::Text(text) => {
                    debug!("Received WebSocket message: {}", text);
                }
                Message::Close(_) => {
                    debug!("WebSocket close message received");
                    break;
                }
                Message::Ping(_data) => {
                    // Respond to ping with pong
                    debug!("Received ping");
                }
                Message::Pong(_) => {
                    debug!("Received pong");
                }
                _ => {}
            }
        }
    });

    // Wait for either task to finish
    tokio::select! {
        _ = (&mut send_task) => {
            info!("Send task completed");
            recv_task.abort();
        }
        _ = (&mut recv_task) => {
            info!("Receive task completed");
            send_task.abort();
        }
    }

    info!("🔌 WebSocket connection closed");
}

