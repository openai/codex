use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use chrono::DateTime;
use chrono::Utc;
use codex_app_server_protocol::AuthMode;
use codex_otel::otel_event_manager::OtelEventManager;
use codex_protocol::ConversationId;
use codex_protocol::config_types::ReasoningEffort as ReasoningEffortConfig;
use codex_protocol::config_types::ReasoningSummary as ReasoningSummaryConfig;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::SessionSource;
use eventsource_stream::Eventsource;
use futures::prelude::*;
use regex_lite::Regex;
use reqwest::StatusCode;
use reqwest::header::HeaderMap;
use serde::Deserialize;
use serde_json::Value;
use tokio::sync::mpsc;
use tokio::time::timeout;
use tracing::debug;
use tracing::trace;
use tracing::warn;

use crate::AuthManager;
use crate::auth::CodexAuth;
use crate::auth::RefreshTokenError;
use crate::client::AggregateStreamExt;
use crate::client::Reasoning;
use crate::client::ResponseEvent;
use crate::client::ResponseStream;
use crate::client::ResponsesApiRequest;
use crate::client::create_text_param_for_request;
use crate::client_common::Prompt;
use crate::config::Config;
use crate::default_client::CodexHttpClient;
use crate::default_client::create_client;
use crate::error::CodexErr;
use crate::error::ConnectionFailedError;
use crate::error::ResponseStreamFailed;
use crate::error::Result;
use crate::error::UnexpectedResponseError;
use crate::error::UsageLimitReachedError;
use crate::flags::CODEX_RS_SSE_FIXTURE;
use crate::model_family::ModelFamily;
use crate::model_provider_info::ModelProviderInfo;
use crate::model_provider_info::WireApi;
use crate::openai_model_info::get_model_info;
use crate::protocol::RateLimitSnapshot;
use crate::protocol::RateLimitWindow;
use crate::protocol::TokenUsage;
use crate::token_data::PlanType;
use crate::tools::spec::create_tools_json_for_responses_api;
use crate::util::backoff;

#[derive(Debug, Deserialize)]
struct ErrorResponse {
    error: Error,
}

#[derive(Debug, Deserialize)]
struct Error {
    r#type: Option<String>,
    code: Option<String>,
    message: Option<String>,

    // Optional fields available on "usage_limit_reached" and "usage_not_included" errors
    plan_type: Option<PlanType>,
    resets_at: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct ModelClient {
    config: Arc<Config>,
    auth_manager: Option<Arc<AuthManager>>,
    otel_event_manager: OtelEventManager,
    client: CodexHttpClient,
    provider: ModelProviderInfo,
    conversation_id: ConversationId,
    effort: Option<ReasoningEffortConfig>,
    summary: ReasoningSummaryConfig,
    session_source: SessionSource,
}

#[allow(clippy::too_many_arguments)]
impl ModelClient {
    pub fn new(
        config: Arc<Config>,
        auth_manager: Option<Arc<AuthManager>>,
        otel_event_manager: OtelEventManager,
        provider: ModelProviderInfo,
        effort: Option<ReasoningEffortConfig>,
        summary: ReasoningSummaryConfig,
        conversation_id: ConversationId,
        session_source: SessionSource,
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
            session_source,
        }
    }

    pub fn get_model_context_window(&self) -> Option<i64> {
        let pct = self.config.model_family.effective_context_window_percent;
        self.config
            .model_context_window
            .or_else(|| get_model_info(&self.config.model_family).map(|info| info.context_window))
            .map(|w| w.saturating_mul(pct) / 100)
    }

    pub fn get_auto_compact_token_limit(&self) -> Option<i64> {
        self.config.model_auto_compact_token_limit.or_else(|| {
            get_model_info(&self.config.model_family).and_then(|info| info.auto_compact_token_limit)
        })
    }

    pub fn config(&self) -> Arc<Config> {
        Arc::clone(&self.config)
    }

    pub fn provider(&self) -> &ModelProviderInfo {
        &self.provider
    }

