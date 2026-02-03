use crate::auth::AuthProvider;
use crate::common::ResponseEvent;
use crate::common::ResponseStream;
use crate::common::ResponsesWsRequest;
use crate::error::ApiError;
use crate::provider::Provider;
use crate::sse::responses::ResponsesStreamEvent;
use crate::sse::responses::process_responses_event;
use crate::telemetry::WebsocketTelemetry;
use codex_client::TransportError;
use futures::SinkExt;
use futures::StreamExt;
use http::HeaderMap;
use http::HeaderValue;
use serde_json::Value;
use std::sync::Arc;
use std::sync::OnceLock;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tokio::time::Instant;
use tokio_tungstenite::MaybeTlsStream;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::Error as WsError;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tracing::debug;
use tracing::error;
use tracing::info;
use tracing::trace;
use url::Url;

type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;
const X_CODEX_TURN_STATE_HEADER: &str = "x-codex-turn-state";
const X_REASONING_INCLUDED_HEADER: &str = "x-reasoning-included";

pub struct ResponsesWebsocketConnection {
    stream: Arc<Mutex<Option<WsStream>>>,
    // TODO (pakrym): is this the right place for timeout?
    idle_timeout: Duration,
    server_reasoning_included: bool,
    telemetry: Option<Arc<dyn WebsocketTelemetry>>,
}

impl ResponsesWebsocketConnection {
    fn new(
        stream: WsStream,
        idle_timeout: Duration,
        server_reasoning_included: bool,
        telemetry: Option<Arc<dyn WebsocketTelemetry>>,
    ) -> Self {
        Self {
            stream: Arc::new(Mutex::new(Some(stream))),
            idle_timeout,
            server_reasoning_included,
            telemetry,
        }
    }

    pub async fn is_closed(&self) -> bool {
        self.stream.lock().await.is_none()
    }

    pub async fn stream_request(
        &self,
        request: ResponsesWsRequest,
    ) -> Result<ResponseStream, ApiError> {
        let (tx_event, rx_event) =
            mpsc::channel::<std::result::Result<ResponseEvent, ApiError>>(1600);
        let stream = Arc::clone(&self.stream);
        let idle_timeout = self.idle_timeout;
        let server_reasoning_included = self.server_reasoning_included;
        let telemetry = self.telemetry.clone();
        let request_body = serde_json::to_value(&request).map_err(|err| {
            ApiError::Stream(format!("failed to encode websocket request: {err}"))
        })?;

        tokio::spawn(async move {
            if server_reasoning_included {
                let _ = tx_event
                    .send(Ok(ResponseEvent::ServerReasoningIncluded(true)))
                    .await;
            }
            let mut guard = stream.lock().await;
            let Some(ws_stream) = guard.as_mut() else {
                let _ = tx_event
                    .send(Err(ApiError::Stream(
                        "websocket connection is closed".to_string(),
                    )))
                    .await;
                return;
            };

            if let Err(err) = run_websocket_response_stream(
                ws_stream,
                tx_event.clone(),
                request_body,
                idle_timeout,
                telemetry,
            )
            .await
            {
                let _ = ws_stream.close(None).await;
                *guard = None;
                let _ = tx_event.send(Err(err)).await;
            }
        });

        Ok(ResponseStream { rx_event })
    }
}

pub struct ResponsesWebsocketClient<A: AuthProvider> {
    provider: Provider,
    auth: A,
}

impl<A: AuthProvider> ResponsesWebsocketClient<A> {
    pub fn new(provider: Provider, auth: A) -> Self {
        Self { provider, auth }
    }

    pub async fn connect(
        &self,
        extra_headers: HeaderMap,
        turn_state: Option<Arc<OnceLock<String>>>,
        telemetry: Option<Arc<dyn WebsocketTelemetry>>,
    ) -> Result<ResponsesWebsocketConnection, ApiError> {
        let ws_url = self
            .provider
            .websocket_url_for_path("responses")
            .map_err(|err| ApiError::Stream(format!("failed to build websocket URL: {err}")))?;

        let mut headers = self.provider.headers.clone();
        headers.extend(extra_headers);
        apply_auth_headers(&mut headers, &self.auth);

        let (stream, server_reasoning_included) =
            connect_websocket(ws_url, headers, turn_state).await?;
        Ok(ResponsesWebsocketConnection::new(
            stream,
            self.provider.stream_idle_timeout,
            server_reasoning_included,
            telemetry,
        ))
    }
}

// TODO (pakrym): share with /auth
fn apply_auth_headers(headers: &mut HeaderMap, auth: &impl AuthProvider) {
    if let Some(token) = auth.bearer_token()
        && let Ok(header) = HeaderValue::from_str(&format!("Bearer {token}"))
    {
        let _ = headers.insert(http::header::AUTHORIZATION, header);
    }
    if let Some(account_id) = auth.account_id()
        && let Ok(header) = HeaderValue::from_str(&account_id)
    {
        let _ = headers.insert("ChatGPT-Account-ID", header);
    }
}

