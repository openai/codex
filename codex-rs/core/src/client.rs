use std::sync::Arc;

use crate::api_bridge::auth_provider_from_auth;
use crate::api_bridge::create_transport;
use crate::api_bridge::map_api_error;
use crate::api_bridge::to_api_provider;
use codex_api::CompactClient as ApiCompactClient;
use codex_api::ResponsesClient as ApiResponsesClient;
use codex_api::SseTelemetry;
use codex_api::error::ApiError;
use codex_app_server_protocol::AuthMode;
use codex_client::RequestTelemetry;
use codex_client::TransportError;
use codex_otel::otel_event_manager::OtelEventManager;
use codex_protocol::ConversationId;
use codex_protocol::config_types::ReasoningEffort as ReasoningEffortConfig;
use codex_protocol::config_types::ReasoningSummary as ReasoningSummaryConfig;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::SessionSource;
use eventsource_stream::Event;
use eventsource_stream::EventStreamError;
use futures::StreamExt;
use http::HeaderMap as ApiHeaderMap;
use http::HeaderValue;
use http::StatusCode as HttpStatusCode;
use reqwest::StatusCode;
use serde::Serialize;
use serde_json::Value;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::warn;

use crate::AuthManager;
use crate::auth::RefreshTokenError;
use crate::chat_completions::AggregateStreamExt;
use crate::chat_completions::stream_chat_completions;
use crate::client_common::Prompt;
use crate::client_common::Reasoning;
use crate::client_common::ResponseEvent;
use crate::client_common::ResponseStream;
use crate::client_common::ResponsesApiRequest;
use crate::client_common::create_text_param_for_request;
use crate::config::Config;
use crate::default_client::CodexHttpClient;
use crate::default_client::create_client;
use crate::error::CodexErr;
use crate::error::Result;
use crate::flags::CODEX_RS_SSE_FIXTURE;
use crate::model_family::ModelFamily;
use crate::model_provider_info::ModelProviderInfo;
use crate::model_provider_info::WireApi;
use crate::openai_model_info::get_model_info;
use crate::tools::spec::create_tools_json_for_responses_api;

