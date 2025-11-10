use std::io::BufRead;
use std::path::Path;
use std::sync::Arc;
use std::sync::OnceLock;
use std::time::Duration;

use async_trait::async_trait;
use bytes::Bytes;
use codex_app_server_protocol::AuthMode;
use codex_otel::otel_event_manager::OtelEventManager;
use codex_protocol::ConversationId;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::RateLimitSnapshot;
use codex_protocol::protocol::RateLimitWindow;
use codex_protocol::protocol::TokenUsage;
use futures::Stream;
use futures::StreamExt;
use futures::TryStreamExt;
use regex_lite::Regex;
use reqwest::StatusCode;
use reqwest::header::HeaderMap;
use serde::Deserialize;
use serde_json::Value;
use serde_json::json;
use tokio::sync::mpsc;
use tokio::time::timeout;
use tokio_util::io::ReaderStream;
use tracing::debug;
use tracing::trace;

use crate::api::ApiClient;
use crate::auth::AuthProvider;
use crate::common::apply_subagent_header;
use crate::common::backoff;
use crate::error::Error;
use crate::model_provider::ModelProviderInfo;
use crate::prompt::Prompt;
use crate::stream::ResponseEvent;
use crate::stream::ResponseStream;

type Result<T> = std::result::Result<T, Error>;

#[derive(Clone)]
pub struct ResponsesApiClientConfig {
    pub http_client: reqwest::Client,
    pub provider: ModelProviderInfo,
    pub model: String,
    pub conversation_id: ConversationId,
    pub auth_provider: Option<Arc<dyn AuthProvider>>,
    pub otel_event_manager: OtelEventManager,
}

#[derive(Clone)]
pub struct ResponsesApiClient {
    config: ResponsesApiClientConfig,
}

#[async_trait]
impl ApiClient for ResponsesApiClient {
    type Config = ResponsesApiClientConfig;

    async fn new(config: Self::Config) -> Result<Self> {
        Ok(Self { config })
    }

    async fn stream(&self, prompt: Prompt) -> Result<ResponseStream> {
        if self.config.provider.wire_api != crate::model_provider::WireApi::Responses {
            return Err(Error::UnsupportedOperation(
                "ResponsesApiClient requires a Responses provider".to_string(),
            ));
        }

        let mut payload_json = self.build_payload(&prompt)?;

        if self.config.provider.is_azure_responses_endpoint()
            && let Some(input_value) = payload_json.get_mut("input")
            && let Some(array) = input_value.as_array_mut()
        {
            attach_item_ids_array(array, &prompt.input);
        }

        let max_attempts = self.config.provider.request_max_retries();
        for attempt in 0..=max_attempts {
            match self
                .attempt_stream_responses(attempt, &prompt, &payload_json)
                .await
            {
                Ok(stream) => return Ok(stream),
                Err(StreamAttemptError::Fatal(err)) => return Err(err),
                Err(retryable) => {
                    if attempt == max_attempts {
                        return Err(retryable.into_error());
                    }

                    tokio::time::sleep(retryable.delay(attempt)).await;
                }
            }
        }

        unreachable!("attempt_stream_responses should always return");
    }
}

impl ResponsesApiClient {
    fn build_payload(&self, prompt: &Prompt) -> Result<Value> {
        let azure_workaround = self.config.provider.is_azure_responses_endpoint();

        let mut payload = json!({
            "model": self.config.model,
            "instructions": prompt.instructions,
            "input": prompt.input,
            "tools": prompt.tools,
            "tool_choice": "auto",
            "parallel_tool_calls": prompt.parallel_tool_calls,
            "store": azure_workaround,
            "stream": true,
            "prompt_cache_key": prompt
                .prompt_cache_key
                .clone()
                .unwrap_or_else(|| self.config.conversation_id.to_string()),
        });

        if let Some(reasoning) = prompt.reasoning.as_ref()
            && let Some(obj) = payload.as_object_mut()
        {
            obj.insert("reasoning".to_string(), serde_json::to_value(reasoning)?);
        }

        if let Some(text) = prompt.text_controls.as_ref()
            && let Some(obj) = payload.as_object_mut()
        {
            obj.insert("text".to_string(), serde_json::to_value(text)?);
        }

        let include = if prompt.reasoning.is_some() {
            vec!["reasoning.encrypted_content".to_string()]
        } else {
            Vec::new()
        };
        if let Some(obj) = payload.as_object_mut() {
            obj.insert(
                "include".to_string(),
                Value::Array(include.into_iter().map(Value::String).collect()),
            );
        }

        Ok(payload)
    }

