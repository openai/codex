use std::fmt;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;

use crate::GatewayAuthManager;
use codex_api::ApiError;
use codex_api::Provider;
use codex_api::SharedAuthProvider;
use codex_api::TransportError;
use codex_features::RuntimeAuthMode;
use codex_login::AuthManager;
use codex_login::AuthManagerConfig;
use codex_login::AuthManagerInitializationError;
use codex_login::CodexAuth;
use codex_login::WorkspaceRoutingRequest;
use codex_login::default_client::ClientRedirectPolicy;
use codex_model_provider_info::ModelProviderInfo;
use codex_models_manager::cache::ModelsCache;
use codex_models_manager::manager::ModelCatalogSettings;
use codex_models_manager::manager::OpenAiModelsManager;
use codex_models_manager::manager::SharedModelsManager;
use codex_models_manager::manager::StaticModelsManager;
use codex_protocol::account::ProviderAccount;
use codex_protocol::auth::RefreshTokenFailedError;
use codex_protocol::error::CodexErr;
use codex_protocol::openai_models::ModelsResponse;
use codex_runtime_agent_instance::RuntimeAuthManager;

use crate::ProviderCapabilities;
use crate::ResolvedResponsesProvider;
use crate::amazon_bedrock::AmazonBedrockModelProvider;
use crate::auth::ProviderAuthScope;
use crate::auth::ResolvedProviderAuth;
use crate::auth::auth_manager_for_provider;
use crate::auth::resolve_provider_auth;
use crate::auth::resolve_provider_auth_for_scope;
use crate::auth::uses_fedramp_api_key;
use crate::combined_auth::compose_auth;
use crate::gcp::GcpModelProvider;
use crate::models_endpoint::OpenAiModelsEndpoint;
use crate::request_setup::resolve_request_auth;
use crate::workspace_routing::WorkspaceRoutingContext;

/// Current app-visible account state for a model provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderAccountState {
    pub account: Option<ProviderAccount>,
    pub requires_openai_auth: bool,
}

/// Outcome of a provider-owned attempt to recover from an authentication failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderUnauthorizedRecovery {
    /// The provider has no provider-specific authentication recovery configured.
    NotConfigured,
    /// The provider recovered its authentication state and the request can be retried.
    Recovered,
}

/// User-facing lifecycle messages for provider-owned authentication recovery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderAuthRecoveryMessages {
    pub started: &'static str,
    pub succeeded: &'static str,
}

/// Error returned when a provider cannot construct its app-visible account state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderAccountError {
    MissingChatgptAccountDetails,
    UnsupportedBedrockApiKeyAuth,
}

impl fmt::Display for ProviderAccountError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingChatgptAccountDetails => {
                write!(f, "plan type is required for chatgpt authentication")
            }
            Self::UnsupportedBedrockApiKeyAuth => {
                write!(
                    f,
                    "Bedrock API key auth is only supported by the Amazon Bedrock model provider"
                )
            }
        }
    }
}

impl std::error::Error for ProviderAccountError {}

pub type ProviderAccountResult = std::result::Result<ProviderAccountState, ProviderAccountError>;

/// Default model used for automatic approval review when a provider does not
/// require a backend-specific model ID.
pub const DEFAULT_APPROVAL_REVIEW_PREFERRED_MODEL: &str = "codex-auto-review";

const API_KEY_APPROVAL_REVIEW_PREFERRED_MODEL: &str = "gpt-5.6-luna";

const FEDRAMP_API_BASE_URL: &str = "https://gov.api.openai.com/v1";

/// Default model used for memory extraction when a provider does not require a
/// backend-specific model ID.
pub const DEFAULT_MEMORY_EXTRACTION_PREFERRED_MODEL: &str = "gpt-6-luna";

/// Default model used for memory consolidation when a provider does not require
/// a backend-specific model ID.
pub const DEFAULT_MEMORY_CONSOLIDATION_PREFERRED_MODEL: &str = "gpt-6.1-terra";

