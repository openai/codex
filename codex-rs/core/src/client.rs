use std::fmt;
use std::sync::Arc;

use async_trait::async_trait;
use codex_api_client::AggregateStreamExt;
use codex_api_client::ApiClient;
use codex_api_client::AuthContext;
use codex_api_client::AuthProvider;
use codex_api_client::ChatAggregationMode;
use codex_api_client::ChatCompletionsApiClient;
use codex_api_client::ChatCompletionsApiClientConfig;
use codex_api_client::ResponsesApiClient;
use codex_api_client::ResponsesApiClientConfig;
use codex_api_client::Result as ApiClientResult;
use codex_api_client::stream_from_fixture;
use codex_otel::otel_event_manager::OtelEventManager;
use codex_protocol::ConversationId;
use codex_protocol::config_types::ReasoningEffort as ReasoningEffortConfig;
use codex_protocol::config_types::ReasoningSummary as ReasoningSummaryConfig;
use codex_protocol::protocol::SessionSource;
use futures::StreamExt;
use futures::stream::BoxStream;
use tokio::sync::OnceCell;
use tokio::sync::mpsc;
use tracing::warn;

use crate::AuthManager;
use crate::client_common::Prompt;
use crate::client_common::ResponseEvent;
use crate::client_common::ResponseStream;
use crate::client_common::create_reasoning_param_for_request;
use crate::client_common::create_text_param_for_request;
use crate::config::Config;
use crate::default_client::create_client;
use crate::error::CodexErr;
use crate::error::ConnectionFailedError;
use crate::error::EnvVarError;
use crate::error::ResponseStreamFailed;
use crate::error::Result;
use crate::error::RetryLimitReachedError;
use crate::error::UnexpectedResponseError;
use crate::features::Feature;
use crate::flags::CODEX_RS_SSE_FIXTURE;
use crate::model_family::ModelFamily;
use crate::openai_model_info::get_model_info;
use codex_api_client::ModelProviderInfo;
use codex_api_client::WireApi;

#[derive(Clone)]
pub struct ModelClient {
    config: Arc<Config>,
    auth_manager: Option<Arc<AuthManager>>,
    otel_event_manager: OtelEventManager,
    http_client: reqwest::Client,
    provider: ModelProviderInfo,
    backend: Arc<OnceCell<ModelBackend>>,
    conversation_id: ConversationId,
    effort: Option<ReasoningEffortConfig>,
    summary: ReasoningSummaryConfig,
    session_source: SessionSource,
}

impl fmt::Debug for ModelClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ModelClient")
            .field("provider", &self.provider.name)
            .field("model", &self.config.model)
            .field("conversation_id", &self.conversation_id)
            .field("backend_initialized", &self.backend.get().is_some())
            .finish()
    }
}

#[derive(Debug, Clone)]
pub struct StreamPayload {
    pub prompt: Prompt,
}

type ApiClientStream = BoxStream<'static, ApiClientResult<ResponseEvent>>;

enum ModelBackend {
    Responses(ResponsesBackend),
    Chat(ChatBackend),
}

impl ModelBackend {
    async fn stream(&self, prompt: Prompt) -> ApiClientResult<ApiClientStream> {
        match self {
            ModelBackend::Responses(backend) => backend.stream(prompt).await,
            ModelBackend::Chat(backend) => backend.stream(prompt).await,
        }
    }
}

struct ResponsesBackend {
    client: ResponsesApiClient,
}

impl ResponsesBackend {
    async fn stream(&self, prompt: Prompt) -> ApiClientResult<ApiClientStream> {
        self.client
            .stream(prompt)
            .await
            .map(futures::StreamExt::boxed)
    }
}

struct ChatBackend {
    client: ChatCompletionsApiClient,
    show_reasoning: bool,
}

impl ChatBackend {
    async fn stream(&self, prompt: Prompt) -> ApiClientResult<ApiClientStream> {
        let stream = self.client.stream(prompt).await?;
        let stream = if self.show_reasoning {
            stream.streaming_mode().boxed()
        } else {
            stream.aggregate().boxed()
        };
        Ok(stream)
    }
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
        let http_client = create_client().clone_inner();
        let backend = Arc::new(OnceCell::new());

