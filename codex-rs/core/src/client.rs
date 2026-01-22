use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::OnceLock;
use std::time::SystemTime;

use chrono::DateTime;
use chrono::Utc;
use crate::api_bridge::CoreAuthProvider;
use crate::api_bridge::auth_provider_from_auth;
use crate::api_bridge::map_api_error;
use crate::auth::UnauthorizedRecovery;
use codex_api::AggregateStreamExt;
use codex_api::ChatClient as ApiChatClient;
use codex_api::CompactClient as ApiCompactClient;
use codex_api::CompactionInput as ApiCompactionInput;
use codex_api::Prompt as ApiPrompt;
use codex_api::RequestTelemetry;
use codex_api::ReqwestTransport;
use codex_api::ResponseAppendWsRequest;
use codex_api::ResponseCreateWsRequest;
use codex_api::ResponseStream as ApiResponseStream;
use codex_api::ResponsesClient as ApiResponsesClient;
use codex_api::ResponsesOptions as ApiResponsesOptions;
use codex_api::ResponsesWebsocketClient as ApiWebSocketResponsesClient;
use codex_api::ResponsesWebsocketConnection as ApiWebSocketConnection;
use codex_api::SseTelemetry;
use codex_api::TransportError;
use codex_api::build_conversation_headers;
use codex_api::common::Reasoning;
use codex_api::common::ResponsesWsRequest;
use codex_api::create_text_param_for_request;
use codex_api::error::ApiError;
use codex_api::requests::responses::Compression;
use codex_app_server_protocol::AuthMode;
use codex_otel::OtelManager;

use codex_protocol::ThreadId;
use codex_protocol::config_types::ReasoningSummary as ReasoningSummaryConfig;
use codex_protocol::config_types::WebSearchMode;
use codex_protocol::models::ResponseItem;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ReasoningEffort as ReasoningEffortConfig;
use codex_protocol::protocol::SessionSource;
use eventsource_stream::Event;
use eventsource_stream::EventStreamError;
use futures::StreamExt;
use http::HeaderMap as ApiHeaderMap;
use http::HeaderValue;
use http::StatusCode as HttpStatusCode;
use reqwest::StatusCode;
use serde_json::Value;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::warn;

use crate::AuthManager;
use crate::CodexAuth;
use crate::auth::RefreshTokenError;
use crate::auth::DEFAULT_OAUTH_NAMESPACE;
use crate::client_common::Prompt;
use crate::client_common::ResponseEvent;
use crate::client_common::ResponseStream;
use crate::config::Config;
use crate::default_client::build_reqwest_client;
use crate::error::CodexErr;
use crate::error::RefreshTokenFailedReason;
use crate::error::Result;
use crate::features::FEATURES;
use crate::features::Feature;
use crate::flags::CODEX_RS_SSE_FIXTURE;
use crate::model_provider_info::ModelProviderInfo;
use crate::model_provider_info::WireApi;
use crate::tools::spec::create_tools_json_for_chat_completions_api;
use crate::tools::spec::create_tools_json_for_responses_api;

pub const WEB_SEARCH_ELIGIBLE_HEADER: &str = "x-oai-web-search-eligible";
pub const X_CODEX_TURN_STATE_HEADER: &str = "x-codex-turn-state";
const DEFAULT_RATE_LIMIT_COOLDOWN_MS: u64 = 30_000;
const DEFAULT_AUTH_FAILURE_COOLDOWN_MS: u64 = 5 * 60_000;
const DEFAULT_PAYMENT_REQUIRED_COOLDOWN_MS: u64 = 60 * 60_000;
const DEFAULT_NETWORK_RETRY_ATTEMPTS: u32 = 1;

#[derive(Debug)]
struct ModelClientState {
    config: Arc<Config>,
    auth_manager: Option<Arc<AuthManager>>,
    model_info: ModelInfo,
    otel_manager: OtelManager,
    provider: ModelProviderInfo,
    conversation_id: ThreadId,
    effort: Option<ReasoningEffortConfig>,
    summary: ReasoningSummaryConfig,
    session_source: SessionSource,
}

#[derive(Debug, Clone)]
pub struct ModelClient {
    state: Arc<ModelClientState>,
}

pub struct ModelClientSession {
    state: Arc<ModelClientState>,
    connection: Option<ApiWebSocketConnection>,
    websocket_last_items: Vec<ResponseItem>,
    /// Turn state for sticky routing.
    ///
    /// This is an `OnceLock` that stores the turn state value received from the server
    /// on turn start via the `x-codex-turn-state` response header. Once set, this value
    /// should be sent back to the server in the `x-codex-turn-state` request header for
    /// all subsequent requests within the same turn to maintain sticky routing.
    ///
    /// This is a contract between the client and server: we receive it at turn start,
    /// keep sending it unchanged between turn requests (e.g., for retries, incremental
    /// appends, or continuation requests), and must not send it between different turns.
    turn_state: Arc<OnceLock<String>>,
}

#[allow(clippy::too_many_arguments)]
impl ModelClient {
    pub fn new(
        config: Arc<Config>,
        auth_manager: Option<Arc<AuthManager>>,
        model_info: ModelInfo,
        otel_manager: OtelManager,
        provider: ModelProviderInfo,
        effort: Option<ReasoningEffortConfig>,
        summary: ReasoningSummaryConfig,
        conversation_id: ThreadId,
        session_source: SessionSource,
    ) -> Self {
        Self {
            state: Arc::new(ModelClientState {
                config,
                auth_manager,
                model_info,
                otel_manager,
                provider,
                conversation_id,
                effort,
                summary,
                session_source,
            }),
        }
    }

    pub fn new_session(&self) -> ModelClientSession {
        ModelClientSession {
            state: Arc::clone(&self.state),
            connection: None,
            websocket_last_items: Vec::new(),
            turn_state: Arc::new(OnceLock::new()),
        }
    }
}

impl ModelClient {
    pub fn get_model_context_window(&self) -> Option<i64> {
        let model_info = &self.state.model_info;
        let effective_context_window_percent = model_info.effective_context_window_percent;
        model_info.context_window.map(|context_window| {
            context_window.saturating_mul(effective_context_window_percent) / 100
        })
    }

