use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use codex_agent_identity::AgentIdentityKey;
use codex_agent_identity::authorization_header_for_agent_task;
use codex_api::AgentIdentityTelemetry;
use codex_api::AuthError;
use codex_api::AuthHeadersFuture;
use codex_api::AuthProvider;
use codex_api::SharedAuthProvider;
use codex_features::RuntimeAuthMode;
use codex_http_client::CHATGPT_ACCOUNT_ID_HEADER;
use codex_http_client::HttpClientFactory;
use codex_http_client::RouteAwareClientPool;
use codex_login::AuthHeaders;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use codex_login::auth::AgentIdentityAuth;
use codex_login::auth::AgentIdentityAuthError;
use codex_login::auth::AgentIdentityAuthPolicy;
use codex_model_provider_info::ModelProviderInfo;
use codex_protocol::error::CodexErr;
use codex_protocol::protocol::SessionSource;
use codex_runtime_agent_instance::RuntimeAuth;
use codex_runtime_agent_instance::RuntimeAuthManager;
use http::HeaderMap;
use http::HeaderValue;

use crate::bearer_auth_provider::BearerAuthProvider;

const BEDROCK_API_KEY_UNSUPPORTED_MESSAGE: &str =
    "Bedrock API key auth is only supported by the Amazon Bedrock model provider";

/// Credential ownership and optional OpenAI runtime auth for one harness/thread context.
/// Publish these together so auxiliary clients cannot select another thread's credentials.
#[derive(Clone, Debug)]
pub struct ProviderAuthContext {
    pub credentials: Arc<AuthManager>,
    pub runtime_auth: Option<(Arc<RuntimeAuthManager>, RuntimeAuthMode)>,
}

impl ProviderAuthContext {
    pub fn model_provider(&self, info: ModelProviderInfo) -> crate::SharedModelProvider {
        crate::provider::create_model_provider_with_runtime(
            info,
            Some(Arc::clone(&self.credentials)),
            self.runtime_auth.clone(),
        )
    }
}

#[derive(Clone, Debug)]
pub struct ProviderAuthScope {
    pub agent_identity_policy: AgentIdentityAuthPolicy,
    pub session_source: SessionSource,
    pub agent_identity_session_fallback: AgentIdentitySessionFallback,
}

#[derive(Clone, Debug, Default)]
pub struct AgentIdentitySessionFallback {
    engaged: Arc<AtomicBool>,
}

impl AgentIdentitySessionFallback {
    pub fn is_engaged(&self) -> bool {
        self.engaged.load(Ordering::Relaxed)
    }

    fn engage(&self) -> bool {
        !self.engaged.swap(true, Ordering::Relaxed)
    }
}

/// Provider auth resolved for a request, plus metadata describing the effective auth.
#[derive(Clone)]
pub struct ResolvedProviderAuth {
    /// Login selected with these headers; routing must use the same credential.
    pub credential: Option<CodexAuth>,
    pub auth: SharedAuthProvider,
    pub agent_identity_telemetry: Option<AgentIdentityTelemetry>,
    pub runtime: Option<RuntimeAuth>,
}

impl ResolvedProviderAuth {
    pub(crate) fn new(auth: SharedAuthProvider) -> Self {
        Self {
            credential: None,
            auth,
            agent_identity_telemetry: None,
            runtime: None,
        }
    }

    pub(crate) fn for_runtime(selected: RuntimeAuth, mode: RuntimeAuthMode) -> Self {
        Self {
            credential: Some(selected.auth().clone()),
            auth: selected.clone().into_auth_provider(mode),
            agent_identity_telemetry: None,
            runtime: Some(selected),
        }
    }

    /// Preserve the caller's transport configuration and bind it to the selected login.
    pub fn bind_http_client_factory(
        &self,
        factory: HttpClientFactory,
    ) -> Result<HttpClientFactory, AuthError> {
        match &self.runtime {
            Some(runtime) => runtime.bind_http_client_factory(factory),
            None => Ok(factory),
        }
    }