    async fn attempt_stream_responses(
        &self,
        attempt: i64,
        prompt: &Prompt,
        payload_json: &Value,
    ) -> std::result::Result<ResponseStream, StreamAttemptError> {
        let auth = if let Some(provider) = &self.config.auth_provider {
            provider.auth_context().await
        } else {
            None
        };

        trace!(
            "POST to {}: {:?}",
            self.config.provider.get_full_url(auth.as_ref()),
            serde_json::to_string(payload_json)
                .unwrap_or_else(|_| "<unable to serialize payload>".to_string())
        );

        let mut req_builder = self
            .config
            .provider
            .create_request_builder(&self.config.http_client, &auth)
            .await
            .map_err(StreamAttemptError::Fatal)?;
        req_builder = apply_subagent_header(req_builder, prompt.session_source.as_ref());

        req_builder = req_builder
            .header("conversation_id", self.config.conversation_id.to_string())
            .header("session_id", self.config.conversation_id.to_string())
            .header(reqwest::header::ACCEPT, "text/event-stream")
            .json(payload_json);

        if let Some(auth_ctx) = auth.as_ref()
            && auth_ctx.mode == AuthMode::ChatGPT
            && let Some(account_id) = auth_ctx.account_id.clone()
        {
            req_builder = req_builder.header("chatgpt-account-id", account_id);
        }

        let res = self
            .config
            .otel_event_manager
            .log_request(attempt as u64, || req_builder.send())
            .await;

        let mut request_id = None;
        if let Ok(resp) = &res {
            request_id = resp
                .headers()
                .get("cf-ray")
                .and_then(|v| v.to_str().ok())
                .map(std::string::ToString::to_string);
        }

        match res {
            Ok(resp) if resp.status().is_success() => {
                let (tx_event, rx_event) = mpsc::channel::<Result<ResponseEvent>>(1600);

                if let Some(snapshot) = parse_rate_limit_snapshot(resp.headers())
                    && tx_event
                        .send(Ok(ResponseEvent::RateLimits(snapshot)))
                        .await
                        .is_err()
                {
                    debug!("receiver dropped rate limit snapshot event");
                }

                let stream = resp
                    .bytes_stream()
                    .map_err(move |err| Error::ResponseStreamFailed {
                        source: err,
                        request_id: request_id.clone(),
                    });
                let idle_timeout = self.config.provider.stream_idle_timeout();
                let otel = self.config.otel_event_manager.clone();

                tokio::spawn(process_sse(stream, tx_event, idle_timeout, otel));

                Ok(ResponseStream { rx_event })
            }
            Ok(resp) => Err(handle_error_response(resp, request_id, &self.config).await),
            Err(err) => Err(StreamAttemptError::RetryableTransportError(Error::Http(
                err,
            ))),
        }
    }
}

async fn handle_error_response(
    resp: reqwest::Response,
    request_id: Option<String>,
    _config: &ResponsesApiClientConfig,
) -> StreamAttemptError {
    let status = resp.status();
    let retry_after_secs = resp
        .headers()
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<i64>().ok());
    let retry_after = retry_after_secs.map(|secs| {
        let clamped = if secs < 0 { 0 } else { secs as u64 };
        Duration::from_secs(clamped)
    });

    if !(status == StatusCode::TOO_MANY_REQUESTS
        || status == StatusCode::UNAUTHORIZED
        || status.is_server_error())
    {
        let body = resp.text().await.unwrap_or_default();
        return StreamAttemptError::Fatal(Error::UnexpectedStatus { status, body });
    }

    if status == StatusCode::TOO_MANY_REQUESTS {
        let rate_limits = parse_rate_limit_snapshot(resp.headers());
        let body = resp.json::<ErrorResponse>().await.ok();
        if let Some(ErrorResponse { error }) = body {
            if error.r#type.as_deref() == Some("usage_limit_reached") {
                return StreamAttemptError::Fatal(Error::UsageLimitReached {
                    plan_type: error.plan_type,
                    resets_at: error.resets_at,
                    rate_limits,
                });
            } else if error.r#type.as_deref() == Some("usage_not_included") {
                return StreamAttemptError::Fatal(Error::Stream(
                    "usage not included".to_string(),
                    None,
                ));
            } else if is_quota_exceeded_error(&error) {
                return StreamAttemptError::Fatal(Error::Stream(
                    "quota exceeded".to_string(),
                    None,
                ));
            }
        }
    }

    StreamAttemptError::RetryableHttpError {
        status,
        retry_after,
        request_id,
    }
}