    pub fn config(&self) -> Arc<Config> {
        Arc::clone(&self.state.config)
    }

    pub fn provider(&self) -> &ModelProviderInfo {
        &self.state.provider
    }

    pub fn get_provider(&self) -> ModelProviderInfo {
        self.state.provider.clone()
    }

    pub fn get_otel_manager(&self) -> OtelManager {
        self.state.otel_manager.clone()
    }

    pub fn get_session_source(&self) -> SessionSource {
        self.state.session_source.clone()
    }

    /// Returns the currently configured model slug.
    pub fn get_model(&self) -> String {
        self.state.model_info.slug.clone()
    }

    pub fn get_model_info(&self) -> ModelInfo {
        self.state.model_info.clone()
    }

    /// Returns the current reasoning effort setting.
    pub fn get_reasoning_effort(&self) -> Option<ReasoningEffortConfig> {
        self.state.effort
    }

    /// Returns the current reasoning summary setting.
    pub fn get_reasoning_summary(&self) -> ReasoningSummaryConfig {
        self.state.summary
    }

    pub fn get_auth_manager(&self) -> Option<Arc<AuthManager>> {
        self.state.auth_manager.clone()
    }

    /// Compacts the current conversation history using the Compact endpoint.
    ///
    /// This is a unary call (no streaming) that returns a new list of
    /// `ResponseItem`s representing the compacted transcript.
    pub async fn compact_conversation_history(&self, prompt: &Prompt) -> Result<Vec<ResponseItem>> {
        if prompt.input.is_empty() {
            return Ok(Vec::new());
        }
        let auth_manager = self.state.auth_manager.clone();
        let instructions = prompt.base_instructions.text.clone();
        let mut extra_headers = ApiHeaderMap::new();
        if let SessionSource::SubAgent(sub) = &self.state.session_source {
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

        if let Some(manager) = auth_manager.clone()
            && let Some(rotation) = OAuthRotationPlan::try_new(
                manager,
                &self.state.provider,
                &self.state.config,
                DEFAULT_OAUTH_NAMESPACE,
            )
            .await
        {
            let api_provider = self
                .state
                .provider
                .to_api_provider(Some(AuthMode::ChatGPT))?;
            let request_telemetry = self.build_request_telemetry();
            let response = rotation
                .execute(|_, auth| async {
                    let api_auth = auth_provider_from_auth(Some(auth), &self.state.provider)
                        .map_err(|err| ApiError::Stream(err.to_string()))?;
                    let transport = ReqwestTransport::new(build_reqwest_client());
                    let client = ApiCompactClient::new(transport, api_provider.clone(), api_auth)
                        .with_telemetry(Some(request_telemetry.clone()));
                    let payload = ApiCompactionInput {
                        model: &self.state.model_info.slug,
                        input: &prompt.input,
                        instructions: &instructions,
                    };
                    client.compact_input(&payload, extra_headers.clone()).await
                })
                .await
                .map_err(map_api_error)?;
            return Ok(response);
        }

        let auth = match auth_manager.as_ref() {
            Some(manager) => manager.auth().await,
            None => None,
        };
        let api_provider = self
            .state
            .provider
            .to_api_provider(auth.as_ref().map(|a| a.mode))?;
        let api_auth = auth_provider_from_auth(auth.clone(), &self.state.provider)?;
        let transport = ReqwestTransport::new(build_reqwest_client());
        let request_telemetry = self.build_request_telemetry();
        let client = ApiCompactClient::new(transport, api_provider, api_auth)
            .with_telemetry(Some(request_telemetry));

        let payload = ApiCompactionInput {
            model: &self.state.model_info.slug,
            input: &prompt.input,
            instructions: &instructions,
        };

        client
            .compact_input(&payload, extra_headers)
            .await
            .map_err(map_api_error)
    }
}

impl ModelClientSession {
    /// Streams a single model turn using either the Responses or Chat
    /// Completions wire API, depending on the configured provider.
    ///
    /// For Chat providers, the underlying stream is optionally aggregated
    /// based on the `show_raw_agent_reasoning` flag in the config.
    pub async fn stream(&mut self, prompt: &Prompt) -> Result<ResponseStream> {
        match self.state.provider.wire_api {
            WireApi::Responses => self.stream_responses_api(prompt).await,
            WireApi::ResponsesWebsocket => self.stream_responses_websocket(prompt).await,
            WireApi::Chat => {
                let api_stream = self.stream_chat_completions(prompt).await?;

                if self.state.config.show_raw_agent_reasoning {
                    Ok(map_response_stream(
                        api_stream.streaming_mode(),
                        self.state.otel_manager.clone(),
                    ))
                } else {
                    Ok(map_response_stream(
                        api_stream.aggregate(),
                        self.state.otel_manager.clone(),
                    ))
                }
            }
        }
    }

    fn build_responses_request(&self, prompt: &Prompt) -> Result<ApiPrompt> {
        let instructions = prompt.base_instructions.text.clone();
        let tools_json: Vec<Value> = create_tools_json_for_responses_api(&prompt.tools)?;
        Ok(build_api_prompt(prompt, instructions, tools_json))
    }

    fn build_responses_options(
        &self,
        prompt: &Prompt,
        compression: Compression,
    ) -> ApiResponsesOptions {
        build_responses_options_from_state(&self.state, &self.turn_state, prompt, compression)
    }

    fn prepare_websocket_request(
        &self,
        api_prompt: &ApiPrompt,
        options: &ApiResponsesOptions,
    ) -> ResponsesWsRequest {
        prepare_websocket_request_with_last_items(
            &self.state.model_info.slug,
            api_prompt,
            options,
            &self.websocket_last_items,
        )
    }

    async fn websocket_connection(
        &mut self,
        api_provider: codex_api::Provider,
        api_auth: CoreAuthProvider,
        options: &ApiResponsesOptions,
    ) -> std::result::Result<&ApiWebSocketConnection, ApiError> {
        let needs_new = match self.connection.as_ref() {
            Some(conn) => conn.is_closed().await,
            None => true,
        };

        if needs_new {
            let mut headers = options.extra_headers.clone();
            headers.extend(build_conversation_headers(options.conversation_id.clone()));
            let new_conn: ApiWebSocketConnection =
                ApiWebSocketResponsesClient::new(api_provider, api_auth)
                    .connect(headers, options.turn_state.clone())
                    .await?;
            self.connection = Some(new_conn);
        }

        self.connection.as_ref().ok_or(ApiError::Stream(
            "websocket connection is unavailable".to_string(),
        ))
    }