async fn connect_websocket(
    url: Url,
    headers: HeaderMap,
    turn_state: Option<Arc<OnceLock<String>>>,
) -> Result<(WsStream, bool), ApiError> {
    info!("connecting to websocket: {url}");

    let mut request = url
        .as_str()
        .into_client_request()
        .map_err(|err| ApiError::Stream(format!("failed to build websocket request: {err}")))?;
    request.headers_mut().extend(headers);

    let response = tokio_tungstenite::connect_async(request).await;

    let (stream, response) = match response {
        Ok((stream, response)) => {
            info!(
                "successfully connected to websocket: {url}, headers: {:?}",
                response.headers()
            );
            (stream, response)
        }
        Err(err) => {
            error!("failed to connect to websocket: {err}, url: {url}");
            return Err(map_ws_error(err, &url));
        }
    };

    let reasoning_included = response.headers().contains_key(X_REASONING_INCLUDED_HEADER);
    if let Some(turn_state) = turn_state
        && let Some(header_value) = response
            .headers()
            .get(X_CODEX_TURN_STATE_HEADER)
            .and_then(|value| value.to_str().ok())
    {
        let _ = turn_state.set(header_value.to_string());
    }
    Ok((stream, reasoning_included))
}

fn map_ws_error(err: WsError, url: &Url) -> ApiError {
    match err {
        WsError::Http(response) => {
            let status = response.status();
            let headers = response.headers().clone();
            let body = response
                .body()
                .as_ref()
                .and_then(|bytes| String::from_utf8(bytes.clone()).ok());
            ApiError::Transport(TransportError::Http {
                status,
                url: Some(url.to_string()),
                headers: Some(headers),
                body,
            })
        }
        WsError::ConnectionClosed | WsError::AlreadyClosed => {
            ApiError::Stream("websocket closed".to_string())
        }
        WsError::Io(err) => ApiError::Transport(TransportError::Network(err.to_string())),
        other => ApiError::Transport(TransportError::Network(other.to_string())),
    }
}

async fn run_websocket_response_stream(
    ws_stream: &mut WsStream,
    tx_event: mpsc::Sender<std::result::Result<ResponseEvent, ApiError>>,
    request_body: Value,
    idle_timeout: Duration,
    telemetry: Option<Arc<dyn WebsocketTelemetry>>,
) -> Result<(), ApiError> {
    let request_text = match serde_json::to_string(&request_body) {
        Ok(text) => text,
        Err(err) => {
            return Err(ApiError::Stream(format!(
                "failed to encode websocket request: {err}"
            )));
        }
    };

    let request_start = Instant::now();
    let result = ws_stream
        .send(Message::Text(request_text.into()))
        .await
        .map_err(|err| ApiError::Stream(format!("failed to send websocket request: {err}")));

    if let Some(t) = telemetry.as_ref() {
        t.on_ws_request(request_start.elapsed(), result.as_ref().err());
    }

    result?;

    loop {
        let poll_start = Instant::now();
        let response = tokio::time::timeout(idle_timeout, ws_stream.next())
            .await
            .map_err(|_| ApiError::Stream("idle timeout waiting for websocket".into()));
        if let Some(t) = telemetry.as_ref() {
            t.on_ws_event(&response, poll_start.elapsed());
        }
        let message = match response {
            Ok(Some(Ok(msg))) => msg,
            Ok(Some(Err(err))) => {
                return Err(ApiError::Stream(err.to_string()));
            }
            Ok(None) => {
                return Err(ApiError::Stream(
                    "stream closed before response.completed".into(),
                ));
            }
            Err(err) => {
                return Err(err);
            }
        };

        match message {
            Message::Text(text) => {
                trace!("websocket event: {text}");
                let event = match serde_json::from_str::<ResponsesStreamEvent>(&text) {
                    Ok(event) => event,
                    Err(err) => {
                        debug!("failed to parse websocket event: {err}, data: {text}");
                        continue;
                    }
                };
                match process_responses_event(event) {
                    Ok(Some(event)) => {
                        let is_completed = matches!(event, ResponseEvent::Completed { .. });
                        let _ = tx_event.send(Ok(event)).await;
                        if is_completed {
                            break;
                        }
                    }
                    Ok(None) => {}
                    Err(error) => {
                        return Err(error.into_api_error());
                    }
                }
            }
            Message::Binary(_) => {
                return Err(ApiError::Stream("unexpected binary websocket event".into()));
            }
            Message::Ping(payload) => {
                if ws_stream.send(Message::Pong(payload)).await.is_err() {
                    return Err(ApiError::Stream("websocket ping failed".into()));
                }
            }
            Message::Pong(_) => {}
            Message::Close(_) => {
                return Err(ApiError::Stream(
                    "websocket closed by server before response.completed".into(),
                ));
            }
            _ => {}
        }
    }

    Ok(())
}