    pub async fn stream(&self, prompt: &Prompt) -> Result<ResponseStream> {
        match self.provider.wire_api {
            WireApi::Responses => self.stream_responses(prompt).await,
            WireApi::Chat => {
                // Create the raw streaming connection first.
                let response_stream = crate::client::stream_chat_completions(
                    prompt,
                    &self.config.model_family,
                    &self.client,
                    &self.provider,
                    &self.otel_event_manager,
                    &self.session_source,
                )
                .await?;

                // Wrap it with the aggregation adapter so callers see *only*
                // the final assistant message per turn (matching the
                // behaviour of the Responses API).
                let mut aggregated = if self.config.show_raw_agent_reasoning {
                    crate::client::AggregatedChatStream::streaming_mode(response_stream)
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
        if let Some(path) = *CODEX_RS_SSE_FIXTURE {
            // short circuit for tests
            warn!(path, "Streaming from fixture");
            return stream_from_fixture(
                Path::new(path),
                self.provider.clone(),
                self.otel_event_manager.clone(),
            )
            .await;
        }

        let auth_manager = self.auth_manager.clone();

        let full_instructions = prompt.get_full_instructions(&self.config.model_family);
        let tools_json: Vec<Value> = create_tools_json_for_responses_api(&prompt.tools)?;

        let reasoning = if self.config.model_family.supports_reasoning_summaries {
            Some(Reasoning {
                effort: self
                    .effort
                    .or(self.config.model_family.default_reasoning_effort),
                summary: Some(self.summary),
            })
        } else {
            None
        };

        let include: Vec<String> = if reasoning.is_some() {
            vec!["reasoning.encrypted_content".to_string()]
        } else {
            vec![]
        };

        let input_with_instructions = prompt.get_formatted_input();

        let verbosity = if self.config.model_family.support_verbosity {
            self.config
                .model_verbosity
                .or(self.config.model_family.default_verbosity)
        } else if self.config.model_verbosity.is_some() {
            warn!(
                "model_verbosity is set but ignored as the model does not support verbosity: {}",
                self.config.model_family.family
            );
            None
        } else {
            None
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
            parallel_tool_calls: prompt.parallel_tool_calls,
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

                    if let Some(delay) = retryable_attempt_error.delay(attempt) {
                        tokio::time::sleep(delay).await;
                    }
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
            "POST to {}: {}",
            self.provider.get_full_url(&auth),
            payload_json.to_string()
        );

        let mut req_builder = self
            .provider
            .create_request_builder(&self.client, &auth)
            .await
            .map_err(StreamAttemptError::Fatal)?;

        // Include subagent header only for subagent sessions.
        if let SessionSource::SubAgent(sub) = &self.session_source {
            let subagent = if let crate::protocol::SubAgentSource::Other(label) = sub {
                label.clone()
            } else {
                serde_json::to_value(sub)
                    .ok()
                    .and_then(|v| v.as_str().map(std::string::ToString::to_string))
                    .unwrap_or_else(|| "other".to_string())
            };
            req_builder = req_builder.header("x-openai-subagent", subagent);
        }

        req_builder = req_builder
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
        }

        match res {
            Ok(resp) if resp.status().is_success() => {
                let (tx_event, rx_event) = mpsc::channel::<Result<ResponseEvent>>(1600);

                // Always emit a RateLimits event at the start of a successful
                // stream so downstream consumers (Session) can surface an
                // initial TokenCount snapshot, even when the provider does not
                // send explicit rate limit headers.
                let snapshot =
                    parse_rate_limit_snapshot(resp.headers()).or(Some(RateLimitSnapshot {
                        primary: None,
                        secondary: None,
                    }));
                if let Some(snapshot) = snapshot
                    && tx_event
                        .send(Ok(ResponseEvent::RateLimits(snapshot)))
                        .await
                        .is_err()
                {
                    debug!("receiver dropped rate limit snapshot event");
                }

                // spawn task to process SSE
                let stream = resp.bytes_stream().map_err(move |e| {
                    CodexErr::ResponseStreamFailed(ResponseStreamFailed {
                        source: e,
                        request_id: request_id.clone(),
                    })
                });
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
                    && let Some(auth) = auth.as_ref()
                    && auth.mode == AuthMode::ChatGPT
                    && let Err(err) = manager.refresh_token().await
                {
                    let stream_error = match err {
                        RefreshTokenError::Permanent(failed) => {
                            StreamAttemptError::Fatal(CodexErr::RefreshTokenFailed(failed))
                        }
                        RefreshTokenError::Transient(other) => {
                            StreamAttemptError::RetryableTransportError(CodexErr::Io(other))
                        }
                    };
                    return Err(stream_error);
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
                            let resets_at = error
                                .resets_at
                                .and_then(|seconds| DateTime::<Utc>::from_timestamp(seconds, 0));
                            let codex_err = CodexErr::UsageLimitReached(UsageLimitReachedError {
                                plan_type,
                                resets_at,
                                rate_limits: rate_limit_snapshot,
                            });
                            return Err(StreamAttemptError::Fatal(codex_err));
                        } else if error.r#type.as_deref() == Some("usage_not_included") {
                            return Err(StreamAttemptError::Fatal(CodexErr::UsageNotIncluded));
                        } else if is_quota_exceeded_error(&error) {
                            return Err(StreamAttemptError::Fatal(CodexErr::QuotaExceeded));
                        }
                    }
                }

                Err(StreamAttemptError::RetryableHttpError {
                    status,
                    retry_after,
                    request_id,
                })
            }
            Err(e) => Err(StreamAttemptError::RetryableTransportError(
                CodexErr::ConnectionFailed(ConnectionFailedError { source: e }),
            )),
        }
    }

    pub fn get_provider(&self) -> ModelProviderInfo {
        self.provider.clone()
    }

    pub fn get_otel_event_manager(&self) -> OtelEventManager {
        self.otel_event_manager.clone()
    }

    pub fn get_session_source(&self) -> SessionSource {
        self.session_source.clone()
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

fn parse_rate_limit_snapshot(headers: &HeaderMap) -> Option<RateLimitSnapshot> {
    // Prefer codex-specific aggregate rate limit headers if present; fall back
    // to raw OpenAI-style request headers otherwise.
    parse_codex_rate_limits(headers).or_else(|| parse_openai_rate_limits(headers))
}

fn parse_codex_rate_limits(headers: &HeaderMap) -> Option<RateLimitSnapshot> {
    fn parse_f64(headers: &HeaderMap, name: &str) -> Option<f64> {
        headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<f64>().ok())
    }

    fn parse_i64(headers: &HeaderMap, name: &str) -> Option<i64> {
        headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<i64>().ok())
    }

    let primary_used = parse_f64(headers, "x-codex-primary-used-percent");
    let secondary_used = parse_f64(headers, "x-codex-secondary-used-percent");

    if primary_used.is_none() && secondary_used.is_none() {
        return None;
    }

    let primary = primary_used.map(|used_percent| RateLimitWindow {
        used_percent,
        window_minutes: parse_i64(headers, "x-codex-primary-window-minutes"),
        resets_at: parse_i64(headers, "x-codex-primary-reset-at"),
    });

    let secondary = secondary_used.map(|used_percent| RateLimitWindow {
        used_percent,
        window_minutes: parse_i64(headers, "x-codex-secondary-window-minutes"),
        resets_at: parse_i64(headers, "x-codex-secondary-reset-at"),
    });

    Some(RateLimitSnapshot { primary, secondary })
}