    fn responses_request_compression(&self, auth: Option<&crate::auth::CodexAuth>) -> Compression {
        responses_request_compression_from_state(&self.state, auth)
    }

    /// Streams a turn via the OpenAI Chat Completions API.
    ///
    /// This path is only used when the provider is configured with
    /// `WireApi::Chat`; it does not support `output_schema` today.
    async fn stream_chat_completions(&self, prompt: &Prompt) -> Result<ApiResponseStream> {
        if prompt.output_schema.is_some() {
            return Err(CodexErr::UnsupportedOperation(
                "output_schema is not supported for Chat Completions API".to_string(),
            ));
        }

        let auth_manager = self.state.auth_manager.clone();
        let instructions = prompt.base_instructions.text.clone();
        let tools_json = create_tools_json_for_chat_completions_api(&prompt.tools)?;
        let api_prompt = build_api_prompt(prompt, instructions, tools_json);
        let conversation_id = self.state.conversation_id.to_string();
        let session_source = self.state.session_source.clone();

        if let Some(manager) = auth_manager.clone()
            && let Some(rotation) = OAuthRotationPlan::try_new(
                manager,
                &self.state.provider,
                &self.state.config,
                DEFAULT_OAUTH_NAMESPACE,
            )
            .await
        {
            let api_provider = self
                .state
                .provider
                .to_api_provider(Some(AuthMode::ChatGPT))?;
            let api_stream = rotation
                .execute(|_, auth| async {
                    let api_auth = auth_provider_from_auth(Some(auth), &self.state.provider)
                        .map_err(|err| ApiError::Stream(err.to_string()))?;
                    let transport = ReqwestTransport::new(build_reqwest_client());
                    let (request_telemetry, sse_telemetry) = self.build_streaming_telemetry();
                    let client = ApiChatClient::new(transport, api_provider.clone(), api_auth)
                        .with_telemetry(Some(request_telemetry), Some(sse_telemetry));

                    client
                        .stream_prompt(
                            &self.state.model_info.slug,
                            &api_prompt,
                            Some(conversation_id.clone()),
                            Some(session_source.clone()),
                        )
                        .await
                })
                .await
                .map_err(map_api_error)?;
            return Ok(api_stream);
        }

        let mut auth_recovery = auth_manager
            .as_ref()
            .map(super::auth::AuthManager::unauthorized_recovery);
        loop {
            let auth = match auth_manager.as_ref() {
                Some(manager) => manager.auth().await,
                None => None,
            };
            let api_provider = self
                .state
                .provider
                .to_api_provider(auth.as_ref().map(|a| a.mode))?;
            let api_auth = auth_provider_from_auth(auth.clone(), &self.state.provider)?;
            let transport = ReqwestTransport::new(build_reqwest_client());
            let (request_telemetry, sse_telemetry) = self.build_streaming_telemetry();
            let client = ApiChatClient::new(transport, api_provider, api_auth)
                .with_telemetry(Some(request_telemetry), Some(sse_telemetry));

            let stream_result = client
                .stream_prompt(
                    &self.state.model_info.slug,
                    &api_prompt,
                    Some(conversation_id.clone()),
                    Some(session_source.clone()),
                )
                .await;

            match stream_result {
                Ok(stream) => return Ok(stream),
                Err(ApiError::Transport(TransportError::Http { status, .. }))
                    if status == StatusCode::UNAUTHORIZED =>
                {
                    handle_unauthorized(status, &mut auth_recovery).await?;
                    continue;
                }
                Err(err) => return Err(map_api_error(err)),
            }
        }
    }

    /// Streams a turn via the OpenAI Responses API.
    ///
    /// Handles SSE fixtures, reasoning summaries, verbosity, and the
    /// `text` controls used for output schemas.
    async fn stream_responses_api(&self, prompt: &Prompt) -> Result<ResponseStream> {
        if let Some(path) = &*CODEX_RS_SSE_FIXTURE {
            warn!(path, "Streaming from fixture");
            let stream =
                codex_api::stream_from_fixture(path, self.state.provider.stream_idle_timeout())
                    .map_err(map_api_error)?;
            return Ok(map_response_stream(stream, self.state.otel_manager.clone()));
        }

        let auth_manager = self.state.auth_manager.clone();
        let api_prompt = self.build_responses_request(prompt)?;

        if let Some(manager) = auth_manager.clone()
            && let Some(rotation) = OAuthRotationPlan::try_new(
                manager,
                &self.state.provider,
                &self.state.config,
                DEFAULT_OAUTH_NAMESPACE,
            )
            .await
        {
            let api_provider = self
                .state
                .provider
                .to_api_provider(Some(AuthMode::ChatGPT))?;
            let api_stream = rotation
                .execute(|_, auth| async {
                    let transport = ReqwestTransport::new(build_reqwest_client());
                    let (request_telemetry, sse_telemetry) = self.build_streaming_telemetry();
                    let compression = responses_request_compression_from_state(&self.state, Some(&auth));
                    let api_auth = auth_provider_from_auth(Some(auth), &self.state.provider)
                        .map_err(|err| ApiError::Stream(err.to_string()))?;
                    let options = self.build_responses_options(prompt, compression);

                    let client = ApiResponsesClient::new(transport, api_provider.clone(), api_auth)
                        .with_telemetry(Some(request_telemetry), Some(sse_telemetry));

                    client
                        .stream_prompt(&self.state.model_info.slug, &api_prompt, options)
                        .await
                })
                .await
                .map_err(map_api_error)?;
            return Ok(map_response_stream(api_stream, self.state.otel_manager.clone()));
        }

        let mut auth_recovery = auth_manager
            .as_ref()
            .map(super::auth::AuthManager::unauthorized_recovery);
        loop {
            let auth = match auth_manager.as_ref() {
                Some(manager) => manager.auth().await,
                None => None,
            };
            let api_provider = self
                .state
                .provider
                .to_api_provider(auth.as_ref().map(|a| a.mode))?;
            let api_auth = auth_provider_from_auth(auth.clone(), &self.state.provider)?;
            let transport = ReqwestTransport::new(build_reqwest_client());
            let (request_telemetry, sse_telemetry) = self.build_streaming_telemetry();
            let compression = self.responses_request_compression(auth.as_ref());

            let client = ApiResponsesClient::new(transport, api_provider, api_auth)
                .with_telemetry(Some(request_telemetry), Some(sse_telemetry));

            let options = self.build_responses_options(prompt, compression);

            let stream_result = client
                .stream_prompt(&self.state.model_info.slug, &api_prompt, options)
                .await;

            match stream_result {
                Ok(stream) => {
                    return Ok(map_response_stream(stream, self.state.otel_manager.clone()));
                }
                Err(ApiError::Transport(TransportError::Http { status, .. }))
                    if status == StatusCode::UNAUTHORIZED =>
                {
                    handle_unauthorized(status, &mut auth_recovery).await?;
                    continue;
                }
                Err(err) => return Err(map_api_error(err)),
            }
        }
    }

