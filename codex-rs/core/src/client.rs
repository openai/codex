use std::io::BufRead;
use std::path::Path;
use std::sync::OnceLock;
use std::time::Duration;

use crate::AuthManager;
use crate::auth::CodexAuth;
use crate::error::RetryLimitReachedError;
use crate::error::UnexpectedResponseError;
use bytes::Bytes;
use codex_app_server_protocol::AuthMode;
use codex_protocol::ConversationId;
use eventsource_stream::Eventsource;
use futures::prelude::*;
use regex_lite::Regex;
use reqwest::StatusCode;
use reqwest::header::HeaderMap;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use tokio::sync::mpsc;
use tokio::time::timeout;
use tokio_util::io::ReaderStream;
use tracing::debug;
use tracing::trace;
use tracing::warn;

use crate::chat_completions::AggregateStreamExt;
use crate::chat_completions::stream_chat_completions;
use crate::client_common::Prompt;
use crate::client_common::ResponseEvent;
use crate::client_common::ResponseStream;
use crate::client_common::ResponsesApiRequest;
use crate::client_common::create_reasoning_param_for_request;
use crate::client_common::create_text_param_for_request;
use crate::config::Config;
use crate::default_client::create_client;
use crate::error::CodexErr;
use crate::error::Result;
use crate::error::UsageLimitReachedError;
use crate::flags::CODEX_RS_SSE_FIXTURE;
use crate::model_family::ModelFamily;
use crate::model_provider_info::ModelProviderInfo;
use crate::model_provider_info::WireApi;
use crate::openai_model_info::get_model_info;
use crate::openai_tools::create_tools_json_for_responses_api;
use crate::protocol::RateLimitSnapshot;
use crate::protocol::RateLimitWindow;
use crate::protocol::TokenUsage;
use crate::token_data::PlanType;
use crate::util::backoff;
use codex_otel::otel_event_manager::OtelEventManager;
use codex_protocol::config_types::ReasoningEffort as ReasoningEffortConfig;
use codex_protocol::config_types::ReasoningSummary as ReasoningSummaryConfig;
use codex_protocol::models::ResponseItem;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
struct ErrorResponse {
    error: Error,
}

#[derive(Debug, Deserialize)]
struct Error {
    r#type: Option<String>,
    #[allow(dead_code)]
    code: Option<String>,
    message: Option<String>,

    // Optional fields available on "usage_limit_reached" and "usage_not_included" errors
    plan_type: Option<PlanType>,
    resets_in_seconds: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct ModelClient {
    config: Arc<Config>,
    auth_manager: Option<Arc<AuthManager>>,
    otel_event_manager: OtelEventManager,
    client: reqwest::Client,
    provider: ModelProviderInfo,
    conversation_id: ConversationId,
    effort: Option<ReasoningEffortConfig>,
    summary: ReasoningSummaryConfig,
}

impl ModelClient {
    pub fn new(
        config: Arc<Config>,
        auth_manager: Option<Arc<AuthManager>>,
        otel_event_manager: OtelEventManager,
        provider: ModelProviderInfo,
        effort: Option<ReasoningEffortConfig>,
        summary: ReasoningSummaryConfig,
        conversation_id: ConversationId,
    ) -> Self {
        let client = create_client();

        Self {
            config,
            auth_manager,
            otel_event_manager,
            client,
            provider,
            conversation_id,
            effort,
            summary,
        }
    }

    pub fn get_model_context_window(&self) -> Option<u64> {
        self.config
            .model_context_window
            .or_else(|| get_model_info(&self.config.model_family).map(|info| info.context_window))
    }

    pub fn get_auto_compact_token_limit(&self) -> Option<i64> {
        self.config.model_auto_compact_token_limit.or_else(|| {
            get_model_info(&self.config.model_family).and_then(|info| info.auto_compact_token_limit)
        })
    }

    /// Dispatches to either the Responses or Chat implementation depending on
    /// the provider config.  Public callers always invoke `stream()` – the
    /// specialised helpers are private to avoid accidental misuse.
    pub async fn stream(&self, prompt: &Prompt) -> Result<ResponseStream> {
        match self.provider.wire_api {
            WireApi::Responses => self.stream_responses(prompt).await,
            WireApi::Chat => {
                // Create the raw streaming connection first.
                let response_stream = stream_chat_completions(
                    prompt,
                    &self.config.model_family,
                    &self.client,
                    &self.provider,
                    &self.otel_event_manager,
                )
                .await?;

                // Wrap it with the aggregation adapter so callers see *only*
                // the final assistant message per turn (matching the
                // behaviour of the Responses API).
                let mut aggregated = if self.config.show_raw_agent_reasoning {
                    crate::chat_completions::AggregatedChatStream::streaming_mode(response_stream)
                } else {
                    response_stream.aggregate()
                };

                // Bridge the aggregated stream back into a standard
                // `ResponseStream` by forwarding events through a channel.
                let (tx, rx) = mpsc::channel::<Result<ResponseEvent>>(16);

                tokio::spawn(async move {
                    use futures::StreamExt;
                    while let Some(ev) = aggregated.next().await {
                        // Exit early if receiver hung up.
                        if tx.send(ev).await.is_err() {
                            break;
                        }
                    }
                });

                Ok(ResponseStream { rx_event: rx })
            }
        }
    }