/// Runtime provider abstraction used by model execution.
///
/// Implementations own provider-specific behavior for a model backend. The
/// `ModelProviderInfo` returned by `info` is the serialized/configured provider
/// metadata used by the default OpenAI-compatible implementation.
///
/// OpenAI-compatible catalog caches use header auth's account ID instead of auth
/// headers when available. Missing IDs and explicit provider API key, bearer, or
/// command auth retain credential-sensitive keys. Provider settings remain in the key.
/// Hosts using header auth must isolate model managers and cache storage by user and
/// catalog-varying request context, such as country: an account ID may identify a
/// workspace shared by multiple users. Request authentication, credential refresh,
/// and permission checks are unchanged.
pub trait ModelProvider: fmt::Debug + Send + Sync {
    /// Returns the configured provider metadata.
    fn info(&self) -> &ModelProviderInfo;

    /// Returns whether the resolved Responses provider may receive internal tool metadata.
    fn include_internal_metadata(&self, provider: &Provider) -> bool {
        self.info().include_internal_metadata
            || url::Url::parse(&provider.base_url).ok().is_some_and(|url| {
                url.scheme() == "https"
                    && url.host_str().is_some_and(|host| {
                        host == "api.openai.com" || codex_http_client::is_allowed_chatgpt_host(host)
                    })
            })
    }