    /// Streams a turn via the Responses API over WebSocket transport.
    async fn stream_responses_websocket(&mut self, prompt: &Prompt) -> Result<ResponseStream> {
        let auth_manager = self.state.auth_manager.clone();
        let api_prompt = self.build_responses_request(prompt)?;

        if let Some(manager) = auth_manager.clone()
            && let Some(rotation) = OAuthRotationPlan::try_new(
                manager,
                &self.state.provider,
                &self.state.config,
                DEFAULT_OAUTH_NAMESPACE,
            )
            .await
        {
            let api_provider = self
                .state
                .provider
                .to_api_provider(Some(AuthMode::ChatGPT))?;
            let state = Arc::clone(&self.state);
            let turn_state = Arc::clone(&self.turn_state);
            let api_prompt_for_request = api_prompt.clone();
            let last_items = self.websocket_last_items.clone();
            let connection_cell =
                Arc::new(tokio::sync::Mutex::new(self.connection.take()));
            let stream_result = rotation
                .execute({
                    let connection_cell = Arc::clone(&connection_cell);
                    let state = Arc::clone(&state);
                    let turn_state = Arc::clone(&turn_state);
                    let api_prompt = api_prompt_for_request.clone();
                    let last_items = last_items.clone();
                    let api_provider = api_provider.clone();
                    move |_, auth| {
                        let connection_cell = Arc::clone(&connection_cell);
                        let state = Arc::clone(&state);
                        let turn_state = Arc::clone(&turn_state);
                        let api_prompt = api_prompt.clone();
                        let last_items = last_items.clone();
                        let api_provider = api_provider.clone();
                        async move {
                            let compression =
                                responses_request_compression_from_state(&state, Some(&auth));
                            let api_auth =
                                auth_provider_from_auth(Some(auth), &state.provider)
                                    .map_err(|err| ApiError::Stream(err.to_string()))?;
                            let options = build_responses_options_from_state(
                                &state,
                                &turn_state,
                                prompt,
                                compression,
                            );
                            let request = prepare_websocket_request_with_last_items(
                                &state.model_info.slug,
                                &api_prompt,
                                &options,
                                &last_items,
                            );

                            let mut headers = options.extra_headers.clone();
                            headers.extend(build_conversation_headers(
                                options.conversation_id.clone(),
                            ));
                            let new_conn = ApiWebSocketResponsesClient::new(
                                api_provider.clone(),
                                api_auth,
                            )
                            .connect(headers, options.turn_state.clone())
                            .await?;
                            let stream = new_conn.stream_request(request).await?;
                            {
                                let mut guard = connection_cell.lock().await;
                                *guard = Some(new_conn);
                            }
                            Ok(stream)
                        }
                    }
                })
                .await
                .map_err(map_api_error)?;
            self.websocket_last_items = api_prompt.input.clone();
            self.connection = connection_cell.lock().await.take();
            return Ok(map_response_stream(
                stream_result,
                self.state.otel_manager.clone(),
            ));
        }

        let mut auth_recovery = auth_manager
            .as_ref()
            .map(super::auth::AuthManager::unauthorized_recovery);
        loop {
            let auth = match auth_manager.as_ref() {
                Some(manager) => manager.auth().await,
                None => None,
            };
            let api_provider = self
                .state
                .provider
                .to_api_provider(auth.as_ref().map(|a| a.mode))?;
            let api_auth = auth_provider_from_auth(auth.clone(), &self.state.provider)?;
            let compression = self.responses_request_compression(auth.as_ref());

            let options = self.build_responses_options(prompt, compression);
            let request = self.prepare_websocket_request(&api_prompt, &options);

            let connection = match self
                .websocket_connection(api_provider.clone(), api_auth.clone(), &options)
                .await
            {
                Ok(connection) => connection,
                Err(ApiError::Transport(TransportError::Http { status, .. }))
                    if status == StatusCode::UNAUTHORIZED =>
                {
                    handle_unauthorized(status, &mut auth_recovery).await?;
                    continue;
                }
                Err(err) => return Err(map_api_error(err)),
            };

            let stream_result = connection
                .stream_request(request)
                .await
                .map_err(map_api_error)?;
            self.websocket_last_items = api_prompt.input.clone();

            return Ok(map_response_stream(
                stream_result,
                self.state.otel_manager.clone(),
            ));
        }
    }