#[allow(clippy::too_many_arguments)]
async fn process_sse<S>(
    stream: S,
    tx_event: mpsc::Sender<Result<ResponseEvent>>,
    max_idle_duration: Duration,
    otel_event_manager: OtelEventManager,
) where
    S: Stream<Item = Result<Bytes>> + Send + 'static + Unpin,
{
    let mut stream = stream;
    let mut response_completed: Option<ResponseCompleted> = None;
    let mut response_error: Option<Error> = None;
    let mut data_buffer = String::new();

    loop {
        let result = timeout(max_idle_duration, stream.next()).await;
        match result {
            Err(_) => {
                if let Some(completed) = response_completed.take() {
                    let _ = emit_response_completed(
                        tx_event.clone(),
                        completed,
                        response_error.take(),
                        &otel_event_manager,
                    )
                    .await;
                    return;
                }

                let _ = tx_event
                    .send(Err(Error::Stream(
                        "stream idle timeout fired before Completed event".to_string(),
                        None,
                    )))
                    .await;
                return;
            }
            Ok(Some(Err(err))) => {
                let _ = tx_event.send(Err(err)).await;
                return;
            }
            Ok(Some(Ok(chunk))) => {
                if let Err(err) = process_sse_chunk(
                    chunk,
                    &tx_event,
                    &mut data_buffer,
                    &mut response_completed,
                    &mut response_error,
                    &otel_event_manager,
                )
                .await
                {
                    let _ = tx_event.send(Err(err)).await;
                    return;
                }
            }
            Ok(None) => {
                if let Some(completed) = response_completed.take() {
                    let _ = emit_response_completed(
                        tx_event.clone(),
                        completed,
                        response_error.take(),
                        &otel_event_manager,
                    )
                    .await;
                } else {
                    otel_event_manager.sse_event_failed(
                        None,
                        Duration::from_millis(0),
                        &"stream closed before response.completed".to_string(),
                    );
                    let _ = tx_event
                        .send(Err(Error::Stream(
                            "stream closed before response.completed".to_string(),
                            None,
                        )))
                        .await;
                }
                return;
            }
        }
    }
}

async fn emit_response_completed(
    tx_event: mpsc::Sender<Result<ResponseEvent>>,
    completed: ResponseCompleted,
    response_error: Option<Error>,
    otel_event_manager: &OtelEventManager,
) -> Result<()> {
    if let Some(err) = response_error {
        tx_event.send(Err(err)).await.ok();
        return Ok(());
    }

    let usage = completed.usage.clone();
    let response_id = completed.id.clone();
    let event = ResponseEvent::Completed {
        response_id,
        token_usage: usage.clone(),
    };
    tx_event.send(Ok(event)).await.ok();
    if let Some(usage) = &usage {
        otel_event_manager.sse_event_completed(
            usage.input_tokens,
            usage.output_tokens,
            Some(usage.cached_input_tokens),
            Some(usage.reasoning_output_tokens),
            usage.total_tokens,
        );
    } else {
        otel_event_manager.see_event_completed_failed(&"missing token usage".to_string());
    }

    Ok(())
}

fn parse_rate_limit_snapshot(headers: &HeaderMap) -> Option<RateLimitSnapshot> {
    let primary = parse_rate_limit_window(
        headers,
        "x-codex-primary-used-percent",
        "x-codex-primary-window-minutes",
        "x-codex-primary-reset-at",
    );

    let secondary = parse_rate_limit_window(
        headers,
        "x-codex-secondary-used-percent",
        "x-codex-secondary-window-minutes",
        "x-codex-secondary-reset-at",
    );

    Some(RateLimitSnapshot { primary, secondary })
}