    /// Returns the provider-owned capability upper bounds.
    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities::default()
    }

    /// Returns the preferred model used for automatic approval review.
    ///
    /// Providers that require backend-specific model IDs should override this.
    fn approval_review_preferred_model(&self) -> &'static str {
        DEFAULT_APPROVAL_REVIEW_PREFERRED_MODEL
    }

    /// Returns the preferred model used for memory extraction.
    ///
    /// Providers that require backend-specific model IDs should override this.
    fn memory_extraction_preferred_model(&self) -> &'static str {
        DEFAULT_MEMORY_EXTRACTION_PREFERRED_MODEL
    }

    /// Returns the preferred model used for memory consolidation.
    ///
    /// Providers that require backend-specific model IDs should override this.
    fn memory_consolidation_preferred_model(&self) -> &'static str {
        DEFAULT_MEMORY_CONSOLIDATION_PREFERRED_MODEL
    }

    /// Returns whether requests made through this provider should include attestation.
    fn supports_attestation(&self) -> bool {
        false
    }

    /// Returns the provider-scoped auth manager, when this provider uses one.
    ///
    /// TODO(celia-oai): Make auth manager access internal to this crate so callers
    /// resolve provider-specific auth only through `ModelProvider`. We first need
    /// to think through whether Codex should have a unified provider-specific auth
    /// manager throughout the codebase; that is a larger refactor than this change.
    fn auth_manager(&self) -> Option<Arc<AuthManager>>;

    /// Runtime authentication selected for this provider's inference destination.
    fn runtime_auth(&self) -> Option<&(Arc<RuntimeAuthManager>, RuntimeAuthMode)> {
        None
    }

    /// Returns the gateway credential manager shared with inference and model discovery.
    /// Hosts use this handle for explicit login; configured setup failures remain errors.
    fn gateway_auth_manager(&self) -> std::io::Result<Option<Arc<GatewayAuthManager>>> {
        Ok(None)
    }

    /// Returns whether this transport failure can be recovered by provider-scoped authentication.
    ///
    /// The default preserves existing unauthorized-response handling. Providers with other
    /// authentication failure shapes may recognize additional response or request-signing errors.
    fn is_recoverable_auth_error(&self, error: &TransportError) -> bool {
        matches!(
            error,
            TransportError::Http { status, .. } if *status == http::StatusCode::UNAUTHORIZED
        )
    }

    /// Returns lifecycle messages when provider-owned authentication recovery is active.
    fn auth_recovery_messages(&self) -> Option<ProviderAuthRecoveryMessages> {
        None
    }

    /// Attempts provider-owned authentication recovery before using the auth manager.
    fn recover_from_unauthorized(
        &self,
    ) -> ModelProviderFuture<'_, codex_protocol::error::Result<ProviderUnauthorizedRecovery>> {
        Box::pin(async { Ok(ProviderUnauthorizedRecovery::NotConfigured) })
    }

    /// Returns the current provider-scoped auth value, if one is configured.
    fn auth(&self) -> ModelProviderFuture<'_, Option<CodexAuth>>;

    /// Returns the current app-visible account state for this provider.
    fn account_state(&self) -> ProviderAccountResult;

    /// Maps an API client error into the provider's user-facing error representation.
    fn map_api_error(&self, error: ApiError) -> CodexErr {
        codex_api::map_api_error(error)
    }

    /// Returns provider configuration adapted for the API client.
    fn api_provider(&self) -> ModelProviderFuture<'_, codex_protocol::error::Result<Provider>> {
        Box::pin(async move {
            let auth = self.auth().await;
            self.api_provider_for_auth(auth.as_ref())
        })
    }

    /// Resolves routing for Responses HTTP, compaction, and WebSocket handshakes.
    fn responses_api_provider<'a>(
        &'a self,
        routing_context: &'a WorkspaceRoutingContext,
    ) -> ModelProviderFuture<'a, codex_protocol::error::Result<ResolvedResponsesProvider>> {
        Box::pin(async move {
            let provider = self.api_provider().await?;
            let auth = self.auth().await;
            self.route_responses_api_provider(provider, auth.as_ref(), routing_context)
                .await
        })
    }

    /// Applies workspace routing using the credentials already selected for this request.
    #[expect(
        clippy::await_holding_invalid_type,
        reason = "serialize discovery and the session's first successful routing transition"
    )]
    fn route_responses_api_provider<'a>(
        &'a self,
        mut provider: Provider,
        auth: Option<&'a CodexAuth>,
        routing_context: &'a WorkspaceRoutingContext,
    ) -> ModelProviderFuture<'a, codex_protocol::error::Result<ResolvedResponsesProvider>> {
        Box::pin(async move {
            let mut redirect_policy = ClientRedirectPolicy::Default;
            if provider_uses_first_party_auth_path(self.info())
                && self.info().supports_codex_backend_routes()
                && let Some(auth) = auth.filter(|auth| auth.is_chatgpt_auth())
                && let Some(auth_manager) = self.auth_manager()
            {
                let mut previously_routed = routing_context.previously_routed.lock().await;
                if let Some(routing) = auth_manager
                    .workspace_routing(
                        auth,
                        WorkspaceRoutingRequest {
                            provider_base_url: provider.base_url.clone(),
                            chatgpt_base_url: routing_context.chatgpt_base_url.clone(),
                            previously_routed: *previously_routed,
                            session: routing_context.session.clone(),
                        },
                    )
                    .await
                    .map_err(|error| match error.downcast::<RefreshTokenFailedError>() {
                        Ok(rejection) => CodexErr::RefreshTokenFailed(rejection),
                        Err(error) => error.into(),
                    })?
                {
                    crate::workspace_routing::apply_workspace_routing(&mut provider, routing)?;
                    redirect_policy = ClientRedirectPolicy::Reject;
                    *previously_routed = true;
                }
            }
            Ok(ResolvedResponsesProvider {
                provider,
                redirect_policy,
            })
        })
    }

    /// Adapt routing from the same frozen credential selected for the request.
    fn api_provider_for_auth(
        &self,
        auth: Option<&CodexAuth>,
    ) -> codex_protocol::error::Result<Provider> {
        resolve_api_provider(self.info(), auth)
    }

    /// Returns the provider base URL that will be used at request time.
    fn runtime_base_url(
        &self,
    ) -> ModelProviderFuture<'_, codex_protocol::error::Result<Option<String>>> {
        Box::pin(async { Ok(self.info().base_url.clone()) })
    }

    /// Returns the auth provider used to attach request credentials.
    fn api_auth(
        &self,
    ) -> ModelProviderFuture<'_, codex_protocol::error::Result<SharedAuthProvider>> {
        Box::pin(async move {
            let auth = self.auth().await;
            resolve_provider_auth(auth.as_ref(), self.info())
        })
    }

    /// Adds provider-owned credentials, such as gateway OAuth, to already selected primary auth.
    fn compose_auth(
        &self,
        resolved: ResolvedProviderAuth,
    ) -> ModelProviderFuture<'_, codex_protocol::error::Result<ResolvedProviderAuth>> {
        Box::pin(async move { Ok(resolved) })
    }

    /// Returns request credentials, optionally scoped to a Codex session task.
    fn api_auth_for_scope(
        &self,
        scope: ProviderAuthScope,
    ) -> ModelProviderFuture<'_, codex_protocol::error::Result<ResolvedProviderAuth>> {
        Box::pin(async move {
            if !provider_uses_first_party_auth_path(self.info()) {
                return self.api_auth().await.map(ResolvedProviderAuth::new);
            }
            let auth = self.auth().await;
            resolve_provider_auth_for_scope(self.auth_manager(), auth.as_ref(), self.info(), scope)
                .await
        })
    }

    /// Resolve auth together with metadata needed to route and admit this request.
    fn request_auth(
        &self,
        scope: Option<ProviderAuthScope>,
    ) -> ModelProviderFuture<'_, codex_protocol::error::Result<ResolvedProviderAuth>> {
        match scope {
            Some(scope) => self.api_auth_for_scope(scope),
            None => Box::pin(async { self.api_auth().await.map(ResolvedProviderAuth::new) }),
        }
    }

    /// Resolve the endpoint before headers for providers whose setup may refresh credentials.
    fn request_provider_and_auth(
        &self,
        scope: Option<ProviderAuthScope>,
    ) -> ModelProviderFuture<'_, codex_protocol::error::Result<(Provider, ResolvedProviderAuth)>>
    {
        Box::pin(async move { Ok((self.api_provider().await?, self.request_auth(scope).await?)) })
    }

    /// Creates the model manager implementation appropriate for this provider.
    fn models_manager(
        &self,
        codex_home: PathBuf,
        config_model_catalog: Option<ModelsResponse>,
    ) -> SharedModelsManager;

    /// Build a thread catalog with the same cache and live discovery policy as its harness.
    fn models_manager_with_settings(
        &self,
        config_model_catalog: Option<ModelsResponse>,
        _settings: ModelCatalogSettings,
    ) -> SharedModelsManager {
        self.models_manager_without_cache(config_model_catalog)
    }

    /// Creates a model manager with caching disabled.
    ///
    /// Providers that fetch model catalogs should override this method. The default uses an
    /// authoritative in-memory catalog so hosted callers cannot accidentally write to disk.
    fn models_manager_without_cache(
        &self,
        config_model_catalog: Option<ModelsResponse>,
    ) -> SharedModelsManager {
        let origin = if config_model_catalog.is_some() {
            codex_models_manager::ModelOrigin::ExplicitConfig
        } else {
            codex_models_manager::ModelOrigin::Bundled
        };
        let model_catalog = config_model_catalog
            .or_else(|| codex_models_manager::bundled_models_response().ok())
            .unwrap_or_default();
        Arc::new(StaticModelsManager::new(
            self.auth_manager(),
            model_catalog,
            origin,
        ))
    }

    /// Creates a model manager that can use a caller-provided cache for remote catalogs.
    ///
    /// Providers with remote catalogs should override this method. The default preserves the
    /// authoritative catalog returned by [`ModelProvider::models_manager_without_cache`] and does
    /// not consult `cache`. Implementations should likewise ignore the cache when
    /// `config_model_catalog` supplies an authoritative static catalog.
    fn models_manager_with_cache(
        &self,
        config_model_catalog: Option<ModelsResponse>,
        cache: Arc<dyn ModelsCache>,
    ) -> SharedModelsManager {
        drop(cache);
        self.models_manager_without_cache(config_model_catalog)
    }
}

