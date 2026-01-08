use crate::auth::AuthProvider;
use crate::common::Prompt as ApiPrompt;
use crate::common::ResponseEvent;
use crate::common::ResponseStream;
use crate::endpoint::responses::ResponsesOptions;
use crate::error::ApiError;
use crate::provider::Provider;
use crate::requests::ResponsesRequestBuilder;
use codex_client::TransportError;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::TokenUsage;
use futures::SinkExt;
use futures::StreamExt;
use http::HeaderMap;
use http::HeaderValue;
use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio::sync::OwnedSemaphorePermit;
use tokio::sync::Semaphore;
use tokio::sync::mpsc;
use tokio_tungstenite::MaybeTlsStream;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite;
use tokio_tungstenite::tungstenite::Message;
use tracing::debug;
use tracing::trace;
use url::Url;

const WS_BUFFER: usize = 1600;

type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;
type WsSender = futures::stream::SplitSink<WsStream, Message>;

#[derive(Clone)]
pub struct ResponsesWsSession<A: AuthProvider + Clone> {
    inner: Arc<ResponsesWsInner<A>>,
}

struct ResponsesWsInner<A: AuthProvider + Clone> {
    provider: Provider,
    auth: A,
    connection: Mutex<Option<Arc<ResponsesWsConnection>>>,
    state: Arc<Mutex<WsSessionState>>,
    turn_gate: Arc<Semaphore>,
}

#[derive(Default)]
struct WsSessionState {
    last_sent_len: usize,
    active: bool,
}

struct ResponsesWsConnection {
    sender: Mutex<WsSender>,
    receiver: Mutex<mpsc::Receiver<Result<String, ApiError>>>,
}

impl<A: AuthProvider + Clone> ResponsesWsSession<A> {
    pub fn new(provider: Provider, auth: A) -> Self {
        Self {
            inner: Arc::new(ResponsesWsInner {
                provider,
                auth,
                connection: Mutex::new(None),
                state: Arc::new(Mutex::new(WsSessionState::default())),
                turn_gate: Arc::new(Semaphore::new(1)),
            }),
        }
    }

    pub async fn reset(&self) {
        {
            let mut guard = self.inner.connection.lock().await;
            *guard = None;
        }
        let mut state = self.inner.state.lock().await;
        state.last_sent_len = 0;
        state.active = false;
    }

    pub async fn stream_prompt(
        &self,
        model: &str,
        prompt: &ApiPrompt,
        options: ResponsesOptions,
    ) -> Result<ResponseStream, ApiError> {
        let ResponsesOptions {
            reasoning,
            include,
            prompt_cache_key,
            text,
            store_override,
            conversation_id,
            session_source,
            extra_headers,
        } = options;

        let request = ResponsesRequestBuilder::new(model, &prompt.instructions, &prompt.input)
            .tools(&prompt.tools)
            .parallel_tool_calls(prompt.parallel_tool_calls)
            .reasoning(reasoning)
            .include(include)
            .prompt_cache_key(prompt_cache_key)
            .text(text)
            .conversation(conversation_id)
            .session_source(session_source)
            .store_override(store_override)
            .extra_headers(extra_headers)
            .build(&self.inner.provider)?;

        let input_len = prompt.input.len();
        let event = {
            let mut state = self.inner.state.lock().await;
            let should_reset = !state.active || input_len < state.last_sent_len;
            if should_reset {
                state.last_sent_len = 0;
            }
            state.active = true;
            if should_reset {
                build_create_event(request.body)?
            } else {
                let delta = prompt
                    .input
                    .get(state.last_sent_len..)
                    .unwrap_or_default()
                    .to_vec();
                build_append_event(delta)
            }
        };

        let permit = self
            .inner
            .turn_gate
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| ApiError::Stream("responses websocket closed".into()))?;

        let connection = self.ensure_connection(request.headers).await?;
        if let Err(err) = connection.send(&event).await {
            self.reset().await;
            return Err(err);
        }

