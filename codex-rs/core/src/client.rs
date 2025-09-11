use std::io::BufRead;
use std::path::Path;
use std::sync::OnceLock;
use std::time::Duration;

use crate::AuthManager;
use bytes::Bytes;
use codex_protocol::mcp_protocol::AuthMode;
use codex_protocol::mcp_protocol::ConversationId;
use eventsource_stream::Eventsource;
use futures::prelude::*;
use regex_lite::Regex;
use reqwest::StatusCode;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use tokio::sync::mpsc;
use tokio::time::timeout;
use tokio_util::io::ReaderStream;
use tracing::Instrument;
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
use crate::protocol::TokenUsage;
use crate::token_data::PlanType;
use crate::util::backoff;
use codex_otel::trace_manager::TraceManager;
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
    trace_manager: TraceManager,
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
        trace_manager: TraceManager,
        provider: ModelProviderInfo,
        effort: Option<ReasoningEffortConfig>,
        summary: ReasoningSummaryConfig,
        conversation_id: ConversationId,
    ) -> Self {
        let client = create_client();

        Self {
            config,
            auth_manager,
            trace_manager,
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
                    &self.trace_manager,
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
            return stream_from_fixture(path, self.provider.clone(), self.trace_manager.clone())
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

        // Only include `text.verbosity` for GPT-5 family models
        let text = if self.config.model_family.family == "gpt-5" {
            create_text_param_for_request(self.config.model_verbosity)
        } else {
            if self.config.model_verbosity.is_some() {
                warn!(
                    "model_verbosity is set but ignored for non-gpt-5 model family: {}",
                    self.config.model_family.family
                );
            }
            None
        };

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
        let payload_body = serde_json::to_string(&payload_json)?;

        let mut attempt = 0;
        let max_retries = self.provider.request_max_retries();

        loop {
            attempt += 1;

            // Always fetch the latest auth in case a prior attempt refreshed the token.
            let auth = auth_manager.as_ref().and_then(|m| m.auth());

            trace!(
                "POST to {}: {}",
                self.provider.get_full_url(&auth),
                payload_body.as_str()
            );

            let request_span = self.trace_manager.request(&input_with_instructions);

            let mut req_builder = self
                .provider
                .create_request_builder(&self.client, &auth)
                .await?;

            req_builder = req_builder
                .header("OpenAI-Beta", "responses=experimental")
                // Send session_id for compatibility.
                .header("conversation_id", self.conversation_id.to_string())
                .header("session_id", self.conversation_id.to_string())
                .header(reqwest::header::ACCEPT, "text/event-stream")
                .json(&payload_json);

            if let Some(auth) = auth.as_ref()
                && auth.mode == AuthMode::ChatGPT
                && let Some(account_id) = auth.get_account_id()
            {
                req_builder = req_builder.header("chatgpt-account-id", account_id);
            }

            let tracing_headers = TraceManager::headers(&request_span);

            req_builder = req_builder.headers(tracing_headers);

            let res = req_builder.send().instrument(request_span.span()).await;

            if let Ok(resp) = &res {
                let request_id = resp
                    .headers()
                    .get("x-request-id")
                    .map(|v| v.to_str().unwrap_or_default())
                    .unwrap_or_default();
                trace!(
                    "Response status: {}, cf-ray: {}",
                    resp.status(),
                    resp.headers()
                        .get("cf-ray")
                        .map(|v| v.to_str().unwrap_or_default())
                        .unwrap_or_default()
                );
                request_span.request_id(request_id);
            }

            match res {
                Ok(resp) if resp.status().is_success() => {
                    request_span.status_code(resp.status());
                    let (tx_event, rx_event) = mpsc::channel::<Result<ResponseEvent>>(1600);

                    // spawn task to process SSE
                    let stream = resp.bytes_stream().map_err(CodexErr::Reqwest);
                    tokio::spawn(process_sse(
                        stream,
                        tx_event,
                        self.provider.stream_idle_timeout(),
                        self.trace_manager.clone(),
                    ));

                    return Ok(ResponseStream { rx_event });
                }
                Ok(res) => {
                    let status = res.status();
                    let mut log_message = status.to_string();

                    // Pull out Retry‑After header if present.
                    let retry_after_secs = res
                        .headers()
                        .get(reqwest::header::RETRY_AFTER)
                        .and_then(|v| v.to_str().ok())
                        .and_then(|s| s.parse::<u64>().ok());

                    if status == StatusCode::UNAUTHORIZED
                        && let Some(manager) = auth_manager.as_ref()
                        && manager.auth().is_some()
                    {
                        let _ = manager.refresh_token().await;
                    }

                    if !(status == StatusCode::TOO_MANY_REQUESTS
                        || status == StatusCode::UNAUTHORIZED
                        || status.is_server_error())
                    {
                        let body = res.text().await.unwrap_or_default();
                        if !body.is_empty() {
                            log_message = format!("{log_message}: {body}");
                        }
                        request_span.error(attempt, Some(status), &log_message);
                        return Err(CodexErr::UnexpectedStatus(status, body));
                    }

                    if status == StatusCode::TOO_MANY_REQUESTS {
                        let body = res.json::<ErrorResponse>().await.ok();
                        if let Some(ErrorResponse { error }) = body {
                            if error.r#type.as_deref() == Some("usage_limit_reached") {
                                let plan_type = error
                                    .plan_type
                                    .or_else(|| auth.as_ref().and_then(|a| a.get_plan_type()));
                                let resets_in_seconds = error.resets_in_seconds;
                                request_span.error(attempt, Some(status), &log_message);
                                return Err(CodexErr::UsageLimitReached(UsageLimitReachedError {
                                    plan_type,
                                    resets_in_seconds,
                                }));
                            } else if error.r#type.as_deref() == Some("usage_not_included") {
                                request_span.error(attempt, Some(status), &log_message);
                                return Err(CodexErr::UsageNotIncluded);
                            }
                        }
                    }

                    if attempt > max_retries {
                        request_span.error(attempt, Some(status), &log_message);
                        if status == StatusCode::INTERNAL_SERVER_ERROR {
                            return Err(CodexErr::InternalServerError);
                        }

                        return Err(CodexErr::RetryLimit(status));
                    }

                    request_span.error(attempt, Some(status), &log_message);

                    let delay = retry_after_secs
                        .map(|s| Duration::from_millis(s * 1_000))
                        .unwrap_or_else(|| backoff(attempt));
                    tokio::time::sleep(delay).await;
                }
                Err(e) => {
                    let error_message = format!("{e:#}");
                    request_span.error(attempt, None, &error_message);
                    if attempt > max_retries {
                        return Err(e.into());
                    }
                    let delay = backoff(attempt);
                    tokio::time::sleep(delay).await;
                }
            }
        }
    }

    pub fn get_provider(&self) -> ModelProviderInfo {
        self.provider.clone()
    }

    pub fn get_trace_manager(&self) -> TraceManager {
        self.trace_manager.clone()
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

    pub fn get_conversation_id(&self) -> ConversationId {
        self.conversation_id
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
struct ResponseCreated {}

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

async fn process_sse<S>(
    stream: S,
    tx_event: mpsc::Sender<Result<ResponseEvent>>,
    idle_timeout: Duration,
    trace_manager: TraceManager,
) where
    S: Stream<Item = Result<Bytes>> + Unpin,
{
    let mut stream = stream.eventsource();

    // If the stream stays completely silent for an extended period treat it as disconnected.
    // The response id returned from the "complete" message.
    let mut response_completed: Option<ResponseCompleted> = None;
    let mut response_error: Option<CodexErr> = None;

    let sse_span = trace_manager.response();

    loop {
        let sse = match timeout(idle_timeout, stream.next().instrument(sse_span.span())).await {
            Ok(Some(Ok(sse))) => sse,
            Ok(Some(Err(e))) => {
                debug!("SSE Error: {e:#}");
                let error = e.to_string();
                sse_span.error(error.as_str());
                let event = CodexErr::Stream(error, None);
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
                            sse_span.token_usage(
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

                        if let CodexErr::Stream(message, _) = &error {
                            sse_span.error(message.as_str());
                        }

                        let _ = tx_event.send(Err(error)).await;
                    }
                }
                return;
            }
            Err(_) => {
                let error = "idle timeout waiting for SSE";

                sse_span.error(error);

                let _ = tx_event
                    .send(Err(CodexErr::Stream(error.into(), None)))
                    .await;
                return;
            }
        };

        let raw = sse.data.clone();
        trace!("SSE event: {}", raw);

        sse_span.body(raw.as_str());

        let event: SseEvent = match serde_json::from_str(&sse.data) {
            Ok(event) => event,
            Err(e) => {
                let error = format!("Failed to parse SSE event: {e}, data: {}", &sse.data);
                sse_span.error(error.as_str());
                debug!(error);
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
                    let error = "failed to parse ResponseItem from output_item.done";
                    debug!(error);
                    sse_span.error(error);
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
                    let error_message = "response.failed event received";
                    response_error = Some(CodexErr::Stream(
                        error_message.to_string(),
                        None,
                    ));

                    sse_span.error(error_message);

                    let error = resp_val.get("error");

                    if let Some(error) = error {
                        match serde_json::from_value::<Error>(error.clone()) {
                            Ok(error) => {
                                let delay = try_parse_retry_after(&error);
                                let message = error.message.unwrap_or_default();
                                sse_span.error(message.as_str());
                                response_error = Some(CodexErr::Stream(message, delay));
                            }
                            Err(e) => {
                                let error_message = format!("failed to parse ErrorResponse: {e}");
                                debug!(error_message);
                                sse_span.error(error_message.as_str());
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
                            let error_message = format!("failed to parse ResponseCompleted: {e}");
                            debug!(error_message);
                            sse_span.error(error_message.as_str());
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
    trace_manager: TraceManager,
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
        trace_manager,
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
        trace_manager: TraceManager,
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
            trace_manager,
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
        trace_manager: TraceManager,
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
            trace_manager,
        ));

        let mut out = Vec::new();
        while let Some(ev) = rx.recv().await {
            out.push(ev.expect("channel closed"));
        }
        out
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

        let trace_manager = TraceManager::new(
            ConversationId::new(),
            "test",
            "test",
            None,
            AuthMode::ChatGPT,
            false,
            "test".to_string(),
        );

        let events = collect_events(
            &[sse1.as_bytes(), sse2.as_bytes(), sse3.as_bytes()],
            provider,
            trace_manager,
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

        let trace_manager = TraceManager::new(
            ConversationId::new(),
            "test",
            "test",
            None,
            AuthMode::ChatGPT,
            false,
            "test".to_string(),
        );

        let events = collect_events(&[sse1.as_bytes()], provider, trace_manager).await;

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

        let trace_manager = TraceManager::new(
            ConversationId::new(),
            "test",
            "test",
            None,
            AuthMode::ChatGPT,
            false,
            "test".to_string(),
        );

        let events = collect_events(&[sse1.as_bytes()], provider, trace_manager).await;

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

            let trace_manager = TraceManager::new(
                ConversationId::new(),
                "test",
                "test",
                None,
                AuthMode::ChatGPT,
                false,
                "test".to_string(),
            );

            let out = run_sse(evs, provider, trace_manager).await;
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
