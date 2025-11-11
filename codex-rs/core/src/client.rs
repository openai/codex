use std::fmt;
use std::sync::Arc;

use async_trait::async_trait;
use codex_api_client::AuthContext;
use codex_api_client::AuthProvider;
use codex_api_client::ModelProviderInfo;
use codex_api_client::Result as ApiClientResult;
use codex_api_client::RoutedApiClient;
use codex_api_client::RoutedApiClientConfig;
use codex_api_client::WireApi;
use codex_api_client::stream::WireRateLimitWindow;
use codex_otel::otel_event_manager::OtelEventManager;
use codex_protocol::ConversationId;
use codex_protocol::config_types::ReasoningEffort as ReasoningEffortConfig;
use codex_protocol::config_types::ReasoningSummary as ReasoningSummaryConfig;
use codex_protocol::protocol::SessionSource;
use futures::StreamExt;
use reqwest::StatusCode;
use std::path::PathBuf;
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
use crate::error::UsageLimitReachedError;
use crate::flags::CODEX_RS_SSE_FIXTURE;
use crate::model_family::ModelFamily;
use crate::openai_model_info::get_model_info;
use crate::token_data::KnownPlan;
use crate::token_data::PlanType;

#[derive(Clone)]
pub struct ModelClient {
    config: Arc<Config>,
    auth_manager: Option<Arc<AuthManager>>,
    otel_event_manager: OtelEventManager,
    provider: ModelProviderInfo,
    api_client: Arc<OnceCell<RoutedApiClient>>,
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
            .field("client_initialized", &self.api_client.get().is_some())
            .finish()
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
        let api_client = Arc::new(OnceCell::new());

        Self {
            config,
            auth_manager,
            otel_event_manager,
            provider,
            api_client,
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
            .map(|wid| wid.saturating_mul(pct) / 100)
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
        let instructions = prompt
            .get_full_instructions(&self.config.model_family)
            .into_owned();

        let reasoning = create_reasoning_param_for_request(
            &self.config.model_family,
            self.effort,
            self.summary,
        );

        if !self.config.model_family.support_verbosity && self.config.model_verbosity.is_some() {
            warn!(
                "model_verbosity is set but ignored as the model does not support verbosity: {}",
                self.config.model_family.family
            );
        }

        let verbosity = if self.config.model_family.support_verbosity {
            self.config.model_verbosity
        } else {
            None
        };
        let text_controls = create_text_param_for_request(verbosity, &prompt.output_schema);

        let payload_json = match self.provider.wire_api {
            WireApi::Responses => crate::wire_payload::build_responses_payload(
                prompt,
                &self.config.model,
                self.conversation_id,
                self.provider.is_azure_responses_endpoint(),
                reasoning,
                text_controls,
                instructions,
            ),
            WireApi::Chat => {
                crate::wire_payload::build_chat_payload(prompt, &self.config.model, instructions)
            }
        };

        let client = self
            .api_client
            .get_or_try_init(|| async { self.build_api_client().await })
            .await
            .map_err(map_api_error)?;

        let api_stream = client
            .stream_payload_wire(&payload_json)
            .await
            .map_err(map_api_error)?;
        Ok(wrap_wire_stream(api_stream))
    }