    /// Builds request and SSE telemetry for streaming API calls (Chat/Responses).
    fn build_streaming_telemetry(&self) -> (Arc<dyn RequestTelemetry>, Arc<dyn SseTelemetry>) {
        let telemetry = Arc::new(ApiTelemetry::new(self.state.otel_manager.clone()));
        let request_telemetry: Arc<dyn RequestTelemetry> = telemetry.clone();
        let sse_telemetry: Arc<dyn SseTelemetry> = telemetry;
        (request_telemetry, sse_telemetry)
    }
}

impl ModelClient {
    /// Builds request telemetry for unary API calls (e.g., Compact endpoint).
    fn build_request_telemetry(&self) -> Arc<dyn RequestTelemetry> {
        let telemetry = Arc::new(ApiTelemetry::new(self.state.otel_manager.clone()));
        let request_telemetry: Arc<dyn RequestTelemetry> = telemetry;
        request_telemetry
    }
}

#[derive(Clone, Debug)]
struct OAuthRotationConfigResolved {
    max_attempts: usize,
    rate_limit_cooldown: Duration,
    auth_failure_cooldown: Duration,
    payment_required_cooldown: Duration,
    network_retry_attempts: u32,
}

struct OAuthRotationPlan {
    auth_manager: Arc<AuthManager>,
    namespace: String,
    candidates: Vec<String>,
    record_by_id: HashMap<String, crate::auth::OAuthPoolRecord>,
    config: OAuthRotationConfigResolved,
}

impl OAuthRotationPlan {
    async fn try_new(
        auth_manager: Arc<AuthManager>,
        provider: &ModelProviderInfo,
        config: &Config,
        namespace: &str,
    ) -> Option<Self> {
        if !provider.is_openai() {
            return None;
        }
        let auth = auth_manager.auth().await?;
        if auth.mode != AuthMode::ChatGPT {
            return None;
        }

        let snapshot = auth_manager.oauth_snapshot(namespace).ok()?;
        if snapshot.records.len() <= 1 {
            return None;
        }

        let record_by_id: HashMap<_, _> = snapshot
            .records
            .into_iter()
            .map(|record| (record.id.clone(), record))
            .collect();
        let candidates: Vec<String> = snapshot
            .ordered_ids
            .into_iter()
            .filter(|id| record_by_id.contains_key(id))
            .collect();
        if candidates.is_empty() {
            return None;
        }

        let resolved = resolve_oauth_rotation_config(config, candidates.len());

        Some(Self {
            auth_manager,
            namespace: namespace.to_string(),
            candidates,
            record_by_id,
            config: resolved,
        })
    }

    fn refreshed_snapshot(
        &self,
    ) -> (
        Vec<String>,
        HashMap<String, crate::auth::OAuthPoolRecord>,
    ) {
        if let Ok(snapshot) = self.auth_manager.oauth_snapshot(&self.namespace) {
            let record_by_id: HashMap<_, _> = snapshot
                .records
                .into_iter()
                .map(|record| (record.id.clone(), record))
                .collect();
            if !record_by_id.is_empty() {
                let candidates: Vec<String> = snapshot
                    .ordered_ids
                    .into_iter()
                    .filter(|id| self.candidates.contains(id))
                    .collect();
                if !candidates.is_empty() {
                    return (candidates, record_by_id);
                }
                return (self.candidates.clone(), record_by_id);
            }
        }

        (self.candidates.clone(), self.record_by_id.clone())
    }

