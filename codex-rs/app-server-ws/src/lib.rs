use axum::Router;
use axum::extract::State;
use axum::extract::ws::Message;
use axum::extract::ws::WebSocket;
use axum::extract::ws::WebSocketUpgrade;
use axum::http::HeaderMap;
use axum::response::IntoResponse;
use axum::routing::get;
use codex_app_server::public_api::AppServerEngine;
use codex_app_server_protocol::JSONRPCMessage;
use futures_util::StreamExt;
use futures_util::sink::SinkExt;
use tracing::warn;

#[derive(Clone)]
pub struct AppState {
    pub auth_token: Option<String>,
    pub engine: AppServerEngine,
}

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/ws", get(ws_handler))
        .with_state(state)
}

async fn ws_handler(
    headers: HeaderMap,
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    if let Some(expected) = state.auth_token.clone() {
        let ok = headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|h| h == format!("Bearer {expected}"));
        if !ok {
            return axum::http::StatusCode::UNAUTHORIZED.into_response();
        }
    }
    ws.on_upgrade(move |socket| handle_socket(state, socket))
}

async fn handle_socket(state: AppState, socket: WebSocket) {
    let (mut conn, mut rx_json) = state.engine.new_connection();

    let (mut ws_tx, mut ws_rx) = socket.split();
    // Forward server → client messages.
    let to_ws = tokio::spawn(async move {
        while let Some(value) = rx_json.recv().await {
            let Ok(mut text) = serde_json::to_string(&value) else {
                continue;
            };
            text.push('\n');
            if ws_tx.send(Message::Text(text.into())).await.is_err() {
                break;
            }
        }
    });

    // Forward client → server messages.
    while let Some(msg) = ws_rx.next().await {
        match msg {
            Ok(Message::Text(text)) => match serde_json::from_str::<JSONRPCMessage>(&text) {
                Ok(JSONRPCMessage::Request(req)) => conn.process_request(req).await,
                Ok(JSONRPCMessage::Notification(n)) => conn.process_notification(n).await,
                Ok(JSONRPCMessage::Response(resp)) => conn.process_response(resp).await,
                Ok(JSONRPCMessage::Error(_)) => {}
                Err(_) => {}
            },
            Ok(Message::Close(_)) => break,
            Ok(Message::Binary(_)) => {}
            Ok(Message::Ping(_)) | Ok(Message::Pong(_)) => {}
            Err(e) => {
                warn!("websocket error: {e}");
                break;
            }
        }
    }
    let _ = to_ws.await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::Router;
    use serde_json::json;
    use tokio::net::TcpListener;
    use tokio_tungstenite::connect_async;

    use codex_common::CliConfigOverrides;
    use codex_core::config::Config;
    use codex_core::config::ConfigOverrides;

    async fn spawn_server(state: AppState) -> std::net::SocketAddr {
        let app: Router = crate::build_router(state);
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        addr
    }

    async fn default_engine() -> AppServerEngine {
        let overrides_cli = CliConfigOverrides {
            raw_overrides: vec![],
        };
        let cli_overrides = overrides_cli.parse_overrides().unwrap();
        let config = Config::load_with_cli_overrides(cli_overrides, ConfigOverrides::default())
            .await
            .expect("load config");
        AppServerEngine::new(std::sync::Arc::new(config), None)
    }

    #[tokio::test]
    async fn auth_accepts_with_correct_header() {
        let engine = default_engine().await;
        let state = AppState {
            auth_token: Some("topsecret".to_string()),
            engine,
        };
        let addr = spawn_server(state).await;

        use tokio_tungstenite::tungstenite::client::IntoClientRequest;
        let url = format!("ws://{addr}/ws");
        let mut req = url.into_client_request().expect("build client request");
        req.headers_mut().insert(
            http::header::AUTHORIZATION,
            http::HeaderValue::from_str("Bearer topsecret").unwrap(),
        );
        let (_ws, _resp) = connect_async(req).await.expect("handshake ok");
        // If we reached here, the auth header was accepted and the connection opened.
    }

    #[tokio::test]
    async fn auth_rejects_with_wrong_header() {
        let engine = default_engine().await;
        let state = AppState {
            auth_token: Some("topsecret".to_string()),
            engine,
        };
        let addr = spawn_server(state).await;

        let url = format!("ws://{addr}/ws");
        let req = http::Request::builder()
            .uri(url)
            .header(http::header::AUTHORIZATION, "Bearer notit")
            .body(())
            .unwrap();
        let res = connect_async(req).await;
        assert!(
            res.is_err(),
            "expected handshake rejection with wrong token"
        );
    }

    #[tokio::test]
    async fn invalid_json_is_ignored_then_valid_flow_works() {
        use tokio::time::Duration;
        use tokio::time::timeout;
        use tokio_tungstenite::tungstenite::Message as WsMsg;

        let engine = default_engine().await;
        let state = AppState {
            auth_token: None,
            engine,
        };
        let addr = spawn_server(state).await;

        let url = format!("ws://{addr}/ws");
        let (mut ws, _resp) = connect_async(url).await.unwrap();

        // Send garbage JSON first; the server should ignore it and keep the connection alive.
        ws.send(WsMsg::Text("this is not json".into()))
            .await
            .unwrap();

        // Now perform the normal init + newConversation flow and expect sessionConfigured.
        let init = json!({
            "method": "initialize",
            "id": 1,
            "params": { "clientInfo": { "name": "tests", "version": "0.0.0" } }
        });
        ws.send(WsMsg::Text(init.to_string().into())).await.unwrap();

        let tmp = tempfile::tempdir().unwrap();
        let new_conv = json!({
            "method": "newConversation",
            "id": 2,
            "params": { "cwd": tmp.path().to_string_lossy() }
        });
        ws.send(WsMsg::Text(new_conv.to_string().into()))
            .await
            .unwrap();

        let mut saw_session_configured = false;
        for _ in 0..50 {
            if let Ok(Some(Ok(WsMsg::Text(txt)))) =
                timeout(Duration::from_millis(200), ws.next()).await
                && let Ok(v) = serde_json::from_str::<serde_json::Value>(&txt)
                    && v.get("method").and_then(|m| m.as_str()) == Some("sessionConfigured")
                {
                    saw_session_configured = true;
                    break;
                }
        }
        assert!(
            saw_session_configured,
            "expected sessionConfigured after valid flow"
        );
    }

    #[tokio::test]
    async fn binary_ping_frames_are_ignored() {
        use tokio::time::Duration;
        use tokio::time::timeout;
        use tokio_tungstenite::tungstenite::Message as WsMsg;

        let engine = default_engine().await;
        let state = AppState {
            auth_token: None,
            engine,
        };
        let addr = spawn_server(state).await;

        let url = format!("ws://{addr}/ws");
        let (mut ws, _resp) = connect_async(url).await.unwrap();

        // Send frames the server ignores.
        ws.send(WsMsg::Binary(vec![1, 2, 3].into())).await.unwrap();
        ws.send(WsMsg::Ping(b"hi".to_vec().into())).await.unwrap();

        // Proceed with init/newConversation to ensure connection still works.
        let init = json!({
            "method": "initialize",
            "id": 1,
            "params": { "clientInfo": { "name": "tests", "version": "0.0.0" } }
        });
        ws.send(WsMsg::Text(init.to_string().into())).await.unwrap();

        let tmp = tempfile::tempdir().unwrap();
        let new_conv = json!({
            "method": "newConversation",
            "id": 2,
            "params": { "cwd": tmp.path().to_string_lossy() }
        });
        ws.send(WsMsg::Text(new_conv.to_string().into()))
            .await
            .unwrap();

        // Expect an ack for id=2 to confirm progress.
        let mut saw_ack = false;
        for _ in 0..50 {
            if let Ok(Some(Ok(WsMsg::Text(txt)))) =
                timeout(Duration::from_millis(200), ws.next()).await
                && serde_json::from_str::<serde_json::Value>(&txt)
                    .ok()
                    .and_then(|v| v.get("id").and_then(serde_json::Value::as_i64))
                    == Some(2)
                {
                    saw_ack = true;
                    break;
                }
        }
        assert!(
            saw_ack,
            "expected ack for newConversation despite prior ignored frames"
        );
    }
}