        Ok(spawn_ws_response_stream(
            connection,
            self.inner.state.clone(),
            input_len,
            permit,
        ))
    }

    async fn ensure_connection(
        &self,
        extra_headers: HeaderMap,
    ) -> Result<Arc<ResponsesWsConnection>, ApiError> {
        let existing = { self.inner.connection.lock().await.clone() };
        if let Some(connection) = existing {
            return Ok(connection);
        }

        let connection =
            ResponsesWsConnection::connect(&self.inner.provider, &self.inner.auth, extra_headers)
                .await?;
        let connection = Arc::new(connection);

        let mut guard = self.inner.connection.lock().await;
        if guard.is_none() {
            *guard = Some(connection.clone());
        }
        Ok(connection)
    }
}

impl ResponsesWsConnection {
    async fn connect<A: AuthProvider>(
        provider: &Provider,
        auth: &A,
        extra_headers: HeaderMap,
    ) -> Result<Self, ApiError> {
        let url = ws_url(provider)?;
        let headers = build_ws_headers(provider, auth, extra_headers);
        let request = build_ws_request(url, headers)?;
        let (stream, _response) = connect_async(request).await.map_err(map_ws_error)?;
        let (sender, mut receiver) = stream.split();
        let (tx, rx) = mpsc::channel(WS_BUFFER);

        tokio::spawn(async move {
            loop {
                let message = receiver.next().await;
                let message = match message {
                    Some(Ok(message)) => message,
                    Some(Err(err)) => {
                        let _ = tx
                            .send(Err(ApiError::Stream(format!("websocket error: {err}"))))
                            .await;
                        return;
                    }
                    None => {
                        let _ = tx
                            .send(Err(ApiError::Stream(
                                "websocket closed unexpectedly".into(),
                            )))
                            .await;
                        return;
                    }
                };

                match message {
                    Message::Text(text) => {
                        if tx.send(Ok(text.to_string())).await.is_err() {
                            return;
                        }
                    }
                    Message::Binary(bytes) => {
                        if let Ok(text) = String::from_utf8(bytes.to_vec())
                            && tx.send(Ok(text)).await.is_err()
                        {
                            return;
                        }
                    }
                    Message::Close(_) => {
                        let _ = tx
                            .send(Err(ApiError::Stream("websocket closed".into())))
                            .await;
                        return;
                    }
                    Message::Ping(_) | Message::Pong(_) => {}
                    _ => {}
                }
            }
        });

        Ok(Self {
            sender: Mutex::new(sender),
            receiver: Mutex::new(rx),
        })
    }

    async fn send(&self, payload: &Value) -> Result<(), ApiError> {
        let text = serde_json::to_string(payload)
            .map_err(|err| ApiError::Stream(format!("failed to encode ws payload: {err}")))?;
        let mut sender = self.sender.lock().await;
        sender
            .send(Message::Text(text.into()))
            .await
            .map_err(|err| ApiError::Stream(format!("websocket send failed: {err}")))
    }
}

fn build_create_event(body: Value) -> Result<Value, ApiError> {
    let Value::Object(mut payload) = body else {
        return Err(ApiError::Stream(
            "responses create body was not an object".into(),
        ));
    };
    payload.remove("stream");
    payload.remove("background");
    let mut event = serde_json::Map::new();
    event.insert(
        "type".to_string(),
        Value::String("response.create".to_string()),
    );
    event.extend(payload);
    Ok(Value::Object(event))
}

fn build_append_event(input: Vec<ResponseItem>) -> Value {
    serde_json::json!({
        "type": "response.append",
        "input": input,
    })
}

fn ws_url(provider: &Provider) -> Result<Url, ApiError> {
    let url = provider.url_for_path("responses");
    let mut url = Url::parse(&url)
        .map_err(|err| ApiError::Stream(format!("invalid websocket url: {err}")))?;
    let scheme = match url.scheme() {
        "https" => "wss",
        "http" => "ws",
        "wss" => "wss",
        "ws" => "ws",
        other => {
            return Err(ApiError::Stream(format!(
                "unsupported websocket scheme: {other}"
            )));
        }
    };
    if url.scheme() != scheme {
        url.set_scheme(scheme)
            .map_err(|_| ApiError::Stream("failed to set websocket scheme".into()))?;
    }
    Ok(url)
}