pub type ModelProviderFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Shared runtime model provider handle.
pub type SharedModelProvider = Arc<dyn ModelProvider>;

/// Resolve credential-dependent defaults without changing configured endpoints.
pub fn resolve_api_provider(
    info: &ModelProviderInfo,
    auth: Option<&CodexAuth>,
) -> codex_protocol::error::Result<Provider> {
    let mut provider = info.to_api_provider(auth.map(CodexAuth::auth_mode))?;
    if info.base_url.is_none() && uses_fedramp_api_key(auth, info)? {
        provider.base_url = FEDRAMP_API_BASE_URL.to_string();
    }
    Ok(provider)
}

pub fn provider_uses_first_party_auth_path(provider: &ModelProviderInfo) -> bool {
    provider.requires_openai_auth
        && provider.env_key.is_none()
        && provider.experimental_bearer_token.is_none()
        && provider.auth.is_none()
        && provider.aws.is_none()
}

/// Creates the default model provider for configured provider metadata.
/// Harnesses with runtime identity use [`crate::ProviderAuthContext::model_provider`].
pub fn create_model_provider(
    provider_info: ModelProviderInfo,
    auth_manager: Option<Arc<AuthManager>>,
) -> SharedModelProvider {
    create_model_provider_with_runtime(provider_info, auth_manager, /*runtime_auth*/ None)
}