fn parse_rate_limit_window(
    headers: &HeaderMap,
    used_percent_header: &str,
    window_minutes_header: &str,
    resets_at_header: &str,
) -> Option<RateLimitWindow> {
    let used_percent: Option<f64> = parse_header_f64(headers, used_percent_header);

    used_percent.and_then(|used_percent| {
        let window_minutes = parse_header_i64(headers, window_minutes_header);
        let resets_at = parse_header_i64(headers, resets_at_header);

        let has_data = used_percent != 0.0
            || window_minutes.is_some_and(|minutes| minutes != 0)
            || resets_at.is_some();

        has_data.then_some(RateLimitWindow {
            used_percent,
            window_minutes,
            resets_at,
        })
    })
}

fn parse_header_f64(headers: &HeaderMap, name: &str) -> Option<f64> {
    parse_header_str(headers, name)?
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
}

fn parse_header_i64(headers: &HeaderMap, name: &str) -> Option<i64> {
    parse_header_str(headers, name)?.parse::<i64>().ok()
}

fn parse_header_str<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name)?.to_str().ok()
}

async fn process_sse_chunk(
    chunk: Bytes,
    tx_event: &mpsc::Sender<Result<ResponseEvent>>,
    data_buffer: &mut String,
    response_completed: &mut Option<ResponseCompleted>,
    response_error: &mut Option<Error>,
    otel_event_manager: &OtelEventManager,
) -> Result<()> {
    let chunk_str = std::str::from_utf8(&chunk)
        .map_err(|err| Error::Other(format!("Invalid UTF-8 in SSE chunk: {err}")))?;
    trace!("responses api chunk ({chunk_str:?})");

    for line in chunk_str.lines() {
        if let Some(tail) = line.strip_prefix("data:") {
            data_buffer.push_str(tail.trim_start());
        } else if !line.is_empty() && !data_buffer.is_empty() {
            // Continuation of a long data: line split across chunks; append raw.
            data_buffer.push_str(line);
        }

        if line.is_empty() {
            // First try the "event-shaped" payload used by test harness
            if let Ok(event) = serde_json::from_str::<StreamEvent>(data_buffer) {
                otel_event_manager.sse_event_kind(&event.r#type);
                handle_stream_event(
                    event,
                    tx_event.clone(),
                    response_completed,
                    response_error,
                    otel_event_manager,
                )
                .await;
            } else {
                // Log parse errors for otel tracing (event-shaped)
                otel_event_manager.sse_event_failed(
                    None,
                    Duration::from_millis(0),
                    &format!("Cannot parse SSE JSON: {data_buffer}"),
                );
                // Fall back to field-shaped payload used by Responses API variants
                match serde_json::from_str::<sse::Payload>(data_buffer) {
                    Ok(payload) => {
                        handle_sse_payload(payload, tx_event, otel_event_manager).await?;
                    }
                    Err(err) => {
                        // Also emit failure when field-shaped parse fails
                        otel_event_manager.sse_event_failed(
                            None,
                            Duration::from_millis(0),
                            &format!("Cannot parse SSE JSON: {err}"),
                        );
                        return Err(Error::Other(format!("Cannot parse SSE JSON: {err}")));
                    }
                }
            }
            data_buffer.clear();
        }
    }

    Ok(())
}

async fn handle_sse_payload(
    payload: sse::Payload,
    tx_event: &mpsc::Sender<Result<ResponseEvent>>,
    otel_event_manager: &OtelEventManager,
) -> Result<()> {
    if let Some(responses) = payload.responses {
        for ev in responses {
            let event = match ev {
                sse::Response::Completed(complete) => {
                    if let Some(usage) = &complete.usage {
                        otel_event_manager.sse_event_completed(
                            usage.input_tokens,
                            usage.output_tokens,
                            Some(usage.cached_input_tokens),
                            Some(usage.reasoning_output_tokens),
                            usage.total_tokens,
                        );
                    } else {
                        otel_event_manager
                            .see_event_completed_failed(&"missing token usage".to_string());
                    }
                    ResponseEvent::Completed {
                        response_id: complete.id,
                        token_usage: complete.usage,
                    }
                }
                sse::Response::Error(err) => {
                    let retry_after = err
                        .retry_after
                        .map(|secs| Duration::from_secs(if secs < 0 { 0 } else { secs as u64 }));
                    return Err(Error::Stream(
                        err.message.unwrap_or_else(|| "fatal error".to_string()),
                        retry_after,
                    ));
                }
            };
            tx_event.send(Ok(event)).await.ok();
        }
    }

    if let Some(message_delta) = payload.response_message_delta {
        let ev = ResponseEvent::OutputTextDelta(message_delta.text.clone());
        tx_event.send(Ok(ev)).await.ok();
    }

    if let Some(_response_content) = payload.response_content {
        // Not used currently
    }

    if let Some(ev) = payload.response_event {
        debug!("Unhandled response_event: {ev:?}");
    }

    if let Some(item) = payload.response_output_item {
        match item.r#type {
            sse::OutputItem::Created => {
                tx_event.send(Ok(ResponseEvent::Created)).await.ok();
                otel_event_manager.sse_event_kind("response.output_item.done");
            }
        }
    }

    if let Some(done) = payload.response_output_text_delta {
        tx_event
            .send(Ok(ResponseEvent::OutputTextDelta(done.text)))
            .await
            .ok();
    }

    if let Some(completed) = payload.response_output_item_done {
        let response_item =
            serde_json::from_value::<ResponseItem>(completed.item).map_err(Error::Json)?;
        tx_event
            .send(Ok(ResponseEvent::OutputItemDone(response_item)))
            .await
            .ok();
        otel_event_manager.sse_event_kind("response.output_item.done");
    }

    if let Some(reasoning_content_delta) = payload.response_output_reasoning_delta {
        tx_event
            .send(Ok(ResponseEvent::ReasoningContentDelta(
                reasoning_content_delta.text,
            )))
            .await
            .ok();
    }

    if let Some(reasoning_summary_delta) = payload.response_output_reasoning_summary_delta {
        tx_event
            .send(Ok(ResponseEvent::ReasoningSummaryDelta(
                reasoning_summary_delta.text,
            )))
            .await
            .ok();
    }

    if let Some(ev) = payload.response_error
        && ev.code.as_deref() == Some("max_response_tokens")
    {
        let _ = tx_event
            .send(Err(Error::Stream(
                "context window exceeded".to_string(),
                None,
            )))
            .await;
    }

    Ok(())
}

