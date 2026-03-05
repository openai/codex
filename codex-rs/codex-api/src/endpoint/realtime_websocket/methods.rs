use crate::endpoint::realtime_websocket::mode_v1;
use crate::endpoint::realtime_websocket::mode_v2;
use crate::endpoint::realtime_websocket::protocol_v1;
use crate::endpoint::realtime_websocket::protocol_v2;
use crate::endpoint::realtime_websocket::types::RealtimeApiMode;
use crate::endpoint::realtime_websocket::types::RealtimeAudioFrame;
use crate::endpoint::realtime_websocket::types::RealtimeEvent;
use crate::endpoint::realtime_websocket::types::RealtimeOutboundMessage;
use crate::endpoint::realtime_websocket::types::RealtimeSessionConfig;
use crate::error::ApiError;
use crate::provider::Provider;
use codex_utils_rustls_provider::ensure_rustls_crypto_provider;
use futures::SinkExt;
use futures::StreamExt;
use http::HeaderMap;
use http::HeaderValue;
use http::header::Entry;
use std::collections::HashMap;
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
use tracing::debug;
use tracing::error;
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
                                debug!("realtime websocket sending message");
                                let result = inner.send(message).await;
                                let should_break = result.is_err();
                                if let Err(err) = &result {
                                    error!("realtime websocket send failed: {err}");
                                }
                                let _ = tx_result.send(result);
                                if should_break {
                                    break;
                                }
                            }
                            WsCommand::Close { tx_result } => {
                                info!("realtime websocket sending close");
                                let result = inner.close(None).await;
                                if let Err(err) = &result {
                                    error!("realtime websocket close failed: {err}");
                                }
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
                                trace!(payload_len = payload.len(), "realtime websocket received ping");
                                if let Err(err) = inner.send(Message::Pong(payload)).await {
                                    error!("realtime websocket failed to send pong: {err}");
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
                                match &message {
                                    Message::Text(_) => trace!("realtime websocket received text frame"),
                                    Message::Binary(binary) => {
                                        error!(
                                            payload_len = binary.len(),
                                            "realtime websocket received unexpected binary frame"
                                        );
                                    }
                                    Message::Close(frame) => info!(
                                        "realtime websocket received close frame: code={:?} reason={:?}",
                                        frame.as_ref().map(|frame| frame.code),
                                        frame.as_ref().map(|frame| frame.reason.as_str())
                                    ),
                                    Message::Frame(_) => {
                                        trace!("realtime websocket received raw frame");
                                    }
                                    Message::Ping(_) | Message::Pong(_) => {}
                                }
                                if tx_message.send(Ok(message)).is_err() {
                                    break;
                                }
                                if is_close {
                                    break;
                                }
                            }
                            Err(err) => {
                                error!("realtime websocket receive failed: {err}");
                                let _ = tx_message.send(Err(err));
                                break;
                            }
                        }
                    }
                }
            }
            info!("realtime websocket pump exiting");
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
    mode: RealtimeApiMode,
}

#[derive(Clone)]
pub struct RealtimeWebsocketEvents {
    rx_message: Arc<Mutex<mpsc::UnboundedReceiver<Result<Message, WsError>>>>,
    is_closed: Arc<AtomicBool>,
    mode: RealtimeApiMode,
}

impl RealtimeWebsocketConnection {
    pub async fn send_audio_frame(&self, frame: RealtimeAudioFrame) -> Result<(), ApiError> {
        self.writer.send_audio_frame(frame).await
    }

    pub async fn send_conversation_item_create(&self, text: String) -> Result<(), ApiError> {
        self.writer.send_conversation_item_create(text).await
    }

    pub async fn send_conversation_handoff_append(
        &self,
        handoff_id: String,
        output_text: String,
    ) -> Result<(), ApiError> {
        self.writer
            .send_conversation_handoff_append(handoff_id, output_text)
            .await
    }

    pub async fn send_function_call_output(
        &self,
        call_id: String,
        output_text: String,
    ) -> Result<(), ApiError> {
        self.writer
            .send_function_call_output(call_id, output_text)
            .await
    }

    pub async fn send_response_create(&self) -> Result<(), ApiError> {
        self.writer.send_response_create().await
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
        mode: RealtimeApiMode,
    ) -> Self {
        let stream = Arc::new(stream);
        let is_closed = Arc::new(AtomicBool::new(false));
        Self {
            writer: RealtimeWebsocketWriter {
                stream: Arc::clone(&stream),
                is_closed: Arc::clone(&is_closed),
                mode,
            },
            events: RealtimeWebsocketEvents {
                rx_message: Arc::new(Mutex::new(rx_message)),
                is_closed,
                mode,
            },
        }
    }
}