fn build_ws_headers<A: AuthProvider>(
    provider: &Provider,
    auth: &A,
    extra_headers: HeaderMap,
) -> HeaderMap {
    let mut headers = provider.headers.clone();
    headers.extend(extra_headers);
    if let Some(token) = auth.bearer_token()
        && let Ok(header) = format!("Bearer {token}").parse()
    {
        let _ = headers.insert(http::header::AUTHORIZATION, header);
    }
    if let Some(account_id) = auth.account_id()
        && let Ok(header) = HeaderValue::from_str(&account_id)
    {
        let _ = headers.insert("ChatGPT-Account-ID", header);
    }
    headers
}

fn build_ws_request(url: Url, headers: HeaderMap) -> Result<http::Request<()>, ApiError> {
    let mut builder = http::Request::builder()
        .method(http::Method::GET)
        .uri(url.as_str());
    for (name, value) in headers.iter() {
        builder = builder.header(name, value);
    }
    builder
        .body(())
        .map_err(|err| ApiError::Stream(format!("failed to build websocket request: {err}")))
}

fn map_ws_error(err: tungstenite::Error) -> ApiError {
    let transport = match err {
        tungstenite::Error::Http(response) => TransportError::Http {
            status: response.status(),
            headers: Some(response.headers().clone()),
            body: None,
        },
        tungstenite::Error::Url(err) => TransportError::Build(err.to_string()),
        tungstenite::Error::Io(err) => TransportError::Network(err.to_string()),
        other => TransportError::Network(other.to_string()),
    };
    ApiError::Transport(transport)
}