#[derive(Debug, Deserialize)]
struct ResponseCompleted {
    id: String,
    usage: Option<TokenUsage>,
}

#[derive(Debug, Deserialize)]
struct StreamResponseCompleted {
    id: String,
    usage: Option<TokenUsagePartial>,
}

#[derive(Debug, Deserialize)]
struct ErrorResponse {
    error: ErrorBody,
}
#[derive(Debug, Deserialize)]
struct ErrorBody {
    r#type: Option<String>,
    code: Option<String>,
    message: Option<String>,
    plan_type: Option<String>,
    resets_at: Option<i64>,
}

fn is_quota_exceeded_error(error: &ErrorBody) -> bool {
    error.code.as_deref() == Some("quota_exceeded")
}

enum StreamAttemptError {
    RetryableHttpError {
        status: StatusCode,
        retry_after: Option<Duration>,
        request_id: Option<String>,
    },
    RetryableTransportError(Error),
    Fatal(Error),
}

impl StreamAttemptError {
    fn delay(&self, attempt: i64) -> Duration {
        match self {
            StreamAttemptError::RetryableHttpError {
                retry_after: Some(retry_after),
                ..
            } => *retry_after,
            StreamAttemptError::RetryableHttpError {
                retry_after: None, ..
            }
            | StreamAttemptError::RetryableTransportError(..) => backoff(attempt),
            StreamAttemptError::Fatal(..) => Duration::from_millis(0),
        }
    }

    fn into_error(self) -> Error {
        match self {
            StreamAttemptError::RetryableHttpError {
                status, request_id, ..
            } => Error::RetryLimit {
                status: Some(status),
                request_id,
            },
            StreamAttemptError::RetryableTransportError(err) | StreamAttemptError::Fatal(err) => {
                err
            }
        }
    }
}

// backoff moved to crate::common

fn rate_limit_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();

    #[expect(clippy::unwrap_used)]
    RE.get_or_init(|| Regex::new(r"(?i)try again in\s*(\d+(?:\.\d+)?)\s*(s|ms|seconds?)").unwrap())
}