    async fn build_api_client(&self) -> ApiClientResult<RoutedApiClient> {
        let auth_provider = self.auth_manager.as_ref().map(|manager| {
            Arc::new(AuthManagerProvider::new(Arc::clone(manager))) as Arc<dyn AuthProvider>
        });
        let responses_fixture_path: Option<PathBuf> =
            CODEX_RS_SSE_FIXTURE.as_ref().map(PathBuf::from);
        let http_client = create_client().clone_inner();
        // Compose extra headers (conversation/session + subagent)
        let mut extra_headers: Vec<(String, String)> = vec![
            (
                "conversation_id".to_string(),
                self.conversation_id.to_string(),
            ),
            ("session_id".to_string(), self.conversation_id.to_string()),
        ];
        if let Some((name, value)) = build_subagent_header(&self.session_source) {
            extra_headers.push((name, value));
        }

        let config = RoutedApiClientConfig {
            http_client,
            provider: self.provider.clone(),
            model: self.config.model.clone(),
            conversation_id: self.conversation_id,
            auth_provider,
            otel_event_manager: self.otel_event_manager.clone(),
            responses_fixture_path,
            extra_headers,
        };

        Ok(RoutedApiClient::new(config))
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

    pub fn get_model(&self) -> String {
        self.config.model.clone()
    }

    pub fn get_model_family(&self) -> ModelFamily {
        self.config.model_family.clone()
    }

    pub fn get_reasoning_effort(&self) -> Option<ReasoningEffortConfig> {
        self.effort
    }

    pub fn get_reasoning_summary(&self) -> ReasoningSummaryConfig {
        self.summary
    }

    pub fn get_auth_manager(&self) -> Option<Arc<AuthManager>> {
        self.auth_manager.clone()
    }
}

fn build_subagent_header(session_source: &SessionSource) -> Option<(String, String)> {
    use codex_protocol::protocol::SubAgentSource;
    if let SessionSource::SubAgent(sub) = session_source {
        let value = match sub {
            SubAgentSource::Other(label) => label.clone(),
            _ => serde_json::to_value(sub)
                .ok()
                .and_then(|v| v.as_str().map(std::string::ToString::to_string))
                .unwrap_or_else(|| "other".to_string()),
        };
        Some(("x-openai-subagent".to_string(), value))
    } else {
        None
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

    async fn refresh_token(&self) -> codex_api_client::Result<Option<String>> {
        self.manager
            .refresh_token()
            .await
            .map_err(|err| codex_api_client::Error::Auth(err.to_string()))
    }
}

fn wrap_wire_stream(stream: codex_api_client::WireResponseStream) -> ResponseStream {
    let (tx, rx) = mpsc::channel::<Result<ResponseEvent>>(1600);

    tokio::spawn(async move {
        let mut stream = stream;
        while let Some(item) = stream.next().await {
            let mapped = item.map(|ev| map_wire_event(ev)).map_err(map_api_error);
            if tx.send(mapped).await.is_err() {
                break;
            }
        }
    });

    codex_api_client::EventStream::from_receiver(rx)
}

fn map_wire_event(ev: codex_api_client::WireEvent) -> ResponseEvent {
    match ev {
        codex_api_client::WireEvent::Created => ResponseEvent::Created,
        codex_api_client::WireEvent::OutputTextDelta(s) => ResponseEvent::OutputTextDelta(s),
        codex_api_client::WireEvent::ReasoningSummaryDelta(s) => {
            ResponseEvent::ReasoningSummaryDelta(s)
        }
        codex_api_client::WireEvent::ReasoningContentDelta(s) => {
            ResponseEvent::ReasoningContentDelta(s)
        }
        codex_api_client::WireEvent::ReasoningSummaryPartAdded => {
            ResponseEvent::ReasoningSummaryPartAdded
        }
        codex_api_client::WireEvent::RateLimits(w) => {
            use codex_protocol::protocol::RateLimitSnapshot;
            use codex_protocol::protocol::RateLimitWindow;
            let to_win = |ow: Option<WireRateLimitWindow>| -> Option<RateLimitWindow> {
                ow.map(|w| RateLimitWindow {
                    used_percent: w.used_percent.unwrap_or(0.0),
                    window_minutes: w.window_minutes,
                    resets_at: w.resets_at,
                })
            };
            ResponseEvent::RateLimits(RateLimitSnapshot {
                primary: to_win(w.primary),
                secondary: to_win(w.secondary),
            })
        }
        codex_api_client::WireEvent::Completed {
            response_id,
            token_usage,
        } => {
            let mapped = token_usage.map(|u| codex_protocol::protocol::TokenUsage {
                input_tokens: u.input_tokens,
                cached_input_tokens: u.cached_input_tokens,
                output_tokens: u.output_tokens,
                reasoning_output_tokens: u.reasoning_output_tokens,
                total_tokens: u.total_tokens,
            });
            ResponseEvent::Completed {
                response_id,
                token_usage: mapped,
            }
        }
        codex_api_client::WireEvent::OutputItemAdded(v) => {
            let item = serde_json::from_value::<codex_protocol::models::ResponseItem>(v)
                .unwrap_or(codex_protocol::models::ResponseItem::Other);
            ResponseEvent::OutputItemAdded(item)
        }
        codex_api_client::WireEvent::OutputItemDone(v) => {
            let item = serde_json::from_value::<codex_protocol::models::ResponseItem>(v)
                .unwrap_or(codex_protocol::models::ResponseItem::Other);
            ResponseEvent::OutputItemDone(item)
        }
    }
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
        codex_api_client::Error::Stream(message, delay) => {
            let lower = message.to_lowercase();
            if lower.contains("context window exceeded") {
                CodexErr::ContextWindowExceeded
            } else if lower.contains("quota exceeded") {
                CodexErr::QuotaExceeded
            } else {
                CodexErr::Stream(message, delay)
            }
        }
        codex_api_client::Error::UsageLimitReached {
            plan_type,
            resets_at,
            rate_limits,
        } => {
            let plan_type = plan_type.map(normalize_plan_type);
            let resets_at =
                resets_at.and_then(|secs| chrono::DateTime::<chrono::Utc>::from_timestamp(secs, 0));
            CodexErr::UsageLimitReached(UsageLimitReachedError {
                plan_type,
                resets_at,
                rate_limits,
            })
        }
        codex_api_client::Error::UnexpectedStatus { status, body } => {
            CodexErr::UnexpectedStatus(UnexpectedResponseError {
                status,
                body,
                request_id: None,
            })
        }
        codex_api_client::Error::RetryLimit { status, request_id } => {
            CodexErr::RetryLimit(RetryLimitReachedError {
                status: status.unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                request_id,
            })
        }
        codex_api_client::Error::MissingEnvVar { var, instructions } => {
            CodexErr::EnvVar(EnvVarError { var, instructions })
        }
        codex_api_client::Error::Auth(message) => CodexErr::Fatal(message),
        codex_api_client::Error::Json(err) => CodexErr::Json(err),
        codex_api_client::Error::Other(message) => CodexErr::Fatal(message),
    }
}

fn normalize_plan_type(plan: String) -> PlanType {
    match plan.to_lowercase().as_str() {
        "free" => PlanType::Known(KnownPlan::Free),
        "plus" => PlanType::Known(KnownPlan::Plus),
        "pro" => PlanType::Known(KnownPlan::Pro),
        "team" => PlanType::Known(KnownPlan::Team),
        "business" => PlanType::Known(KnownPlan::Business),
        "enterprise" => PlanType::Known(KnownPlan::Enterprise),
        "edu" => PlanType::Known(KnownPlan::Edu),
        other => PlanType::Unknown(other.to_string()),
    }
}

/// Stream using the codex-api-client directly from a `TurnContext` without `ModelClient` indirection.
pub async fn stream_for_turn(
    ctx: &crate::codex::TurnContext,
    prompt: &Prompt,
) -> Result<ResponseStream> {
    ctx.client.stream(prompt).await
}