/// Creates a model provider using the local login settings.
pub async fn create_model_provider_from_local_config(
    provider_info: ModelProviderInfo,
    config: &impl AuthManagerConfig,
) -> Result<SharedModelProvider, AuthManagerInitializationError> {
    let credentials =
        AuthManager::shared_from_config(config, /*enable_codex_api_key_env*/ false).await?;
    Ok(create_model_provider(provider_info, Some(credentials)))
}

pub(crate) fn create_model_provider_with_runtime(
    provider_info: ModelProviderInfo,
    auth_manager: Option<Arc<AuthManager>>,
    runtime_auth: Option<(Arc<RuntimeAuthManager>, RuntimeAuthMode)>,
) -> SharedModelProvider {
    if provider_info.is_amazon_bedrock() {
        return Arc::new(AmazonBedrockModelProvider::new(provider_info, auth_manager));
    }
    if provider_info.is_gcp() {
        return Arc::new(GcpModelProvider::new(provider_info, auth_manager));
    }
    Arc::new(ConfiguredModelProvider::new(
        provider_info,
        auth_manager,
        runtime_auth,
    ))
}

/// Runtime model provider that orchestrates primary and gateway credentials.
#[derive(Clone, Debug)]
struct ConfiguredModelProvider {
    info: ModelProviderInfo,
    auth_manager: Option<Arc<AuthManager>>,
    // Construct eagerly; report setup failures when auth is requested because the factory is infallible.
    gateway_auth_manager: Option<Result<Arc<GatewayAuthManager>, String>>,
    runtime_auth: Option<(Arc<RuntimeAuthManager>, RuntimeAuthMode)>,
    catalog_runtime_auth: Option<(Arc<RuntimeAuthManager>, RuntimeAuthMode)>,
}

impl ConfiguredModelProvider {
    fn new(
        info: ModelProviderInfo,
        auth_manager: Option<Arc<AuthManager>>,
        runtime_auth: Option<(Arc<RuntimeAuthManager>, RuntimeAuthMode)>,
    ) -> Self {
        let gateway_auth_manager = info.gateway_oauth.as_ref().map(|config| {
            info.validate()?;
            let manager = auth_manager
                .as_ref()
                .ok_or_else(|| "gateway_oauth requires auth runtime configuration".to_string())?;
            GatewayAuthManager::new(&info, config, manager).map(Arc::new)
        });
        let auth_manager = auth_manager_for_provider(auth_manager, &info);
        let runtime_for_destination = |destination: Option<&str>| {
            runtime_auth.clone().filter(|_| {
                destination.is_none_or(|url| {
                    codex_http_client::classify_openai_api_url(url)
                        != codex_http_client::OpenAiDestination::Other
                })
            })
        };
        let inference_runtime_auth = runtime_for_destination(info.base_url.as_deref());
        // Catalog overrides are absolute URLs and can have a different origin from inference.
        let catalog_runtime_auth = runtime_for_destination(
            info.model_catalog_url
                .as_deref()
                .map(String::as_str)
                .or(info.base_url.as_deref()),
        );
        Self {
            info,
            auth_manager,
            gateway_auth_manager,
            runtime_auth: inference_runtime_auth,
            catalog_runtime_auth,
        }
    }

    async fn resolve_auth(
        &self,
        scope: Option<ProviderAuthScope>,
    ) -> codex_protocol::error::Result<ResolvedProviderAuth> {
        let resolved = resolve_request_auth(
            self.info.clone(),
            self.auth_manager.clone(),
            self.runtime_auth.clone(),
            scope,
            self.info.base_url.clone(),
        )
        .await?;
        self.compose_auth(resolved).await
    }
}

