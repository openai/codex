use crate::auth::AuthProvider;
use crate::auth::add_auth_headers_to_header_map;
use crate::common::ResponseEvent;
use crate::common::ResponseStream;
use crate::common::ResponsesWsRequest;
use crate::error::ApiError;
use crate::provider::Provider;
use crate::rate_limits::parse_rate_limit_event;
use crate::sse::responses::ResponsesStreamEvent;
use crate::sse::responses::process_responses_event;
use crate::telemetry::WebsocketTelemetry;
use codex_client::TransportError;
use futures::SinkExt;
use futures::StreamExt;
use http::HeaderMap;
use http::HeaderName;
use http::HeaderValue;
use http::StatusCode;
use serde::Deserialize;
use serde_json::Value;
use serde_json::map::Map as JsonMap;
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
const X_MODELS_ETAG_HEADER: &str = "x-models-etag";
const X_REASONING_INCLUDED_HEADER: &str = "x-reasoning-included";

pub struct ResponsesWebsocketConnection {
    stream: Arc<Mutex<Option<WsStream>>>,
    // TODO (pakrym): is this the right place for timeout?
    idle_timeout: Duration,
    server_reasoning_included: bool,
    models_etag: Option<String>,
    telemetry: Option<Arc<dyn WebsocketTelemetry>>,
}

impl ResponsesWebsocketConnection {
    fn new(
        stream: WsStream,
        idle_timeout: Duration,
        server_reasoning_included: bool,
        models_etag: Option<String>,
        telemetry: Option<Arc<dyn WebsocketTelemetry>>,
    ) -> Self {
        Self {
            stream: Arc::new(Mutex::new(Some(stream))),
            idle_timeout,
            server_reasoning_included,
            models_etag,
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
        let models_etag = self.models_etag.clone();
        let telemetry = self.telemetry.clone();
        let request_body = serde_json::to_value(&request).map_err(|err| {
            ApiError::Stream(format!("failed to encode websocket request: {err}"))
        })?;

        tokio::spawn(async move {
            if let Some(etag) = models_etag {
                let _ = tx_event.send(Ok(ResponseEvent::ModelsEtag(etag))).await;
            }
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
        add_auth_headers_to_header_map(&self.auth, &mut headers);

        let (stream, server_reasoning_included, models_etag) =
            connect_websocket(ws_url, headers, turn_state.clone()).await?;
        Ok(ResponsesWebsocketConnection::new(
            stream,
            self.provider.stream_idle_timeout,
            server_reasoning_included,
            models_etag,
            telemetry,
        ))
    }
}

async fn connect_websocket(
    url: Url,
    headers: HeaderMap,
    turn_state: Option<Arc<OnceLock<String>>>,
) -> Result<(WsStream, bool, Option<String>), ApiError> {
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
    let models_etag = response
        .headers()
        .get(X_MODELS_ETAG_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(ToString::to_string);
    if let Some(turn_state) = turn_state
        && let Some(header_value) = response
            .headers()
            .get(X_CODEX_TURN_STATE_HEADER)
            .and_then(|value| value.to_str().ok())
    {
        let _ = turn_state.set(header_value.to_string());
    }
    Ok((stream, reasoning_included, models_etag))
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

#[derive(Debug, Deserialize)]
struct WrappedWebsocketErrorEvent {
    #[serde(rename = "type")]
    kind: String,
    status_code: Option<u16>,
    #[serde(default)]
    error: Option<Value>,
    #[serde(default)]
    headers: Option<JsonMap<String, Value>>,
}

fn parse_wrapped_websocket_error_event(payload: &str) -> Option<ApiError> {
    let event: WrappedWebsocketErrorEvent = serde_json::from_str(payload).ok()?;
    if event.kind != "error" {
        return None;
    }

    let status_code = event.status_code?;
    let status = StatusCode::from_u16(status_code).ok()?;
    if status.is_success() {
        return None;
    }

    let body = event.error.map(|error| {
        serde_json::json!({
            "error": error
        })
        .to_string()
    });

    Some(ApiError::Transport(TransportError::Http {
        status,
        url: None,
        headers: event.headers.map(json_headers_to_http_headers),
        body,
    }))
}

fn json_headers_to_http_headers(headers: JsonMap<String, Value>) -> HeaderMap {
    let mut mapped = HeaderMap::new();
    for (name, value) in headers {
        let Ok(header_name) = HeaderName::from_bytes(name.as_bytes()) else {
            continue;
        };
        let Some(header_value) = json_header_value(value) else {
            continue;
        };
        mapped.insert(header_name, header_value);
    }
    mapped
}

fn json_header_value(value: Value) -> Option<HeaderValue> {
    let value = match value {
        Value::String(value) => value,
        Value::Number(value) => value.to_string(),
        Value::Bool(value) => value.to_string(),
        _ => return None,
    };
    HeaderValue::from_str(&value).ok()
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
                if let Some(error) = parse_wrapped_websocket_error_event(&text) {
                    return Err(error);
                }

                let event = match serde_json::from_str::<ResponsesStreamEvent>(&text) {
                    Ok(event) => event,
                    Err(err) => {
                        debug!("failed to parse websocket event: {err}, data: {text}");
                        continue;
                    }
                };
                if event.kind() == "codex.rate_limits" {
                    if let Some(snapshot) = parse_rate_limit_event(&text) {
                        let _ = tx_event.send(Ok(ResponseEvent::RateLimits(snapshot))).await;
                    }
                    continue;
                }
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

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    #[test]
    fn parse_wrapped_websocket_error_event_maps_to_transport_http() {
        let payload = json!({
            "type": "error",
            "status_code": 429,
            "error": {
                "type": "usage_limit_reached",
                "message": "The usage limit has been reached",
                "plan_type": "pro",
                "resets_at": 1738888888
            },
            "headers": {
                "x-codex-primary-used-percent": "100.0",
                "x-codex-primary-window-minutes": 15
            }
        })
        .to_string();

        let api_error = parse_wrapped_websocket_error_event(&payload)
            .expect("expected websocket error payload to be parsed");

        let ApiError::Transport(TransportError::Http {
            status,
            headers,
            body,
            ..
        }) = api_error
        else {
            panic!("expected ApiError::Transport(Http)");
        };

        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        let headers = headers.expect("expected headers");
        assert_eq!(
            headers
                .get("x-codex-primary-used-percent")
                .and_then(|value| value.to_str().ok()),
            Some("100.0")
        );
        assert_eq!(
            headers
                .get("x-codex-primary-window-minutes")
                .and_then(|value| value.to_str().ok()),
            Some("15")
        );
        let body = body.expect("expected body");
        assert!(body.contains("usage_limit_reached"));
        assert!(body.contains("The usage limit has been reached"));
    }

    #[test]
    fn parse_wrapped_websocket_error_event_ignores_non_error_payloads() {
        let payload = json!({
            "type": "response.created",
            "response": {
                "id": "resp-1"
            }
        })
        .to_string();

        let api_error = parse_wrapped_websocket_error_event(&payload);
        assert!(api_error.is_none());
    }
}