#[derive(Debug, Serialize)]
struct CompactHistoryRequest<'a> {
    model: &'a str,
    input: &'a [ResponseItem],
    instructions: &'a str,
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
                let response_stream = stream_chat_completions(
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
            warn!(path, "Streaming from fixture");
            let stream = codex_api::stream_from_fixture(path, self.provider.stream_idle_timeout())
                .map_err(map_api_error)?;
            return Ok(map_response_stream(stream, self.otel_event_manager.clone()));
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
        } else {
            if self.config.model_verbosity.is_some() {
                warn!(
                    "model_verbosity is set but ignored as the model does not support verbosity: {}",
                    self.config.model_family.family
                );
            }
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

        let mut extra_headers = ApiHeaderMap::new();
        if let Ok(value) = HeaderValue::from_str(&self.conversation_id.to_string()) {
            extra_headers.insert("conversation_id", value.clone());
            extra_headers.insert("session_id", value);
        }

        if let SessionSource::SubAgent(sub) = &self.session_source {
            let subagent = if let crate::protocol::SubAgentSource::Other(label) = sub {
                label.clone()
            } else {
                serde_json::to_value(sub)
                    .ok()
                    .and_then(|v| v.as_str().map(std::string::ToString::to_string))
                    .unwrap_or_else(|| "other".to_string())
            };
            if let Ok(value) = HeaderValue::from_str(&subagent) {
                extra_headers.insert("x-openai-subagent", value);
            }
        }

        let mut refreshed = false;
        loop {
            let auth = auth_manager.as_ref().and_then(|m| m.auth());
            let api_provider = to_api_provider(&self.provider, auth.as_ref().map(|a| a.mode))?;
            let api_auth = auth_provider_from_auth(auth.clone(), &self.provider).await?;
            let transport = create_transport();
            let telemetry = Arc::new(ApiTelemetry::new(self.otel_event_manager.clone()));
            let request_telemetry: Arc<dyn RequestTelemetry> = telemetry.clone();
            let sse_telemetry: Arc<dyn SseTelemetry> = telemetry;
            let client = ApiResponsesClient::new(transport, api_provider, api_auth)
                .with_telemetry(Some(request_telemetry), Some(sse_telemetry));

            let stream_result = client
                .stream(payload_json.clone(), extra_headers.clone())
                .await;

            match stream_result {
                Ok(stream) => {
                    return Ok(map_response_stream(stream, self.otel_event_manager.clone()));
                }
                Err(ApiError::Transport(TransportError::Http { status, .. }))
                    if status == StatusCode::UNAUTHORIZED =>
                {
                    if refreshed {
                        return Err(map_api_error(ApiError::Transport(TransportError::Http {
                            status,
                            headers: None,
                            body: None,
                        })));
                    }

                    if let Some(manager) = auth_manager.as_ref()
                        && let Some(auth) = auth.as_ref()
                        && auth.mode == AuthMode::ChatGPT
                    {
                        match manager.refresh_token().await {
                            Ok(_) => {
                                refreshed = true;
                                continue;
                            }
                            Err(RefreshTokenError::Permanent(failed)) => {
                                return Err(CodexErr::RefreshTokenFailed(failed));
                            }
                            Err(RefreshTokenError::Transient(other)) => {
                                return Err(CodexErr::Io(other));
                            }
                        }
                    } else {
                        return Err(map_api_error(ApiError::Transport(TransportError::Http {
                            status,
                            headers: None,
                            body: None,
                        })));
                    }
                }
                Err(err) => return Err(map_api_error(err)),
            }
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

    pub async fn compact_conversation_history(&self, prompt: &Prompt) -> Result<Vec<ResponseItem>> {
        if prompt.input.is_empty() {
            return Ok(Vec::new());
        }
        let auth_manager = self.auth_manager.clone();
        let auth = auth_manager.as_ref().and_then(|m| m.auth());
        let api_provider = to_api_provider(&self.provider, auth.as_ref().map(|a| a.mode))?;
        let api_auth = auth_provider_from_auth(auth.clone(), &self.provider).await?;
        let transport = create_transport();
        let telemetry = Arc::new(ApiTelemetry::new(self.otel_event_manager.clone()));
        let request_telemetry: Arc<dyn RequestTelemetry> = telemetry;
        let client = ApiCompactClient::new(transport, api_provider, api_auth)
            .with_telemetry(Some(request_telemetry));

        let payload = CompactHistoryRequest {
            model: &self.config.model,
            input: &prompt.input,
            instructions: &prompt.get_full_instructions(&self.config.model_family),
        };
        let body = serde_json::to_value(&payload)?;

        let mut extra_headers = ApiHeaderMap::new();
        if let SessionSource::SubAgent(sub) = &self.session_source {
            let subagent = if let crate::protocol::SubAgentSource::Other(label) = sub {
                label.clone()
            } else {
                serde_json::to_value(sub)
                    .ok()
                    .and_then(|v| v.as_str().map(std::string::ToString::to_string))
                    .unwrap_or_else(|| "other".to_string())
            };
            if let Ok(val) = HeaderValue::from_str(&subagent) {
                extra_headers.insert("x-openai-subagent", val);
            }
        }

        client
            .compact(body, extra_headers)
            .await
            .map_err(map_api_error)
    }
}

fn map_response_stream(
    mut api_stream: codex_api::ResponseStream,
    otel_event_manager: OtelEventManager,
) -> ResponseStream {
    let (tx_event, rx_event) = mpsc::channel::<Result<ResponseEvent>>(1600);
    let manager = otel_event_manager.clone();

    tokio::spawn(async move {
        let mut logged_error = false;
        while let Some(event) = api_stream.next().await {
            match event {
                Ok(ResponseEvent::Completed {
                    response_id,
                    token_usage,
                }) => {
                    if let Some(usage) = &token_usage {
                        manager.sse_event_completed(
                            usage.input_tokens,
                            usage.output_tokens,
                            Some(usage.cached_input_tokens),
                            Some(usage.reasoning_output_tokens),
                            usage.total_tokens,
                        );
                    }
                    if tx_event
                        .send(Ok(ResponseEvent::Completed {
                            response_id,
                            token_usage,
                        }))
                        .await
                        .is_err()
                    {
                        return;
                    }
                }
                Ok(event) => {
                    if tx_event.send(Ok(event)).await.is_err() {
                        return;
                    }
                }
                Err(err) => {
                    let mapped = map_api_error(err);
                    if !logged_error {
                        manager.see_event_completed_failed(&mapped);
                        logged_error = true;
                    }
                    if tx_event.send(Err(mapped)).await.is_err() {
                        return;
                    }
                }
            }
        }
    });

    ResponseStream { rx_event }
}

struct ApiTelemetry {
    otel_event_manager: OtelEventManager,
}

impl ApiTelemetry {
    fn new(otel_event_manager: OtelEventManager) -> Self {
        Self { otel_event_manager }
    }
}

impl RequestTelemetry for ApiTelemetry {
    fn on_request(
        &self,
        attempt: u64,
        status: Option<HttpStatusCode>,
        error: Option<&TransportError>,
        duration: Duration,
    ) {
        let error_message = error.map(std::string::ToString::to_string);
        self.otel_event_manager.record_api_request(
            attempt,
            status.map(|s| s.as_u16()),
            error_message.as_deref(),
            duration,
        );
    }
}

impl SseTelemetry for ApiTelemetry {
    fn on_sse_poll(
        &self,
        result: &std::result::Result<
            Option<std::result::Result<Event, EventStreamError<TransportError>>>,
            tokio::time::error::Elapsed,
        >,
        duration: Duration,
    ) {
        self.otel_event_manager.log_sse_event(result, duration);
    }
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