    /// Preserve the caller's connection pool and bind it to the selected login.
    pub fn bind_http_client_pool(
        &self,
        pool: &RouteAwareClientPool,
    ) -> Result<RouteAwareClientPool, AuthError> {
        match &self.runtime {
            Some(runtime) => pool
                .with_account_binding(runtime.http_client_factory().network_policy())
                .ok_or_else(|| {
                    AuthError::Build(
                        "runtime credentials and transport must share the application network policy"
                            .to_string(),
                    )
                }),
            None => Ok(pool.clone()),
        }
    }

    /// Reject account selectors that disagree with the selected runtime.
    pub fn validate_headers(&self, headers: &HeaderMap) -> codex_protocol::error::Result<()> {
        if let Some(runtime) = &self.runtime {
            crate::validate_runtime_selectors(headers, runtime.runtime().actor().info())?;
        }
        Ok(())
    }

    fn for_agent_identity(auth: AgentIdentityAuth) -> Self {
        let agent_identity_telemetry = agent_identity_telemetry(&auth);
        Self {
            credential: None,
            auth: Arc::new(AgentIdentityAuthProvider { auth }),
            agent_identity_telemetry: Some(agent_identity_telemetry),
            runtime: None,
        }
    }
}

pub(crate) fn agent_identity_telemetry(auth: &AgentIdentityAuth) -> AgentIdentityTelemetry {
    AgentIdentityTelemetry {
        agent_id: auth.record().agent_runtime_id.clone(),
        task_id: auth.run_task_id().to_string(),
    }
}

#[derive(Clone, Debug)]
struct AgentIdentityAuthProvider {
    auth: AgentIdentityAuth,
}

