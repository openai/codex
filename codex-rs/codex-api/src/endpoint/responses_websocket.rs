use crate::auth::AuthProvider;
use crate::common::ResponseEvent;
use crate::common::ResponseStream;
use crate::common::ResponsesWsRequest;
use crate::error::ApiError;
use crate::provider::Provider;
use crate::sse::responses::ResponsesStreamEvent;
use crate::sse::responses::process_responses_event;
use codex_client::TransportError;
use futures::SinkExt;
use futures::StreamExt;
use http::HeaderMap;
use http::HeaderValue;
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tokio_tungstenite::MaybeTlsStream;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::Error as WsError;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tracing::debug;
use tracing::trace;
use url::Url;

type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;

pub struct ResponsesWebsocketConnection {
    stream: Arc<Mutex<Option<WsStream>>>,
    // TODO (pakrym): is this the right place for timeout?
    idle_timeout: Duration,
}

impl ResponsesWebsocketConnection {
    fn new(stream: WsStream, idle_timeout: Duration) -> Self {
        Self {
            stream: Arc::new(Mutex::new(Some(stream))),
            idle_timeout,
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
        let request_body = serde_json::to_value(&request).map_err(|err| {
            ApiError::Stream(format!("failed to encode websocket request: {err}"))
        })?;

        tokio::spawn(async move {
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
    ) -> Result<ResponsesWebsocketConnection, ApiError> {
        let ws_url = Url::parse(&self.provider.url_for_path("responses"))
            .map_err(|err| ApiError::Stream(format!("failed to build websocket URL: {err}")))?;

        let mut headers = self.provider.headers.clone();
        headers.extend(extra_headers);
        apply_auth_headers(&mut headers, &self.auth);

        let stream = connect_websocket(ws_url, headers).await?;
        Ok(ResponsesWebsocketConnection::new(
            stream,
            self.provider.stream_idle_timeout,
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

async fn connect_websocket(url: Url, headers: HeaderMap) -> Result<WsStream, ApiError> {
    let mut request = url
        .clone()
        .into_client_request()
        .map_err(|err| ApiError::Stream(format!("failed to build websocket request: {err}")))?;
    request.headers_mut().extend(headers);

    let (stream, _) = tokio_tungstenite::connect_async(request)
        .await
        .map_err(|err| map_ws_error(err, &url))?;
    Ok(stream)
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
) -> Result<(), ApiError> {
    let request_text = match serde_json::to_string(&request_body) {
        Ok(text) => text,
        Err(err) => {
            return Err(ApiError::Stream(format!(
                "failed to encode websocket request: {err}"
            )));
        }
    };

    if let Err(err) = ws_stream.send(Message::Text(request_text)).await {
        return Err(ApiError::Stream(format!(
            "failed to send websocket request: {err}"
        )));
    }

    loop {
        let response = tokio::time::timeout(idle_timeout, ws_stream.next())
            .await
            .map_err(|_| ApiError::Stream("idle timeout waiting for websocket".into()));
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
                    "websocket closed before response.completed".into(),
                ));
            }
            _ => {}
        }
    }

    Ok(())
}