    /// Implementation for the OpenAI *Responses* experimental API.
    async fn stream_responses(&self, prompt: &Prompt) -> Result<ResponseStream> {
        if let Some(path) = &*CODEX_RS_SSE_FIXTURE {
            // short circuit for tests
            warn!(path, "Streaming from fixture");
            return stream_from_fixture(
                path,
                self.provider.clone(),
                self.otel_event_manager.clone(),
            )
            .await;
        }

        let auth_manager = self.auth_manager.clone();

        let full_instructions = prompt.get_full_instructions(&self.config.model_family);
        let tools_json = create_tools_json_for_responses_api(&prompt.tools)?;
        let reasoning = create_reasoning_param_for_request(
            &self.config.model_family,
            self.effort,
            self.summary,
        );

        let include: Vec<String> = if reasoning.is_some() {
            vec!["reasoning.encrypted_content".to_string()]
        } else {
            vec![]
        };

        let input_with_instructions = prompt.get_formatted_input();

        let verbosity = match &self.config.model_family.family {
            family if family == "gpt-5" => self.config.model_verbosity,
            _ => {
                if self.config.model_verbosity.is_some() {
                    warn!(
                        "model_verbosity is set but ignored for non-gpt-5 model family: {}",
                        self.config.model_family.family
                    );
                }

                None
            }
        };

        // Only include `text.verbosity` for GPT-5 family models
        let text = create_text_param_for_request(verbosity, &prompt.output_schema);

        // In general, we want to explicitly send `store: false` when using the Responses API,
        // but in practice, the Azure Responses API rejects `store: false`:
        //
        // - If store = false and id is sent an error is thrown that ID is not found
        // - If store = false and id is not sent an error is thrown that ID is required
        //
        // For Azure, we send `store: true` and preserve reasoning item IDs.
        let azure_workaround = self.provider.is_azure_responses_endpoint();

        let payload = ResponsesApiRequest {
            model: &self.config.model,
            instructions: &full_instructions,
            input: &input_with_instructions,
            tools: &tools_json,
            tool_choice: "auto",
            parallel_tool_calls: false,
            reasoning,
            store: azure_workaround,
            stream: true,
            include,
            prompt_cache_key: Some(self.conversation_id.to_string()),
            text,
        };

        let mut payload_json = serde_json::to_value(&payload)?;
        if azure_workaround {
            attach_item_ids(&mut payload_json, &input_with_instructions);
        }

        let max_attempts = self.provider.request_max_retries();
        for attempt in 0..=max_attempts {
            match self
                .attempt_stream_responses(attempt, &payload_json, &auth_manager)
                .await
            {
                Ok(stream) => {
                    return Ok(stream);
                }
                Err(StreamAttemptError::Fatal(e)) => {
                    return Err(e);
                }
                Err(retryable_attempt_error) => {
                    if attempt == max_attempts {
                        return Err(retryable_attempt_error.into_error());
                    }

                    tokio::time::sleep(retryable_attempt_error.delay(attempt)).await;
                }
            }
        }

        unreachable!("stream_responses_attempt should always return");
    }