impl AuthProvider for AgentIdentityAuthProvider {
    fn add_auth_headers(&self, headers: &mut HeaderMap) -> Result<(), AuthError> {
        let record = self.auth.record();
        let header_value = authorization_header_for_agent_task(
            AgentIdentityKey {
                agent_runtime_id: &record.agent_runtime_id,
                private_key_pkcs8_base64: &record.agent_private_key,
            },
            self.auth.run_task_id(),
        )
        .map_err(std::io::Error::other);

        if let Ok(header_value) = header_value
            && let Ok(header) = HeaderValue::from_str(&header_value)
        {
            let _ = headers.insert(http::header::AUTHORIZATION, header);
        }

        if let Ok(header) = HeaderValue::from_str(self.auth.account_id()) {
            let _ = headers.insert(CHATGPT_ACCOUNT_ID_HEADER, header);
        }

        if self.auth.is_fedramp_account() {
            let _ = headers.insert("X-OpenAI-Fedramp", HeaderValue::from_static("true"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
struct HeaderAuthProvider {
    auth: AuthHeaders,
}

impl AuthProvider for HeaderAuthProvider {
    fn add_auth_headers(&self, headers: &mut HeaderMap) -> Result<(), AuthError> {
        headers.extend(self.auth.headers().clone());
        Ok(())
    }
}

struct AuthManagerAuthProvider {
    auth_manager: Arc<AuthManager>,
    // Startup auth is only the account-scoped identity anchor. Request
    // headers always come from the current AuthManager snapshot below.
    expected_auth: CodexAuth,
}

impl AuthManagerAuthProvider {
    fn is_expected_auth(&self, auth: &CodexAuth) -> bool {
        auth.uses_codex_backend()
            && auth.get_account_id() == self.expected_auth.get_account_id()
            && auth.get_chatgpt_user_id() == self.expected_auth.get_chatgpt_user_id()
            && auth.is_workspace_account() == self.expected_auth.is_workspace_account()
    }

    fn current_auth(&self) -> Option<CodexAuth> {
        self.auth_manager
            .auth_cached()
            .filter(|auth| self.is_expected_auth(auth))
    }
}

impl AuthProvider for AuthManagerAuthProvider {
    fn add_auth_headers(&self, headers: &mut HeaderMap) -> Result<(), AuthError> {
        let auth = self.current_auth().ok_or_else(|| {
            AuthError::Transient("managed authentication is unavailable".to_string())
        })?;
        auth_provider_from_auth(&auth).add_auth_headers(headers)
    }

    fn apply_auth_headers(&self, mut headers: HeaderMap) -> AuthHeadersFuture<'_> {
        Box::pin(async move {
            let auth = self
                .auth_manager
                .auth()
                .await
                .filter(|auth| self.is_expected_auth(auth))
                .ok_or_else(|| {
                    AuthError::Transient("managed authentication is unavailable".to_string())
                })?;
            headers.extend(auth_provider_from_auth(&auth).to_auth_headers()?);
            Ok(headers)
        })
    }

    fn refresh_after_unauthorized(
        &self,
        rejected_headers: HeaderMap,
    ) -> codex_api::AuthRefreshFuture<'_> {
        Box::pin(async move {
            let Some(auth) = self.current_auth() else {
                return false;
            };
            let Ok(headers) = auth_provider_from_auth(&auth).to_auth_headers() else {
                return false;
            };
            if headers.is_empty() {
                return false;
            }
            if headers
                .iter()
                .any(|(name, value)| rejected_headers.get(name) != Some(value))
                && self.auth_manager.refresh_failure_for_auth(&auth).is_none()
            {
                return true;
            }
            if !self.auth_manager.unauthorized_recovery().has_next() {
                return false;
            }
            let result = self.auth_manager.refresh_token_if_unchanged(&auth).await;
            let Some(refreshed_auth) = self.current_auth() else {
                return false;
            };
            if self
                .auth_manager
                .refresh_failure_for_auth(&refreshed_auth)
                .is_some()
            {
                return false;
            }
            let Ok(refreshed_headers) = auth_provider_from_auth(&refreshed_auth).to_auth_headers()
            else {
                return false;
            };
            !refreshed_headers.is_empty()
                && refreshed_headers != headers
                && !matches!(result, Err(codex_login::RefreshTokenError::Policy(_)))
        })
    }
}

// Some providers are meant to send no auth headers. Examples include local OSS
// providers and custom test providers with `requires_openai_auth = false`.
#[derive(Clone, Debug)]
struct UnauthenticatedAuthProvider;

impl AuthProvider for UnauthenticatedAuthProvider {
    fn add_auth_headers(&self, _headers: &mut HeaderMap) -> Result<(), AuthError> {
        Ok(())
    }
}

pub fn unauthenticated_auth_provider() -> SharedAuthProvider {
    Arc::new(UnauthenticatedAuthProvider)
}

/// Returns the provider-scoped auth manager when this provider uses command-backed auth.
///
/// Providers without custom auth continue using the caller-supplied base manager, when present.
pub(crate) fn auth_manager_for_provider(
    auth_manager: Option<Arc<AuthManager>>,
    provider: &ModelProviderInfo,
) -> Option<Arc<AuthManager>> {
    match provider.auth.clone() {
        Some(config) => {
            if provider.is_gcp() {
                let mut env = HashMap::new();
                if let Some(credential_file) = &provider.credential_file {
                    env.insert(
                        "GOOGLE_APPLICATION_CREDENTIALS".to_string(),
                        credential_file.as_os_str().to_os_string(),
                    );
                }
                Some(AuthManager::external_bearer_only_with_env(config, env))
            } else {
                Some(match auth_manager {
                    Some(manager) => crate::shared_state::process_shared_state()
                        .command_credentials(&config, &manager.runtime_config()),
                    None => AuthManager::external_bearer_only(config),
                })
            }
        }
        None => auth_manager,
    }
}

pub(crate) fn resolve_provider_auth(
    auth: Option<&CodexAuth>,
    provider: &ModelProviderInfo,
) -> codex_protocol::error::Result<SharedAuthProvider> {
    if let Some(auth) = bearer_auth_for_provider(provider)? {
        return Ok(Arc::new(auth));
    }

    if !provider.requires_openai_auth && provider.auth.is_none() {
        return Ok(unauthenticated_auth_provider());
    }

    if matches!(
        auth,
        Some(CodexAuth::BedrockApiKey(_) | CodexAuth::BedrockAccessKeys(_))
    ) {
        return Err(CodexErr::UnsupportedOperation(
            BEDROCK_API_KEY_UNSUPPORTED_MESSAGE.to_string(),
        ));
    }

    Ok(match auth {
        Some(auth) => auth_provider_from_auth(auth),
        None => unauthenticated_auth_provider(),
    })
}

pub(crate) async fn resolve_provider_auth_for_scope(
    auth_manager: Option<Arc<AuthManager>>,
    auth: Option<&CodexAuth>,
    provider: &ModelProviderInfo,
    scope: ProviderAuthScope,
) -> codex_protocol::error::Result<ResolvedProviderAuth> {
    let ProviderAuthScope {
        agent_identity_policy,
        session_source,
        agent_identity_session_fallback,
    } = scope;
    if let Some(CodexAuth::AgentIdentity(agent_identity_auth)) = auth {
        return Ok(ResolvedProviderAuth::for_agent_identity(
            agent_identity_auth.clone(),
        ));
    }

    if !should_bootstrap_chatgpt_agent_identity(agent_identity_policy, auth)
        || agent_identity_session_fallback.is_engaged()
    {
        return resolve_provider_auth(auth, provider).map(ResolvedProviderAuth::new);
    }

    let Some(auth_manager) = auth_manager else {
        return resolve_provider_auth(auth, provider).map(ResolvedProviderAuth::new);
    };

    match auth_manager
        .agent_identity_auth(agent_identity_policy, session_source)
        .await
    {
        Ok(Some(agent_identity_auth)) => Ok(ResolvedProviderAuth::for_agent_identity(
            agent_identity_auth,
        )),
        Ok(None) => resolve_provider_auth(auth, provider).map(ResolvedProviderAuth::new),
        Err(err) => {
            if let Some(AgentIdentityAuthError::BootstrapUnavailable {
                operation,
                attempts,
                message,
            }) = err
                .get_ref()
                .and_then(|source| source.downcast_ref::<AgentIdentityAuthError>())
            {
                let newly_engaged = agent_identity_session_fallback.engage();
                tracing::warn!(
                    operation,
                    attempts = *attempts,
                    error = %message,
                    newly_engaged,
                    "agent identity bootstrap unavailable; using ChatGPT bearer auth for this session"
                );
                resolve_provider_auth(auth, provider).map(ResolvedProviderAuth::new)
            } else {
                Err(err.into())
            }
        }
    }
}

fn should_bootstrap_chatgpt_agent_identity(
    agent_identity_policy: AgentIdentityAuthPolicy,
    auth: Option<&CodexAuth>,
) -> bool {
    agent_identity_policy == AgentIdentityAuthPolicy::ChatGptAuth
        && matches!(auth, Some(CodexAuth::Chatgpt(_)))
}

pub(crate) fn bearer_auth_for_provider(
    provider: &ModelProviderInfo,
) -> codex_protocol::error::Result<Option<BearerAuthProvider>> {
    if let Some(api_key) = provider.api_key()? {
        return Ok(Some(BearerAuthProvider::new(api_key)));
    }

    if let Some(token) = provider.experimental_bearer_token.clone() {
        return Ok(Some(BearerAuthProvider::new(token.into_inner())));
    }

    Ok(None)
}

/// Whether the selected API key enables OpenAI's default FedRAMP endpoints.
/// Command-backed providers retain their configured routing across token refreshes.
pub(crate) fn uses_fedramp_api_key(
    auth: Option<&CodexAuth>,
    provider: &ModelProviderInfo,
) -> codex_protocol::error::Result<bool> {
    if !provider.is_openai() || provider.has_command_auth() {
        return Ok(false);
    }
    if let Some(auth) = bearer_auth_for_provider(provider)? {
        return Ok(auth.is_fedramp_account);
    }
    Ok(provider.requires_openai_auth
        && matches!(auth, Some(CodexAuth::ApiKey(_)))
        && auth.is_some_and(CodexAuth::is_fedramp_account))
}

/// Select the shared runtime without changing the identity of captured request data.
pub async fn auth_provider_for_request(
    auth: &CodexAuth,
    runtime: Option<&(Arc<RuntimeAuthManager>, RuntimeAuthMode)>,
    headers: &HeaderMap,
) -> Result<ResolvedProviderAuth, AuthError> {
    observe_auth_selection(runtime.map(|(_, mode)| *mode), "request", async {
        match runtime {
            Some((manager, mode)) => {
                let manager = Arc::clone(manager);
                let auth = auth.clone();
                let selection = crate::runtime_auth::resource_selection(headers)?;
                let selected = crate::runtime_auth::finish_runtime_selection(async move {
                    manager.auth_for(&auth, &selection).await
                })
                .await?;
                Ok(ResolvedProviderAuth::for_runtime(selected, *mode))
            }
            None => {
                let mut resolved = ResolvedProviderAuth::new(auth_provider_from_auth(auth));
                resolved.credential = Some(auth.clone());
                Ok(resolved)
            }
        }
    })
    .await
}

/// Record bounded auth-mode and outcome labels without changing the selection result.
pub(crate) async fn observe_auth_selection<T, E>(
    mode: Option<RuntimeAuthMode>,
    operation: &'static str,
    selection: impl Future<Output = Result<T, E>>,
) -> Result<T, E> {
    let started = std::time::Instant::now();
    let result = selection.await;
    let mode = match mode {
        None => "ordinary",
        Some(RuntimeAuthMode::DualHeader) => "dual_header",
        Some(RuntimeAuthMode::AssertionOnly) => "assertion_only",
    };
    let _ = codex_otel::record_global_operation(
        "codex.auth.selection",
        "codex.auth.selection.duration",
        started.elapsed(),
        &[
            ("mode", mode),
            ("operation", operation),
            ("outcome", if result.is_ok() { "success" } else { "error" }),
        ],
    );
    result
}

/// Builds request-header auth for a first-party Codex credential.
pub fn auth_provider_from_auth(auth: &CodexAuth) -> SharedAuthProvider {
    match auth {
        CodexAuth::AgentIdentity(auth) => {
            Arc::new(AgentIdentityAuthProvider { auth: auth.clone() })
        }
        CodexAuth::Headers(auth) => Arc::new(HeaderAuthProvider { auth: auth.clone() }),
        CodexAuth::BedrockApiKey(_) | CodexAuth::BedrockAccessKeys(_) => {
            unreachable!("{BEDROCK_API_KEY_UNSUPPORTED_MESSAGE}")
        }
        CodexAuth::ApiKey(_)
        | CodexAuth::Chatgpt(_)
        | CodexAuth::ChatgptAuthTokens(_)
        | CodexAuth::PersonalAccessToken(_) => Arc::new(BearerAuthProvider {
            token: auth.get_token().ok(),
            account_id: auth.get_account_id(),
            is_fedramp_account: auth.is_fedramp_account(),
        }),
    }
}

/// Builds request-header auth that reads the current managed auth snapshot on
/// every request while remaining scoped to the expected auth identity.
///
/// Callers with account-scoped state should pass the same snapshot that keyed
/// that state so a later account switch cannot reuse it.
pub fn auth_provider_from_auth_manager(
    auth_manager: Arc<AuthManager>,
    expected_auth: &CodexAuth,
) -> SharedAuthProvider {
    Arc::new(AuthManagerAuthProvider {
        auth_manager,
        expected_auth: expected_auth.clone(),
    })
}

#[cfg(test)]
#[path = "auth_refresh_tests.rs"]
mod auth_refresh_tests;

#[cfg(test)]
#[path = "auth_tests.rs"]
mod tests;
