use crate::endpoint::realtime_websocket::protocol::ConversationItem;
use crate::endpoint::realtime_websocket::protocol::ConversationItemContent;
use crate::endpoint::realtime_websocket::protocol::RealtimeAudioFrame;
use crate::endpoint::realtime_websocket::protocol::RealtimeEvent;
use crate::endpoint::realtime_websocket::protocol::RealtimeOutboundMessage;
use crate::endpoint::realtime_websocket::protocol::RealtimeSessionConfig;
use crate::endpoint::realtime_websocket::protocol::SessionCreateSession;
use crate::endpoint::realtime_websocket::protocol::SessionUpdateSession;
use crate::endpoint::realtime_websocket::protocol::parse_realtime_event;
use crate::endpoint::websocket_pump::WebsocketMessage;
use crate::endpoint::websocket_pump::WebsocketPump;
use crate::error::ApiError;
use crate::provider::Provider;
use codex_utils_rustls_provider::ensure_rustls_crypto_provider;
use http::HeaderMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Error as WsError;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tracing::info;
use tracing::trace;
use tungstenite::protocol::WebSocketConfig;
use url::Url;

pub struct RealtimeWebsocketConnection {
    writer: RealtimeWebsocketWriter,
    events: RealtimeWebsocketEvents,
}

#[derive(Clone)]
pub struct RealtimeWebsocketWriter {
    stream: Arc<WebsocketPump>,
    is_closed: Arc<AtomicBool>,
}

#[derive(Clone)]
pub struct RealtimeWebsocketEvents {
    rx_message: Arc<Mutex<mpsc::UnboundedReceiver<WebsocketMessage>>>,
    is_closed: Arc<AtomicBool>,
}

impl RealtimeWebsocketConnection {
    pub async fn send_audio_frame(&self, frame: RealtimeAudioFrame) -> Result<(), ApiError> {
        self.writer.send_audio_frame(frame).await
    }

    pub async fn send_conversation_item_create(&self, text: String) -> Result<(), ApiError> {
        self.writer.send_conversation_item_create(text).await
    }

    pub async fn send_session_update(
        &self,
        backend_prompt: String,
        conversation_id: Option<String>,
    ) -> Result<(), ApiError> {
        self.writer
            .send_session_update(backend_prompt, conversation_id)
            .await
    }

    pub async fn send_session_create(
        &self,
        backend_prompt: String,
        conversation_id: Option<String>,
    ) -> Result<(), ApiError> {
        self.writer
            .send_session_create(backend_prompt, conversation_id)
            .await
    }

    pub async fn close(&self) -> Result<(), ApiError> {
        self.writer.close().await
    }

    pub async fn next_event(&self) -> Result<Option<RealtimeEvent>, ApiError> {
        self.events.next_event().await
    }

    pub fn writer(&self) -> RealtimeWebsocketWriter {
        self.writer.clone()
    }

    pub fn events(&self) -> RealtimeWebsocketEvents {
        self.events.clone()
    }

    fn new(stream: WebsocketPump, rx_message: mpsc::UnboundedReceiver<WebsocketMessage>) -> Self {
        let stream = Arc::new(stream);
        let is_closed = Arc::new(AtomicBool::new(false));
        Self {
            writer: RealtimeWebsocketWriter {
                stream: Arc::clone(&stream),
                is_closed: Arc::clone(&is_closed),
            },
            events: RealtimeWebsocketEvents {
                rx_message: Arc::new(Mutex::new(rx_message)),
                is_closed,
            },
        }
    }
}

impl RealtimeWebsocketWriter {
    pub async fn send_audio_frame(&self, frame: RealtimeAudioFrame) -> Result<(), ApiError> {
        self.send_json(RealtimeOutboundMessage::InputAudioDelta {
            delta: frame.data,
            sample_rate: frame.sample_rate,
            num_channels: frame.num_channels,
            samples_per_channel: frame.samples_per_channel,
        })
        .await
    }

    pub async fn send_conversation_item_create(&self, text: String) -> Result<(), ApiError> {
        self.send_json(RealtimeOutboundMessage::ConversationItemCreate {
            item: ConversationItem {
                kind: "message".to_string(),
                role: "user".to_string(),
                content: vec![ConversationItemContent {
                    kind: "text".to_string(),
                    text,
                }],
            },
        })
        .await
    }