    /// Single attempt to start a streaming Responses API call.
    async fn attempt_stream_responses(
        &self,
        attempt: u64,
        payload_json: &Value,
        auth_manager: &Option<Arc<AuthManager>>,
    ) -> std::result::Result<ResponseStream, StreamAttemptError> {
        // Always fetch the latest auth in case a prior attempt refreshed the token.
        let auth = auth_manager.as_ref().and_then(|m| m.auth());

        trace!(
            "POST to {}: {:?}",
            self.provider.get_full_url(&auth),
            serde_json::to_string(payload_json)
        );

        let mut req_builder = self
            .provider
            .create_request_builder(&self.client, &auth)
            .await
            .map_err(StreamAttemptError::Fatal)?;

        req_builder = req_builder
            .header("OpenAI-Beta", "responses=experimental")
            // Send session_id for compatibility.
            .header("conversation_id", self.conversation_id.to_string())
            .header("session_id", self.conversation_id.to_string())
            .header(reqwest::header::ACCEPT, "text/event-stream")
            .json(payload_json);

        if let Some(auth) = auth.as_ref()
            && auth.mode == AuthMode::ChatGPT
            && let Some(account_id) = auth.get_account_id()
        {
            req_builder = req_builder.header("chatgpt-account-id", account_id);
        }

        let res = self
            .otel_event_manager
            .log_request(attempt, || req_builder.send())
            .await;

        let mut request_id = None;
        if let Ok(resp) = &res {
            request_id = resp
                .headers()
                .get("cf-ray")
                .map(|v| v.to_str().unwrap_or_default().to_string());

            trace!(
                "Response status: {}, cf-ray: {:?}",
                resp.status(),
                request_id
            );
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

                // spawn task to process SSE
                let stream = resp.bytes_stream().map_err(CodexErr::Reqwest);
                tokio::spawn(process_sse(
                    stream,
                    tx_event,
                    self.provider.stream_idle_timeout(),
                    self.otel_event_manager.clone(),
                ));

                Ok(ResponseStream { rx_event })
            }
            Ok(res) => {
                let status = res.status();

                // Pull out Retry‑After header if present.
                let retry_after_secs = res
                    .headers()
                    .get(reqwest::header::RETRY_AFTER)
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.parse::<u64>().ok());
                let retry_after = retry_after_secs.map(|s| Duration::from_millis(s * 1_000));

                if status == StatusCode::UNAUTHORIZED
                    && let Some(manager) = auth_manager.as_ref()
                    && manager.auth().is_some()
                {
                    let _ = manager.refresh_token().await;
                }

                // The OpenAI Responses endpoint returns structured JSON bodies even for 4xx/5xx
                // errors. When we bubble early with only the HTTP status the caller sees an opaque
                // "unexpected status 400 Bad Request" which makes debugging nearly impossible.
                // Instead, read (and include) the response text so higher layers and users see the
                // exact error message (e.g. "Unknown parameter: 'input[0].metadata'"). The body is
                // small and this branch only runs on error paths so the extra allocation is
                // negligible.
                if !(status == StatusCode::TOO_MANY_REQUESTS
                    || status == StatusCode::UNAUTHORIZED
                    || status.is_server_error())
                {
                    // Surface the error body to callers. Use `unwrap_or_default` per Clippy.
                    let body = res.text().await.unwrap_or_default();
                    return Err(StreamAttemptError::Fatal(CodexErr::UnexpectedStatus(
                        UnexpectedResponseError {
                            status,
                            body,
                            request_id: None,
                        },
                    )));
                }

                if status == StatusCode::TOO_MANY_REQUESTS {
                    let rate_limit_snapshot = parse_rate_limit_snapshot(res.headers());
                    let body = res.json::<ErrorResponse>().await.ok();
                    if let Some(ErrorResponse { error }) = body {
                        if error.r#type.as_deref() == Some("usage_limit_reached") {
                            // Prefer the plan_type provided in the error message if present
                            // because it's more up to date than the one encoded in the auth
                            // token.
                            let plan_type = error
                                .plan_type
                                .or_else(|| auth.as_ref().and_then(CodexAuth::get_plan_type));
                            let resets_in_seconds = error.resets_in_seconds;
                            let codex_err = CodexErr::UsageLimitReached(UsageLimitReachedError {
                                plan_type,
                                resets_in_seconds,
                                rate_limits: rate_limit_snapshot,
                            });
                            return Err(StreamAttemptError::Fatal(codex_err));
                        } else if error.r#type.as_deref() == Some("usage_not_included") {
                            return Err(StreamAttemptError::Fatal(CodexErr::UsageNotIncluded));
                        }
                    }
                }

                Err(StreamAttemptError::RetryableHttpError {
                    status,
                    retry_after,
                    request_id,
                })
            }
            Err(e) => Err(StreamAttemptError::RetryableTransportError(e.into())),
        }
    }

    pub fn get_provider(&self) -> ModelProviderInfo {
        self.provider.clone()
    }

    pub fn get_otel_event_manager(&self) -> OtelEventManager {
        self.otel_event_manager.clone()
    }

    /// Returns the currently configured model slug.
    pub fn get_model(&self) -> String {
        self.config.model.clone()
    }

    /// Returns the currently configured model family.
    pub fn get_model_family(&self) -> ModelFamily {
        self.config.model_family.clone()
    }

    /// Returns the current reasoning effort setting.
    pub fn get_reasoning_effort(&self) -> Option<ReasoningEffortConfig> {
        self.effort
    }

    /// Returns the current reasoning summary setting.
    pub fn get_reasoning_summary(&self) -> ReasoningSummaryConfig {
        self.summary
    }

    pub fn get_auth_manager(&self) -> Option<Arc<AuthManager>> {
        self.auth_manager.clone()
    }
}