impl RealtimeWebsocketWriter {
    pub async fn send_audio_frame(&self, frame: RealtimeAudioFrame) -> Result<(), ApiError> {
        self.send_json(RealtimeOutboundMessage::InputAudioBufferAppend { audio: frame.data })
            .await
    }

    pub async fn send_conversation_item_create(&self, text: String) -> Result<(), ApiError> {
        let message = match self.mode {
            RealtimeApiMode::V1 => mode_v1::conversation_item_create(text),
            RealtimeApiMode::V2 => mode_v2::conversation_item_create(text),
        };
        self.send_json(message).await
    }

    pub async fn send_conversation_handoff_append(
        &self,
        handoff_id: String,
        output_text: String,
    ) -> Result<(), ApiError> {
        let message = match self.mode {
            RealtimeApiMode::V1 => mode_v1::handoff_append(handoff_id, output_text),
            RealtimeApiMode::V2 => mode_v2::handoff_append(output_text),
        };
        self.send_json(message).await
    }

    pub async fn send_function_call_output(
        &self,
        call_id: String,
        output_text: String,
    ) -> Result<(), ApiError> {
        match self.mode {
            RealtimeApiMode::V1 => Ok(()),
            RealtimeApiMode::V2 => {
                self.send_json(mode_v2::function_call_output(call_id, output_text))
                    .await
            }
        }
    }

    pub async fn send_response_create(&self) -> Result<(), ApiError> {
        match self.mode {
            RealtimeApiMode::V1 => Ok(()),
            RealtimeApiMode::V2 => {
                self.send_json(RealtimeOutboundMessage::ResponseCreate)
                    .await
            }
        }
    }