    pub async fn send_session_update(
        &self,
        backend_prompt: String,
        conversation_id: Option<String>,
    ) -> Result<(), ApiError> {
        self.send_json(RealtimeOutboundMessage::SessionUpdate {
            session: Some(SessionUpdateSession {
                backend_prompt,
                conversation_id,
            }),
        })
        .await
    }

    pub async fn send_session_create(
        &self,
        backend_prompt: String,
        conversation_id: Option<String>,
    ) -> Result<(), ApiError> {
        self.send_json(RealtimeOutboundMessage::SessionCreate {
            session: SessionCreateSession {
                backend_prompt,
                conversation_id,
            },
        })
        .await
    }

    pub async fn close(&self) -> Result<(), ApiError> {
        if self.is_closed.swap(true, Ordering::SeqCst) {
            return Ok(());
        }
        if let Err(err) = self.stream.close().await
            && !matches!(err, WsError::ConnectionClosed | WsError::AlreadyClosed)
        {
            return Err(ApiError::Stream(format!(
                "failed to close websocket: {err}"
            )));
        }
        Ok(())
    }

    async fn send_json(&self, message: RealtimeOutboundMessage) -> Result<(), ApiError> {
        let payload = serde_json::to_string(&message)
            .map_err(|err| ApiError::Stream(format!("failed to encode realtime request: {err}")))?;
        trace!("realtime websocket request: {payload}");

        if self.is_closed.load(Ordering::SeqCst) {
            return Err(ApiError::Stream(
                "realtime websocket connection is closed".to_string(),
            ));
        }

        self.stream
            .send(Message::Text(payload.into()))
            .await
            .map_err(|err| ApiError::Stream(format!("failed to send realtime request: {err}")))?;
        Ok(())
    }
}

impl RealtimeWebsocketEvents {
    pub async fn next_event(&self) -> Result<Option<RealtimeEvent>, ApiError> {
        if self.is_closed.load(Ordering::SeqCst) {
            return Ok(None);
        }

        loop {
            let msg = match self.rx_message.lock().await.recv().await {
                Some(Ok(msg)) => msg,
                Some(Err(err)) => {
                    self.is_closed.store(true, Ordering::SeqCst);
                    return Err(ApiError::Stream(format!(
                        "failed to read websocket message: {err}"
                    )));
                }
                None => {
                    self.is_closed.store(true, Ordering::SeqCst);
                    return Ok(None);
                }
            };

            match msg {
                Message::Text(text) => {
                    if let Some(event) = parse_realtime_event(&text) {
                        return Ok(Some(event));
                    }
                }
                Message::Close(_) => {
                    self.is_closed.store(true, Ordering::SeqCst);
                    return Ok(None);
                }
                Message::Binary(_) => {
                    return Ok(Some(RealtimeEvent::Error(
                        "unexpected binary realtime websocket event".to_string(),
                    )));
                }
                Message::Frame(_) | Message::Ping(_) | Message::Pong(_) => {}
            }
        }
    }
}

pub struct RealtimeWebsocketClient {
    provider: Provider,
}

impl RealtimeWebsocketClient {
    pub fn new(provider: Provider) -> Self {
        Self { provider }
    }

    pub async fn connect(
        &self,
        config: RealtimeSessionConfig,
        extra_headers: HeaderMap,
        default_headers: HeaderMap,
    ) -> Result<RealtimeWebsocketConnection, ApiError> {
        ensure_rustls_crypto_provider();
        let ws_url = websocket_url_from_api_url(
            config.api_url.as_str(),
            self.provider.query_params.as_ref(),
        )?;

        let mut request = ws_url
            .as_str()
            .into_client_request()
            .map_err(|err| ApiError::Stream(format!("failed to build websocket request: {err}")))?;
        let headers = merge_request_headers(&self.provider.headers, extra_headers, default_headers);
        request.headers_mut().extend(headers);

        info!("connecting realtime websocket: {ws_url}");
        let (stream, _) =
            tokio_tungstenite::connect_async_with_config(request, Some(websocket_config()), false)
                .await
                .map_err(|err| {
                    ApiError::Stream(format!("failed to connect realtime websocket: {err}"))
                })?;

        let (stream, rx_message) = WebsocketPump::new(stream);
        let connection = RealtimeWebsocketConnection::new(stream, rx_message);
        connection
            .send_session_create(config.prompt, config.conversation_id)
            .await?;
        Ok(connection)
    }
}