fn parse_openai_rate_limits(headers: &HeaderMap) -> Option<RateLimitSnapshot> {
    let limit = headers.get("x-ratelimit-limit-requests")?;
    let remaining = headers.get("x-ratelimit-remaining-requests")?;
    let reset_ms = headers.get("x-ratelimit-reset-requests")?;

    let limit = limit.to_str().ok()?.parse::<f64>().ok()?;
    let remaining = remaining.to_str().ok()?.parse::<f64>().ok()?;
    let reset_ms = reset_ms.to_str().ok()?.parse::<i64>().ok()?;

    if limit <= 0.0 {
        return None;
    }

    let used = (limit - remaining).max(0.0);
    let used_percent = (used / limit) * 100.0;

    let window_minutes = if reset_ms <= 0 {
        None
    } else {
        let seconds = reset_ms / 1000;
        Some((seconds + 59) / 60)
    };

    let resets_at = if reset_ms > 0 {
        Some(Utc::now().timestamp() + reset_ms / 1000)
    } else {
        None
    };

    Some(RateLimitSnapshot {
        primary: Some(RateLimitWindow {
            used_percent,
            window_minutes,
            resets_at,
        }),
        secondary: None,
    })
}

/// For Azure Responses endpoints we must use `store: true` and preserve
/// per-item identifiers on the input payload. The `ResponseItem` schema
/// deliberately skips serializing these IDs by default, so we patch them
/// back into the JSON body here based on the original input vector.
fn attach_item_ids(payload_json: &mut Value, original_input: &[ResponseItem]) {
    let Some(input_json) = payload_json.get_mut("input").and_then(Value::as_array_mut) else {
        return;
    };

    for (json_item, item) in input_json.iter_mut().zip(original_input.iter()) {
        let Some(obj) = json_item.as_object_mut() else {
            continue;
        };

        match item {
            ResponseItem::Message { id: Some(id), .. }
            | ResponseItem::LocalShellCall { id: Some(id), .. }
            | ResponseItem::FunctionCall { id: Some(id), .. }
            | ResponseItem::CustomToolCall { id: Some(id), .. }
            | ResponseItem::WebSearchCall { id: Some(id), .. } => {
                obj.insert("id".to_string(), Value::String(id.clone()));
            }
            ResponseItem::Reasoning { id, .. } if !id.is_empty() => {
                obj.insert("id".to_string(), Value::String(id.clone()));
            }
            _ => {}
        }
    }
}