    async fn execute<T, F, Fut>(&self, mut request_fn: F) -> std::result::Result<T, ApiError>
    where
        F: FnMut(&str, CodexAuth) -> Fut,
        Fut: std::future::Future<Output = std::result::Result<T, ApiError>>,
    {
        let mut attempted: HashSet<String> = HashSet::new();
        let mut refreshed: HashSet<String> = HashSet::new();
        let mut last_error: Option<ApiError> = None;

        let max_attempts = self
            .config
            .max_attempts
            .max(1)
            .min(self.candidates.len().max(1));

        for attempt in 0..max_attempts {
            let (candidates, record_by_id) = self.refreshed_snapshot();
            let Some(record_id) =
                pick_next_candidate(&candidates, &record_by_id, &attempted)
            else {
                break;
            };
            attempted.insert(record_id.clone());

            let mut network_attempt = 0u32;
            loop {
                let Some(auth) = self.auth_manager.auth_for_record(&record_id) else {
                    break;
                };

                let result = request_fn(&record_id, auth).await;
                match result {
                    Ok(value) => {
                        let _ = self
                            .auth_manager
                            .oauth_record_outcome(&record_id, 200, true, None);
                        return Ok(value);
                    }
                    Err(ApiError::Transport(TransportError::Http {
                        status,
                        url,
                        headers,
                        body,
                    })) => {
                        let status_code = status.as_u16();
                        let headers_ref = headers.as_ref();

                        if status == StatusCode::BAD_REQUEST {
                            let _ = self
                                .auth_manager
                                .oauth_record_outcome(&record_id, status_code, false, None);
                            return Err(ApiError::Transport(TransportError::Http {
                                status,
                                url,
                                headers,
                                body,
                            }));
                        }

                        if status == StatusCode::PAYMENT_REQUIRED {
                            let exhausted_until = cooldown_until_from_duration(
                                self.config.payment_required_cooldown,
                            );
                            let _ = self.auth_manager.oauth_record_exhausted(
                                &record_id,
                                status_code,
                                Some(exhausted_until),
                            );
                            let _ =
                                self.auth_manager
                                    .oauth_move_to_back(&self.namespace, &record_id);
                            let err = ApiError::Transport(TransportError::Http {
                                status,
                                url,
                                headers,
                                body,
                            });
                            if attempt + 1 >= max_attempts {
                                return Err(err);
                            }
                            last_error = Some(err);
                            break;
                        }

                        if status == StatusCode::TOO_MANY_REQUESTS {
                            let cooldown = retry_after_duration(headers_ref)
                                .unwrap_or(self.config.rate_limit_cooldown);
                            let cooldown_until = cooldown_until_from_duration(cooldown);
                            let _ = self.auth_manager.oauth_record_outcome(
                                &record_id,
                                status_code,
                                false,
                                Some(cooldown_until),
                            );
                            let _ =
                                self.auth_manager
                                    .oauth_move_to_back(&self.namespace, &record_id);
                            let err = ApiError::Transport(TransportError::Http {
                                status,
                                url,
                                headers,
                                body,
                            });
                            if attempt + 1 >= max_attempts {
                                return Err(err);
                            }
                            last_error = Some(err);
                            break;
                        }

                        if is_auth_failure_status(status) {
                            if !refreshed.contains(&record_id) {
                                refreshed.insert(record_id.clone());
                                match self.auth_manager.refresh_record(&record_id).await {
                                    Ok(()) => {
                                        let Some(auth) = self.auth_manager.auth_for_record(&record_id) else {
                                            break;
                                        };
                                        let retry = request_fn(&record_id, auth).await;
                                        match retry {
                                            Ok(value) => {
                                                let _ = self.auth_manager.oauth_record_outcome(
                                                    &record_id,
                                                    200,
                                                    true,
                                                    None,
                                                );
                                                return Ok(value);
                                            }
                                            Err(ApiError::Transport(TransportError::Http {
                                                status,
                                                url,
                                                headers,
                                                body,
                                            })) => {
                                                if status == StatusCode::BAD_REQUEST {
                                                    let _ = self.auth_manager.oauth_record_outcome(
                                                        &record_id,
                                                        status.as_u16(),
                                                        false,
                                                        None,
                                                    );
                                                    return Err(ApiError::Transport(
                                                        TransportError::Http {
                                                            status,
                                                            url,
                                                            headers,
                                                            body,
                                                        },
                                                    ));
                                                }

                                                if status == StatusCode::PAYMENT_REQUIRED {
                                                    let exhausted_until = cooldown_until_from_duration(
                                                        self.config.payment_required_cooldown,
                                                    );
                                                    let _ = self.auth_manager.oauth_record_exhausted(
                                                        &record_id,
                                                        status.as_u16(),
                                                        Some(exhausted_until),
                                                    );
                                                    let _ = self.auth_manager.oauth_move_to_back(
                                                        &self.namespace,
                                                        &record_id,
                                                    );
                                                    let err = ApiError::Transport(
                                                        TransportError::Http {
                                                            status,
                                                            url,
                                                            headers,
                                                            body,
                                                        },
                                                    );
                                                    if attempt + 1 >= max_attempts {
                                                        return Err(err);
                                                    }
                                                    last_error = Some(err);
                                                    break;
                                                }

                                                if status == StatusCode::TOO_MANY_REQUESTS {
                                                    let cooldown = retry_after_duration(headers.as_ref())
                                                        .unwrap_or(self.config.rate_limit_cooldown);
                                                    let cooldown_until =
                                                        cooldown_until_from_duration(cooldown);
                                                    let _ = self.auth_manager.oauth_record_outcome(
                                                        &record_id,
                                                        status.as_u16(),
                                                        false,
                                                        Some(cooldown_until),
                                                    );
                                                    let _ = self.auth_manager.oauth_move_to_back(
                                                        &self.namespace,
                                                        &record_id,
                                                    );
                                                    let err = ApiError::Transport(
                                                        TransportError::Http {
                                                            status,
                                                            url,
                                                            headers,
                                                            body,
                                                        },
                                                    );
                                                    if attempt + 1 >= max_attempts {
                                                        return Err(err);
                                                    }
                                                    last_error = Some(err);
                                                    break;
                                                }

                                                let cooldown_until = cooldown_until_from_duration(
                                                    self.config.auth_failure_cooldown,
                                                );
                                                let _ = self.auth_manager.oauth_record_outcome(
                                                    &record_id,
                                                    status.as_u16(),
                                                    false,
                                                    Some(cooldown_until),
                                                );
                                                let _ = self.auth_manager.oauth_move_to_back(
                                                    &self.namespace,
                                                    &record_id,
                                                );
                                                let err = ApiError::Transport(TransportError::Http {
                                                    status,
                                                    url,
                                                    headers,
                                                    body,
                                                });
                                                if attempt + 1 >= max_attempts {
                                                    return Err(err);
                                                }
                                                last_error = Some(err);
                                                break;
                                            }
                                            Err(ApiError::Transport(TransportError::Timeout))
                                            | Err(ApiError::Transport(TransportError::Network(_)))
                                            | Err(ApiError::Transport(TransportError::Build(_))) => {
                                                return retry;
                                            }
                                            Err(err) => return Err(err),
                                        }
                                    }
                                    Err(err) => {
                                        let requires_relogin = matches!(
                                            err.failed_reason(),
                                            Some(RefreshTokenFailedReason::Expired)
                                                | Some(RefreshTokenFailedReason::Exhausted)
                                                | Some(RefreshTokenFailedReason::Revoked)
                                                | Some(RefreshTokenFailedReason::Other)
                                        );
                                        if requires_relogin {
                                            let _ = self
                                                .auth_manager
                                                .oauth_record_requires_relogin(
                                                    &record_id,
                                                    status_code,
                                                );
                                        } else {
                                            let cooldown_until = cooldown_until_from_duration(
                                                self.config.auth_failure_cooldown,
                                            );
                                            let _ = self.auth_manager.oauth_record_outcome(
                                                &record_id,
                                                status_code,
                                                false,
                                                Some(cooldown_until),
                                            );
                                        }
                                        let _ = self.auth_manager.oauth_move_to_back(
                                            &self.namespace,
                                            &record_id,
                                        );
                                        let err = ApiError::Transport(TransportError::Http {
                                            status,
                                            url,
                                            headers,
                                            body,
                                        });
                                        if attempt + 1 >= max_attempts {
                                            return Err(err);
                                        }
                                        last_error = Some(err);
                                        break;
                                    }
                                }
                            } else {
                                let cooldown_until =
                                    cooldown_until_from_duration(self.config.auth_failure_cooldown);
                                let _ = self.auth_manager.oauth_record_outcome(
                                    &record_id,
                                    status_code,
                                    false,
                                    Some(cooldown_until),
                                );
                                let _ = self
                                    .auth_manager
                                    .oauth_move_to_back(&self.namespace, &record_id);
                                let err = ApiError::Transport(TransportError::Http {
                                    status,
                                    url,
                                    headers,
                                    body,
                                });
                                if attempt + 1 >= max_attempts {
                                    return Err(err);
                                }
                                last_error = Some(err);
                                break;
                            }
                        }

                        let _ = self
                            .auth_manager
                            .oauth_record_outcome(&record_id, status_code, false, None);
                        let _ = self
                            .auth_manager
                            .oauth_move_to_back(&self.namespace, &record_id);
                        let err = ApiError::Transport(TransportError::Http {
                            status,
                            url,
                            headers,
                            body,
                        });
                        if attempt + 1 >= max_attempts {
                            return Err(err);
                        }
                        last_error = Some(err);
                        break;
                    }
                    Err(ApiError::Transport(TransportError::Timeout)) => {
                        let err = ApiError::Transport(TransportError::Timeout);
                        let _ =
                            self.auth_manager
                                .oauth_record_outcome(&record_id, 0, false, None);
                        if network_attempt < self.config.network_retry_attempts {
                            network_attempt += 1;
                            continue;
                        }
                        last_error = Some(err);
                        break;
                    }
                    Err(ApiError::Transport(TransportError::Network(msg))) => {
                        let err = ApiError::Transport(TransportError::Network(msg));
                        let _ =
                            self.auth_manager
                                .oauth_record_outcome(&record_id, 0, false, None);
                        if network_attempt < self.config.network_retry_attempts {
                            network_attempt += 1;
                            continue;
                        }
                        last_error = Some(err);
                        break;
                    }
                    Err(ApiError::Transport(TransportError::Build(msg))) => {
                        let err = ApiError::Transport(TransportError::Build(msg));
                        let _ =
                            self.auth_manager
                                .oauth_record_outcome(&record_id, 0, false, None);
                        if network_attempt < self.config.network_retry_attempts {
                            network_attempt += 1;
                            continue;
                        }
                        last_error = Some(err);
                        break;
                    }
                    Err(err) => return Err(err),
                }
            }
        }

        match last_error {
            Some(err) => Err(err),
            None => Err(ApiError::Stream("OAuth rotation exhausted".to_string())),
        }
    }
}

fn resolve_oauth_rotation_config(config: &Config, candidate_len: usize) -> OAuthRotationConfigResolved {
    let settings = &config.oauth_rotation;
    let max_attempts = settings
        .max_attempts
        .unwrap_or(candidate_len as u32)
        .max(1) as usize;
    let max_attempts = max_attempts.min(candidate_len.max(1));
    let rate_limit_cooldown = Duration::from_millis(
        settings
            .rate_limit_cooldown_ms
            .unwrap_or(DEFAULT_RATE_LIMIT_COOLDOWN_MS),
    );
    let auth_failure_cooldown = Duration::from_millis(
        settings
            .auth_failure_cooldown_ms
            .unwrap_or(DEFAULT_AUTH_FAILURE_COOLDOWN_MS),
    );
    let payment_required_cooldown = Duration::from_millis(
        settings
            .payment_required_cooldown_ms
            .unwrap_or(DEFAULT_PAYMENT_REQUIRED_COOLDOWN_MS),
    );
    let network_retry_attempts = settings
        .network_retry_attempts
        .unwrap_or(DEFAULT_NETWORK_RETRY_ATTEMPTS);

    OAuthRotationConfigResolved {
        max_attempts,
        rate_limit_cooldown,
        auth_failure_cooldown,
        payment_required_cooldown,
        network_retry_attempts,
    }
}

fn pick_next_candidate(
    candidates: &[String],
    record_by_id: &HashMap<String, crate::auth::OAuthPoolRecord>,
    attempted: &HashSet<String>,
) -> Option<String> {
    let now = Utc::now();
    candidates
        .iter()
        .find(|id| {
            if attempted.contains(*id) {
                return false;
            }
            let Some(record) = record_by_id.get(*id) else {
                return false;
            };
            if record.health.requires_relogin {
                return false;
            }
            if record
                .health
                .exhausted_until
                .is_some_and(|until| until > now)
            {
                return false;
            }
            let cooldown = record.health.cooldown_until;
            cooldown.map(|until| until <= now).unwrap_or(true)
        })
        .or_else(|| {
            candidates.iter().find(|id| {
                if attempted.contains(*id) {
                    return false;
                }
                let Some(record) = record_by_id.get(*id) else {
                    return false;
                };
                if record.health.requires_relogin {
                    return false;
                }
                if record
                    .health
                    .exhausted_until
                    .is_some_and(|until| until > now)
                {
                    return false;
                }
                true
            })
        })
        .cloned()
}

fn retry_after_duration(headers: Option<&ApiHeaderMap>) -> Option<Duration> {
    let headers = headers?;
    let value = headers.get("retry-after")?;
    let value = value.to_str().ok()?.trim();
    if value.is_empty() {
        return None;
    }

    if let Ok(seconds) = value.parse::<u64>() {
        return Some(Duration::from_secs(seconds));
    }

    match httpdate::parse_http_date(value) {
        Ok(time) => Some(time.duration_since(SystemTime::now()).unwrap_or_default()),
        Err(_) => None,
    }
}

fn cooldown_until_from_duration(duration: Duration) -> DateTime<Utc> {
    let chrono_duration = chrono::Duration::from_std(duration)
        .unwrap_or_else(|_| chrono::Duration::zero());
    Utc::now() + chrono_duration
}

fn is_auth_failure_status(status: StatusCode) -> bool {
    status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN
}

fn responses_request_compression_from_state(
    state: &ModelClientState,
    auth: Option<&CodexAuth>,
) -> Compression {
    if state
        .config
        .features
        .enabled(Feature::EnableRequestCompression)
        && auth.is_some_and(|auth| auth.mode == AuthMode::ChatGPT)
        && state.provider.is_openai()
    {
        Compression::Zstd
    } else {
        Compression::None
    }
}

fn build_responses_options_from_state(
    state: &ModelClientState,
    turn_state: &Arc<OnceLock<String>>,
    prompt: &Prompt,
    compression: Compression,
) -> ApiResponsesOptions {
    let model_info = &state.model_info;

    let default_reasoning_effort = model_info.default_reasoning_level;
    let reasoning = if model_info.supports_reasoning_summaries {
        Some(Reasoning {
            effort: state.effort.or(default_reasoning_effort),
            summary: if state.summary == ReasoningSummaryConfig::None {
                None
            } else {
                Some(state.summary)
            },
        })
    } else {
        None
    };

    let include = if reasoning.is_some() {
        vec!["reasoning.encrypted_content".to_string()]
    } else {
        Vec::new()
    };

    let verbosity = if model_info.support_verbosity {
        state.config.model_verbosity.or(model_info.default_verbosity)
    } else {
        if state.config.model_verbosity.is_some() {
            warn!(
                "model_verbosity is set but ignored as the model does not support verbosity: {}",
                model_info.slug
            );
        }
        None
    };

    let text = create_text_param_for_request(verbosity, &prompt.output_schema);
    let conversation_id = state.conversation_id.to_string();

    ApiResponsesOptions {
        reasoning,
        include,
        prompt_cache_key: Some(conversation_id.clone()),
        text,
        store_override: None,
        conversation_id: Some(conversation_id),
        session_source: Some(state.session_source.clone()),
        extra_headers: build_responses_headers(&state.config, Some(turn_state)),
        compression,
        turn_state: Some(Arc::clone(turn_state)),
    }
}

fn get_incremental_items_from(
    last_items: &[ResponseItem],
    input_items: &[ResponseItem],
) -> Option<Vec<ResponseItem>> {
    // Checks whether the current request input is an incremental append to the previous request.
    // If items in the new request contain all the items from the previous request we build
    // a response.append request otherwise we start with a fresh response.create request.
    let previous_len = last_items.len();
    let can_append = previous_len > 0
        && input_items.starts_with(last_items)
        && previous_len < input_items.len();
    if can_append {
        Some(input_items[previous_len..].to_vec())
    } else {
        None
    }
}

fn prepare_websocket_request_with_last_items(
    model_slug: &str,
    api_prompt: &ApiPrompt,
    options: &ApiResponsesOptions,
    last_items: &[ResponseItem],
) -> ResponsesWsRequest {
    if let Some(append_items) = get_incremental_items_from(last_items, &api_prompt.input) {
        return ResponsesWsRequest::ResponseAppend(ResponseAppendWsRequest { input: append_items });
    }

    let ApiResponsesOptions {
        reasoning,
        include,
        prompt_cache_key,
        text,
        store_override,
        ..
    } = options;

    let store = store_override.unwrap_or(false);
    let payload = ResponseCreateWsRequest {
        model: model_slug.to_string(),
        instructions: api_prompt.instructions.clone(),
        input: api_prompt.input.clone(),
        tools: api_prompt.tools.clone(),
        tool_choice: "auto".to_string(),
        parallel_tool_calls: api_prompt.parallel_tool_calls,
        reasoning: reasoning.clone(),
        store,
        stream: true,
        include: include.clone(),
        prompt_cache_key: prompt_cache_key.clone(),
        text: text.clone(),
    };

    ResponsesWsRequest::ResponseCreate(payload)
}

/// Adapts the core `Prompt` type into the `codex-api` payload shape.
fn build_api_prompt(prompt: &Prompt, instructions: String, tools_json: Vec<Value>) -> ApiPrompt {
    ApiPrompt {
        instructions,
        input: prompt.get_formatted_input(),
        tools: tools_json,
        parallel_tool_calls: prompt.parallel_tool_calls,
        output_schema: prompt.output_schema.clone(),
    }
}

fn beta_feature_headers(config: &Config) -> ApiHeaderMap {
    let enabled = FEATURES
        .iter()
        .filter_map(|spec| {
            if spec.stage.beta_menu_description().is_some() && config.features.enabled(spec.id) {
                Some(spec.key)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    let value = enabled.join(",");
    let mut headers = ApiHeaderMap::new();
    if !value.is_empty()
        && let Ok(header_value) = HeaderValue::from_str(value.as_str())
    {
        headers.insert("x-codex-beta-features", header_value);
    }
    headers
}

fn build_responses_headers(
    config: &Config,
    turn_state: Option<&Arc<OnceLock<String>>>,
) -> ApiHeaderMap {
    let mut headers = beta_feature_headers(config);
    headers.insert(
        WEB_SEARCH_ELIGIBLE_HEADER,
        HeaderValue::from_static(
            if matches!(config.web_search_mode, Some(WebSearchMode::Disabled)) {
                "false"
            } else {
                "true"
            },
        ),
    );
    if let Some(turn_state) = turn_state
        && let Some(state) = turn_state.get()
        && let Ok(header_value) = HeaderValue::from_str(state)
    {
        headers.insert(X_CODEX_TURN_STATE_HEADER, header_value);
    }
    headers
}

fn map_response_stream<S>(api_stream: S, otel_manager: OtelManager) -> ResponseStream
where
    S: futures::Stream<Item = std::result::Result<ResponseEvent, ApiError>>
        + Unpin
        + Send
        + 'static,
{
    let (tx_event, rx_event) = mpsc::channel::<Result<ResponseEvent>>(1600);

    tokio::spawn(async move {
        let mut logged_error = false;
        let mut api_stream = api_stream;
        while let Some(event) = api_stream.next().await {
            match event {
                Ok(ResponseEvent::Completed {
                    response_id,
                    token_usage,
                }) => {
                    if let Some(usage) = &token_usage {
                        otel_manager.sse_event_completed(
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
                        otel_manager.see_event_completed_failed(&mapped);
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

/// Handles a 401 response by optionally refreshing ChatGPT tokens once.
///
/// When refresh succeeds, the caller should retry the API call; otherwise
/// the mapped `CodexErr` is returned to the caller.
async fn handle_unauthorized(
    status: StatusCode,
    auth_recovery: &mut Option<UnauthorizedRecovery>,
) -> Result<()> {
    if let Some(recovery) = auth_recovery
        && recovery.has_next()
    {
        return match recovery.next().await {
            Ok(_) => Ok(()),
            Err(RefreshTokenError::Permanent(failed)) => Err(CodexErr::RefreshTokenFailed(failed)),
            Err(RefreshTokenError::Transient(other)) => Err(CodexErr::Io(other)),
        };
    }

    Err(map_unauthorized_status(status))
}

fn map_unauthorized_status(status: StatusCode) -> CodexErr {
    map_api_error(ApiError::Transport(TransportError::Http {
        status,
        url: None,
        headers: None,
        body: None,
    }))
}

struct ApiTelemetry {
    otel_manager: OtelManager,
}

impl ApiTelemetry {
    fn new(otel_manager: OtelManager) -> Self {
        Self { otel_manager }
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
        self.otel_manager.record_api_request(
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
        self.otel_manager.log_sse_event(result, duration);
    }
}