fn spawn_ws_response_stream(
    connection: Arc<ResponsesWsConnection>,
    state: Arc<Mutex<WsSessionState>>,
    input_len: usize,
    permit: OwnedSemaphorePermit,
) -> ResponseStream {
    let (tx_event, rx_event) = mpsc::channel::<Result<ResponseEvent, ApiError>>(WS_BUFFER);
    tokio::spawn(async move {
        let _permit = permit;
        let mut output_count: usize = 0;
        let mut draining = false;
        let mut can_send = true;
        let mut receiver = connection.receiver.lock().await;
        loop {
            let message = receiver.recv().await;
            let message = match message {
                Some(message) => message,
                None => {
                    if can_send && !draining {
                        let _ = tx_event
                            .send(Err(ApiError::Stream(
                                "websocket closed while awaiting responses".into(),
                            )))
                            .await;
                    }
                    let mut state = state.lock().await;
                    state.active = false;
                    state.last_sent_len = 0;
                    return;
                }
            };

            match message {
                Ok(text) => {
                    trace!("WS event: {text}");
                    let event: WsEvent = match serde_json::from_str(&text) {
                        Ok(event) => event,
                        Err(err) => {
                            debug!("Failed to parse WS event: {err}");
                            continue;
                        }
                    };

                    match event.kind.as_str() {
                        "response.output_item.done" => {
                            let Some(item_val) = event.item else {
                                continue;
                            };
                            let Ok(item) = serde_json::from_value::<ResponseItem>(item_val) else {
                                debug!("failed to parse ResponseItem from output_item.done");
                                continue;
                            };
                            output_count = output_count.saturating_add(1);
                            if can_send
                                && tx_event
                                    .send(Ok(ResponseEvent::OutputItemDone(item)))
                                    .await
                                    .is_err()
                            {
                                can_send = false;
                            }
                        }
                        "response.output_item.added" => {
                            let Some(item_val) = event.item else {
                                continue;
                            };
                            let Ok(item) = serde_json::from_value::<ResponseItem>(item_val) else {
                                debug!("failed to parse ResponseItem from output_item.added");
                                continue;
                            };
                            if can_send
                                && tx_event
                                    .send(Ok(ResponseEvent::OutputItemAdded(item)))
                                    .await
                                    .is_err()
                            {
                                can_send = false;
                            }
                        }
                        "response.output_text.delta" => {
                            if let Some(delta) = event.delta
                                && can_send
                                && tx_event
                                    .send(Ok(ResponseEvent::OutputTextDelta(delta)))
                                    .await
                                    .is_err()
                            {
                                can_send = false;
                            }
                        }
                        "response.reasoning_summary_text.delta" => {
                            if let (Some(delta), Some(summary_index)) =
                                (event.delta, event.summary_index)
                                && can_send
                                && tx_event
                                    .send(Ok(ResponseEvent::ReasoningSummaryDelta {
                                        delta,
                                        summary_index,
                                    }))
                                    .await
                                    .is_err()
                            {
                                can_send = false;
                            }
                        }
                        "response.reasoning_text.delta" => {
                            if let (Some(delta), Some(content_index)) =
                                (event.delta, event.content_index)
                                && can_send
                                && tx_event
                                    .send(Ok(ResponseEvent::ReasoningContentDelta {
                                        delta,
                                        content_index,
                                    }))
                                    .await
                                    .is_err()
                            {
                                can_send = false;
                            }
                        }
                        "response.reasoning_summary_part.added" => {
                            if let Some(summary_index) = event.summary_index
                                && can_send
                                && tx_event
                                    .send(Ok(ResponseEvent::ReasoningSummaryPartAdded {
                                        summary_index,
                                    }))
                                    .await
                                    .is_err()
                            {
                                can_send = false;
                            }
                        }
                        "response.created" => {
                            if can_send
                                && tx_event.send(Ok(ResponseEvent::Created {})).await.is_err()
                            {
                                can_send = false;
                            }
                        }
                        "response.failed" => {
                            let error = map_failed_response(&event);
                            if can_send && tx_event.send(Err(error)).await.is_err() {
                                can_send = false;
                            }
                            let mut state = state.lock().await;
                            state.active = false;
                            state.last_sent_len = 0;
                            draining = true;
                        }
                        "response.done" | "response.completed" => {
                            let completed = match completed_event(&event) {
                                Ok(event) => event,
                                Err(err) => {
                                    if can_send {
                                        let _ = tx_event.send(Err(err)).await;
                                    }
                                    let mut state = state.lock().await;
                                    state.active = false;
                                    state.last_sent_len = 0;
                                    return;
                                }
                            };

                            if !draining {
                                if can_send {
                                    let _ = tx_event.send(Ok(completed)).await;
                                }
                                let mut state = state.lock().await;
                                state.last_sent_len = input_len.saturating_add(output_count);
                                state.active = true;
                            }
                            return;
                        }
                        _ => {}
                    }
                }
                Err(err) => {
                    if can_send && !draining {
                        let _ = tx_event.send(Err(err)).await;
                    }
                    let mut state = state.lock().await;
                    state.active = false;
                    state.last_sent_len = 0;
                    return;
                }
            }
        }
    });

    ResponseStream { rx_event }
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Error {
    r#type: Option<String>,
    code: Option<String>,
    message: Option<String>,
    plan_type: Option<String>,
    resets_at: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct ResponseCompleted {
    id: String,
    #[serde(default)]
    usage: Option<ResponseUsage>,
}

#[derive(Debug, Deserialize, Clone)]
struct ResponseUsage {
    #[serde(default)]
    input_tokens: i64,
    #[serde(default)]
    input_tokens_details: Option<ResponseInputTokensDetails>,
    #[serde(default)]
    output_tokens: i64,
    #[serde(default)]
    output_tokens_details: Option<ResponseOutputTokensDetails>,
    #[serde(default)]
    total_tokens: i64,
}

impl From<ResponseUsage> for TokenUsage {
    fn from(value: ResponseUsage) -> Self {
        TokenUsage {
            input_tokens: value.input_tokens,
            cached_input_tokens: value
                .input_tokens_details
                .map(|d| d.cached_tokens)
                .unwrap_or(0),
            output_tokens: value.output_tokens,
            reasoning_output_tokens: value
                .output_tokens_details
                .map(|d| d.reasoning_tokens)
                .unwrap_or(0),
            total_tokens: value.total_tokens,
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
struct ResponseInputTokensDetails {
    cached_tokens: i64,
}

#[derive(Debug, Deserialize, Clone)]
struct ResponseOutputTokensDetails {
    reasoning_tokens: i64,
}

#[derive(Deserialize, Debug)]
struct WsEvent {
    #[serde(rename = "type")]
    kind: String,
    response: Option<Value>,
    item: Option<Value>,
    delta: Option<String>,
    summary_index: Option<i64>,
    content_index: Option<i64>,
    #[serde(default)]
    usage: Option<ResponseUsage>,
}

fn completed_event(event: &WsEvent) -> Result<ResponseEvent, ApiError> {
    if let Some(response) = &event.response {
        let completed =
            serde_json::from_value::<ResponseCompleted>(response.clone()).map_err(|err| {
                ApiError::Stream(format!("failed to parse response.completed: {err}"))
            })?;
        return Ok(ResponseEvent::Completed {
            response_id: completed.id,
            token_usage: completed.usage.map(Into::into),
        });
    }

    if let Some(usage) = event.usage.clone() {
        return Ok(ResponseEvent::Completed {
            response_id: String::new(),
            token_usage: Some(usage.into()),
        });
    }

    Ok(ResponseEvent::Completed {
        response_id: String::new(),
        token_usage: None,
    })
}

fn map_failed_response(event: &WsEvent) -> ApiError {
    let Some(resp_val) = event.response.clone() else {
        return ApiError::Stream("response.failed event received".into());
    };

    let Some(error) = resp_val.get("error") else {
        return ApiError::Stream("response.failed event received".into());
    };

    let Ok(error) = serde_json::from_value::<Error>(error.clone()) else {
        return ApiError::Stream("response.failed event received".into());
    };

    if is_context_window_error(&error) {
        ApiError::ContextWindowExceeded
    } else if is_quota_exceeded_error(&error) {
        ApiError::QuotaExceeded
    } else if is_usage_not_included(&error) {
        ApiError::UsageNotIncluded
    } else {
        let delay = try_parse_retry_after(&error);
        let message = error.message.unwrap_or_default();
        ApiError::Retryable { message, delay }
    }
}

fn try_parse_retry_after(err: &Error) -> Option<std::time::Duration> {
    if err.code.as_deref() != Some("rate_limit_exceeded") {
        return None;
    }

    let re = rate_limit_regex();
    if let Some(message) = &err.message
        && let Some(captures) = re.captures(message)
    {
        let seconds = captures.get(1);
        let unit = captures.get(2);

        if let (Some(value), Some(unit)) = (seconds, unit) {
            let value = value.as_str().parse::<f64>().ok()?;
            let unit = unit.as_str().to_ascii_lowercase();

            if unit == "s" || unit.starts_with("second") {
                return Some(std::time::Duration::from_secs_f64(value));
            } else if unit == "ms" {
                return Some(std::time::Duration::from_millis(value as u64));
            }
        }
    }
    None
}

fn is_context_window_error(error: &Error) -> bool {
    error.code.as_deref() == Some("context_length_exceeded")
}

fn is_quota_exceeded_error(error: &Error) -> bool {
    error.code.as_deref() == Some("insufficient_quota")
}

fn is_usage_not_included(error: &Error) -> bool {
    error.code.as_deref() == Some("usage_not_included")
}

fn rate_limit_regex() -> &'static regex_lite::Regex {
    static RE: std::sync::OnceLock<regex_lite::Regex> = std::sync::OnceLock::new();
    #[expect(clippy::unwrap_used)]
    RE.get_or_init(|| {
        regex_lite::Regex::new(r"(?i)try again in\\s*(\\d+(?:\\.\\d+)?)\\s*(s|ms|seconds?)")
            .unwrap()
    })
}