fn try_parse_retry_after(err: &ErrorResponse) -> Option<Duration> {
    if err.error.code.as_deref() != Some("rate_limit_exceeded") {
        return None;
    }

    let re = rate_limit_regex();
    if let Some(message) = &err.error.message
        && let Some(captures) = re.captures(message)
    {
        let seconds = captures.get(1);
        let unit = captures.get(2);

        if let (Some(value), Some(unit)) = (seconds, unit) {
            let value = value.as_str().parse::<f64>().ok()?;
            let unit = unit.as_str().to_ascii_lowercase();

            if unit == "s" || unit.starts_with("second") {
                return Some(Duration::from_secs_f64(value));
            } else if unit == "ms" {
                return Some(Duration::from_millis(value as u64));
            }
        }
    }
    None
}

/// used in tests to stream from a text SSE file
pub async fn stream_from_fixture(
    path: impl AsRef<Path>,
    provider: ModelProviderInfo,
    otel_event_manager: OtelEventManager,
) -> Result<ResponseStream> {
    let (tx_event, rx_event) = mpsc::channel::<Result<ResponseEvent>>(1600);
    let display_path = path.as_ref().display().to_string();
    let file = std::fs::File::open(path.as_ref())
        .map_err(|err| Error::Other(format!("failed to open fixture {display_path}: {err}")))?;
    let lines = std::io::BufReader::new(file).lines();

    let mut content = String::new();
    for line in lines {
        let line = line
            .map_err(|err| Error::Other(format!("failed to read fixture {display_path}: {err}")))?;
        content.push_str(&line);
        content.push('\n');
        content.push('\n');
    }

    let rdr = std::io::Cursor::new(content);
    let stream = ReaderStream::new(rdr).map_err(|err| Error::Other(err.to_string()));
    tokio::spawn(process_sse(
        stream,
        tx_event,
        provider.stream_idle_timeout(),
        otel_event_manager,
    ));
    Ok(ResponseStream { rx_event })
}

fn attach_item_ids_array(json_array: &mut [Value], prompt_input: &[ResponseItem]) {
    for (json_item, item) in json_array.iter_mut().zip(prompt_input.iter()) {
        let Some(obj) = json_item.as_object_mut() else {
            continue;
        };

        // Helper to set id only if missing/null
        let mut set_id_if_absent = |id: &str| match obj.get("id") {
            Some(Value::String(s)) if !s.is_empty() => {}
            Some(Value::Null) | None => {
                obj.insert("id".to_string(), Value::String(id.to_string()));
            }
            _ => {}
        };

        match item {
            ResponseItem::Reasoning { id, .. } => set_id_if_absent(id),
            ResponseItem::Message { id, .. } => {
                if let Some(id) = id.as_ref() {
                    set_id_if_absent(id);
                }
            }
            ResponseItem::WebSearchCall { id, .. } => {
                if let Some(id) = id.as_ref() {
                    set_id_if_absent(id);
                }
            }
            ResponseItem::FunctionCall { id, .. } => {
                if let Some(id) = id.as_ref() {
                    set_id_if_absent(id);
                }
            }
            ResponseItem::LocalShellCall { id, .. } => {
                if let Some(id) = id.as_ref() {
                    set_id_if_absent(id);
                }
            }
            ResponseItem::CustomToolCall { id, .. } => {
                if let Some(id) = id.as_ref() {
                    set_id_if_absent(id);
                }
            }
            _ => {}
        }
    }
}