fn merge_request_headers(
    provider_headers: &HeaderMap,
    extra_headers: HeaderMap,
    default_headers: HeaderMap,
) -> HeaderMap {
    let mut headers = provider_headers.clone();
    headers.extend(extra_headers);
    for (name, value) in &default_headers {
        if let http::header::Entry::Vacant(entry) = headers.entry(name) {
            entry.insert(value.clone());
        }
    }
    headers
}

fn websocket_config() -> WebSocketConfig {
    WebSocketConfig::default()
}

fn websocket_url_from_api_url(
    api_url: &str,
    query_params: Option<&std::collections::HashMap<String, String>>,
) -> Result<Url, ApiError> {
    let mut url = Url::parse(api_url)
        .map_err(|err| ApiError::Stream(format!("failed to parse realtime api_url: {err}")))?;

    match url.scheme() {
        "ws" | "wss" => {
            if url.path().is_empty() || url.path() == "/" {
                url.set_path("/ws");
            }
            Ok(url)
        }
        "http" | "https" => {
            if url.path().is_empty() || url.path() == "/" {
                url.set_path("/ws");
            }
            let scheme = if url.scheme() == "http" { "ws" } else { "wss" };
            let _ = url.set_scheme(scheme);
            Ok(url)
        }
        scheme => Err(ApiError::Stream(format!(
            "unsupported realtime api_url scheme: {scheme}"
        ))),
    }?;

    if let Some(params) = query_params
        && !params.is_empty()
    {
        let mut url_query = url.query_pairs_mut();
        for (key, value) in params {
            url_query.append_pair(key, value);
        }
    }

    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::SinkExt;
    use futures::StreamExt;
    use http::HeaderValue;
    use pretty_assertions::assert_eq;
    use serde_json::Value;
    use serde_json::json;
    use std::collections::HashMap;
    use std::time::Duration;
    use tokio::net::TcpListener;
    use tokio_tungstenite::accept_async;
    use tokio_tungstenite::tungstenite::Message;

    #[test]
    fn parse_session_created_event() {
        let payload = json!({
            "type": "session.created",
            "session": {"id": "sess_123"}
        })
        .to_string();

        assert_eq!(
            parse_realtime_event(payload.as_str()),
            Some(RealtimeEvent::SessionCreated {
                session_id: "sess_123".to_string()
            })
        );
    }

    #[test]
    fn parse_audio_delta_event() {
        let payload = json!({
            "type": "response.output_audio.delta",
            "delta": "AAA=",
            "sample_rate": 48000,
            "num_channels": 1,
            "samples_per_channel": 960
        })
        .to_string();
        assert_eq!(
            parse_realtime_event(payload.as_str()),
            Some(RealtimeEvent::AudioOut(RealtimeAudioFrame {
                data: "AAA=".to_string(),
                sample_rate: 48000,
                num_channels: 1,
                samples_per_channel: Some(960),
            }))
        );
    }

    #[test]
    fn parse_conversation_item_added_event() {
        let payload = json!({
            "type": "conversation.item.added",
            "item": {"type": "spawn_transcript", "seq": 7}
        })
        .to_string();
        assert_eq!(
            parse_realtime_event(payload.as_str()),
            Some(RealtimeEvent::ConversationItemAdded(
                json!({"type": "spawn_transcript", "seq": 7})
            ))
        );
    }

    #[test]
    fn merge_request_headers_matches_http_precedence() {
        let mut provider_headers = HeaderMap::new();
        provider_headers.insert(
            "originator",
            HeaderValue::from_static("provider-originator"),
        );
        provider_headers.insert("x-priority", HeaderValue::from_static("provider"));

        let mut extra_headers = HeaderMap::new();
        extra_headers.insert("x-priority", HeaderValue::from_static("extra"));

        let mut default_headers = HeaderMap::new();
        default_headers.insert("originator", HeaderValue::from_static("default-originator"));
        default_headers.insert("x-priority", HeaderValue::from_static("default"));
        default_headers.insert("x-default-only", HeaderValue::from_static("default-only"));

        let merged = merge_request_headers(&provider_headers, extra_headers, default_headers);

        assert_eq!(
            merged.get("originator"),
            Some(&HeaderValue::from_static("provider-originator"))
        );
        assert_eq!(
            merged.get("x-priority"),
            Some(&HeaderValue::from_static("extra"))
        );
        assert_eq!(
            merged.get("x-default-only"),
            Some(&HeaderValue::from_static("default-only"))
        );
    }

    #[test]
    fn websocket_url_from_http_base_defaults_to_ws_path() {
        let url = websocket_url_from_api_url("http://127.0.0.1:8011", None).expect("build ws url");
        assert_eq!(url.as_str(), "ws://127.0.0.1:8011/ws");
    }

    #[test]
    fn websocket_url_from_ws_base_defaults_to_ws_path() {
        let url = websocket_url_from_api_url("wss://example.com", None).expect("build ws url");
        assert_eq!(url.as_str(), "wss://example.com/ws");
    }

    #[test]
    fn websocket_url_includes_provider_query_params() {
        let mut query_params = HashMap::new();
        query_params.insert("api-version".to_string(), "2024-10-01-preview".to_string());

        let url = websocket_url_from_api_url("https://example.com/ws", Some(&query_params))
            .expect("build ws url");
        let api_version = url
            .query_pairs()
            .find(|(key, _)| key == "api-version")
            .map(|(_, value)| value.into_owned());

        assert_eq!(url.scheme(), "wss");
        assert_eq!(api_version, Some("2024-10-01-preview".to_string()));
    }

    #[tokio::test]
    async fn e2e_connect_and_exchange_events_against_mock_ws_server() {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("local addr");

        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("accept");
            let mut ws = accept_async(stream).await.expect("accept ws");

            let first = ws
                .next()
                .await
                .expect("first msg")
                .expect("first msg ok")
                .into_text()
                .expect("text");
            let first_json: Value = serde_json::from_str(&first).expect("json");
            assert_eq!(first_json["type"], "session.create");
            assert_eq!(
                first_json["session"]["backend_prompt"],
                Value::String("backend prompt".to_string())
            );
            assert_eq!(
                first_json["session"]["conversation_id"],
                Value::String("conv_1".to_string())
            );

            ws.send(Message::Text(
                json!({
                    "type": "session.created",
                    "session": {"id": "sess_mock"}
                })
                .to_string()
                .into(),
            ))
            .await
            .expect("send session.created");

            let second = ws
                .next()
                .await
                .expect("second msg")
                .expect("second msg ok")
                .into_text()
                .expect("text");
            let second_json: Value = serde_json::from_str(&second).expect("json");
            assert_eq!(second_json["type"], "response.input_audio.delta");

            let third = ws
                .next()
                .await
                .expect("third msg")
                .expect("third msg ok")
                .into_text()
                .expect("text");
            let third_json: Value = serde_json::from_str(&third).expect("json");
            assert_eq!(third_json["type"], "conversation.item.create");
            assert_eq!(third_json["item"]["content"][0]["text"], "hello agent");

            ws.send(Message::Text(
                json!({
                    "type": "response.output_audio.delta",
                    "delta": "AQID",
                    "sample_rate": 48000,
                    "num_channels": 1
                })
                .to_string()
                .into(),
            ))
            .await
            .expect("send audio");

            ws.send(Message::Text(
                json!({
                    "type": "conversation.item.added",
                    "item": {"type": "spawn_transcript", "seq": 2}
                })
                .to_string()
                .into(),
            ))
            .await
            .expect("send item added");
        });

        let provider = Provider {
            name: "test".to_string(),
            base_url: "http://localhost".to_string(),
            query_params: Some(HashMap::new()),
            headers: HeaderMap::new(),
            retry: crate::provider::RetryConfig {
                max_attempts: 1,
                base_delay: Duration::from_millis(1),
                retry_429: false,
                retry_5xx: false,
                retry_transport: false,
            },
            stream_idle_timeout: Duration::from_secs(5),
        };
        let client = RealtimeWebsocketClient::new(provider);
        let connection = client
            .connect(
                RealtimeSessionConfig {
                    api_url: format!("ws://{addr}"),
                    prompt: "backend prompt".to_string(),
                    conversation_id: Some("conv_1".to_string()),
                },
                HeaderMap::new(),
                HeaderMap::new(),
            )
            .await
            .expect("connect");

        let created = connection
            .next_event()
            .await
            .expect("next event")
            .expect("event");
        assert_eq!(
            created,
            RealtimeEvent::SessionCreated {
                session_id: "sess_mock".to_string()
            }
        );

        connection
            .send_audio_frame(RealtimeAudioFrame {
                data: "AQID".to_string(),
                sample_rate: 48000,
                num_channels: 1,
                samples_per_channel: Some(960),
            })
            .await
            .expect("send audio");
        connection
            .send_conversation_item_create("hello agent".to_string())
            .await
            .expect("send item");

        let audio_event = connection
            .next_event()
            .await
            .expect("next event")
            .expect("event");
        assert_eq!(
            audio_event,
            RealtimeEvent::AudioOut(RealtimeAudioFrame {
                data: "AQID".to_string(),
                sample_rate: 48000,
                num_channels: 1,
                samples_per_channel: None,
            })
        );

        let added_event = connection
            .next_event()
            .await
            .expect("next event")
            .expect("event");
        assert_eq!(
            added_event,
            RealtimeEvent::ConversationItemAdded(json!({
                "type": "spawn_transcript",
                "seq": 2
            }))
        );

        connection.close().await.expect("close");
        server.await.expect("server task");
    }

    #[tokio::test]
    async fn send_does_not_block_while_next_event_waits_for_inbound_data() {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("local addr");

        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("accept");
            let mut ws = accept_async(stream).await.expect("accept ws");

            let first = ws
                .next()
                .await
                .expect("first msg")
                .expect("first msg ok")
                .into_text()
                .expect("text");
            let first_json: Value = serde_json::from_str(&first).expect("json");
            assert_eq!(first_json["type"], "session.create");

            let second = ws
                .next()
                .await
                .expect("second msg")
                .expect("second msg ok")
                .into_text()
                .expect("text");
            let second_json: Value = serde_json::from_str(&second).expect("json");
            assert_eq!(second_json["type"], "response.input_audio.delta");

            ws.send(Message::Text(
                json!({
                    "type": "session.created",
                    "session": {"id": "sess_after_send"}
                })
                .to_string()
                .into(),
            ))
            .await
            .expect("send session.created");
        });

        let provider = Provider {
            name: "test".to_string(),
            base_url: "http://localhost".to_string(),
            query_params: Some(HashMap::new()),
            headers: HeaderMap::new(),
            retry: crate::provider::RetryConfig {
                max_attempts: 1,
                base_delay: Duration::from_millis(1),
                retry_429: false,
                retry_5xx: false,
                retry_transport: false,
            },
            stream_idle_timeout: Duration::from_secs(5),
        };
        let client = RealtimeWebsocketClient::new(provider);
        let connection = client
            .connect(
                RealtimeSessionConfig {
                    api_url: format!("ws://{addr}"),
                    prompt: "backend prompt".to_string(),
                    conversation_id: Some("conv_1".to_string()),
                },
                HeaderMap::new(),
                HeaderMap::new(),
            )
            .await
            .expect("connect");

        let (send_result, next_result) = tokio::join!(
            async {
                tokio::time::timeout(
                    Duration::from_millis(200),
                    connection.send_audio_frame(RealtimeAudioFrame {
                        data: "AQID".to_string(),
                        sample_rate: 48000,
                        num_channels: 1,
                        samples_per_channel: Some(960),
                    }),
                )
                .await
            },
            connection.next_event()
        );

        send_result
            .expect("send should not block on next_event")
            .expect("send audio");
        let next_event = next_result.expect("next event").expect("event");
        assert_eq!(
            next_event,
            RealtimeEvent::SessionCreated {
                session_id: "sess_after_send".to_string()
            }
        );

        connection.close().await.expect("close");
        server.await.expect("server task");
    }
}