impl ModelProvider for ConfiguredModelProvider {
    fn is_recoverable_auth_error(&self, error: &TransportError) -> bool {
        let TransportError::Http { status, body, .. } = error else {
            return false;
        };
        *status == http::StatusCode::UNAUTHORIZED
            && (self.runtime_auth.is_none()
                || !codex_protocol::error::is_runtime_auth_denial(
                    *status,
                    body.as_deref().unwrap_or_default(),
                    codex_response_debug_context::extract_response_debug_context(error)
                        .auth_error_code
                        .as_deref(),
                ))
    }

    fn info(&self) -> &ModelProviderInfo {
        &self.info
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities::from_config(&self.info)
    }

    fn approval_review_preferred_model(&self) -> &'static str {
        if self
            .auth_manager
            .as_ref()
            .and_then(|auth_manager| auth_manager.auth_cached())
            .is_some_and(|auth| auth.is_api_key_auth())
        {
            API_KEY_APPROVAL_REVIEW_PREFERRED_MODEL
        } else {
            DEFAULT_APPROVAL_REVIEW_PREFERRED_MODEL
        }
    }

    fn auth_manager(&self) -> Option<Arc<AuthManager>> {
        self.auth_manager.clone()
    }

    fn gateway_auth_manager(&self) -> std::io::Result<Option<Arc<GatewayAuthManager>>> {
        self.gateway_auth_manager
            .clone()
            .transpose()
            .map_err(std::io::Error::other)
    }

    fn recover_from_unauthorized(
        &self,
    ) -> ModelProviderFuture<'_, codex_protocol::error::Result<ProviderUnauthorizedRecovery>> {
        Box::pin(async {
            let Some(gateway) = self.gateway_auth_manager()? else {
                return Ok(ProviderUnauthorizedRecovery::NotConfigured);
            };
            Ok(gateway.prepare_retry().await)
        })
    }

    fn supports_attestation(&self) -> bool {
        self.auth_manager
            .as_ref()
            .and_then(|auth_manager| auth_manager.auth_cached())
            .is_some_and(|auth| auth.is_chatgpt_auth())
    }

    fn auth(&self) -> ModelProviderFuture<'_, Option<CodexAuth>> {
        Box::pin(async move {
            match self.auth_manager.as_ref() {
                Some(auth_manager) => auth_manager.auth().await,
                None => None,
            }
        })
    }

    fn runtime_auth(&self) -> Option<&(Arc<RuntimeAuthManager>, RuntimeAuthMode)> {
        self.runtime_auth.as_ref()
    }

    fn request_auth(
        &self,
        scope: Option<ProviderAuthScope>,
    ) -> ModelProviderFuture<'_, codex_protocol::error::Result<ResolvedProviderAuth>> {
        Box::pin(self.resolve_auth(scope))
    }

    fn request_provider_and_auth(
        &self,
        scope: Option<ProviderAuthScope>,
    ) -> ModelProviderFuture<'_, codex_protocol::error::Result<(Provider, ResolvedProviderAuth)>>
    {
        Box::pin(async move {
            let resolved = self.resolve_auth(scope).await?;
            let provider = self.api_provider_for_auth(resolved.credential.as_ref())?;
            Ok((provider, resolved))
        })
    }

    fn api_auth(
        &self,
    ) -> ModelProviderFuture<'_, codex_protocol::error::Result<SharedAuthProvider>> {
        Box::pin(async move {
            Ok(self.resolve_auth(/*scope*/ None).await?.auth)
        })
    }

    fn compose_auth(
        &self,
        resolved: ResolvedProviderAuth,
    ) -> ModelProviderFuture<'_, codex_protocol::error::Result<ResolvedProviderAuth>> {
        Box::pin(compose_auth(
            &self.info,
            self.gateway_auth_manager.as_ref(),
            resolved,
        ))
    }

    fn api_auth_for_scope(
        &self,
        scope: ProviderAuthScope,
    ) -> ModelProviderFuture<'_, codex_protocol::error::Result<ResolvedProviderAuth>> {
        Box::pin(self.resolve_auth(Some(scope)))
    }

    fn account_state(&self) -> ProviderAccountResult {
        let account = if self.info.requires_openai_auth {
            self.auth_manager
                .as_ref()
                .and_then(|auth_manager| {
                    let auth = auth_manager.auth_cached()?;
                    if auth_manager.refresh_failure_for_auth(&auth).is_some() {
                        return None;
                    }
                    if matches!(auth, CodexAuth::Headers(_)) {
                        return None;
                    }
                    Some(auth)
                })
                .map(|auth| match &auth {
                    CodexAuth::ApiKey(_) => Ok(ProviderAccount::ApiKey),
                    CodexAuth::BedrockApiKey(_) | CodexAuth::BedrockAccessKeys(_) => {
                        Err(ProviderAccountError::UnsupportedBedrockApiKeyAuth)
                    }
                    CodexAuth::Chatgpt(_)
                    | CodexAuth::ChatgptAuthTokens(_)
                    | CodexAuth::Headers(_)
                    | CodexAuth::AgentIdentity(_)
                    | CodexAuth::PersonalAccessToken(_) => {
                        let email = auth.get_account_email();
                        let plan_type = auth.account_plan_type();

                        plan_type
                            .map(|plan_type| ProviderAccount::Chatgpt { email, plan_type })
                            .ok_or(ProviderAccountError::MissingChatgptAccountDetails)
                    }
                })
                .transpose()?
        } else {
            None
        };

        Ok(ProviderAccountState {
            account,
            requires_openai_auth: self.info.requires_openai_auth,
        })
    }

    fn models_manager(
        &self,
        codex_home: PathBuf,
        config_model_catalog: Option<ModelsResponse>,
    ) -> SharedModelsManager {
        self.models_manager_with_settings(
            config_model_catalog,
            ModelCatalogSettings::disk(codex_home),
        )
    }

    fn models_manager_with_settings(
        &self,
        config_model_catalog: Option<ModelsResponse>,
        settings: ModelCatalogSettings,
    ) -> SharedModelsManager {
        if let Some(model_catalog) = config_model_catalog {
            return Arc::new(StaticModelsManager::new(
                self.auth_manager.clone(),
                model_catalog,
                codex_models_manager::ModelOrigin::ExplicitConfig,
            ));
        }
        let endpoint = Arc::new(
            OpenAiModelsEndpoint::new(
                self.info.clone(),
                self.auth_manager.clone(),
                self.gateway_auth_manager.clone(),
            )
            .with_runtime_auth(self.catalog_runtime_auth.clone()),
        );
        let auth_manager = self.auth_manager.clone();
        let manager = OpenAiModelsManager::with_settings(settings, endpoint, auth_manager);
        match &self.info.model_catalog_url {
            Some(_) => Arc::new(manager.with_provider_catalog()),
            None => Arc::new(manager),
        }
    }

    fn models_manager_without_cache(
        &self,
        config_model_catalog: Option<ModelsResponse>,
    ) -> SharedModelsManager {
        self.models_manager_with_settings(config_model_catalog, ModelCatalogSettings::default())
    }

    fn models_manager_with_cache(
        &self,
        config_model_catalog: Option<ModelsResponse>,
        cache: Arc<dyn ModelsCache>,
    ) -> SharedModelsManager {
        self.models_manager_with_settings(
            config_model_catalog,
            ModelCatalogSettings::with_cache(cache),
        )
    }
}

pub(crate) mod test_support {
    use super::*;

    /// Use an injected runtime with a local transport fixture. Production callers use the
    /// destination-aware `ProviderAuthContext::model_provider` factory.
    pub fn runtime_model_provider(
        info: ModelProviderInfo,
        credentials: Arc<AuthManager>,
        runtime: (Arc<RuntimeAuthManager>, RuntimeAuthMode),
    ) -> SharedModelProvider {
        let mut provider =
            ConfiguredModelProvider::new(info, Some(credentials), Some(runtime.clone()));
        provider.runtime_auth = Some(runtime);
        Arc::new(provider)
    }
}

#[cfg(test)]
#[path = "provider_tests.rs"]
mod tests;