#[derive(Debug, Deserialize)]
struct StreamEvent {
    r#type: String,
    response: Option<Value>,
    item: Option<Value>,
    error: Option<Value>,
    #[serde(default)]
    delta: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TokenUsagePartial {
    #[serde(default)]
    input_tokens: i64,
    #[serde(default)]
    cached_input_tokens: i64,
    #[serde(default)]
    input_tokens_details: Option<TokenUsageInputDetails>,
    #[serde(default)]
    output_tokens: i64,
    #[serde(default)]
    output_tokens_details: Option<TokenUsageOutputDetails>,
    #[serde(default)]
    reasoning_output_tokens: i64,
    #[serde(default)]
    total_tokens: i64,
}

impl From<TokenUsagePartial> for TokenUsage {
    fn from(value: TokenUsagePartial) -> Self {
        let cached_input_tokens = if value.cached_input_tokens > 0 {
            Some(value.cached_input_tokens)
        } else {
            value
                .input_tokens_details
                .and_then(|d| d.cached_tokens)
                .filter(|v| *v > 0)
        };
        let reasoning_output_tokens = if value.reasoning_output_tokens > 0 {
            Some(value.reasoning_output_tokens)
        } else {
            value
                .output_tokens_details
                .and_then(|d| d.reasoning_tokens)
                .filter(|v| *v > 0)
        };
        Self {
            input_tokens: value.input_tokens,
            cached_input_tokens: cached_input_tokens.unwrap_or(0),
            output_tokens: value.output_tokens,
            reasoning_output_tokens: reasoning_output_tokens.unwrap_or(0),
            total_tokens: value.total_tokens,
        }
    }
}

#[derive(Debug, Deserialize)]
struct TokenUsageInputDetails {
    #[serde(default)]
    cached_tokens: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct TokenUsageOutputDetails {
    #[serde(default)]
    reasoning_tokens: Option<i64>,
}

async fn handle_stream_event(
    event: StreamEvent,
    tx_event: mpsc::Sender<Result<ResponseEvent>>,
    response_completed: &mut Option<ResponseCompleted>,
    response_error: &mut Option<Error>,
    otel_event_manager: &OtelEventManager,
) {
    trace!("response event: {}", event.r#type);
    match event.r#type.as_str() {
        "response.created" => {
            // Emit Created as soon as we see a created event
            let _ = tx_event.send(Ok(ResponseEvent::Created)).await;
        }
        "response.output_text.delta" => {
            if let Some(item_val) = event.item {
                let resp = serde_json::from_value::<TextDelta>(item_val);
                if let Ok(delta) = resp {
                    let event = ResponseEvent::OutputTextDelta(delta.delta);
                    let _ = tx_event.send(Ok(event)).await;
                }
            } else if let Some(delta) = event.delta {
                let _ = tx_event
                    .send(Ok(ResponseEvent::OutputTextDelta(delta)))
                    .await;
            }
        }
        "response.reasoning_text.delta" => {
            if let Some(delta) = event.delta {
                let event = ResponseEvent::ReasoningContentDelta(delta);
                let _ = tx_event.send(Ok(event)).await;
            }
        }
        "response.reasoning_summary_text.delta" => {
            if let Some(delta) = event.delta {
                let event = ResponseEvent::ReasoningSummaryDelta(delta);
                let _ = tx_event.send(Ok(event)).await;
            }
        }
        "response.output_item.done" => {
            if let Some(item_val) = event.item
                && let Ok(item) = serde_json::from_value::<ResponseItem>(item_val)
            {
                let event = ResponseEvent::OutputItemDone(item);
                if tx_event.send(Ok(event)).await.is_err() {}
            }
        }
        "response.failed" => {
            if let Some(resp_val) = event.response {
                otel_event_manager.sse_event_failed(
                    Some(&"response.failed".to_string()),
                    Duration::from_millis(0),
                    &resp_val,
                );

                // Propagate failure downstream; map context window errors to a
                // stable message that core can handle specially.
                if let Some(err) = resp_val
                    .get("error")
                    .cloned()
                    .and_then(|v| serde_json::from_value::<ErrorBody>(v).ok())
                {
                    let msg = if err.code.as_deref() == Some("context_length_exceeded") {
                        "context window exceeded".to_string()
                    } else if err.code.as_deref() == Some("insufficient_quota") {
                        "quota exceeded".to_string()
                    } else {
                        err.message.unwrap_or_else(|| "fatal error".to_string())
                    };
                    let _ = tx_event.send(Err(Error::Stream(msg, None))).await;
                }
            }
        }
        "response.error" => {
            if let Some(err_val) = event.error {
                let err_resp = serde_json::from_value::<ErrorResponse>(err_val);
                match err_resp {
                    Ok(err) => {
                        let retry_after = try_parse_retry_after(&err);
                        *response_error = Some(Error::Stream(
                            err.error
                                .message
                                .unwrap_or_else(|| "unknown error".to_string()),
                            retry_after,
                        ));
                    }
                    Err(err) => {
                        let _ = tx_event
                            .send(Err(Error::Stream(
                                format!("failed to parse ErrorResponse: {err}"),
                                None,
                            )))
                            .await;
                    }
                }
            }
        }
        "response.completed" => {
            if let Some(resp_val) = event.response {
                match serde_json::from_value::<StreamResponseCompleted>(resp_val) {
                    Ok(resp) => {
                        let usage = resp.usage.map(TokenUsage::from);
                        let completed = ResponseCompleted {
                            id: resp.id.clone(),
                            usage: usage.clone(),
                        };
                        // Emit Completed immediately to match field-shaped behavior.
                        let ev = ResponseEvent::Completed {
                            response_id: resp.id,
                            token_usage: usage,
                        };
                        let _ = tx_event.send(Ok(ev)).await;
                        if let Some(usage) = &completed.usage {
                            otel_event_manager.sse_event_completed(
                                usage.input_tokens,
                                usage.output_tokens,
                                Some(usage.cached_input_tokens),
                                Some(usage.reasoning_output_tokens),
                                usage.total_tokens,
                            );
                        } else {
                            otel_event_manager
                                .see_event_completed_failed(&"missing token usage".to_string());
                        }
                        *response_completed = Some(completed);
                    }
                    Err(err) => {
                        otel_event_manager.sse_event_failed(
                            Some(&"response.completed".to_string()),
                            Duration::from_millis(0),
                            &format!("failed to parse ResponseCompleted: {err}"),
                        );
                        let _ = tx_event
                            .send(Err(Error::Stream(
                                format!("failed to parse ResponseCompleted: {err}"),
                                None,
                            )))
                            .await;
                    }
                };
            };
        }
        "response.output_item.added" => {
            if let Some(item_val) = event.item
                && let Ok(item) = serde_json::from_value::<ResponseItem>(item_val)
            {
                let event = ResponseEvent::OutputItemAdded(item);
                if tx_event.send(Ok(event)).await.is_err() {}
            }
        }
        "response.reasoning_summary_part.added" => {
            let event = ResponseEvent::ReasoningSummaryPartAdded;
            let _ = tx_event.send(Ok(event)).await;
        }
        _ => {}
    }
}

#[derive(Debug, Deserialize)]
struct TextDelta {
    delta: String,
}

mod sse {
    use serde::Deserialize;
    use serde_json::Value;