fn try_parse_retry_after(error: &Error) -> Option<Duration> {
    let message = error.message.as_ref()?;
    let re = Regex::new(r"Try again in (\d+)ms").ok()?;
    let caps = re.captures(message)?;
    let delay_ms = caps.get(1)?.as_str().parse::<u64>().ok()?;
    Some(Duration::from_millis(delay_ms))
}

fn is_context_window_error(error: &Error) -> bool {
    error.r#type.as_deref() == Some("context_length_exceeded")
        || error.code.as_deref() == Some("context_length_exceeded")
}

fn is_quota_exceeded_error(error: &Error) -> bool {
    if let Some(code) = error.code.as_deref() {
        matches!(
            code,
            "insufficient_quota"
                | "insufficient_quota_org"
                | "insufficient_quota_project"
                | "insufficient_quota_user"
        )
    } else {
        false
    }
}

enum StreamAttemptError {
    Fatal(CodexErr),
    RetryableHttpError {
        status: StatusCode,
        retry_after: Option<Duration>,
        request_id: Option<String>,
    },
    RetryableTransportError(CodexErr),
}

impl StreamAttemptError {
    fn delay(&self, attempt: u64) -> Option<Duration> {
        match self {
            StreamAttemptError::RetryableHttpError { retry_after, .. } => {
                Some(retry_after.unwrap_or_else(|| backoff(attempt)))
            }
            StreamAttemptError::RetryableTransportError(_) => Some(backoff(attempt)),
            StreamAttemptError::Fatal(_) => None,
        }
    }

    fn into_error(self) -> CodexErr {
        match self {
            StreamAttemptError::Fatal(e) => e,
            StreamAttemptError::RetryableHttpError {
                status, request_id, ..
            } => CodexErr::UnexpectedStatus(UnexpectedResponseError {
                status,
                body: String::new(),
                request_id,
            }),
            StreamAttemptError::RetryableTransportError(e) => e,
        }
    }
}

