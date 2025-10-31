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
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::protocol::TokenUsage;
use eventsource_stream::Eventsource;
use futures::Stream;
use futures::StreamExt;
use futures::TryStreamExt;
use regex_lite::Regex;
use reqwest::StatusCode;
use reqwest::header::HeaderMap;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use serde_json::json;
use tokio::sync::mpsc;
use tokio::time::timeout;
use tokio_util::io::ReaderStream;
use tracing::debug;
use tracing::trace;

use crate::api::ApiClient;
use crate::auth::AuthProvider;
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

        if let Some(prev) = prompt.previous_response_id.as_ref()
            && let Some(obj) = payload.as_object_mut()
        {
            obj.insert(
                "previous_response_id".to_string(),
                Value::String(prev.clone()),
            );
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
        attempt: u64,
        prompt: &Prompt,
        payload_json: &Value,
    ) -> std::result::Result<ResponseStream, StreamAttemptError> {
        let auth = match &self.config.auth_provider {
            Some(provider) => provider.auth_context().await,
            None => None,
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

        if let Some(SessionSource::SubAgent(sub)) = prompt.session_source.as_ref() {
            let subagent = match sub {
                SubAgentSource::Other(label) => label.clone(),
                other => serde_json::to_value(other)
                    .ok()
                    .and_then(|v| v.as_str().map(ToString::to_string))
                    .unwrap_or_else(|| "other".to_string()),
            };
            req_builder = req_builder.header("x-openai-subagent", subagent);
        }

        req_builder = req_builder
            .header("conversation_id", self.config.conversation_id.to_string())
            .header("session_id", self.config.conversation_id.to_string())
            .header(reqwest::header::ACCEPT, "text/event-stream")
            .json(payload_json);

        if let Some(ctx) = auth.as_ref()
            && ctx.mode == AuthMode::ChatGPT
            && let Some(account_id) = ctx.account_id.as_ref()
        {
            req_builder = req_builder.header("chatgpt-account-id", account_id);
        }

        let res = self
            .config
            .otel_event_manager
            .log_request(attempt, || req_builder.send())
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

                let request_id_for_stream = request_id.clone();
                let stream = resp
                    .bytes_stream()
                    .map_err(move |err| Error::ResponseStreamFailed {
                        source: err,
                        request_id: request_id_for_stream.clone(),
                    });
                tokio::spawn(process_sse(
                    stream,
                    tx_event,
                    self.config.provider.stream_idle_timeout(),
                    self.config.otel_event_manager.clone(),
                ));

                Ok(ResponseStream { rx_event })
            }
            Ok(res) => {
                let status = res.status();

                let retry_after_secs = res
                    .headers()
                    .get(reqwest::header::RETRY_AFTER)
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.parse::<u64>().ok());
                let retry_after = retry_after_secs.map(|s| Duration::from_millis(s * 1_000));

                if status == StatusCode::UNAUTHORIZED
                    && let Some(provider) = self.config.auth_provider.as_ref()
                    && let Some(ctx) = auth.as_ref()
                    && ctx.mode == AuthMode::ChatGPT
                {
                    provider
                        .refresh_token()
                        .await
                        .map_err(|err| StreamAttemptError::Fatal(Error::Auth(err)))?;
                }

                if !(status == StatusCode::TOO_MANY_REQUESTS
                    || status == StatusCode::UNAUTHORIZED
                    || status.is_server_error())
                {
                    // Surface error body.
                    let body = res
                        .text()
                        .await
                        .unwrap_or_else(|_| "<failed to read response>".to_string());
                    return Err(StreamAttemptError::Fatal(Error::UnexpectedStatus {
                        status,
                        body,
                    }));
                }

                Err(StreamAttemptError::RetryableHttpError {
                    status,
                    retry_after,
                    request_id,
                })
            }
            Err(err) => Err(StreamAttemptError::RetryableTransportError(Error::Http(
                err,
            ))),
        }
    }
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
    fn delay(&self, attempt: u64) -> Duration {
        let backoff_attempt = attempt + 1;
        match self {
            StreamAttemptError::RetryableHttpError { retry_after, .. } => {
                retry_after.unwrap_or_else(|| backoff(backoff_attempt))
            }
            StreamAttemptError::RetryableTransportError { .. } => backoff(backoff_attempt),
            StreamAttemptError::Fatal(_) => Duration::from_secs(0),
        }
    }

    fn into_error(self) -> Error {
        match self {
            StreamAttemptError::RetryableHttpError {
                status, request_id, ..
            } => Error::RetryLimit { status, request_id },
            StreamAttemptError::RetryableTransportError(error) => error,
            StreamAttemptError::Fatal(error) => error,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct SseEvent {
    #[serde(rename = "type")]
    kind: String,
    response: Option<Value>,
    item: Option<Value>,
    delta: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ResponseCompleted {
    id: String,
    usage: Option<ResponseCompletedUsage>,
}

#[derive(Debug, Deserialize)]
struct ResponseCompletedUsage {
    input_tokens: i64,
    input_tokens_details: Option<ResponseCompletedInputTokensDetails>,
    output_tokens: i64,
    output_tokens_details: Option<ResponseCompletedOutputTokensDetails>,
    total_tokens: i64,
}

impl From<ResponseCompletedUsage> for TokenUsage {
    fn from(val: ResponseCompletedUsage) -> Self {
        TokenUsage {
            input_tokens: val.input_tokens,
            cached_input_tokens: val
                .input_tokens_details
                .map(|d| d.cached_tokens)
                .unwrap_or(0),
            output_tokens: val.output_tokens,
            reasoning_output_tokens: val
                .output_tokens_details
                .map(|d| d.reasoning_tokens)
                .unwrap_or(0),
            total_tokens: val.total_tokens,
        }
    }
}

#[derive(Debug, Deserialize)]
struct ResponseCompletedInputTokensDetails {
    cached_tokens: i64,
}

#[derive(Debug, Deserialize)]
struct ResponseCompletedOutputTokensDetails {
    reasoning_tokens: i64,
}

fn attach_item_ids_array(items: &mut [Value], original_items: &[ResponseItem]) {
    for (value, item) in items.iter_mut().zip(original_items.iter()) {
        if let ResponseItem::Reasoning { id, .. }
        | ResponseItem::Message { id: Some(id), .. }
        | ResponseItem::WebSearchCall { id: Some(id), .. }
        | ResponseItem::FunctionCall { id: Some(id), .. }
        | ResponseItem::LocalShellCall { id: Some(id), .. }
        | ResponseItem::CustomToolCall { id: Some(id), .. }
        | ResponseItem::CustomToolCallOutput { call_id: id, .. }
        | ResponseItem::FunctionCallOutput { call_id: id, .. } = item
        {
            if id.is_empty() {
                continue;
            }

            if let Some(obj) = value.as_object_mut() {
                obj.insert("id".to_string(), Value::String(id.clone()));
            }
        }
    }
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

async fn process_sse<S>(
    stream: S,
    tx_event: mpsc::Sender<Result<ResponseEvent>>,
    idle_timeout: Duration,
    otel_event_manager: OtelEventManager,
) where
    S: Stream<Item = Result<Bytes>> + Unpin + Send + 'static,
{
    let mut stream = stream.eventsource();

    let mut response_completed: Option<ResponseCompleted> = None;
    let mut response_error: Option<Error> = None;

    loop {
        let start = std::time::Instant::now();
        let response = timeout(idle_timeout, stream.next()).await;
        let duration = start.elapsed();
        otel_event_manager.log_sse_event(&response, duration);

        let sse = match response {
            Ok(Some(Ok(sse))) => sse,
            Ok(Some(Err(e))) => {
                debug!("SSE Error: {e:#}");
                let event = Error::Stream(e.to_string(), None);
                let _ = tx_event.send(Err(event)).await;
                return;
            }
            Ok(None) => {
                match response_completed {
                    Some(ResponseCompleted {
                        id: response_id,
                        usage,
                    }) => {
                        if let Some(token_usage) = &usage {
                            otel_event_manager.sse_event_completed(
                                token_usage.input_tokens,
                                token_usage.output_tokens,
                                token_usage
                                    .input_tokens_details
                                    .as_ref()
                                    .map(|d| d.cached_tokens),
                                token_usage
                                    .output_tokens_details
                                    .as_ref()
                                    .map(|d| d.reasoning_tokens),
                                token_usage.total_tokens,
                            );
                        }
                        let event = ResponseEvent::Completed {
                            response_id,
                            token_usage: usage.map(Into::into),
                        };
                        let _ = tx_event.send(Ok(event)).await;
                    }
                    None => {
                        let error = response_error.unwrap_or(Error::Stream(
                            "stream closed before response.completed".into(),
                            None,
                        ));
                        otel_event_manager.see_event_completed_failed(&error);

                        let _ = tx_event.send(Err(error)).await;
                    }
                }
                return;
            }
            Err(_) => {
                let _ = tx_event
                    .send(Err(Error::Stream(
                        "idle timeout waiting for SSE".into(),
                        None,
                    )))
                    .await;
                return;
            }
        };

        let raw = sse.data.clone();
        trace!("SSE event: {}", raw);

        let event: SseEvent = match serde_json::from_str(&sse.data) {
            Ok(event) => event,
            Err(e) => {
                debug!("Failed to parse SSE event: {e}, data: {}", &sse.data);
                continue;
            }
        };

        match event.kind.as_str() {
            "response.output_item.done" => {
                let Some(item_val) = event.item else { continue };
                let Ok(item) = serde_json::from_value::<ResponseItem>(item_val) else {
                    debug!("failed to parse ResponseItem from output_item.done");
                    continue;
                };

                let event = ResponseEvent::OutputItemDone(item);
                if tx_event.send(Ok(event)).await.is_err() {
                    return;
                }
            }
            "response.output_text.delta" => {
                if let Some(delta) = event.delta {
                    let event = ResponseEvent::OutputTextDelta(delta);
                    if tx_event.send(Ok(event)).await.is_err() {
                        return;
                    }
                }
            }
            "response.reasoning_summary_text.delta" => {
                if let Some(delta) = event.delta {
                    let event = ResponseEvent::ReasoningSummaryDelta(delta);
                    if tx_event.send(Ok(event)).await.is_err() {
                        return;
                    }
                }
            }
            "response.reasoning_text.delta" => {
                if let Some(delta) = event.delta {
                    let event = ResponseEvent::ReasoningContentDelta(delta);
                    if tx_event.send(Ok(event)).await.is_err() {
                        return;
                    }
                }
            }
            "response.created" => {
                if event.response.is_some() {
                    let _ = tx_event.send(Ok(ResponseEvent::Created)).await;
                }
            }
            "response.failed" => {
                if let Some(resp_val) = event.response {
                    response_error = Some(Error::Stream(
                        "response.failed event received".to_string(),
                        None,
                    ));

                    if let Some(error) = resp_val.get("error") {
                        match serde_json::from_value::<ErrorResponse>(error.clone()) {
                            Ok(error) => {
                                if is_context_window_error(&error) {
                                    response_error = Some(Error::UnsupportedOperation(
                                        "context window exceeded".to_string(),
                                    ));
                                } else {
                                    let delay = try_parse_retry_after(&error);
                                    let message = error.message.clone().unwrap_or_default();
                                    response_error = Some(Error::Stream(message, delay));
                                }
                            }
                            Err(e) => {
                                let error = format!("failed to parse ErrorResponse: {e}");
                                debug!(error);
                                response_error = Some(Error::Stream(error, None))
                            }
                        }
                    }
                }
            }
            "response.completed" => {
                if let Some(resp_val) = event.response {
                    match serde_json::from_value::<ResponseCompleted>(resp_val) {
                        Ok(r) => {
                            response_completed = Some(r);
                        }
                        Err(e) => {
                            let error = format!("failed to parse ResponseCompleted: {e}");
                            debug!(error);
                            response_error = Some(Error::Stream(error, None));
                            continue;
                        }
                    };
                };
            }
            "response.output_item.added" => {
                let Some(item_val) = event.item else { continue };
                let Ok(item) = serde_json::from_value::<ResponseItem>(item_val) else {
                    debug!("failed to parse ResponseItem from output_item.done");
                    continue;
                };

                let event = ResponseEvent::OutputItemAdded(item);
                if tx_event.send(Ok(event)).await.is_err() {
                    return;
                }
            }
            "response.reasoning_summary_part.added" => {
                let event = ResponseEvent::ReasoningSummaryPartAdded;
                if tx_event.send(Ok(event)).await.is_err() {
                    return;
                }
            }
            _ => {}
        }
    }
}

#[derive(Debug, Deserialize)]
struct ErrorResponse {
    code: Option<String>,
    message: Option<String>,
}

fn backoff(attempt: u64) -> Duration {
    let exponent = attempt.min(6) as u32;
    let base = 2u64.pow(exponent);
    Duration::from_millis(base * 100)
}

fn rate_limit_regex() -> Option<&'static Regex> {
    static RE: OnceLock<Option<Regex>> = OnceLock::new();

    RE.get_or_init(|| Regex::new(r"Please try again in (\d+(?:\.\d+)?)(s|ms)").ok())
        .as_ref()
}

fn try_parse_retry_after(err: &ErrorResponse) -> Option<Duration> {
    if err.code.as_deref() != Some("rate_limit_exceeded") {
        return None;
    }

    if let Some(re) = rate_limit_regex()
        && let Some(message) = &err.message
        && let Some(captures) = re.captures(message)
    {
        let seconds = captures.get(1);
        let unit = captures.get(2);

        if let (Some(value), Some(unit)) = (seconds, unit) {
            let value = value.as_str().parse::<f64>().ok()?;
            let unit = unit.as_str();

            if unit == "s" {
                return Some(Duration::from_secs_f64(value));
            } else if unit == "ms" {
                return Some(Duration::from_millis(value as u64));
            }
        }
    }
    None
}

fn is_context_window_error(error: &ErrorResponse) -> bool {
    error.code.as_deref() == Some("context_length_exceeded")
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
        .map_err(|e| Error::Other(format!("failed to open fixture {display_path}: {e}")))?;
    let lines = std::io::BufReader::new(file).lines();

    let mut content = String::new();
    for line in lines {
        let line =
            line.map_err(|e| Error::Other(format!("failed to read fixture {display_path}: {e}")))?;
        content.push_str(&line);
        content.push_str("\n\n");
    }

    let rdr = std::io::Cursor::new(content);
    let stream = ReaderStream::new(rdr).map_err(|e| Error::Other(e.to_string()));
    tokio::spawn(process_sse(
        stream,
        tx_event,
        provider.stream_idle_timeout(),
        otel_event_manager,
    ));
    Ok(ResponseStream { rx_event })
}