    #[derive(Debug, Deserialize)]
    pub struct Payload {
        pub responses: Option<Vec<Response>>,
        pub response_content: Option<Value>,
        pub response_error: Option<ResponseError>,
        pub response_event: Option<String>,
        pub response_message_delta: Option<ResponseMessageDelta>,
        pub response_output_item: Option<ResponseOutputItem>,
        pub response_output_text_delta: Option<ResponseOutputTextDelta>,
        pub response_output_item_done: Option<ResponseOutputItemDone>,
        pub response_output_reasoning_delta: Option<ResponseOutputReasoningDelta>,
        pub response_output_reasoning_summary_delta: Option<ResponseOutputReasoningSummaryDelta>,
    }

    #[derive(Debug, Deserialize)]
    pub enum Response {
        #[serde(rename = "response.completed")]
        Completed(ResponseCompleted),
        #[serde(rename = "response.error")]
        Error(ResponseError),
    }

    #[derive(Debug, Deserialize)]
    pub struct ResponseCompleted {
        pub id: String,
        pub usage: Option<codex_protocol::protocol::TokenUsage>,
    }

    #[derive(Debug, Deserialize)]
    pub struct ResponseError {
        pub code: Option<String>,
        pub message: Option<String>,
        pub retry_after: Option<i64>,
    }

    #[derive(Debug, Deserialize)]
    pub struct ResponseMessageDelta {
        pub text: String,
    }

    #[derive(Debug, Deserialize)]
    pub enum OutputItem {
        #[serde(rename = "response.output_item.created")]
        Created,
    }

    #[derive(Debug, Deserialize)]
    pub struct ResponseOutputItem {
        pub r#type: OutputItem,
    }

    #[derive(Debug, Deserialize)]
    pub struct ResponseOutputTextDelta {
        pub text: String,
    }

    #[derive(Debug, Deserialize)]
    pub struct ResponseOutputItemDone {
        pub item: Value,
    }

    #[derive(Debug, Deserialize)]
    pub struct ResponseOutputReasoningDelta {
        pub text: String,
    }

    #[derive(Debug, Deserialize)]
    pub struct ResponseOutputReasoningSummaryDelta {
        pub text: String,
    }
}