async fn process_sse(
    stream: impl Stream<Item = std::result::Result<Bytes, CodexErr>> + Unpin + Send + 'static,
    tx_event: mpsc::Sender<Result<ResponseEvent>>,
    idle_timeout: Duration,
    otel_event_manager: OtelEventManager,
) {
    let mut stream = stream.eventsource();
    let mut response_completed: Option<ResponseCompleted> = None;
    let mut response_error: Option<CodexErr> = None;

    loop {
        let start = tokio::time::Instant::now();
        let next_event = timeout(idle_timeout, stream.next()).await;
        let duration = start.elapsed();
        otel_event_manager.log_sse_event(&next_event, duration);

        match next_event {
            Ok(Some(Ok(ev))) => {
                if let Err(e) =
                    handle_sse_event(ev, &mut response_completed, &mut response_error, &tx_event)
                        .await
                {
                    let _ = tx_event.send(Err(e)).await;
                    break;
                }
            }
            Ok(Some(Err(e))) => {
                let _ = tx_event
                    .send(Err(CodexErr::Stream(e.to_string(), None)))
                    .await;
                return;
            }
            Ok(None) => {
                break;
            }
            Err(_) => {
                let _ = tx_event
                    .send(Err(CodexErr::Stream(
                        "idle timeout waiting for SSE".to_string(),
                        None,
                    )))
                    .await;
                return;
            }
        }
    }

    match response_completed {
        Some(ResponseCompleted { id, usage }) => {
            if let Some(ref token_usage) = usage {
                otel_event_manager.sse_event_completed(
                    token_usage.input_tokens,
                    token_usage.output_tokens,
                    Some(token_usage.cached_input_tokens),
                    Some(token_usage.reasoning_output_tokens),
                    token_usage.total_tokens,
                );
            }

            let _ = tx_event
                .send(Ok(ResponseEvent::Completed {
                    response_id: id,
                    token_usage: usage,
                }))
                .await;
        }
        None => {
            let err = response_error.unwrap_or(CodexErr::Stream(
                "stream closed before response.completed".to_string(),
                None,
            ));
            otel_event_manager.see_event_completed_failed(&err);
            let _ = tx_event.send(Err(err)).await;
        }
    }
}

#[derive(Debug, Deserialize)]
struct SseEventWrapper {
    r#type: String,
    item: Option<Value>,
    response: Option<Value>,
    delta: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ResponseCompleted {
    id: String,
    #[serde(default, deserialize_with = "deserialize_usage")]
    usage: Option<TokenUsage>,
}

fn deserialize_usage<'de, D>(deserializer: D) -> std::result::Result<Option<TokenUsage>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    struct RawUsage {
        input_tokens: i64,
        #[serde(default)]
        input_tokens_details: Option<InputTokensDetails>,
        #[serde(default)]
        output_tokens: i64,
        #[serde(default)]
        output_tokens_details: Option<OutputTokensDetails>,
        #[serde(default)]
        total_tokens: i64,
    }

    #[derive(Deserialize)]
    struct InputTokensDetails {
        #[serde(default)]
        cached_tokens: Option<i64>,
    }

    #[derive(Deserialize)]
    struct OutputTokensDetails {
        #[serde(default)]
        reasoning_tokens: Option<i64>,
    }

    let raw: Option<RawUsage> = Option::deserialize(deserializer)?;
    let Some(raw) = raw else {
        return Ok(None);
    };

    let cached_input_tokens = raw
        .input_tokens_details
        .and_then(|d| d.cached_tokens)
        .unwrap_or(0);

    let reasoning_output_tokens = raw
        .output_tokens_details
        .and_then(|d| d.reasoning_tokens)
        .unwrap_or(0);

    Ok(Some(TokenUsage {
        input_tokens: raw.input_tokens,
        cached_input_tokens,
        output_tokens: raw.output_tokens,
        reasoning_output_tokens,
        total_tokens: raw.total_tokens,
    }))
}