    pub async fn send_session_update(&self, instructions: String) -> Result<(), ApiError> {
        let message = match self.mode {
            RealtimeApiMode::V1 => mode_v1::session_update(instructions),
            RealtimeApiMode::V2 => mode_v2::session_update(instructions),
        };
        self.send_json(message).await
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
        debug!(?message, "realtime websocket request");

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
    fn mode(&self) -> RealtimeApiMode {
        self.mode
    }

    pub async fn next_event(&self) -> Result<Option<RealtimeEvent>, ApiError> {
        if self.is_closed.load(Ordering::SeqCst) {
            return Ok(None);
        }

        loop {
            let msg = match self.rx_message.lock().await.recv().await {
                Some(Ok(msg)) => msg,
                Some(Err(err)) => {
                    self.is_closed.store(true, Ordering::SeqCst);
                    error!("realtime websocket read failed: {err}");
                    return Err(ApiError::Stream(format!(
                        "failed to read websocket message: {err}"
                    )));
                }
                None => {
                    self.is_closed.store(true, Ordering::SeqCst);
                    info!("realtime websocket event stream ended");
                    return Ok(None);
                }
            };

            match msg {
                Message::Text(text) => {
                    if let Some(event) = parse_realtime_event(&text, self.mode()) {
                        debug!(?event, "realtime websocket parsed event");
                        return Ok(Some(event));
                    }
                    debug!("realtime websocket ignored unsupported text frame");
                }
                Message::Close(frame) => {
                    self.is_closed.store(true, Ordering::SeqCst);
                    info!(
                        "realtime websocket closed: code={:?} reason={:?}",
                        frame.as_ref().map(|frame| frame.code),
                        frame.as_ref().map(|frame| frame.reason.as_str())
                    );
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

fn parse_realtime_event(payload: &str, mode: RealtimeApiMode) -> Option<RealtimeEvent> {
    match mode {
        RealtimeApiMode::V1 => protocol_v1::parse_realtime_event(payload),
        RealtimeApiMode::V2 => protocol_v2::parse_realtime_event(payload),
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
            self.provider.base_url.as_str(),
            self.provider.query_params.as_ref(),
            config.model.as_deref(),
            config.mode,
        )?;

        let mut request = ws_url
            .as_str()
            .into_client_request()
            .map_err(|err| ApiError::Stream(format!("failed to build websocket request: {err}")))?;
        let headers = merge_request_headers(
            &self.provider.headers,
            with_session_id_header(extra_headers, config.session_id.as_deref())?,
            default_headers,
        );
        request.headers_mut().extend(headers);

        info!("connecting realtime websocket: {ws_url}");
        let (stream, response) =
            tokio_tungstenite::connect_async_with_config(request, Some(websocket_config()), false)
                .await
                .map_err(|err| {
                    ApiError::Stream(format!("failed to connect realtime websocket: {err}"))
                })?;
        info!(
            ws_url = %ws_url,
            status = %response.status(),
            "realtime websocket connected"
        );

        let (stream, rx_message) = WsStream::new(stream);
        let connection = RealtimeWebsocketConnection::new(stream, rx_message, config.mode);
        debug!(
            session_id = config.session_id.as_deref().unwrap_or("<none>"),
            "realtime websocket sending session.update"
        );
        connection
            .writer
            .send_session_update(config.instructions)
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
        if let Entry::Vacant(entry) = headers.entry(name) {
            entry.insert(value.clone());
        }
    }
    headers
}

fn with_session_id_header(
    mut headers: HeaderMap,
    session_id: Option<&str>,
) -> Result<HeaderMap, ApiError> {
    let Some(session_id) = session_id else {
        return Ok(headers);
    };
    headers.insert(
        "x-session-id",
        HeaderValue::from_str(session_id).map_err(|err| {
            ApiError::Stream(format!("invalid realtime session id header: {err}"))
        })?,
    );
    Ok(headers)
}

fn websocket_config() -> WebSocketConfig {
    WebSocketConfig::default()
}

fn websocket_url_from_api_url(
    api_url: &str,
    query_params: Option<&HashMap<String, String>>,
    model: Option<&str>,
    mode: RealtimeApiMode,
) -> Result<Url, ApiError> {
    let mut url = Url::parse(api_url)
        .map_err(|err| ApiError::Stream(format!("failed to parse realtime api_url: {err}")))?;

    normalize_realtime_path(&mut url);

    match url.scheme() {
        "ws" | "wss" => {}
        "http" | "https" => {
            let scheme = if url.scheme() == "http" { "ws" } else { "wss" };
            let _ = url.set_scheme(scheme);
        }
        scheme => {
            return Err(ApiError::Stream(format!(
                "unsupported realtime api_url scheme: {scheme}"
            )));
        }
    }

    match mode {
        RealtimeApiMode::V1 => mode_v1::append_query_params(&mut url, query_params, model),
        RealtimeApiMode::V2 => mode_v2::append_query_params(&mut url, query_params, model),
    }

    Ok(url)
}

fn normalize_realtime_path(url: &mut Url) {
    let path = url.path().to_string();
    if path.is_empty() || path == "/" {
        url.set_path("/v1/realtime");
        return;
    }

    if path.ends_with("/realtime") {
        return;
    }

    if path.ends_with("/realtime/") {
        url.set_path(path.trim_end_matches('/'));
        return;
    }

    if path.ends_with("/v1") {
        url.set_path(&format!("{path}/realtime"));
        return;
    }

    if path.ends_with("/v1/") {
        url.set_path(&format!("{path}realtime"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::endpoint::realtime_websocket::types::RealtimeHandoffMessage;
    use crate::endpoint::realtime_websocket::types::RealtimeHandoffRequested;
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
    fn parse_session_updated_event() {
        let payload = json!({
            "type": "session.updated",
            "session": {"id": "sess_123", "instructions": "backend prompt"}
        })
        .to_string();

        assert_eq!(
            parse_realtime_event(payload.as_str(), RealtimeApiMode::V2),
            Some(RealtimeEvent::SessionUpdated {
                session_id: "sess_123".to_string(),
                instructions: Some("backend prompt".to_string()),
            })
        );
    }

    #[test]
    fn parse_audio_delta_event() {
        let payload = json!({
            "type": "response.output_audio.delta",
            "delta": "AAA=",
            "sample_rate": 48000,
            "channels": 1,
            "samples_per_channel": 960
        })
        .to_string();
        assert_eq!(
            parse_realtime_event(payload.as_str(), RealtimeApiMode::V2),
            Some(RealtimeEvent::AudioOut(RealtimeAudioFrame {
                data: "AAA=".to_string(),
                sample_rate: 48000,
                num_channels: 1,
                samples_per_channel: Some(960),
            }))
        );
    }

    #[test]
    fn parse_audio_delta_event_defaults_audio_shape() {
        let payload = json!({
            "type": "response.output_audio.delta",
            "delta": "AAA="
        })
        .to_string();

        assert_eq!(
            parse_realtime_event(payload.as_str(), RealtimeApiMode::V2),
            Some(RealtimeEvent::AudioOut(RealtimeAudioFrame {
                data: "AAA=".to_string(),
                sample_rate: 24_000,
                num_channels: 1,
                samples_per_channel: None,
            }))
        );
    }

    #[test]
    fn parse_conversation_item_added_event() {
        let payload = json!({
            "type": "conversation.item.added",
            "item": {"type": "message", "seq": 7}
        })
        .to_string();
        assert_eq!(
            parse_realtime_event(payload.as_str(), RealtimeApiMode::V2),
            Some(RealtimeEvent::ConversationItemAdded(
                json!({"type": "message", "seq": 7})
            ))
        );
    }

    #[test]
    fn parse_conversation_item_done_event() {
        let payload = json!({
            "type": "conversation.item.done",
            "item": {"id": "item_123", "type": "message"}
        })
        .to_string();
        assert_eq!(
            parse_realtime_event(payload.as_str(), RealtimeApiMode::V2),
            Some(RealtimeEvent::ConversationItemDone {
                item_id: "item_123".to_string(),
            })
        );
    }

    #[test]
    fn parse_handoff_requested_event() {
        let payload = json!({
            "type": "response.done",
            "response": {
                "output": [
                    {
                        "id": "item_123",
                        "type": "function_call",
                        "name": "codex",
                        "call_id": "handoff_123",
                        "arguments": "{\"prompt\":\"delegate this\"}"
                    }
                ]
            }
        })
        .to_string();

        assert_eq!(
            parse_realtime_event(payload.as_str(), RealtimeApiMode::V2),
            Some(RealtimeEvent::HandoffRequested(RealtimeHandoffRequested {
                handoff_id: "handoff_123".to_string(),
                item_id: "item_123".to_string(),
                input_transcript: "delegate this".to_string(),
                messages: vec![RealtimeHandoffMessage {
                    role: "user".to_string(),
                    text: "delegate this".to_string(),
                }],
            }))
        );
    }

    #[test]
    fn parse_v1_handoff_requested_event() {
        let payload = json!({
            "type": "conversation.handoff.requested",
            "handoff_id": "handoff_legacy",
            "item_id": "item_legacy",
            "input_transcript": "delegate legacy",
            "messages": [
                {"role": "user", "text": "delegate legacy"}
            ]
        })
        .to_string();

        assert_eq!(
            parse_realtime_event(payload.as_str(), RealtimeApiMode::V1),
            Some(RealtimeEvent::HandoffRequested(RealtimeHandoffRequested {
                handoff_id: "handoff_legacy".to_string(),
                item_id: "item_legacy".to_string(),
                input_transcript: "delegate legacy".to_string(),
                messages: vec![RealtimeHandoffMessage {
                    role: "user".to_string(),
                    text: "delegate legacy".to_string(),
                }],
            }))
        );
    }

    #[test]
    fn parse_v1_audio_delta_event() {
        let payload = json!({
            "type": "conversation.output_audio.delta",
            "delta": "AAA=",
            "sample_rate": 48000,
            "channels": 1,
            "samples_per_channel": 960
        })
        .to_string();
        assert_eq!(
            parse_realtime_event(payload.as_str(), RealtimeApiMode::V1),
            Some(RealtimeEvent::AudioOut(RealtimeAudioFrame {
                data: "AAA=".to_string(),
                sample_rate: 48000,
                num_channels: 1,
                samples_per_channel: Some(960),
            }))
        );
    }

    #[test]
    fn parse_unknown_event_as_conversation_item_added() {
        let payload = json!({
            "type": "response.output_text.delta",
            "delta": "hello",
            "response_id": "resp_1",
        })
        .to_string();
        assert_eq!(
            parse_realtime_event(payload.as_str(), RealtimeApiMode::V2),
            Some(RealtimeEvent::ConversationItemAdded(json!({
                "type": "response.output_text.delta",
                "delta": "hello",
                "response_id": "resp_1",
            })))
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
        let url =
            websocket_url_from_api_url("http://127.0.0.1:8011", None, None, RealtimeApiMode::V2)
                .expect("build ws url");
        assert_eq!(url.as_str(), "ws://127.0.0.1:8011/v1/realtime");
    }

    #[test]
    fn websocket_url_from_ws_base_defaults_to_ws_path() {
        let url = websocket_url_from_api_url(
            "wss://example.com",
            None,
            Some("realtime-test-model"),
            RealtimeApiMode::V2,
        )
        .expect("build ws url");
        assert_eq!(
            url.as_str(),
            "wss://example.com/v1/realtime?model=realtime-test-model"
        );
    }

    #[test]
    fn websocket_url_from_v1_base_appends_realtime_path() {
        let url = websocket_url_from_api_url(
            "https://api.openai.com/v1",
            None,
            Some("snapshot"),
            RealtimeApiMode::V2,
        )
        .expect("build ws url");
        assert_eq!(
            url.as_str(),
            "wss://api.openai.com/v1/realtime?model=snapshot"
        );
    }

    #[test]
    fn websocket_url_from_nested_v1_base_appends_realtime_path() {
        let url = websocket_url_from_api_url(
            "https://example.com/openai/v1",
            None,
            Some("snapshot"),
            RealtimeApiMode::V2,
        )
        .expect("build ws url");
        assert_eq!(
            url.as_str(),
            "wss://example.com/openai/v1/realtime?model=snapshot"
        );
    }

    #[test]
    fn websocket_url_preserves_existing_realtime_path_and_extra_query_params() {
        let url = websocket_url_from_api_url(
            "https://example.com/v1/realtime?foo=bar",
            Some(&HashMap::from([("trace".to_string(), "1".to_string())])),
            Some("snapshot"),
            RealtimeApiMode::V2,
        )
        .expect("build ws url");
        assert_eq!(
            url.as_str(),
            "wss://example.com/v1/realtime?foo=bar&model=snapshot&trace=1"
        );
    }

    #[test]
    fn websocket_url_v1_mode_adds_quicksilver_intent() {
        let url = websocket_url_from_api_url(
            "https://example.com/v1/realtime?foo=bar",
            Some(&HashMap::from([("trace".to_string(), "1".to_string())])),
            Some("snapshot"),
            RealtimeApiMode::V1,
        )
        .expect("build ws url");
        assert_eq!(
            url.as_str(),
            "wss://example.com/v1/realtime?foo=bar&intent=quicksilver&model=snapshot&trace=1"
        );
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
            assert_eq!(first_json["type"], "session.update");
            assert_eq!(
                first_json["session"]["type"],
                Value::String("realtime".to_string())
            );
            assert_eq!(
                first_json["session"]["instructions"],
                Value::String("backend prompt".to_string())
            );
            assert_eq!(
                first_json["session"]["output_modalities"][0],
                Value::String("audio".to_string())
            );
            assert_eq!(
                first_json["session"]["audio"]["input"]["format"]["type"],
                Value::String("audio/pcm".to_string())
            );
            assert_eq!(
                first_json["session"]["audio"]["input"]["format"]["rate"],
                Value::from(24_000)
            );
            assert_eq!(
                first_json["session"]["audio"]["input"]["turn_detection"]["type"],
                Value::String("semantic_vad".to_string())
            );
            assert_eq!(
                first_json["session"]["audio"]["input"]["turn_detection"]["interrupt_response"],
                Value::Bool(false)
            );
            assert_eq!(
                first_json["session"]["audio"]["input"]["turn_detection"]["create_response"],
                Value::Bool(true)
            );
            assert_eq!(
                first_json["session"]["audio"]["output"]["format"]["type"],
                Value::String("audio/pcm".to_string())
            );
            assert_eq!(
                first_json["session"]["audio"]["output"]["format"]["rate"],
                Value::from(24_000)
            );
            assert_eq!(
                first_json["session"]["audio"]["output"]["voice"],
                Value::String("marin".to_string())
            );
            assert_eq!(
                first_json["session"]["tool_choice"],
                Value::String("auto".to_string())
            );
            assert_eq!(
                first_json["session"]["tools"][0]["type"],
                Value::String("function".to_string())
            );
            assert_eq!(
                first_json["session"]["tools"][0]["name"],
                Value::String("codex".to_string())
            );
            assert_eq!(
                first_json["session"]["tools"][0]["parameters"]["required"][0],
                Value::String("prompt".to_string())
            );

            ws.send(Message::Text(
                json!({
                    "type": "session.updated",
                    "session": {"id": "sess_mock", "instructions": "backend prompt"}
                })
                .to_string()
                .into(),
            ))
            .await
            .expect("send session.updated");

            let second = ws
                .next()
                .await
                .expect("second msg")
                .expect("second msg ok")
                .into_text()
                .expect("text");
            let second_json: Value = serde_json::from_str(&second).expect("json");
            assert_eq!(second_json["type"], "input_audio_buffer.append");

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

            let fourth = ws
                .next()
                .await
                .expect("fourth msg")
                .expect("fourth msg ok")
                .into_text()
                .expect("text");
            let fourth_json: Value = serde_json::from_str(&fourth).expect("json");
            assert_eq!(fourth_json["type"], "conversation.item.create");
            assert_eq!(fourth_json["item"]["type"], "message");
            assert_eq!(fourth_json["item"]["role"], "assistant");
            assert_eq!(
                fourth_json["item"]["content"][0]["type"],
                Value::String("output_text".to_string())
            );
            assert_eq!(
                fourth_json["item"]["content"][0]["text"],
                Value::String("hello from codex".to_string())
            );

            let fifth = ws
                .next()
                .await
                .expect("fifth msg")
                .expect("fifth msg ok")
                .into_text()
                .expect("text");
            let fifth_json: Value = serde_json::from_str(&fifth).expect("json");
            assert_eq!(fifth_json["type"], "conversation.item.create");
            assert_eq!(fifth_json["item"]["type"], "function_call_output");
            assert_eq!(fifth_json["item"]["call_id"], "handoff_1");

            let sixth = ws
                .next()
                .await
                .expect("sixth msg")
                .expect("sixth msg ok")
                .into_text()
                .expect("text");
            let sixth_json: Value = serde_json::from_str(&sixth).expect("json");
            assert_eq!(sixth_json["type"], "response.create");

            ws.send(Message::Text(
                json!({
                    "type": "response.output_audio.delta",
                    "delta": "AQID",
                    "sample_rate": 48000,
                    "channels": 1
                })
                .to_string()
                .into(),
            ))
            .await
            .expect("send audio");

            ws.send(Message::Text(
                json!({
                    "type": "response.done",
                    "response": {
                        "output": [
                            {
                                "id": "item_2",
                                "type": "function_call",
                                "name": "codex",
                                "call_id": "handoff_1",
                                "arguments": "{\"prompt\":\"delegate now\"}"
                            }
                        ]
                    }
                })
                .to_string()
                .into(),
            ))
            .await
            .expect("send item added");
        });

        let provider = Provider {
            name: "test".to_string(),
            base_url: format!("http://{addr}"),
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
                    instructions: "backend prompt".to_string(),
                    model: Some("realtime-test-model".to_string()),
                    session_id: Some("conv_1".to_string()),
                    mode: RealtimeApiMode::V2,
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
            RealtimeEvent::SessionUpdated {
                session_id: "sess_mock".to_string(),
                instructions: Some("backend prompt".to_string()),
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
        connection
            .send_conversation_handoff_append(
                "handoff_1".to_string(),
                "hello from codex".to_string(),
            )
            .await
            .expect("send handoff");
        connection
            .send_function_call_output("handoff_1".to_string(), "final from codex".to_string())
            .await
            .expect("send function output");
        connection
            .send_response_create()
            .await
            .expect("send response.create");

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
            RealtimeEvent::HandoffRequested(RealtimeHandoffRequested {
                handoff_id: "handoff_1".to_string(),
                item_id: "item_2".to_string(),
                input_transcript: "delegate now".to_string(),
                messages: vec![RealtimeHandoffMessage {
                    role: "user".to_string(),
                    text: "delegate now".to_string(),
                }],
            })
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
            assert_eq!(first_json["type"], "session.update");

            let second = ws
                .next()
                .await
                .expect("second msg")
                .expect("second msg ok")
                .into_text()
                .expect("text");
            let second_json: Value = serde_json::from_str(&second).expect("json");
            assert_eq!(second_json["type"], "input_audio_buffer.append");

            ws.send(Message::Text(
                json!({
                    "type": "session.updated",
                    "session": {"id": "sess_after_send", "instructions": "backend prompt"}
                })
                .to_string()
                .into(),
            ))
            .await
            .expect("send session.updated");
        });

        let provider = Provider {
            name: "test".to_string(),
            base_url: format!("http://{addr}"),
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
                    instructions: "backend prompt".to_string(),
                    model: Some("realtime-test-model".to_string()),
                    session_id: Some("conv_1".to_string()),
                    mode: RealtimeApiMode::V2,
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
            RealtimeEvent::SessionUpdated {
                session_id: "sess_after_send".to_string(),
                instructions: Some("backend prompt".to_string()),
            }
        );

        connection.close().await.expect("close");
        server.await.expect("server task");
    }
}