enum StreamAttemptError {
    RetryableHttpError {
        status: StatusCode,
        retry_after: Option<Duration>,
        request_id: Option<String>,
    },
    RetryableTransportError(CodexErr),
    Fatal(CodexErr),
}

impl StreamAttemptError {
    /// attempt is 0-based.
    fn delay(&self, attempt: u64) -> Duration {
        // backoff() uses 1-based attempts.
        let backoff_attempt = attempt + 1;
        match self {
            Self::RetryableHttpError { retry_after, .. } => {
                retry_after.unwrap_or_else(|| backoff(backoff_attempt))
            }
            Self::RetryableTransportError { .. } => backoff(backoff_attempt),
            Self::Fatal(_) => {
                // Should not be called on Fatal errors.
                Duration::from_secs(0)
            }
        }
    }

    fn into_error(self) -> CodexErr {
        match self {
            Self::RetryableHttpError {
                status, request_id, ..
            } => {
                if status == StatusCode::INTERNAL_SERVER_ERROR {
                    CodexErr::InternalServerError
                } else {
                    CodexErr::RetryLimit(RetryLimitReachedError { status, request_id })
                }
            }
            Self::RetryableTransportError(error) => error,
            Self::Fatal(error) => error,
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
    input_tokens: u64,
    input_tokens_details: Option<ResponseCompletedInputTokensDetails>,
    output_tokens: u64,
    output_tokens_details: Option<ResponseCompletedOutputTokensDetails>,
    total_tokens: u64,
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
    cached_tokens: u64,
}

#[derive(Debug, Deserialize)]
struct ResponseCompletedOutputTokensDetails {
    reasoning_tokens: u64,
}

fn attach_item_ids(payload_json: &mut Value, original_items: &[ResponseItem]) {
    let Some(input_value) = payload_json.get_mut("input") else {
        return;
    };
    let serde_json::Value::Array(items) = input_value else {
        return;
    };

    for (value, item) in items.iter_mut().zip(original_items.iter()) {
        if let ResponseItem::Reasoning { id, .. }
        | ResponseItem::Message { id: Some(id), .. }
        | ResponseItem::WebSearchCall { id: Some(id), .. }
        | ResponseItem::FunctionCall { id: Some(id), .. }
        | ResponseItem::LocalShellCall { id: Some(id), .. }
        | ResponseItem::CustomToolCall { id: Some(id), .. } = item
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
        "x-codex-primary-reset-after-seconds",
    );

    let secondary = parse_rate_limit_window(
        headers,
        "x-codex-secondary-used-percent",
        "x-codex-secondary-window-minutes",
        "x-codex-secondary-reset-after-seconds",
    );

    Some(RateLimitSnapshot { primary, secondary })
}

fn parse_rate_limit_window(
    headers: &HeaderMap,
    used_percent_header: &str,
    window_minutes_header: &str,
    resets_header: &str,
) -> Option<RateLimitWindow> {
    let used_percent: Option<f64> = parse_header_f64(headers, used_percent_header);

    used_percent.and_then(|used_percent| {
        let window_minutes = parse_header_u64(headers, window_minutes_header);
        let resets_in_seconds = parse_header_u64(headers, resets_header);

        let has_data = used_percent != 0.0
            || window_minutes.is_some_and(|minutes| minutes != 0)
            || resets_in_seconds.is_some_and(|seconds| seconds != 0);

        has_data.then_some(RateLimitWindow {
            used_percent,
            window_minutes,
            resets_in_seconds,
        })
    })
}

fn parse_header_f64(headers: &HeaderMap, name: &str) -> Option<f64> {
    parse_header_str(headers, name)?
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
}

fn parse_header_u64(headers: &HeaderMap, name: &str) -> Option<u64> {
    parse_header_str(headers, name)?.parse::<u64>().ok()
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
    S: Stream<Item = Result<Bytes>> + Unpin,
{
    let mut stream = stream.eventsource();

    // If the stream stays completely silent for an extended period treat it as disconnected.
    // The response id returned from the "complete" message.
    let mut response_completed: Option<ResponseCompleted> = None;
    let mut response_error: Option<CodexErr> = None;

    loop {
        let sse = match otel_event_manager
            .log_sse_event(|| timeout(idle_timeout, stream.next()))
            .await
        {
            Ok(Some(Ok(sse))) => sse,
            Ok(Some(Err(e))) => {
                debug!("SSE Error: {e:#}");
                let event = CodexErr::Stream(e.to_string(), None);
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
                        let error = response_error.unwrap_or(CodexErr::Stream(
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
                    .send(Err(CodexErr::Stream(
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
            // Individual output item finalised. Forward immediately so the
            // rest of the agent can stream assistant text/functions *live*
            // instead of waiting for the final `response.completed` envelope.
            //
            // IMPORTANT: We used to ignore these events and forward the
            // duplicated `output` array embedded in the `response.completed`
            // payload.  That produced two concrete issues:
            //   1. No real‑time streaming – the user only saw output after the
            //      entire turn had finished, which broke the "typing" UX and
            //      made long‑running turns look stalled.
            //   2. Duplicate `function_call_output` items – both the
            //      individual *and* the completed array were forwarded, which
            //      confused the backend and triggered 400
            //      "previous_response_not_found" errors because the duplicated
            //      IDs did not match the incremental turn chain.
            //
            // The fix is to forward the incremental events *as they come* and
            // drop the duplicated list inside `response.completed`.
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
                    let _ = tx_event.send(Ok(ResponseEvent::Created {})).await;
                }
            }
            "response.failed" => {
                if let Some(resp_val) = event.response {
                    response_error = Some(CodexErr::Stream(
                        "response.failed event received".to_string(),
                        None,
                    ));

                    let error = resp_val.get("error");

                    if let Some(error) = error {
                        match serde_json::from_value::<Error>(error.clone()) {
                            Ok(error) => {
                                let delay = try_parse_retry_after(&error);
                                let message = error.message.unwrap_or_default();
                                response_error = Some(CodexErr::Stream(message, delay));
                            }
                            Err(e) => {
                                let error = format!("failed to parse ErrorResponse: {e}");
                                debug!(error);
                                response_error = Some(CodexErr::Stream(error, None))
                            }
                        }
                    }
                }
            }
            // Final response completed – includes array of output items & id
            "response.completed" => {
                if let Some(resp_val) = event.response {
                    match serde_json::from_value::<ResponseCompleted>(resp_val) {
                        Ok(r) => {
                            response_completed = Some(r);
                        }
                        Err(e) => {
                            let error = format!("failed to parse ResponseCompleted: {e}");
                            debug!(error);
                            response_error = Some(CodexErr::Stream(error, None));
                            continue;
                        }
                    };
                };
            }
            "response.content_part.done"
            | "response.function_call_arguments.delta"
            | "response.custom_tool_call_input.delta"
            | "response.custom_tool_call_input.done" // also emitted as response.output_item.done
            | "response.in_progress"
            | "response.output_text.done" => {}
            "response.output_item.added" => {
                if let Some(item) = event.item.as_ref() {
                    // Detect web_search_call begin and forward a synthetic event upstream.
                    if let Some(ty) = item.get("type").and_then(|v| v.as_str())
                        && ty == "web_search_call"
                    {
                        let call_id = item
                            .get("id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let ev = ResponseEvent::WebSearchCallBegin { call_id };
                        if tx_event.send(Ok(ev)).await.is_err() {
                            return;
                        }
                    }
                }
            }
            "response.reasoning_summary_part.added" => {
                // Boundary between reasoning summary sections (e.g., titles).
                let event = ResponseEvent::ReasoningSummaryPartAdded;
                if tx_event.send(Ok(event)).await.is_err() {
                    return;
                }
            }
            "response.reasoning_summary_text.done" => {}
            _ => {}
        }
    }
}

/// used in tests to stream from a text SSE file
async fn stream_from_fixture(
    path: impl AsRef<Path>,
    provider: ModelProviderInfo,
    otel_event_manager: OtelEventManager,
) -> Result<ResponseStream> {
    let (tx_event, rx_event) = mpsc::channel::<Result<ResponseEvent>>(1600);
    let f = std::fs::File::open(path.as_ref())?;
    let lines = std::io::BufReader::new(f).lines();

    // insert \n\n after each line for proper SSE parsing
    let mut content = String::new();
    for line in lines {
        content.push_str(&line?);
        content.push_str("\n\n");
    }

    let rdr = std::io::Cursor::new(content);
    let stream = ReaderStream::new(rdr).map_err(CodexErr::Io);
    tokio::spawn(process_sse(
        stream,
        tx_event,
        provider.stream_idle_timeout(),
        otel_event_manager,
    ));
    Ok(ResponseStream { rx_event })
}

fn rate_limit_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();

    #[expect(clippy::unwrap_used)]
    RE.get_or_init(|| Regex::new(r"Please try again in (\d+(?:\.\d+)?)(s|ms)").unwrap())
}

fn try_parse_retry_after(err: &Error) -> Option<Duration> {
    if err.code != Some("rate_limit_exceeded".to_string()) {
        return None;
    }

    // parse the Please try again in 1.898s format using regex
    let re = rate_limit_regex();
    if let Some(message) = &err.message
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tokio::sync::mpsc;
    use tokio_test::io::Builder as IoBuilder;
    use tokio_util::io::ReaderStream;

    // ────────────────────────────
    // Helpers
    // ────────────────────────────

    /// Runs the SSE parser on pre-chunked byte slices and returns every event
    /// (including any final `Err` from a stream-closure check).
    async fn collect_events(
        chunks: &[&[u8]],
        provider: ModelProviderInfo,
        otel_event_manager: OtelEventManager,
    ) -> Vec<Result<ResponseEvent>> {
        let mut builder = IoBuilder::new();
        for chunk in chunks {
            builder.read(chunk);
        }

        let reader = builder.build();
        let stream = ReaderStream::new(reader).map_err(CodexErr::Io);
        let (tx, mut rx) = mpsc::channel::<Result<ResponseEvent>>(16);
        tokio::spawn(process_sse(
            stream,
            tx,
            provider.stream_idle_timeout(),
            otel_event_manager,
        ));

        let mut events = Vec::new();
        while let Some(ev) = rx.recv().await {
            events.push(ev);
        }
        events
    }

    /// Builds an in-memory SSE stream from JSON fixtures and returns only the
    /// successfully parsed events (panics on internal channel errors).
    async fn run_sse(
        events: Vec<serde_json::Value>,
        provider: ModelProviderInfo,
        otel_event_manager: OtelEventManager,
    ) -> Vec<ResponseEvent> {
        let mut body = String::new();
        for e in events {
            let kind = e
                .get("type")
                .and_then(|v| v.as_str())
                .expect("fixture event missing type");
            if e.as_object().map(|o| o.len() == 1).unwrap_or(false) {
                body.push_str(&format!("event: {kind}\n\n"));
            } else {
                body.push_str(&format!("event: {kind}\ndata: {e}\n\n"));
            }
        }

        let (tx, mut rx) = mpsc::channel::<Result<ResponseEvent>>(8);
        let stream = ReaderStream::new(std::io::Cursor::new(body)).map_err(CodexErr::Io);
        tokio::spawn(process_sse(
            stream,
            tx,
            provider.stream_idle_timeout(),
            otel_event_manager,
        ));

        let mut out = Vec::new();
        while let Some(ev) = rx.recv().await {
            out.push(ev.expect("channel closed"));
        }
        out
    }

    fn otel_event_manager() -> OtelEventManager {
        OtelEventManager::new(
            ConversationId::new(),
            "test",
            "test",
            None,
            Some(AuthMode::ChatGPT),
            false,
            "test".to_string(),
        )
    }

    // ────────────────────────────
    // Tests from `implement-test-for-responses-api-sse-parser`
    // ────────────────────────────

    #[tokio::test]
    async fn parses_items_and_completed() {
        let item1 = json!({
            "type": "response.output_item.done",
            "item": {
                "type": "message",
                "role": "assistant",
                "content": [{"type": "output_text", "text": "Hello"}]
            }
        })
        .to_string();

        let item2 = json!({
            "type": "response.output_item.done",
            "item": {
                "type": "message",
                "role": "assistant",
                "content": [{"type": "output_text", "text": "World"}]
            }
        })
        .to_string();

        let completed = json!({
            "type": "response.completed",
            "response": { "id": "resp1" }
        })
        .to_string();

        let sse1 = format!("event: response.output_item.done\ndata: {item1}\n\n");
        let sse2 = format!("event: response.output_item.done\ndata: {item2}\n\n");
        let sse3 = format!("event: response.completed\ndata: {completed}\n\n");

        let provider = ModelProviderInfo {
            name: "test".to_string(),
            base_url: Some("https://test.com".to_string()),
            env_key: Some("TEST_API_KEY".to_string()),
            env_key_instructions: None,
            wire_api: WireApi::Responses,
            query_params: None,
            http_headers: None,
            env_http_headers: None,
            request_max_retries: Some(0),
            stream_max_retries: Some(0),
            stream_idle_timeout_ms: Some(1000),
            requires_openai_auth: false,
        };

        let otel_event_manager = otel_event_manager();

        let events = collect_events(
            &[sse1.as_bytes(), sse2.as_bytes(), sse3.as_bytes()],
            provider,
            otel_event_manager,
        )
        .await;

        assert_eq!(events.len(), 3);

        matches!(
            &events[0],
            Ok(ResponseEvent::OutputItemDone(ResponseItem::Message { role, .. }))
                if role == "assistant"
        );

        matches!(
            &events[1],
            Ok(ResponseEvent::OutputItemDone(ResponseItem::Message { role, .. }))
                if role == "assistant"
        );

        match &events[2] {
            Ok(ResponseEvent::Completed {
                response_id,
                token_usage,
            }) => {
                assert_eq!(response_id, "resp1");
                assert!(token_usage.is_none());
            }
            other => panic!("unexpected third event: {other:?}"),
        }
    }

    #[tokio::test]
    async fn error_when_missing_completed() {
        let item1 = json!({
            "type": "response.output_item.done",
            "item": {
                "type": "message",
                "role": "assistant",
                "content": [{"type": "output_text", "text": "Hello"}]
            }
        })
        .to_string();

        let sse1 = format!("event: response.output_item.done\ndata: {item1}\n\n");
        let provider = ModelProviderInfo {
            name: "test".to_string(),
            base_url: Some("https://test.com".to_string()),
            env_key: Some("TEST_API_KEY".to_string()),
            env_key_instructions: None,
            wire_api: WireApi::Responses,
            query_params: None,
            http_headers: None,
            env_http_headers: None,
            request_max_retries: Some(0),
            stream_max_retries: Some(0),
            stream_idle_timeout_ms: Some(1000),
            requires_openai_auth: false,
        };

        let otel_event_manager = otel_event_manager();

        let events = collect_events(&[sse1.as_bytes()], provider, otel_event_manager).await;

        assert_eq!(events.len(), 2);

        matches!(events[0], Ok(ResponseEvent::OutputItemDone(_)));

        match &events[1] {
            Err(CodexErr::Stream(msg, _)) => {
                assert_eq!(msg, "stream closed before response.completed")
            }
            other => panic!("unexpected second event: {other:?}"),
        }
    }

    #[tokio::test]
    async fn error_when_error_event() {
        let raw_error = r#"{"type":"response.failed","sequence_number":3,"response":{"id":"resp_689bcf18d7f08194bf3440ba62fe05d803fee0cdac429894","object":"response","created_at":1755041560,"status":"failed","background":false,"error":{"code":"rate_limit_exceeded","message":"Rate limit reached for gpt-5 in organization org-AAA on tokens per min (TPM): Limit 30000, Used 22999, Requested 12528. Please try again in 11.054s. Visit https://platform.openai.com/account/rate-limits to learn more."}, "usage":null,"user":null,"metadata":{}}}"#;

        let sse1 = format!("event: response.failed\ndata: {raw_error}\n\n");
        let provider = ModelProviderInfo {
            name: "test".to_string(),
            base_url: Some("https://test.com".to_string()),
            env_key: Some("TEST_API_KEY".to_string()),
            env_key_instructions: None,
            wire_api: WireApi::Responses,
            query_params: None,
            http_headers: None,
            env_http_headers: None,
            request_max_retries: Some(0),
            stream_max_retries: Some(0),
            stream_idle_timeout_ms: Some(1000),
            requires_openai_auth: false,
        };

        let otel_event_manager = otel_event_manager();

        let events = collect_events(&[sse1.as_bytes()], provider, otel_event_manager).await;

        assert_eq!(events.len(), 1);

        match &events[0] {
            Err(CodexErr::Stream(msg, delay)) => {
                assert_eq!(
                    msg,
                    "Rate limit reached for gpt-5 in organization org-AAA on tokens per min (TPM): Limit 30000, Used 22999, Requested 12528. Please try again in 11.054s. Visit https://platform.openai.com/account/rate-limits to learn more."
                );
                assert_eq!(*delay, Some(Duration::from_secs_f64(11.054)));
            }
            other => panic!("unexpected second event: {other:?}"),
        }
    }

    // ────────────────────────────
    // Table-driven test from `main`
    // ────────────────────────────

    /// Verifies that the adapter produces the right `ResponseEvent` for a
    /// variety of incoming `type` values.
    #[tokio::test]
    async fn table_driven_event_kinds() {
        struct TestCase {
            name: &'static str,
            event: serde_json::Value,
            expect_first: fn(&ResponseEvent) -> bool,
            expected_len: usize,
        }

        fn is_created(ev: &ResponseEvent) -> bool {
            matches!(ev, ResponseEvent::Created)
        }
        fn is_output(ev: &ResponseEvent) -> bool {
            matches!(ev, ResponseEvent::OutputItemDone(_))
        }
        fn is_completed(ev: &ResponseEvent) -> bool {
            matches!(ev, ResponseEvent::Completed { .. })
        }

        let completed = json!({
            "type": "response.completed",
            "response": {
                "id": "c",
                "usage": {
                    "input_tokens": 0,
                    "input_tokens_details": null,
                    "output_tokens": 0,
                    "output_tokens_details": null,
                    "total_tokens": 0
                },
                "output": []
            }
        });

        let cases = vec![
            TestCase {
                name: "created",
                event: json!({"type": "response.created", "response": {}}),
                expect_first: is_created,
                expected_len: 2,
            },
            TestCase {
                name: "output_item.done",
                event: json!({
                    "type": "response.output_item.done",
                    "item": {
                        "type": "message",
                        "role": "assistant",
                        "content": [
                            {"type": "output_text", "text": "hi"}
                        ]
                    }
                }),
                expect_first: is_output,
                expected_len: 2,
            },
            TestCase {
                name: "unknown",
                event: json!({"type": "response.new_tool_event"}),
                expect_first: is_completed,
                expected_len: 1,
            },
        ];

        for case in cases {
            let mut evs = vec![case.event];
            evs.push(completed.clone());

            let provider = ModelProviderInfo {
                name: "test".to_string(),
                base_url: Some("https://test.com".to_string()),
                env_key: Some("TEST_API_KEY".to_string()),
                env_key_instructions: None,
                wire_api: WireApi::Responses,
                query_params: None,
                http_headers: None,
                env_http_headers: None,
                request_max_retries: Some(0),
                stream_max_retries: Some(0),
                stream_idle_timeout_ms: Some(1000),
                requires_openai_auth: false,
            };

            let otel_event_manager = otel_event_manager();

            let out = run_sse(evs, provider, otel_event_manager).await;
            assert_eq!(out.len(), case.expected_len, "case {}", case.name);
            assert!(
                (case.expect_first)(&out[0]),
                "first event mismatch in case {}",
                case.name
            );
        }
    }

    #[test]
    fn test_try_parse_retry_after() {
        let err = Error {
            r#type: None,
            message: Some("Rate limit reached for gpt-5 in organization org- on tokens per min (TPM): Limit 1, Used 1, Requested 19304. Please try again in 28ms. Visit https://platform.openai.com/account/rate-limits to learn more.".to_string()),
            code: Some("rate_limit_exceeded".to_string()),
            plan_type: None,
            resets_in_seconds: None
        };

        let delay = try_parse_retry_after(&err);
        assert_eq!(delay, Some(Duration::from_millis(28)));
    }

    #[test]
    fn test_try_parse_retry_after_no_delay() {
        let err = Error {
            r#type: None,
            message: Some("Rate limit reached for gpt-5 in organization <ORG> on tokens per min (TPM): Limit 30000, Used 6899, Requested 24050. Please try again in 1.898s. Visit https://platform.openai.com/account/rate-limits to learn more.".to_string()),
            code: Some("rate_limit_exceeded".to_string()),
            plan_type: None,
            resets_in_seconds: None
        };
        let delay = try_parse_retry_after(&err);
        assert_eq!(delay, Some(Duration::from_secs_f64(1.898)));
    }

    #[test]
    fn error_response_deserializes_old_schema_known_plan_type_and_serializes_back() {
        use crate::token_data::KnownPlan;
        use crate::token_data::PlanType;

        let json = r#"{"error":{"type":"usage_limit_reached","plan_type":"pro","resets_in_seconds":3600}}"#;
        let resp: ErrorResponse =
            serde_json::from_str(json).expect("should deserialize old schema");

        assert!(matches!(
            resp.error.plan_type,
            Some(PlanType::Known(KnownPlan::Pro))
        ));

        let plan_json = serde_json::to_string(&resp.error.plan_type).expect("serialize plan_type");
        assert_eq!(plan_json, "\"pro\"");
    }

    #[test]
    fn error_response_deserializes_old_schema_unknown_plan_type_and_serializes_back() {
        use crate::token_data::PlanType;

        let json =
            r#"{"error":{"type":"usage_limit_reached","plan_type":"vip","resets_in_seconds":60}}"#;
        let resp: ErrorResponse =
            serde_json::from_str(json).expect("should deserialize old schema");

        assert!(matches!(resp.error.plan_type, Some(PlanType::Unknown(ref s)) if s == "vip"));

        let plan_json = serde_json::to_string(&resp.error.plan_type).expect("serialize plan_type");
        assert_eq!(plan_json, "\"vip\"");
    }
}
