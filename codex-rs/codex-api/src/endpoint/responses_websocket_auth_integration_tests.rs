use super::*;
use crate::auth::AuthProvider;
use crate::provider::RetryConfig;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::sync::Mutex as StdMutex;
use tokio::net::TcpListener;
use tokio_tungstenite::accept_hdr_async_with_config;

const SENT_STATE: &str = "sent-state";
const HANDSHAKE_STATE: &str = "handshake-state";
const WRAPPED_ERROR_STATE: &str = "wrapped-error-state";

struct RecordingAuthProvider {
    state: StdMutex<String>,
    observed_updates: StdMutex<Vec<(String, String, String)>>,
}

impl Default for RecordingAuthProvider {
    fn default() -> Self {
        Self {
            state: StdMutex::new(SENT_STATE.to_string()),
            observed_updates: StdMutex::new(Vec::new()),
        }
    }
}

impl AuthProvider for RecordingAuthProvider {
    fn add_auth_headers(&self, _headers: &mut HeaderMap) {}

    fn add_auth_headers_for_url(&self, _request_url: &str, headers: &mut HeaderMap) {
        let state = self
            .state
            .lock()
            .expect("state lock should not be poisoned");
        headers.insert(
            "x-test-state",
            HeaderValue::from_str(&state).expect("state should be a valid header value"),
        );
    }

    fn observe_response_headers(
        &self,
        request_url: &str,
        request_headers: &HeaderMap,
        response_headers: &HeaderMap,
    ) {
        let Some(sent_state) = request_headers
            .get("x-test-state")
            .and_then(|value| value.to_str().ok())
        else {
            return;
        };
        let Some(update_state) = response_headers
            .get("x-test-state-update")
            .and_then(|value| value.to_str().ok())
        else {
            return;
        };
        self.observed_updates
            .lock()
            .expect("observed updates lock should not be poisoned")
            .push((
                request_url.to_string(),
                sent_state.to_string(),
                update_state.to_string(),
            ));
        *self
            .state
            .lock()
            .expect("state lock should not be poisoned") = update_state.to_string();
    }
}

#[tokio::test]
async fn connect_attaches_auth_and_observes_handshake_and_retryable_wrapped_error_headers() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind websocket server");
    let address = listener.local_addr().expect("websocket server address");
    let observed_request_state = Arc::new(StdMutex::new(None));
    let server_request_state = Arc::clone(&observed_request_state);
    let server_task = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("accept websocket client");
        let mut stream = accept_hdr_async_with_config(
            stream,
            move |request: &tokio_tungstenite::tungstenite::handshake::server::Request,
                  mut response: tokio_tungstenite::tungstenite::handshake::server::Response| {
                *server_request_state
                    .lock()
                    .expect("request state lock should not be poisoned") = request
                    .headers()
                    .get("x-test-state")
                    .and_then(|value| value.to_str().ok())
                    .map(ToString::to_string);
                response.headers_mut().insert(
                    "x-test-state-update",
                    HeaderValue::from_static(HANDSHAKE_STATE),
                );
                Ok(response)
            },
            Some(websocket_config()),
        )
        .await
        .expect("complete websocket handshake");
        let message = stream
            .next()
            .await
            .expect("receive websocket request")
            .expect("read websocket request");
        assert!(matches!(message, Message::Text(_)));
        stream
            .send(Message::Text(
                json!({
                    "type": "error",
                    "status": 400,
                    "error": {
                        "code": WEBSOCKET_CONNECTION_LIMIT_REACHED_CODE,
                        "message": WEBSOCKET_CONNECTION_LIMIT_REACHED_MESSAGE,
                    },
                    "headers": {
                        "x-test-state-update": WRAPPED_ERROR_STATE,
                    },
                })
                .to_string()
                .into(),
            ))
            .await
            .expect("send wrapped websocket error");
    });

    let auth = Arc::new(RecordingAuthProvider::default());
    let request_url = format!("ws://{address}/backend-api/codex/responses");
    let client = ResponsesWebsocketClient::new(websocket_provider(address), auth.clone());
    let connection = client
        .connect(
            HeaderMap::new(),
            HeaderMap::new(),
            /*turn_state*/ None,
            /*telemetry*/ None,
        )
        .await
        .expect("connect websocket client");
    let mut response_stream = connection
        .stream_request(
            ResponsesWsRequest::ResponseProcessed(ResponseProcessedWsRequest {
                response_id: "response-id".to_string(),
            }),
            /*connection_reused*/ false,
        )
        .await
        .expect("start websocket response stream");
    let error = response_stream
        .next()
        .await
        .expect("receive websocket response")
        .expect_err("wrapped error should fail the response stream");

    server_task.await.expect("websocket server task");
    let ApiError::Retryable { message, delay } = error else {
        panic!("expected retryable error");
    };
    assert_eq!(message, WEBSOCKET_CONNECTION_LIMIT_REACHED_MESSAGE);
    assert_eq!(delay, None);
    assert_eq!(
        *observed_request_state
            .lock()
            .expect("request state lock should not be poisoned"),
        Some(SENT_STATE.to_string())
    );
    assert_eq!(
        *auth
            .observed_updates
            .lock()
            .expect("observed updates lock should not be poisoned"),
        vec![
            (
                request_url.clone(),
                SENT_STATE.to_string(),
                HANDSHAKE_STATE.to_string(),
            ),
            (
                request_url,
                HANDSHAKE_STATE.to_string(),
                WRAPPED_ERROR_STATE.to_string(),
            ),
        ]
    );
}

fn websocket_provider(address: std::net::SocketAddr) -> Provider {
    Provider {
        name: "test".to_string(),
        base_url: format!("http://{address}/backend-api/codex"),
        query_params: None,
        headers: HeaderMap::new(),
        retry: RetryConfig {
            max_attempts: 1,
            base_delay: Duration::from_millis(1),
            retry_429: false,
            retry_5xx: false,
            retry_transport: false,
        },
        stream_idle_timeout: Duration::from_secs(1),
    }
}
