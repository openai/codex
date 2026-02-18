use crate::endpoint::realtime_websocket::protocol::ConversationItem;
use crate::endpoint::realtime_websocket::protocol::ConversationItemContent;
use crate::endpoint::realtime_websocket::protocol::RealtimeAudioFrame;
use crate::endpoint::realtime_websocket::protocol::RealtimeEvent;
use crate::endpoint::realtime_websocket::protocol::RealtimeOutboundMessage;
use crate::endpoint::realtime_websocket::protocol::RealtimeSessionConfig;
use crate::endpoint::realtime_websocket::protocol::SessionCreateSession;
use crate::endpoint::realtime_websocket::protocol::SessionUpdateSession;
use crate::endpoint::realtime_websocket::protocol::parse_realtime_event;
use crate::error::ApiError;
use crate::provider::Provider;
use codex_utils_rustls_provider::ensure_rustls_crypto_provider;
use futures::SinkExt;
use futures::StreamExt;
use http::HeaderMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio_tungstenite::MaybeTlsStream;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::Error as WsError;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tracing::info;
use tracing::trace;
use tungstenite::protocol::WebSocketConfig;
use url::Url;

struct WsStream {
    tx_command: mpsc::Sender<WsCommand>,
    pump_task: tokio::task::JoinHandle<()>,
}

enum WsCommand {
    Send {
        message: Message,
        tx_result: oneshot::Sender<Result<(), WsError>>,
    },
    Close {
        tx_result: oneshot::Sender<Result<(), WsError>>,
    },
}

impl WsStream {
    fn new(
        inner: WebSocketStream<MaybeTlsStream<TcpStream>>,
    ) -> (Self, mpsc::UnboundedReceiver<Result<Message, WsError>>) {
        let (tx_command, mut rx_command) = mpsc::channel::<WsCommand>(32);
        let (tx_message, rx_message) = mpsc::unbounded_channel::<Result<Message, WsError>>();

        let pump_task = tokio::spawn(async move {
            let mut inner = inner;
            loop {
                tokio::select! {
                    command = rx_command.recv() => {
                        let Some(command) = command else {
                            break;
                        };
                        match command {
                            WsCommand::Send { message, tx_result } => {
                                let result = inner.send(message).await;
                                let should_break = result.is_err();
                                let _ = tx_result.send(result);
                                if should_break {
                                    break;
                                }
                            }
                            WsCommand::Close { tx_result } => {
                                let result = inner.close(None).await;
                                let _ = tx_result.send(result);
                                break;
                            }
                        }
                    }
                    message = inner.next() => {
                        let Some(message) = message else {
                            break;
                        };
                        match message {
                            Ok(Message::Ping(payload)) => {
                                if let Err(err) = inner.send(Message::Pong(payload)).await {
                                    let _ = tx_message.send(Err(err));
                                    break;
                                }
                            }
                            Ok(Message::Pong(_)) => {}
                            Ok(message @ (Message::Text(_)
                                | Message::Binary(_)
                                | Message::Close(_)
                                | Message::Frame(_))) => {
                                let is_close = matches!(message, Message::Close(_));
                                if tx_message.send(Ok(message)).is_err() {
                                    break;
                                }
                                if is_close {
                                    break;
                                }
                            }
                            Err(err) => {
                                let _ = tx_message.send(Err(err));
                                break;
                            }
                        }
                    }
                }
            }
        });

        (
            Self {
                tx_command,
                pump_task,
            },
            rx_message,
        )
    }

    async fn request(
        &self,
        make_command: impl FnOnce(oneshot::Sender<Result<(), WsError>>) -> WsCommand,
    ) -> Result<(), WsError> {
        let (tx_result, rx_result) = oneshot::channel();
        if self.tx_command.send(make_command(tx_result)).await.is_err() {
            return Err(WsError::ConnectionClosed);
        }
        rx_result.await.unwrap_or(Err(WsError::ConnectionClosed))
    }

    async fn send(&self, message: Message) -> Result<(), WsError> {
        self.request(|tx_result| WsCommand::Send { message, tx_result })
            .await
    }

    async fn close(&self) -> Result<(), WsError> {
        self.request(|tx_result| WsCommand::Close { tx_result })
            .await
    }
}

impl Drop for WsStream {
    fn drop(&mut self) {
        self.pump_task.abort();
    }
}

pub struct RealtimeWebsocketConnection {
    writer: RealtimeWebsocketWriter,
    events: RealtimeWebsocketEvents,
}

#[derive(Clone)]
pub struct RealtimeWebsocketWriter {
    stream: Arc<WsStream>,
    is_closed: Arc<AtomicBool>,
}

#[derive(Clone)]
pub struct RealtimeWebsocketEvents {
    rx_message: Arc<Mutex<mpsc::UnboundedReceiver<Result<Message, WsError>>>>,
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

    fn new(
        stream: WsStream,
        rx_message: mpsc::UnboundedReceiver<Result<Message, WsError>>,
    ) -> Self {
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
        let ws_url = websocket_url_from_api_url(config.api_url.as_str())?;

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

        let (stream, rx_message) = WsStream::new(stream);
        let connection = RealtimeWebsocketConnection::new(stream, rx_message);
        connection
            .send_session_create(config.prompt, config.session_id)
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

fn websocket_url_from_api_url(api_url: &str) -> Result<Url, ApiError> {
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        let url = websocket_url_from_api_url("http://127.0.0.1:8011").expect("build ws url");
        assert_eq!(url.as_str(), "ws://127.0.0.1:8011/ws");
    }

    #[test]
    fn websocket_url_from_ws_base_defaults_to_ws_path() {
        let url = websocket_url_from_api_url("wss://example.com").expect("build ws url");
        assert_eq!(url.as_str(), "wss://example.com/ws");
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
                    session_id: Some("conv_1".to_string()),
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
                    session_id: Some("conv_1".to_string()),
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