async fn handle_sse_event(
    ev: eventsource_stream::Event,
    response_completed: &mut Option<ResponseCompleted>,
    response_error: &mut Option<CodexErr>,
    tx_event: &mpsc::Sender<Result<ResponseEvent>>,
) -> Result<()> {
    let data = ev.data;
    if data == "[DONE]" {
        // terminal event
        return Ok(());
    }

    let event: SseEventWrapper = serde_json::from_str(&data)?;
    debug!("processing SSE event type={}", event.r#type);
    match event.r#type.as_str() {
        "response.completed" => {
            if let Some(resp_val) = event.response {
                match serde_json::from_value::<ResponseCompleted>(resp_val) {
                    Ok(r) => {
                        *response_completed = Some(r);
                    }
                    Err(e) => {
                        let error = format!("failed to parse ResponseCompleted: {e}");
                        debug!(error);
                        *response_error = Some(CodexErr::Stream(error, None));
                        return Ok(());
                    }
                };
            };
        }
        "response.output_item.done" => {
            // For Responses API:
            //   - "response.output_item.done" contains the final item and we should
            //     drop the duplicated list inside `response.completed`.
            let Some(item_val) = event.item else {
                return Ok(());
            };
            let Ok(item) = serde_json::from_value::<ResponseItem>(item_val) else {
                debug!("failed to parse ResponseItem from output_item.done");
                return Ok(());
            };

            let event = ResponseEvent::OutputItemDone(item);
            if tx_event.send(Ok(event)).await.is_err() {
                return Ok(());
            }
        }
        "response.output_text.delta" => {
            if let Some(delta) = event.delta {
                let event = ResponseEvent::OutputTextDelta(delta);
                if tx_event.send(Ok(event)).await.is_err() {
                    return Ok(());
                }
            }
        }
        "response.reasoning_summary_text.delta" => {
            if let Some(delta) = event.delta {
                let event = ResponseEvent::ReasoningSummaryDelta(delta);
                if tx_event.send(Ok(event)).await.is_err() {
                    return Ok(());
                }
            }
        }
        "response.reasoning_text.delta" => {
            if let Some(delta) = event.delta {
                let event = ResponseEvent::ReasoningContentDelta(delta);
                if tx_event.send(Ok(event)).await.is_err() {
                    return Ok(());
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
                *response_error = Some(CodexErr::Stream(
                    "response.failed event received".to_string(),
                    None,
                ));

                let error = resp_val.get("error");

                if let Some(error) = error {
                    match serde_json::from_value::<Error>(error.clone()) {
                        Ok(error) => {
                            if is_context_window_error(&error) {
                                *response_error = Some(CodexErr::ContextWindowExceeded);
                            } else if is_quota_exceeded_error(&error) {
                                *response_error = Some(CodexErr::QuotaExceeded);
                            } else {
                                let delay = try_parse_retry_after(&error);
                                let message = error.message.unwrap_or_default();
                                *response_error = Some(CodexErr::Stream(message, delay));
                            }
                        }
                        Err(e) => {
                            let error = format!("failed to parse ErrorResponse: {e}");
                            debug!(error);
                            *response_error = Some(CodexErr::Stream(error, None))
                        }
                    }
                }
            }
        }
        "response.output_item.added" => {
            let Some(item_val) = event.item else {
                return Ok(());
            };
            let Ok(item) = serde_json::from_value::<ResponseItem>(item_val) else {
                debug!("failed to parse ResponseItem from output_item.done");
                return Ok(());
            };

            let event = ResponseEvent::OutputItemAdded(item);
            if tx_event.send(Ok(event)).await.is_err() {
                return Ok(());
            }
        }
        "response.reasoning_summary_part.added" => {
            // Boundary between reasoning summary sections (e.g., titles).
            let event = ResponseEvent::ReasoningSummaryPartAdded;
            if tx_event.send(Ok(event)).await.is_err() {
                return Ok(());
            }
        }
        "response.content_part.done"
        | "response.function_call_arguments.delta"
        | "response.custom_tool_call_input.delta"
        | "response.custom_tool_call_input.done"
        | "response.in_progress"
        | "response.output_text.done" => {}
        other => {
            debug!("unhandled SSE event type: {other}");
        }
    }

    Ok(())
}

async fn stream_from_fixture(
    fixture_path: &Path,
    provider: ModelProviderInfo,
    otel_event_manager: OtelEventManager,
) -> Result<ResponseStream> {
    // Read the entire fixture into memory so we can normalize SSE framing for
    // the eventsource parser. In particular, ensure the final event is
    // terminated by a blank line so `response.completed` is emitted even when
    // the fixture omits the trailing separator.
    let mut content = std::fs::read_to_string(fixture_path)?;
    if content.ends_with("\n\n") {
        // Already correctly terminated.
    } else if content.ends_with('\n') {
        content.push('\n');
    } else {
        content.push('\n');
        content.push('\n');
    }

    let stream = futures::stream::iter([Ok::<Bytes, CodexErr>(Bytes::from(content))]);

    let (tx_event, rx_event) = mpsc::channel::<Result<ResponseEvent>>(1600);

    tokio::spawn(process_sse(
        stream,
        tx_event,
        provider.stream_idle_timeout(),
        otel_event_manager,
    ));

    Ok(ResponseStream { rx_event })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::ResponseEvent;
    use codex_app_server_protocol::AuthMode;
    use codex_protocol::ConversationId;
    use codex_protocol::models::ResponseItem;

    use futures::StreamExt;
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use std::path::Path;

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

        let provider = test_provider();

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
        let provider = test_provider();

        let otel_event_manager = otel_event_manager();

        let events = collect_events(&[sse1.as_bytes()], provider, otel_event_manager).await;

        assert_eq!(events.len(), 2);
        matches!(events[0], Ok(ResponseEvent::OutputItemDone(_)));
        matches!(
            &events[1],
            Err(CodexErr::Stream(message, _))
                if message.contains("stream closed before response.completed")
        );
    }

    #[tokio::test]
    async fn cli_fixture_streams_created_item_and_completed() {
        let fixture_path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/cli_responses_fixture.sse");

        let provider = test_provider();

        let otel_event_manager = otel_event_manager();
        let result =
            stream_from_fixture(fixture_path.as_path(), provider, otel_event_manager).await;

        let mut stream = result.expect("stream should be created");
        let mut events = Vec::new();
        while let Some(event) = stream.next().await {
            events.push(event);
        }

        assert_eq!(events.len(), 3);
        matches!(events[0], Ok(ResponseEvent::Created));
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
            other => panic!("unexpected final event: {other:?}"),
        }
    }

    #[tokio::test]
    async fn cli_fixture_drain_to_completed_like_loop_succeeds() {
        let fixture_path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/cli_responses_fixture.sse");

        let provider = test_provider();

        let otel_event_manager = otel_event_manager();
        let result =
            stream_from_fixture(fixture_path.as_path(), provider, otel_event_manager).await;

        let mut stream = result.expect("stream should be created");

        loop {
            let maybe_event = stream.next().await;
            let Some(event) = maybe_event else {
                panic!("stream closed before response.completed");
            };

            match event {
                Ok(ResponseEvent::Completed { .. }) => break,
                Ok(_) => continue,
                Err(e) => panic!("unexpected error from stream: {e}"),
            }
        }
    }

    fn otel_event_manager() -> OtelEventManager {
        OtelEventManager::new(
            ConversationId::default(),
            "test-model",
            "test-model",
            None,
            Some("test@test.com".to_string()),
            Some(AuthMode::ChatGPT),
            false,
            "test".to_string(),
        )
    }

    fn test_provider() -> ModelProviderInfo {
        ModelProviderInfo {
            name: "test".to_string(),
            base_url: Some("https://test.com".to_string()),
            env_key: Some("TEST_API_KEY".to_string()),
            env_key_instructions: None,
            experimental_bearer_token: None,
            wire_api: WireApi::Responses,
            query_params: None,
            http_headers: None,
            env_http_headers: None,
            request_max_retries: Some(0),
            stream_max_retries: Some(0),
            stream_idle_timeout_ms: Some(1000),
            requires_openai_auth: false,
        }
    }

    async fn collect_events(
        chunks: &[&[u8]],
        provider: ModelProviderInfo,
        otel_event_manager: OtelEventManager,
    ) -> Vec<Result<ResponseEvent>> {
        let owned_chunks: Vec<Vec<u8>> = chunks.iter().map(|chunk| (*chunk).to_vec()).collect();

        let stream = futures::stream::iter(
            owned_chunks
                .into_iter()
                .map(|bytes| Ok::<Bytes, CodexErr>(Bytes::from(bytes))),
        );

        let (tx_event, rx_event) = mpsc::channel::<Result<ResponseEvent>>(1600);

        process_sse(
            stream,
            tx_event,
            provider.stream_idle_timeout(),
            otel_event_manager,
        )
        .await;

        ResponseStream { rx_event }.collect().await
    }

    #[tokio::test]
    async fn try_parse_retry_after_parses_delay() {
        let error = Error {
            r#type: None,
            code: None,
            message: Some("Try again in 250ms".to_string()),
            plan_type: None,
            resets_at: None,
        };

        let delay = try_parse_retry_after(&error).expect("expected delay");
        assert_eq!(delay, Duration::from_millis(250));
    }

    #[tokio::test]
    async fn try_parse_retry_after_azure_format() {
        let error = Error {
            r#type: None,
            code: None,
            message: Some("Service overloaded. Try again in 500ms.".to_string()),
            plan_type: None,
            resets_at: None,
        };

        let delay = try_parse_retry_after(&error).expect("expected delay");
        assert_eq!(delay, Duration::from_millis(500));
    }

    #[tokio::test]
    async fn try_parse_retry_after_no_delay() {
        let error = Error {
            r#type: None,
            code: None,
            message: Some("No retry suggestion here".to_string()),
            plan_type: None,
            resets_at: None,
        };

        assert!(try_parse_retry_after(&error).is_none());
    }

    #[tokio::test]
    async fn context_window_error_is_fatal() {
        let file = tempfile::NamedTempFile::new().unwrap();
        let fixture_path = file.path().to_path_buf();
        let sse = concat!(
            "data: {\"type\":\"response.failed\",\"response\":{\"error\":",
            "{\"type\":\"context_length_exceeded\",\"message\":\"too long\"}}}\n\n",
            "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp1\"}}\n\n",
            "data: [DONE]\n\n",
        );

        std::fs::write(&fixture_path, sse).unwrap();

        let provider = test_provider();

        let otel_event_manager = otel_event_manager();
        let result =
            stream_from_fixture(fixture_path.as_path(), provider, otel_event_manager.clone()).await;

        let mut stream = result.expect("stream should be created");
        let mut events = Vec::new();
        while let Some(event) = stream.next().await {
            events.push(event);
        }

        assert_eq!(events.len(), 1);
        matches!(events[0], Err(CodexErr::ContextWindowExceeded));
    }

    #[tokio::test]
    async fn quota_exceeded_error_is_fatal() {
        let file = tempfile::NamedTempFile::new().unwrap();
        let fixture_path = file.path().to_path_buf();
        let sse = concat!(
            "data: {\"type\":\"response.failed\",\"response\":{\"error\":",
            "{\"type\":\"usage_limit_reached\",\"code\":\"insufficient_quota\",",
            "\"message\":\"quota exceeded\"}}}\n\n",
            "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp1\"}}\n\n",
            "data: [DONE]\n\n",
        );

        std::fs::write(&fixture_path, sse).unwrap();

        let provider = test_provider();

        let otel_event_manager = otel_event_manager();
        let result =
            stream_from_fixture(fixture_path.as_path(), provider, otel_event_manager.clone()).await;

        let mut stream = result.expect("stream should be created");
        let mut events = Vec::new();
        while let Some(event) = stream.next().await {
            events.push(event);
        }

        assert_eq!(events.len(), 1);
        matches!(events[0], Err(CodexErr::QuotaExceeded));
    }

    #[tokio::test]
    async fn context_window_error_with_newline_is_fatal() {
        let file = tempfile::NamedTempFile::new().unwrap();
        let fixture_path = file.path().to_path_buf();
        let sse = concat!(
            "data: {\"type\":\"response.failed\",\"response\":{\"error\":",
            "{\"type\":\"context_length_exceeded\",\"code\":\"context_length_exceeded\",",
            "\"message\":\"This is a multi-line error\\nwith additional details\"}}}\n\n",
            "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp1\"}}\n\n",
            "data: [DONE]\n\n",
        );

        std::fs::write(&fixture_path, sse).unwrap();

        let provider = test_provider();

        let otel_event_manager = otel_event_manager();
        let result =
            stream_from_fixture(fixture_path.as_path(), provider, otel_event_manager.clone()).await;

        let mut stream = result.expect("stream should be created");
        let mut events = Vec::new();
        while let Some(event) = stream.next().await {
            events.push(event);
        }

        assert_eq!(events.len(), 1);
        matches!(events[0], Err(CodexErr::ContextWindowExceeded));
    }
}