        Self {
            config,
            auth_manager,
            otel_event_manager,
            http_client,
            provider,
            backend,
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

    pub fn supports_responses_api_chaining(&self) -> bool {
        self.provider.wire_api == WireApi::Responses
            && self.config.features.enabled(Feature::ResponsesApiChaining)
    }

    pub async fn stream(&self, payload: &StreamPayload) -> Result<ResponseStream> {
        let mut prompt = payload.prompt.clone();
        self.populate_prompt(&mut prompt);
        if self.provider.wire_api == WireApi::Responses
            && let Some(path) = &*CODEX_RS_SSE_FIXTURE
        {
            warn!(path, "Streaming from fixture");
            let stream =
                stream_from_fixture(path, self.provider.clone(), self.otel_event_manager.clone())
                    .await
                    .map_err(map_api_error)?
                    .boxed();
            return Ok(wrap_stream(stream));
        }

        let backend = self
            .backend
            .get_or_try_init(|| async { self.build_backend().await })
            .await
            .map_err(map_api_error)?;

        let api_stream = backend.stream(prompt).await.map_err(map_api_error)?;

        Ok(wrap_stream(api_stream))
    }

    fn populate_prompt(&self, prompt: &mut Prompt) {
        if prompt.prompt_cache_key.is_none() {
            prompt.prompt_cache_key = Some(self.conversation_id.to_string());
        }

        prompt.session_source = Some(self.session_source.clone());

        prompt.reasoning = create_reasoning_param_for_request(
            &self.config.model_family,
            self.effort,
            self.summary,
        );

        let verbosity = if self.config.model_family.support_verbosity {
            self.config.model_verbosity
        } else {
            if self.config.model_verbosity.is_some() {
                warn!(
                    "model_verbosity is set but ignored as the model does not support verbosity: {}",
                    self.config.model_family.family
                );
            }
            None
        };

        prompt.text_controls = create_text_param_for_request(verbosity, &prompt.output_schema);
    }

    async fn build_backend(&self) -> ApiClientResult<ModelBackend> {
        match self.provider.wire_api {
            WireApi::Responses => self.build_responses_backend().await,
            WireApi::Chat => self.build_chat_backend().await,
        }
    }

    async fn build_responses_backend(&self) -> ApiClientResult<ModelBackend> {
        let auth_provider = self.auth_manager.as_ref().map(|manager| {
            Arc::new(AuthManagerProvider::new(Arc::clone(manager))) as Arc<dyn AuthProvider>
        });

        let config = ResponsesApiClientConfig {
            http_client: self.http_client.clone(),
            provider: self.provider.clone(),
            model: self.config.model.clone(),
            conversation_id: self.conversation_id,
            auth_provider,
            otel_event_manager: self.otel_event_manager.clone(),
        };

        let client = ResponsesApiClient::new(config).await?;
        Ok(ModelBackend::Responses(ResponsesBackend { client }))
    }

    async fn build_chat_backend(&self) -> ApiClientResult<ModelBackend> {
        let show_reasoning = self.config.show_raw_agent_reasoning;
        let config = ChatCompletionsApiClientConfig {
            http_client: self.http_client.clone(),
            provider: self.provider.clone(),
            model: self.config.model.clone(),
            otel_event_manager: self.otel_event_manager.clone(),
            session_source: self.session_source.clone(),
            aggregation_mode: if show_reasoning {
                ChatAggregationMode::Streaming
            } else {
                ChatAggregationMode::AggregatedOnly
            },
        };

        let client = ChatCompletionsApiClient::new(config).await?;
        Ok(ModelBackend::Chat(ChatBackend {
            client,
            show_reasoning,
        }))
    }

    pub async fn stream_for_test(&self, mut prompt: Prompt) -> Result<ResponseStream> {
        crate::conversation_history::format_prompt_items(&mut prompt.input, false);
        let instructions =
            crate::client_common::compute_full_instructions(None, &self.config.model_family, false)
                .into_owned();
        prompt.instructions = instructions;
        prompt.store_response = false;
        prompt.previous_response_id = None;
        let payload = StreamPayload { prompt };
        self.stream(&payload).await
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

struct AuthManagerProvider {
    manager: Arc<AuthManager>,
}

impl AuthManagerProvider {
    fn new(manager: Arc<AuthManager>) -> Self {
        Self { manager }
    }
}

#[async_trait]
impl AuthProvider for AuthManagerProvider {
    async fn auth_context(&self) -> Option<AuthContext> {
        let auth = self.manager.auth()?;
        let mode = auth.mode;
        let account_id = auth.get_account_id();
        let bearer_token = match auth.get_token().await {
            Ok(token) if !token.is_empty() => Some(token),
            Ok(_) => None,
            Err(err) => {
                warn!("failed to resolve auth token: {err}");
                None
            }
        };

        Some(AuthContext {
            mode,
            bearer_token,
            account_id,
        })
    }

    async fn refresh_token(&self) -> std::result::Result<Option<String>, String> {
        self.manager
            .refresh_token()
            .await
            .map_err(|err| err.to_string())
    }
}

fn wrap_stream(stream: ApiClientStream) -> ResponseStream {
    let (tx, rx) = mpsc::channel::<Result<ResponseEvent>>(1600);

    tokio::spawn(async move {
        let mut stream = stream;
        while let Some(item) = stream.next().await {
            let mapped = match item {
                Ok(event) => Ok(event),
                Err(err) => Err(map_api_error(err)),
            };

            if tx.send(mapped).await.is_err() {
                break;
            }
        }
    });

    codex_api_client::EventStream::from_receiver(rx)
}

fn map_api_error(err: codex_api_client::Error) -> CodexErr {
    match err {
        codex_api_client::Error::UnsupportedOperation(msg) => CodexErr::UnsupportedOperation(msg),
        codex_api_client::Error::Http(source) => {
            CodexErr::ConnectionFailed(ConnectionFailedError { source })
        }
        codex_api_client::Error::ResponseStreamFailed { source, request_id } => {
            CodexErr::ResponseStreamFailed(ResponseStreamFailed { source, request_id })
        }
        codex_api_client::Error::Stream(message, delay) => CodexErr::Stream(message, delay),
        codex_api_client::Error::UnexpectedStatus { status, body } => {
            CodexErr::UnexpectedStatus(UnexpectedResponseError {
                status,
                body,
                request_id: None,
            })
        }
        codex_api_client::Error::RetryLimit { status, request_id } => {
            CodexErr::RetryLimit(RetryLimitReachedError { status, request_id })
        }
        codex_api_client::Error::MissingEnvVar { var, instructions } => {
            CodexErr::EnvVar(EnvVarError { var, instructions })
        }
        codex_api_client::Error::Auth(message) => CodexErr::Fatal(message),
        codex_api_client::Error::Json(err) => CodexErr::Json(err),
        codex_api_client::Error::Other(message) => CodexErr::Fatal(message),
    }
}
