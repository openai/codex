use chrono::Utc;
use http::StatusCode;
use serde::Deserialize;
use serde::Serialize;
#[cfg(test)]
use serial_test::serial;
use std::env;
use std::fmt::Debug;
use std::future::Future;
use std::path::Path;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::RwLock;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;
use tokio::sync::Semaphore;
use tokio::sync::watch;
use tracing::instrument;

use codex_agent_identity::ChatGptEnvironment;
use codex_protocol::auth::AuthMode;
use codex_protocol::config_types::ForcedLoginMethod;
use codex_protocol::config_types::ModelProviderAuthInfo;

use super::access_token::CodexAccessToken;
use super::access_token::classify_codex_access_token;
use super::agent_identity::ManagedChatGptAgentIdentityBinding;
use super::agent_identity::agent_identity_authapi_base_url;
use super::agent_identity::classify_bootstrap_error;
use super::agent_identity::record_matches_managed_chatgpt_binding;
use super::agent_identity::record_needs_task_registration;
use super::agent_identity::register_managed_chatgpt_agent_identity;
use super::agent_identity::require_agent_identity_authapi_base_url;
use super::agent_identity::verified_record_from_jwt;
use super::external_bearer::BearerTokenRefresher;
use super::revoke::revoke_auth_tokens;
use crate::auth::AuthHeaders;
pub use crate::auth::agent_identity::AgentIdentityAuth;
pub use crate::auth::agent_identity::AgentIdentityAuthError;
pub use crate::auth::bedrock_api_key::BedrockApiKeyAuth;
pub use crate::auth::personal_access_token::PersonalAccessTokenAuth;
pub use crate::auth::storage::AgentIdentityAuthRecord;
pub use crate::auth::storage::AgentIdentityStorage;
pub use crate::auth::storage::AuthDotJson;
pub use crate::auth::storage::AuthKeyringBackendKind;
use crate::auth::storage::AuthStorageBackend;
use crate::auth::storage::create_auth_storage;
use crate::auth::util::try_parse_error_message;
use crate::default_client::create_client;
use crate::default_client::create_default_auth_client;
use crate::outbound_proxy::AuthRouteConfig;
use crate::token_data::TokenData;
use crate::token_data::parse_chatgpt_jwt_claims;
use crate::token_data::parse_jwt_expiration;
use codex_config::types::AuthCredentialsStoreMode;
use codex_http_client::HttpClient;
use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use codex_protocol::account::PlanType as AccountPlanType;
use codex_protocol::auth::PlanType as InternalPlanType;
use codex_protocol::auth::RefreshTokenFailedError;
use codex_protocol::auth::RefreshTokenFailedReason;
use codex_protocol::protocol::SessionSource;
use serde_json::Value;
use thiserror::Error;

/// Authentication mechanism used by the current user.
#[derive(Debug, Clone)]
pub enum CodexAuth {
    ApiKey(ApiKeyAuth),
    Chatgpt(ChatgptAuth),
    ChatgptAuthTokens(ChatgptAuthTokens),
    Headers(AuthHeaders),
    AgentIdentity(AgentIdentityAuth),
    PersonalAccessToken(PersonalAccessTokenAuth),
    BedrockApiKey(BedrockApiKeyAuth),
}

/// Policy for resolving Agent Identity auth from a broader Codex auth snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentIdentityAuthPolicy {
    /// Use Agent Identity auth only when the current auth is already Agent Identity.
    JwtOnly,
    /// Allow managed ChatGPT auth to register or reuse Agent Identity auth.
    ChatGptAuth,
}

const AGENT_IDENTITY_BOOTSTRAP_FAILURE_COOLDOWN: Duration = Duration::from_secs(60 * 60);

#[derive(Debug)]
struct CachedAgentIdentityBootstrapFailure {
    account_id: String,
    authapi_base_url: String,
    retry_at: Instant,
    error: AgentIdentityAuthError,
}

#[derive(Debug, Default)]
struct AgentIdentityBootstrapCooldown {
    failure: Option<CachedAgentIdentityBootstrapFailure>,
}

impl AgentIdentityBootstrapCooldown {
    fn error_for(
        &mut self,
        account_id: &str,
        authapi_base_url: &str,
        now: Instant,
    ) -> Option<AgentIdentityAuthError> {
        let error = self
            .failure
            .as_ref()
            .filter(|failure| {
                failure.account_id == account_id
                    && failure.authapi_base_url == authapi_base_url
                    && failure.retry_at > now
            })
            .map(|failure| failure.error.clone());
        if error.is_none() {
            self.clear();
        }
        error
    }

    fn record_failure(
        &mut self,
        account_id: String,
        authapi_base_url: String,
        error: AgentIdentityAuthError,
        now: Instant,
    ) {
        self.failure = Some(CachedAgentIdentityBootstrapFailure {
            account_id,
            authapi_base_url,
            retry_at: now + AGENT_IDENTITY_BOOTSTRAP_FAILURE_COOLDOWN,
            error,
        });
    }

    fn clear(&mut self) {
        self.failure = None;
    }
}

impl PartialEq for CodexAuth {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Headers(a), Self::Headers(b)) => a == b,
            (Self::PersonalAccessToken(a), Self::PersonalAccessToken(b)) => a == b,
            (Self::BedrockApiKey(a), Self::BedrockApiKey(b)) => a == b,
            _ => self.api_auth_mode() == other.api_auth_mode(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ApiKeyAuth {
    api_key: String,
}

#[derive(Debug, Clone)]
pub struct ChatgptAuth {
    state: ChatgptAuthState,
    storage: Arc<dyn AuthStorageBackend>,
}

#[derive(Debug, Clone)]
pub struct ChatgptAuthTokens {
    state: ChatgptAuthState,
}

#[derive(Debug, Clone)]
struct ChatgptAuthState {
    auth_dot_json: Arc<Mutex<Option<AuthDotJson>>>,
    client: HttpClient,
}

const TOKEN_REFRESH_INTERVAL: i64 = 8;
const CHATGPT_ACCESS_TOKEN_REFRESH_WINDOW_MINUTES: i64 = 5;

const REFRESH_TOKEN_EXPIRED_MESSAGE: &str = "Your access token could not be refreshed because your refresh token has expired. Please log out and sign in again.";
const REFRESH_TOKEN_REUSED_MESSAGE: &str = "Your access token could not be refreshed because your refresh token was already used. Please log out and sign in again.";
const REFRESH_TOKEN_INVALIDATED_MESSAGE: &str = "Your access token could not be refreshed because your refresh token was revoked. Please log out and sign in again.";
const REFRESH_TOKEN_UNKNOWN_MESSAGE: &str =
    "Your access token could not be refreshed. Please log out and sign in again.";
const REFRESH_TOKEN_ACCOUNT_MISMATCH_MESSAGE: &str = "Your access token could not be refreshed because you have since logged out or signed in to another account. Please sign in again.";
const REFRESH_TOKEN_URL: &str = "https://auth.openai.com/oauth/token";
pub(super) const REVOKE_TOKEN_URL: &str = "https://auth.openai.com/oauth/revoke";
pub const REFRESH_TOKEN_URL_OVERRIDE_ENV_VAR: &str = "CODEX_REFRESH_TOKEN_URL_OVERRIDE";
pub const REVOKE_TOKEN_URL_OVERRIDE_ENV_VAR: &str = "CODEX_REVOKE_TOKEN_URL_OVERRIDE";
pub const CLIENT_ID_OVERRIDE_ENV_VAR: &str = "CODEX_APP_SERVER_LOGIN_CLIENT_ID";
static NEXT_DUMMY_AUTH_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Error)]
pub enum RefreshTokenError {
    #[error("{0}")]
    Permanent(#[from] RefreshTokenFailedError),
    #[error(transparent)]
    Transient(#[from] std::io::Error),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExternalAuthRefreshReason {
    Unauthorized,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExternalAuthRefreshContext {
    pub reason: ExternalAuthRefreshReason,
    pub previous_account_id: Option<String>,
}

/// Pluggable auth provider used by `AuthManager` for externally managed auth flows.
///
/// Implementations own the current auth value and any source-specific refresh mechanism.
pub trait ExternalAuth: Send + Sync {
    /// Returns the provider's current auth value.
    fn resolve(&self) -> ExternalAuthFuture<'_, CodexAuth>;

    /// Refreshes auth and makes the returned value current for future `resolve()` calls.
    fn refresh(&self, context: ExternalAuthRefreshContext) -> ExternalAuthFuture<'_, CodexAuth>;
}

pub type ExternalAuthFuture<'a, T> = Pin<Box<dyn Future<Output = std::io::Result<T>> + Send + 'a>>;

impl RefreshTokenError {
    pub fn failed_reason(&self) -> Option<RefreshTokenFailedReason> {
        match self {
            Self::Permanent(error) => Some(error.reason),
            Self::Transient(_) => None,
        }
    }
}

impl From<RefreshTokenError> for std::io::Error {
    fn from(err: RefreshTokenError) -> Self {
        match err {
            RefreshTokenError::Permanent(failed) => std::io::Error::other(failed),
            RefreshTokenError::Transient(inner) => inner,
        }
    }
}

impl CodexAuth {
    async fn from_auth_dot_json(
        codex_home: &Path,
        auth_dot_json: AuthDotJson,
        auth_credentials_store_mode: AuthCredentialsStoreMode,
        chatgpt_base_url: Option<&str>,
        keyring_backend_kind: AuthKeyringBackendKind,
        agent_identity_authapi_base_url: Option<&str>,
        auth_route_config: &AuthRouteConfig,
    ) -> std::io::Result<Self> {
        let auth_mode = auth_dot_json.resolved_mode();
        if auth_mode == AuthMode::ApiKey {
            let Some(api_key) = auth_dot_json.openai_api_key.as_deref() else {
                return Err(std::io::Error::other("API key auth is missing a key."));
            };
            return Ok(Self::from_api_key(api_key));
        }
        if auth_mode == AuthMode::AgentIdentity {
            let Some(agent_identity) = auth_dot_json.agent_identity.clone() else {
                return Err(std::io::Error::other(
                    "agent identity auth is missing agent identity auth material.",
                ));
            };
            let base_url = chatgpt_base_url
                .unwrap_or(ChatGptEnvironment::default().chatgpt_base_url())
                .trim_end_matches('/')
                .to_string();
            let agent_identity_authapi_base_url =
                require_agent_identity_authapi_base_url(agent_identity_authapi_base_url)?;
            match agent_identity {
                AgentIdentityStorage::Jwt(jwt) => {
                    let auth = AgentIdentityAuth::from_jwt(
                        &jwt,
                        &base_url,
                        agent_identity_authapi_base_url,
                        auth_route_config,
                    )
                    .await?;
                    return Ok(Self::AgentIdentity(auth));
                }
                AgentIdentityStorage::Record(record) => {
                    let auth = AgentIdentityAuth::from_record(
                        record,
                        agent_identity_authapi_base_url,
                        auth_route_config,
                    )
                    .await?;
                    return Ok(Self::AgentIdentity(auth));
                }
            }
        }
        if auth_mode == AuthMode::PersonalAccessToken {
            let Some(personal_access_token) = auth_dot_json.personal_access_token.as_deref() else {
                return Err(std::io::Error::other(
                    "personal access token auth is missing a personal access token.",
                ));
            };
            return Self::from_personal_access_token(personal_access_token, auth_route_config)
                .await;
        }
        if auth_mode == AuthMode::BedrockApiKey {
            let Some(auth) = auth_dot_json.bedrock_api_key else {
                return Err(std::io::Error::other(
                    "Bedrock API key auth is missing a Bedrock API key.",
                ));
            };
            return Ok(Self::BedrockApiKey(auth));
        }
        if auth_mode == AuthMode::Headers {
            return Err(std::io::Error::other(
                "externally provided auth cannot be loaded from auth storage.",
            ));
        }

        let storage_mode = auth_dot_json.storage_mode(auth_credentials_store_mode);
        let client = create_default_auth_client(&refresh_token_endpoint(), auth_route_config)?;
        let state = ChatgptAuthState {
            auth_dot_json: Arc::new(Mutex::new(Some(auth_dot_json))),
            client,
        };

        match auth_mode {
            AuthMode::Chatgpt => {
                let storage = create_auth_storage(
                    codex_home.to_path_buf(),
                    storage_mode,
                    keyring_backend_kind,
                );
                Ok(Self::Chatgpt(ChatgptAuth { state, storage }))
            }
            AuthMode::ChatgptAuthTokens => Ok(Self::ChatgptAuthTokens(ChatgptAuthTokens { state })),
            AuthMode::ApiKey => unreachable!("api key mode is handled above"),
            AuthMode::Headers => {
                unreachable!("externally provided auth is never loaded from auth storage")
            }
            AuthMode::AgentIdentity => unreachable!("agent identity mode is handled above"),
            AuthMode::PersonalAccessToken => {
                unreachable!("personal access token mode is handled above")
            }
            AuthMode::BedrockApiKey => unreachable!("bedrock api key mode is handled above"),
        }
    }

    pub async fn from_auth_storage(
        codex_home: &Path,
        auth_credentials_store_mode: AuthCredentialsStoreMode,
        chatgpt_base_url: Option<&str>,
        keyring_backend_kind: AuthKeyringBackendKind,
        auth_route_config: &AuthRouteConfig,
    ) -> std::io::Result<Option<Self>> {
        let agent_identity_authapi_base_url =
            agent_identity_authapi_base_url(chatgpt_base_url).ok();
        load_auth(
            codex_home,
            /*enable_codex_api_key_env*/ false,
            auth_credentials_store_mode,
            /*forced_chatgpt_workspace_id*/ None,
            chatgpt_base_url,
            keyring_backend_kind,
            agent_identity_authapi_base_url.as_deref(),
            auth_route_config,
        )
        .await
    }

    pub async fn from_agent_identity_jwt(
        jwt: &str,
        chatgpt_base_url: Option<&str>,
        auth_route_config: &AuthRouteConfig,
    ) -> std::io::Result<Self> {
        let agent_identity_authapi_base_url = agent_identity_authapi_base_url(chatgpt_base_url)?;
        Self::from_agent_identity_jwt_with_authapi_base_url(
            jwt,
            chatgpt_base_url,
            &agent_identity_authapi_base_url,
            auth_route_config,
        )
        .await
    }

    async fn from_agent_identity_jwt_with_authapi_base_url(
        jwt: &str,
        chatgpt_base_url: Option<&str>,
        agent_identity_authapi_base_url: &str,
        auth_route_config: &AuthRouteConfig,
    ) -> std::io::Result<Self> {
        let base_url = chatgpt_base_url
            .unwrap_or(ChatGptEnvironment::default().chatgpt_base_url())
            .trim_end_matches('/')
            .to_string();
        Ok(Self::AgentIdentity(
            AgentIdentityAuth::from_jwt(
                jwt,
                &base_url,
                agent_identity_authapi_base_url,
                auth_route_config,
            )
            .await?,
        ))
    }

    pub async fn from_personal_access_token(
        access_token: &str,
        auth_route_config: &AuthRouteConfig,
    ) -> std::io::Result<Self> {
        Ok(Self::PersonalAccessToken(
            PersonalAccessTokenAuth::load(access_token, auth_route_config).await?,
        ))
    }

    /// Returns the effective backend auth mode.
    ///
    /// Externally managed ChatGPT tokens are normalized to [`AuthMode::Chatgpt`].
    pub fn auth_mode(&self) -> AuthMode {
        match self {
            Self::ApiKey(_) => AuthMode::ApiKey,
            Self::Chatgpt(_) | Self::ChatgptAuthTokens(_) => AuthMode::Chatgpt,
            Self::Headers(_) => AuthMode::Headers,
            Self::AgentIdentity(_) => AuthMode::AgentIdentity,
            Self::PersonalAccessToken(_) => AuthMode::PersonalAccessToken,
            Self::BedrockApiKey(_) => AuthMode::BedrockApiKey,
        }
    }

    /// Returns the precise kind of credentials backing this authentication.
    pub fn api_auth_mode(&self) -> AuthMode {
        match self {
            Self::ApiKey(_) => AuthMode::ApiKey,
            Self::Chatgpt(_) => AuthMode::Chatgpt,
            Self::ChatgptAuthTokens(_) => AuthMode::ChatgptAuthTokens,
            Self::Headers(_) => AuthMode::Headers,
            Self::AgentIdentity(_) => AuthMode::AgentIdentity,
            Self::PersonalAccessToken(_) => AuthMode::PersonalAccessToken,
            Self::BedrockApiKey(_) => AuthMode::BedrockApiKey,
        }
    }

    pub fn is_api_key_auth(&self) -> bool {
        self.auth_mode() == AuthMode::ApiKey
    }

    pub fn is_personal_access_token_auth(&self) -> bool {
        self.auth_mode() == AuthMode::PersonalAccessToken
    }

    pub fn is_chatgpt_auth(&self) -> bool {
        self.api_auth_mode().has_chatgpt_account()
    }

    pub fn uses_codex_backend(&self) -> bool {
        self.api_auth_mode().uses_codex_backend()
    }

    pub fn is_external_chatgpt_tokens(&self) -> bool {
        matches!(self, Self::ChatgptAuthTokens(_))
    }

    fn supports_unauthorized_recovery(&self) -> bool {
        matches!(
            self,
            Self::Chatgpt(_) | Self::ChatgptAuthTokens(_) | Self::Headers(_)
        )
    }

    /// Returns `None` if `auth_mode() != AuthMode::ApiKey`.
    pub fn api_key(&self) -> Option<&str> {
        match self {
            Self::ApiKey(auth) => Some(auth.api_key.as_str()),
            Self::Chatgpt(_)
            | Self::ChatgptAuthTokens(_)
            | Self::Headers(_)
            | Self::AgentIdentity(_)
            | Self::PersonalAccessToken(_)
            | Self::BedrockApiKey(_) => None,
        }
    }

    /// Returns `Err` if token-backed ChatGPT auth is unavailable.
    pub fn get_token_data(&self) -> Result<TokenData, std::io::Error> {
        let auth_dot_json: Option<AuthDotJson> = self.get_current_auth_json();
        match auth_dot_json {
            Some(AuthDotJson {
                tokens: Some(tokens),
                last_refresh: Some(_),
                ..
            }) => Ok(tokens),
            _ => Err(std::io::Error::other("Token data is not available.")),
        }
    }

    /// Returns the token string used for bearer authentication.
    pub fn get_token(&self) -> Result<String, std::io::Error> {
        match self {
            Self::ApiKey(auth) => Ok(auth.api_key.clone()),
            Self::Chatgpt(_) | Self::ChatgptAuthTokens(_) => {
                let access_token = self.get_token_data()?.access_token;
                Ok(access_token)
            }
            Self::AgentIdentity(_) => Err(std::io::Error::other(
                "agent identity auth does not expose a bearer token",
            )),
            Self::Headers(_) => Err(std::io::Error::other(
                "header auth does not expose a bearer token",
            )),
            Self::PersonalAccessToken(auth) => Ok(auth.access_token().to_string()),
            Self::BedrockApiKey(_) => Err(std::io::Error::other(
                "Bedrock API key auth does not expose a Codex bearer token",
            )),
        }
    }

    /// Returns `None` if Codex backend auth does not expose an account id.
    pub fn get_account_id(&self) -> Option<String> {
        match self {
            Self::Headers(_) => None,
            Self::AgentIdentity(auth) => Some(auth.account_id().to_string()),
            Self::PersonalAccessToken(auth) => Some(auth.account_id().to_string()),
            _ => self.get_current_token_data().and_then(|t| t.account_id),
        }
    }

    /// Returns false if Codex backend auth omits the FedRAMP claim.
    pub fn is_fedramp_account(&self) -> bool {
        match self {
            Self::Headers(_) => false,
            Self::AgentIdentity(auth) => auth.is_fedramp_account(),
            Self::PersonalAccessToken(auth) => auth.is_fedramp_account(),
            _ => self
                .get_current_token_data()
                .is_some_and(|t| t.id_token.is_fedramp_account()),
        }
    }

    /// Returns `None` if Codex backend auth does not expose an account email.
    pub fn get_account_email(&self) -> Option<String> {
        match self {
            Self::Headers(_) => None,
            Self::AgentIdentity(auth) => auth.email().map(str::to_string),
            Self::PersonalAccessToken(auth) => auth.email().map(str::to_string),
            _ => self.get_current_token_data().and_then(|t| t.id_token.email),
        }
    }

    /// Returns `None` if Codex backend auth does not expose a ChatGPT user id.
    pub fn get_chatgpt_user_id(&self) -> Option<String> {
        match self {
            Self::Headers(_) => None,
            Self::AgentIdentity(auth) => Some(auth.chatgpt_user_id().to_string()),
            Self::PersonalAccessToken(auth) => Some(auth.chatgpt_user_id().to_string()),
            _ => self
                .get_current_token_data()
                .and_then(|t| t.id_token.chatgpt_user_id),
        }
    }

    /// Account-facing plan classification derived from the current auth.
    /// Returns a high-level `AccountPlanType` (e.g., Free/Plus/Pro/Team/…)
    /// for UI or product decisions based on the user's subscription.
    pub fn account_plan_type(&self) -> Option<AccountPlanType> {
        if matches!(self, Self::Headers(_)) {
            return None;
        }
        if let Self::AgentIdentity(auth) = self {
            return Some(auth.plan_type());
        }
        if let Self::PersonalAccessToken(auth) = self {
            return Some(auth.plan_type());
        }

        self.get_current_token_data().map(|t| {
            t.id_token
                .chatgpt_plan_type
                .map(AccountPlanType::from)
                .unwrap_or(AccountPlanType::Unknown)
        })
    }

    pub fn is_workspace_account(&self) -> bool {
        self.account_plan_type()
            .is_some_and(AccountPlanType::is_workspace_account)
    }

    /// Returns `None` if token-backed ChatGPT auth is unavailable.
    fn get_current_auth_json(&self) -> Option<AuthDotJson> {
        let state = match self {
            Self::Chatgpt(auth) => &auth.state,
            Self::ChatgptAuthTokens(auth) => &auth.state,
            Self::ApiKey(_)
            | Self::Headers(_)
            | Self::AgentIdentity(_)
            | Self::PersonalAccessToken(_)
            | Self::BedrockApiKey(_) => return None,
        };
        #[expect(clippy::unwrap_used)]
        state.auth_dot_json.lock().unwrap().clone()
    }

    /// Returns `None` if token-backed ChatGPT auth is unavailable.
    fn get_current_token_data(&self) -> Option<TokenData> {
        self.get_current_auth_json().and_then(|t| t.tokens)
    }

    fn stored_managed_chatgpt_agent_identity_record(
        &self,
        account_id: &str,
    ) -> Option<AgentIdentityAuthRecord> {
        self.get_current_auth_json()
            .and_then(|auth| auth.agent_identity)
            .and_then(|identity| identity.as_record().cloned())
            .filter(|identity| identity.account_id == account_id)
    }

    fn persist_managed_chatgpt_agent_identity_record(
        &self,
        record: AgentIdentityAuthRecord,
    ) -> std::io::Result<()> {
        if let Self::Chatgpt(chatgpt_auth) = self {
            chatgpt_auth.persist_agent_identity_record(record)?;
        }
        Ok(())
    }

    async fn agent_identity_auth(
        &self,
        policy: AgentIdentityAuthPolicy,
        agent_identity_authapi_base_url: Option<&str>,
        forced_chatgpt_workspace_id: Option<Vec<String>>,
        auth_route_config: &AuthRouteConfig,
        session_source: SessionSource,
    ) -> std::io::Result<Option<AgentIdentityAuth>> {
        match self {
            Self::AgentIdentity(auth) => Ok(Some(auth.clone())),
            Self::ApiKey(_)
            | Self::ChatgptAuthTokens(_)
            | Self::Headers(_)
            | Self::PersonalAccessToken(_)
            | Self::BedrockApiKey(_) => Ok(None),
            Self::Chatgpt(_) => {
                if policy == AgentIdentityAuthPolicy::JwtOnly {
                    return Ok(None);
                }
                self.ensure_managed_chatgpt_agent_identity(
                    require_agent_identity_authapi_base_url(agent_identity_authapi_base_url)?,
                    forced_chatgpt_workspace_id,
                    auth_route_config,
                    session_source,
                )
                .await
                .map(Some)
            }
        }
    }

    async fn ensure_managed_chatgpt_agent_identity(
        &self,
        agent_identity_authapi_base_url: &str,
        forced_chatgpt_workspace_id: Option<Vec<String>>,
        auth_route_config: &AuthRouteConfig,
        session_source: SessionSource,
    ) -> std::io::Result<AgentIdentityAuth> {
        let binding =
            ManagedChatGptAgentIdentityBinding::from_auth(self, forced_chatgpt_workspace_id)
                .ok_or_else(|| std::io::Error::other("ChatGPT auth is unavailable"))?;

        // JWT auth is loaded as CodexAuth::AgentIdentity; this path only reuses
        // records created by the managed ChatGPT Agent Identity bootstrap.
        if let Some(record) = self.stored_managed_chatgpt_agent_identity_record(&binding.account_id)
            && record_matches_managed_chatgpt_binding(&record, &binding)
        {
            let should_persist = record_needs_task_registration(&record);
            let auth = AgentIdentityAuth::from_record(
                record,
                agent_identity_authapi_base_url,
                auth_route_config,
            )
            .await
            .map_err(|err| classify_bootstrap_error("agent task registration", err))?;
            if should_persist {
                self.persist_managed_chatgpt_agent_identity_record(auth.record().clone())?;
            }
            return Ok(auth);
        }

        let auth = register_managed_chatgpt_agent_identity(
            binding,
            agent_identity_authapi_base_url,
            session_source,
            auth_route_config,
        )
        .await?;
        self.persist_managed_chatgpt_agent_identity_record(auth.record().clone())?;
        Ok(auth)
    }

    /// Consider this private to integration tests.
    pub fn create_dummy_chatgpt_auth_for_testing() -> Self {
        let auth_dot_json = AuthDotJson {
            auth_mode: Some(AuthMode::Chatgpt),
            openai_api_key: None,
            tokens: Some(TokenData {
                id_token: Default::default(),
                access_token: "Access Token".to_string(),
                refresh_token: "test".to_string(),
                account_id: Some("account_id".to_string()),
            }),
            last_refresh: Some(Utc::now()),
            agent_identity: None,
            personal_access_token: None,
            bedrock_api_key: None,
        };

        let state = ChatgptAuthState {
            auth_dot_json: Arc::new(Mutex::new(Some(auth_dot_json))),
            client: create_client(),
        };
        let dummy_auth_id = NEXT_DUMMY_AUTH_ID.fetch_add(1, Ordering::Relaxed);
        let storage = create_auth_storage(
            PathBuf::from(format!("dummy-chatgpt-auth-{dummy_auth_id}")),
            AuthCredentialsStoreMode::Ephemeral,
            AuthKeyringBackendKind::default(),
        );
        Self::Chatgpt(ChatgptAuth { state, storage })
    }

    /// Constructs in-memory ChatGPT auth from externally managed tokens.
    pub fn from_external_chatgpt_tokens(
        access_token: &str,
        chatgpt_account_id: &str,
        chatgpt_plan_type: Option<&str>,
    ) -> std::io::Result<Self> {
        let auth_dot_json = AuthDotJson::from_external_access_token(
            access_token,
            chatgpt_account_id,
            chatgpt_plan_type,
        )?;
        let state = ChatgptAuthState {
            auth_dot_json: Arc::new(Mutex::new(Some(auth_dot_json))),
            client: create_client(),
        };
        Ok(Self::ChatgptAuthTokens(ChatgptAuthTokens { state }))
    }

    pub fn from_api_key(api_key: &str) -> Self {
        Self::ApiKey(ApiKeyAuth {
            api_key: api_key.to_owned(),
        })
    }
}

impl ManagedChatGptAgentIdentityBinding {
    fn from_auth(auth: &CodexAuth, forced_workspace_id: Option<Vec<String>>) -> Option<Self> {
        if !auth.is_chatgpt_auth() {
            return None;
        }

        let token_data = auth.get_token_data().ok()?;
        let forced_workspace_id =
            forced_workspace_id
                .as_deref()
                .and_then(|workspace_ids| match workspace_ids {
                    [workspace_id] if !workspace_id.is_empty() => Some(workspace_id.clone()),
                    _ => None,
                });
        let account_id = forced_workspace_id
            .or(token_data
                .account_id
                .clone()
                .filter(|value| !value.is_empty()))
            .or(token_data.id_token.chatgpt_account_id.clone())?;
        let chatgpt_user_id = token_data
            .id_token
            .chatgpt_user_id
            .clone()
            .filter(|value| !value.is_empty())?;

        Some(Self {
            account_id,
            chatgpt_user_id,
            email: token_data.id_token.email.clone(),
            plan_type: auth.account_plan_type().unwrap_or(AccountPlanType::Unknown),
            chatgpt_account_is_fedramp: auth.is_fedramp_account(),
            access_token: token_data.access_token,
        })
    }
}

impl ChatgptAuth {
    fn current_auth_json(&self) -> Option<AuthDotJson> {
        #[expect(clippy::unwrap_used)]
        self.state.auth_dot_json.lock().unwrap().clone()
    }

    fn current_token_data(&self) -> Option<TokenData> {
        self.current_auth_json().and_then(|auth| auth.tokens)
    }

    fn storage(&self) -> &Arc<dyn AuthStorageBackend> {
        &self.storage
    }

    fn client(&self) -> &HttpClient {
        &self.state.client
    }

    fn persist_agent_identity_record(
        &self,
        record: AgentIdentityAuthRecord,
    ) -> std::io::Result<()> {
        persist_agent_identity_record(&self.state.auth_dot_json, &self.storage, record)
    }
}

fn persist_agent_identity_record(
    auth_dot_json: &Arc<Mutex<Option<AuthDotJson>>>,
    storage: &Arc<dyn AuthStorageBackend>,
    record: AgentIdentityAuthRecord,
) -> std::io::Result<()> {
    let mut guard = auth_dot_json
        .lock()
        .map_err(|_| std::io::Error::other("failed to lock auth state"))?;
    let mut auth = storage
        .load()?
        .or_else(|| guard.clone())
        .ok_or_else(|| std::io::Error::other("auth data is not available"))?;
    auth.agent_identity = Some(AgentIdentityStorage::Record(record));
    storage.save(&auth)?;
    *guard = Some(auth);
    Ok(())
}

pub const OPENAI_API_KEY_ENV_VAR: &str = "OPENAI_API_KEY";
pub const CODEX_API_KEY_ENV_VAR: &str = "CODEX_API_KEY";
pub const CODEX_ACCESS_TOKEN_ENV_VAR: &str = "CODEX_ACCESS_TOKEN";

pub fn read_openai_api_key_from_env() -> Option<String> {
    env::var(OPENAI_API_KEY_ENV_VAR)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub fn read_codex_api_key_from_env() -> Option<String> {
    read_non_empty_env_var(CODEX_API_KEY_ENV_VAR)
}

pub fn read_codex_access_token_from_env() -> Option<String> {
    read_non_empty_env_var(CODEX_ACCESS_TOKEN_ENV_VAR)
}

fn read_non_empty_env_var(key: &str) -> Option<String> {
    env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// Delete the auth.json file inside `codex_home` if it exists. Returns `Ok(true)`
/// if a file was removed, `Ok(false)` if no auth file was present.
pub fn logout(
    codex_home: &Path,
    auth_credentials_store_mode: AuthCredentialsStoreMode,
    keyring_backend_kind: AuthKeyringBackendKind,
) -> std::io::Result<bool> {
    let storage = create_auth_storage(
        codex_home.to_path_buf(),
        auth_credentials_store_mode,
        keyring_backend_kind,
    );
    storage.delete()
}

pub async fn logout_with_revoke(
    codex_home: &Path,
    auth_credentials_store_mode: AuthCredentialsStoreMode,
    keyring_backend_kind: AuthKeyringBackendKind,
    auth_route_config: &AuthRouteConfig,
) -> std::io::Result<bool> {
    let auth_dot_json = match load_auth_dot_json(
        codex_home,
        auth_credentials_store_mode,
        keyring_backend_kind,
    ) {
        Ok(auth_dot_json) => auth_dot_json,
        Err(err) => {
            tracing::warn!("failed to load stored auth during logout: {err}");
            None
        }
    };
    if let Err(err) = revoke_auth_tokens(auth_dot_json.as_ref(), auth_route_config).await {
        tracing::warn!("failed to revoke auth tokens during logout: {err}");
    }
    logout_all_stores(
        codex_home,
        auth_credentials_store_mode,
        keyring_backend_kind,
    )
}

/// Writes an `auth.json` that contains only the API key.
pub fn login_with_api_key(
    codex_home: &Path,
    api_key: &str,
    auth_credentials_store_mode: AuthCredentialsStoreMode,
    keyring_backend_kind: AuthKeyringBackendKind,
) -> std::io::Result<()> {
    let auth_dot_json = AuthDotJson {
        auth_mode: Some(AuthMode::ApiKey),
        openai_api_key: Some(api_key.to_string()),
        tokens: None,
        last_refresh: None,
        agent_identity: None,
        personal_access_token: None,
        bedrock_api_key: None,
    };
    save_auth(
        codex_home,
        &auth_dot_json,
        auth_credentials_store_mode,
        keyring_backend_kind,
    )
}

/// Writes an `auth.json` that contains only the access token.
pub async fn login_with_access_token(
    codex_home: &Path,
    access_token: &str,
    auth_credentials_store_mode: AuthCredentialsStoreMode,
    forced_chatgpt_workspace_id: Option<&[String]>,
    chatgpt_base_url: Option<&str>,
    keyring_backend_kind: AuthKeyringBackendKind,
    auth_route_config: &AuthRouteConfig,
) -> std::io::Result<()> {
    let auth_dot_json = match classify_codex_access_token(access_token) {
        CodexAccessToken::PersonalAccessToken(access_token) => {
            let auth = PersonalAccessTokenAuth::load(access_token, auth_route_config).await?;
            ensure_personal_access_token_workspace_allowed(forced_chatgpt_workspace_id, &auth)?;
            AuthDotJson {
                // Infer PAT auth from the credential field so older Codex builds can still
                // deserialize auth.json after a rollback.
                auth_mode: None,
                openai_api_key: None,
                tokens: None,
                last_refresh: None,
                agent_identity: None,
                personal_access_token: Some(access_token.to_string()),
                bedrock_api_key: None,
            }
        }
        CodexAccessToken::AgentIdentityJwt(jwt) => {
            let base_url = chatgpt_base_url
                .unwrap_or(ChatGptEnvironment::default().chatgpt_base_url())
                .trim_end_matches('/')
                .to_string();
            verified_record_from_jwt(jwt, &base_url, auth_route_config).await?;
            AuthDotJson {
                auth_mode: Some(AuthMode::AgentIdentity),
                openai_api_key: None,
                tokens: None,
                last_refresh: None,
                agent_identity: Some(AgentIdentityStorage::Jwt(jwt.to_string())),
                personal_access_token: None,
                bedrock_api_key: None,
            }
        }
    };
    save_auth(
        codex_home,
        &auth_dot_json,
        auth_credentials_store_mode,
        keyring_backend_kind,
    )
}

fn ensure_personal_access_token_workspace_allowed(
    expected_workspace_ids: Option<&[String]>,
    auth: &PersonalAccessTokenAuth,
) -> std::io::Result<()> {
    crate::server::ensure_workspace_account_allowed(expected_workspace_ids, auth.account_id())
        .map_err(|message| std::io::Error::new(std::io::ErrorKind::PermissionDenied, message))
}

/// Writes an in-memory auth payload for externally managed ChatGPT tokens.
pub fn login_with_chatgpt_auth_tokens(
    codex_home: &Path,
    access_token: &str,
    chatgpt_account_id: &str,
    chatgpt_plan_type: Option<&str>,
) -> std::io::Result<()> {
    let auth_dot_json = AuthDotJson::from_external_access_token(
        access_token,
        chatgpt_account_id,
        chatgpt_plan_type,
    )?;
    save_auth(
        codex_home,
        &auth_dot_json,
        AuthCredentialsStoreMode::Ephemeral,
        AuthKeyringBackendKind::default(),
    )
}

/// Persist the provided auth payload using the specified backend.
pub fn save_auth(
    codex_home: &Path,
    auth: &AuthDotJson,
    auth_credentials_store_mode: AuthCredentialsStoreMode,
    keyring_backend_kind: AuthKeyringBackendKind,
) -> std::io::Result<()> {
    let storage = create_auth_storage(
        codex_home.to_path_buf(),
        auth_credentials_store_mode,
        keyring_backend_kind,
    );
    storage.save(auth)
}

/// Load the raw stored auth payload without applying environment overrides.
///
/// Returns `None` when no credentials are stored. Prefer `AuthManager` for
/// ordinary production reads; this helper is for tests and write-side
/// maintenance that must inspect the exact payload in storage.
pub fn load_auth_dot_json(
    codex_home: &Path,
    auth_credentials_store_mode: AuthCredentialsStoreMode,
    keyring_backend_kind: AuthKeyringBackendKind,
) -> std::io::Result<Option<AuthDotJson>> {
    let storage = create_auth_storage(
        codex_home.to_path_buf(),
        auth_credentials_store_mode,
        keyring_backend_kind,
    );
    storage.load()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthConfig {
    pub codex_home: PathBuf,
    pub auth_credentials_store_mode: AuthCredentialsStoreMode,
    pub keyring_backend_kind: AuthKeyringBackendKind,
    pub forced_login_method: Option<ForcedLoginMethod>,
    pub chatgpt_base_url: Option<String>,
    pub forced_chatgpt_workspace_id: Option<Vec<String>>,
    pub auth_route_config: AuthRouteConfig,
}

/// Enforces configured login restrictions using auth-owned HTTP settings.
pub async fn enforce_login_restrictions(config: &AuthConfig) -> std::io::Result<()> {
    let agent_identity_authapi_base_url =
        agent_identity_authapi_base_url(config.chatgpt_base_url.as_deref()).ok();
    enforce_login_restrictions_with_agent_identity_authapi_base_url(
        config,
        agent_identity_authapi_base_url.as_deref(),
    )
    .await
}

async fn enforce_login_restrictions_with_agent_identity_authapi_base_url(
    config: &AuthConfig,
    agent_identity_authapi_base_url: Option<&str>,
) -> std::io::Result<()> {
    let Some(auth) = load_auth(
        &config.codex_home,
        /*enable_codex_api_key_env*/ true,
        config.auth_credentials_store_mode,
        /*forced_chatgpt_workspace_id*/ None,
        config.chatgpt_base_url.as_deref(),
        config.keyring_backend_kind,
        agent_identity_authapi_base_url,
        &config.auth_route_config,
    )
    .await?
    else {
        return Ok(());
    };

    if let Some(required_method) = config.forced_login_method {
        let method_violation = match (required_method, auth.auth_mode()) {
            (ForcedLoginMethod::Api, AuthMode::ApiKey)
            | (ForcedLoginMethod::Api, AuthMode::BedrockApiKey) => None,
            (ForcedLoginMethod::Chatgpt, AuthMode::Chatgpt)
            | (ForcedLoginMethod::Chatgpt, AuthMode::ChatgptAuthTokens)
            | (ForcedLoginMethod::Chatgpt, AuthMode::Headers)
            | (ForcedLoginMethod::Chatgpt, AuthMode::AgentIdentity)
            | (ForcedLoginMethod::Chatgpt, AuthMode::PersonalAccessToken) => None,
            (ForcedLoginMethod::Api, AuthMode::Chatgpt)
            | (ForcedLoginMethod::Api, AuthMode::ChatgptAuthTokens)
            | (ForcedLoginMethod::Api, AuthMode::Headers)
            | (ForcedLoginMethod::Api, AuthMode::AgentIdentity)
            | (ForcedLoginMethod::Api, AuthMode::PersonalAccessToken) => Some(
                "API key login is required, but ChatGPT is currently being used. Logging out."
                    .to_string(),
            ),
            (ForcedLoginMethod::Chatgpt, AuthMode::ApiKey)
            | (ForcedLoginMethod::Chatgpt, AuthMode::BedrockApiKey) => Some(
                "ChatGPT login is required, but an API key is currently being used. Logging out."
                    .to_string(),
            ),
        };

        if let Some(message) = method_violation {
            return logout_with_message(
                &config.codex_home,
                message,
                config.auth_credentials_store_mode,
                config.keyring_backend_kind,
            );
        }
    }

    if let Some(expected_account_ids) = config.forced_chatgpt_workspace_id.as_deref() {
        let chatgpt_account_id = match &auth {
            CodexAuth::ApiKey(_) | CodexAuth::Headers(_) | CodexAuth::BedrockApiKey(_) => {
                return Ok(());
            }
            CodexAuth::AgentIdentity(_) | CodexAuth::PersonalAccessToken(_) => {
                auth.get_account_id()
            }
            CodexAuth::Chatgpt(_) | CodexAuth::ChatgptAuthTokens(_) => {
                let token_data = match auth.get_token_data() {
                    Ok(data) => data,
                    Err(err) => {
                        return logout_with_message(
                            &config.codex_home,
                            format!(
                                "Failed to load ChatGPT credentials while enforcing workspace restrictions: {err}. Logging out."
                            ),
                            config.auth_credentials_store_mode,
                            config.keyring_backend_kind,
                        );
                    }
                };
                token_data.id_token.chatgpt_account_id
            }
        };

        // workspace is the external identifier for account id.
        let chatgpt_account_id = chatgpt_account_id.as_deref();
        if !chatgpt_account_id.is_some_and(|actual| {
            expected_account_ids
                .iter()
                .any(|expected| expected == actual)
        }) {
            let expected_workspaces = expected_account_ids.join(", ");
            let message = match chatgpt_account_id {
                Some(actual) => format!(
                    "Login is restricted to workspace(s) {expected_workspaces}, but current credentials belong to {actual}. Logging out."
                ),
                None => format!(
                    "Login is restricted to workspace(s) {expected_workspaces}, but current credentials lack a workspace identifier. Logging out."
                ),
            };
            return logout_with_message(
                &config.codex_home,
                message,
                config.auth_credentials_store_mode,
                config.keyring_backend_kind,
            );
        }
    }

    Ok(())
}

fn logout_with_message(
    codex_home: &Path,
    message: String,
    auth_credentials_store_mode: AuthCredentialsStoreMode,
    keyring_backend_kind: AuthKeyringBackendKind,
) -> std::io::Result<()> {
    // External auth tokens live in the ephemeral store, but persistent auth may still exist
    // from earlier logins. Clear both so a forced logout truly removes all active auth.
    let removal_result = logout_all_stores(
        codex_home,
        auth_credentials_store_mode,
        keyring_backend_kind,
    );
    let error_message = match removal_result {
        Ok(_) => message,
        Err(err) => format!("{message}. Failed to remove auth.json: {err}"),
    };
    Err(std::io::Error::other(error_message))
}

fn logout_all_stores(
    codex_home: &Path,
    auth_credentials_store_mode: AuthCredentialsStoreMode,
    keyring_backend_kind: AuthKeyringBackendKind,
) -> std::io::Result<bool> {
    if auth_credentials_store_mode == AuthCredentialsStoreMode::Ephemeral {
        return logout(
            codex_home,
            AuthCredentialsStoreMode::Ephemeral,
            AuthKeyringBackendKind::default(),
        );
    }
    let removed_ephemeral = logout(
        codex_home,
        AuthCredentialsStoreMode::Ephemeral,
        AuthKeyringBackendKind::default(),
    )?;
    let removed_managed = logout(
        codex_home,
        auth_credentials_store_mode,
        keyring_backend_kind,
    )?;
    Ok(removed_ephemeral || removed_managed)
}

#[allow(clippy::too_many_arguments)]
async fn load_auth(
    codex_home: &Path,
    enable_codex_api_key_env: bool,
    auth_credentials_store_mode: AuthCredentialsStoreMode,
    forced_chatgpt_workspace_id: Option<&[String]>,
    chatgpt_base_url: Option<&str>,
    keyring_backend_kind: AuthKeyringBackendKind,
    agent_identity_authapi_base_url: Option<&str>,
    auth_route_config: &AuthRouteConfig,
) -> std::io::Result<Option<CodexAuth>> {
    // API key via env var takes precedence over any other auth method.
    if enable_codex_api_key_env && let Some(api_key) = read_codex_api_key_from_env() {
        return Ok(Some(CodexAuth::from_api_key(api_key.as_str())));
    }

    // External ChatGPT auth tokens live in the in-memory (ephemeral) store. Always check this
    // first so external auth takes precedence over any persisted credentials.
    let ephemeral_storage = create_auth_storage(
        codex_home.to_path_buf(),
        AuthCredentialsStoreMode::Ephemeral,
        AuthKeyringBackendKind::default(),
    );
    if let Some(auth_dot_json) = ephemeral_storage.load()? {
        let auth = CodexAuth::from_auth_dot_json(
            codex_home,
            auth_dot_json,
            AuthCredentialsStoreMode::Ephemeral,
            chatgpt_base_url,
            keyring_backend_kind,
            agent_identity_authapi_base_url,
            auth_route_config,
        )
        .await?;
        if let CodexAuth::PersonalAccessToken(auth) = &auth {
            ensure_personal_access_token_workspace_allowed(forced_chatgpt_workspace_id, auth)?;
        }
        return Ok(Some(auth));
    }

    if let Some(access_token) = read_codex_access_token_from_env() {
        return match classify_codex_access_token(&access_token) {
            CodexAccessToken::PersonalAccessToken(access_token) => {
                let auth = PersonalAccessTokenAuth::load(access_token, auth_route_config).await?;
                ensure_personal_access_token_workspace_allowed(forced_chatgpt_workspace_id, &auth)?;
                Ok(Some(CodexAuth::PersonalAccessToken(auth)))
            }
            CodexAccessToken::AgentIdentityJwt(jwt) => {
                CodexAuth::from_agent_identity_jwt_with_authapi_base_url(
                    jwt,
                    chatgpt_base_url,
                    require_agent_identity_authapi_base_url(agent_identity_authapi_base_url)?,
                    auth_route_config,
                )
            }
            .await
            .map(Some),
        };
    }

    // If the caller explicitly requested ephemeral auth, there is no persisted fallback.
    if auth_credentials_store_mode == AuthCredentialsStoreMode::Ephemeral {
        return Ok(None);
    }

    // Fall back to the configured persistent store (file/keyring/auto) for managed auth.
    let storage = create_auth_storage(
        codex_home.to_path_buf(),
        auth_credentials_store_mode,
        keyring_backend_kind,
    );
    let auth_dot_json = match storage.load()? {
        Some(auth) => auth,
        None => return Ok(None),
    };

    let auth = CodexAuth::from_auth_dot_json(
        codex_home,
        auth_dot_json,
        auth_credentials_store_mode,
        chatgpt_base_url,
        keyring_backend_kind,
        agent_identity_authapi_base_url,
        auth_route_config,
    )
    .await?;
    if let CodexAuth::PersonalAccessToken(auth) = &auth {
        ensure_personal_access_token_workspace_allowed(forced_chatgpt_workspace_id, auth)?;
    }
    Ok(Some(auth))
}

// Persist refreshed tokens into auth storage and update last_refresh.
fn persist_tokens(
    storage: &Arc<dyn AuthStorageBackend>,
    id_token: Option<String>,
    access_token: Option<String>,
    refresh_token: Option<String>,
) -> std::io::Result<AuthDotJson> {
    let mut auth_dot_json = storage
        .load()?
        .ok_or(std::io::Error::other("Token data is not available."))?;

    let tokens = auth_dot_json.tokens.get_or_insert_with(TokenData::default);
    if let Some(id_token) = id_token {
        tokens.id_token = parse_chatgpt_jwt_claims(&id_token).map_err(std::io::Error::other)?;
    }
    if let Some(access_token) = access_token {
        tokens.access_token = access_token;
    }
    if let Some(refresh_token) = refresh_token {
        tokens.refresh_token = refresh_token;
    }
    auth_dot_json.last_refresh = Some(Utc::now());
    storage.save(&auth_dot_json)?;
    Ok(auth_dot_json)
}

// Requests refreshed ChatGPT OAuth tokens from the auth service using a refresh token.
// The caller is responsible for persisting any returned tokens.
async fn request_chatgpt_token_refresh(
    refresh_token: String,
    client: &HttpClient,
) -> Result<RefreshResponse, RefreshTokenError> {
    let refresh_request = RefreshRequest {
        client_id: oauth_client_id(),
        grant_type: "refresh_token",
        refresh_token,
    };
    let endpoint = refresh_token_endpoint();

    // Use shared client factory to include standard headers
    let response = client
        .post(endpoint.as_str())
        .header("Content-Type", "application/json")
        .json(&refresh_request)
        .send()
        .await
        .map_err(|err| RefreshTokenError::Transient(std::io::Error::other(err)))?;

    let status = response.status();
    if status.is_success() {
        let refresh_response = response
            .json::<RefreshResponse>()
            .await
            .map_err(|err| RefreshTokenError::Transient(std::io::Error::other(err)))?;
        Ok(refresh_response)
    } else {
        let body = response.text().await.unwrap_or_default();
        tracing::error!("Failed to refresh token: {status}: {body}");
        let failed = classify_refresh_token_failure(&body);
        if status == StatusCode::UNAUTHORIZED || failed.reason != RefreshTokenFailedReason::Other {
            Err(RefreshTokenError::Permanent(failed))
        } else {
            let message = try_parse_error_message(&body);
            Err(RefreshTokenError::Transient(std::io::Error::other(
                format!("Failed to refresh token: {status}: {message}"),
            )))
        }
    }
}

fn classify_refresh_token_failure(body: &str) -> RefreshTokenFailedError {
    let code = extract_refresh_token_error_code(body);

    let normalized_code = code.as_deref().map(str::to_ascii_lowercase);
    let reason = match normalized_code.as_deref() {
        Some("refresh_token_expired") => RefreshTokenFailedReason::Expired,
        Some("refresh_token_reused") => RefreshTokenFailedReason::Exhausted,
        Some("refresh_token_invalidated") => RefreshTokenFailedReason::Revoked,
        _ => RefreshTokenFailedReason::Other,
    };

    if reason == RefreshTokenFailedReason::Other {
        tracing::warn!(
            backend_code = normalized_code.as_deref(),
            backend_body = body,
            "Encountered unknown response while refreshing token"
        );
    }

    let message = match reason {
        RefreshTokenFailedReason::Expired => REFRESH_TOKEN_EXPIRED_MESSAGE.to_string(),
        RefreshTokenFailedReason::Exhausted => REFRESH_TOKEN_REUSED_MESSAGE.to_string(),
        RefreshTokenFailedReason::Revoked => REFRESH_TOKEN_INVALIDATED_MESSAGE.to_string(),
        RefreshTokenFailedReason::Other => REFRESH_TOKEN_UNKNOWN_MESSAGE.to_string(),
    };

    RefreshTokenFailedError::new(reason, message)
}

fn extract_refresh_token_error_code(body: &str) -> Option<String> {
    if body.trim().is_empty() {
        return None;
    }

    let Value::Object(map) = serde_json::from_str::<Value>(body).ok()? else {
        return None;
    };

    if let Some(error_value) = map.get("error") {
        match error_value {
            Value::Object(obj) => {
                if let Some(code) = obj.get("code").and_then(Value::as_str) {
                    return Some(code.to_string());
                }
            }
            Value::String(code) => {
                return Some(code.to_string());
            }
            _ => {}
        }
    }

    map.get("code").and_then(Value::as_str).map(str::to_string)
}

#[derive(Serialize)]
struct RefreshRequest {
    client_id: String,
    grant_type: &'static str,
    refresh_token: String,
}

#[derive(Deserialize, Clone)]
struct RefreshResponse {
    id_token: Option<String>,
    access_token: Option<String>,
    refresh_token: Option<String>,
}

// Shared constant for token refresh (client id used for oauth token refresh flow)
pub const CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";

pub fn oauth_client_id() -> String {
    std::env::var(CLIENT_ID_OVERRIDE_ENV_VAR)
        .ok()
        .filter(|client_id| !client_id.trim().is_empty())
        .unwrap_or_else(|| CLIENT_ID.to_string())
}

fn refresh_token_endpoint() -> String {
    std::env::var(REFRESH_TOKEN_URL_OVERRIDE_ENV_VAR)
        .unwrap_or_else(|_| REFRESH_TOKEN_URL.to_string())
}

impl AuthDotJson {
    fn from_external_access_token(
        access_token: &str,
        chatgpt_account_id: &str,
        chatgpt_plan_type: Option<&str>,
    ) -> std::io::Result<Self> {
        let mut token_info =
            parse_chatgpt_jwt_claims(access_token).map_err(std::io::Error::other)?;
        token_info.chatgpt_account_id = Some(chatgpt_account_id.to_string());
        token_info.chatgpt_plan_type = chatgpt_plan_type
            .map(InternalPlanType::from_raw_value)
            .or(token_info.chatgpt_plan_type)
            .or(Some(InternalPlanType::Unknown("unknown".to_string())));
        let tokens = TokenData {
            id_token: token_info,
            access_token: access_token.to_string(),
            refresh_token: String::new(),
            account_id: Some(chatgpt_account_id.to_string()),
        };

        Ok(Self {
            auth_mode: Some(AuthMode::ChatgptAuthTokens),
            openai_api_key: None,
            tokens: Some(tokens),
            last_refresh: Some(Utc::now()),
            agent_identity: None,
            personal_access_token: None,
            bedrock_api_key: None,
        })
    }

    pub(super) fn resolved_mode(&self) -> AuthMode {
        if let Some(mode) = self.auth_mode {
            return mode;
        }
        if self.personal_access_token.is_some() {
            return AuthMode::PersonalAccessToken;
        }
        if self.bedrock_api_key.is_some() {
            return AuthMode::BedrockApiKey;
        }
        if self.openai_api_key.is_some() {
            return AuthMode::ApiKey;
        }
        AuthMode::Chatgpt
    }

    fn storage_mode(
        &self,
        auth_credentials_store_mode: AuthCredentialsStoreMode,
    ) -> AuthCredentialsStoreMode {
        if self.resolved_mode() == AuthMode::ChatgptAuthTokens {
            AuthCredentialsStoreMode::Ephemeral
        } else {
            auth_credentials_store_mode
        }
    }
}

/// Internal cached auth state.
#[derive(Clone)]
struct CachedAuth {
    auth: Option<CodexAuth>,
    /// Permanent refresh failure cached for the current auth snapshot so
    /// later refresh attempts for the same credentials fail fast without network.
    permanent_refresh_failure: Option<AuthScopedRefreshFailure>,
}

#[derive(Clone)]
struct AuthScopedRefreshFailure {
    auth: CodexAuth,
    error: RefreshTokenFailedError,
}

impl Debug for CachedAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CachedAuth")
            .field(
                "auth_mode",
                &self.auth.as_ref().map(CodexAuth::api_auth_mode),
            )
            .field(
                "permanent_refresh_failure",
                &self
                    .permanent_refresh_failure
                    .as_ref()
                    .map(|failure| failure.error.reason),
            )
            .finish()
    }
}

enum UnauthorizedRecoveryStep {
    Reload,
    RefreshToken,
    ExternalRefresh,
    Done,
}

enum ReloadOutcome {
    /// Reload was performed and the cached auth changed
    ReloadedChanged,
    /// Reload was performed and the cached auth remained the same
    ReloadedNoChange,
    /// Reload was skipped (missing or mismatched account id)
    Skipped,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum UnauthorizedRecoveryMode {
    Managed,
    External,
}

// UnauthorizedRecovery is a state machine that handles an attempt to refresh the authentication when requests
// to API fail with 401 status code.
// The client calls next() every time it encounters a 401 error, one time per retry.
// For API key based authentication, we don't do anything and let the error bubble to the user.
//
// For ChatGPT based authentication, we:
// 1. Attempt to reload the auth data from disk. We only reload if the account id matches the one the current process is running as.
// 2. Attempt to refresh the token using OAuth token refresh flow.
// If after both steps the server still responds with 401 we let the error bubble to the user.
//
// For external auth sources, UnauthorizedRecovery retries once by asking the
// configured provider to refresh and caching the returned auth through the same
// path used by other auth sources.
pub struct UnauthorizedRecovery {
    manager: Arc<AuthManager>,
    step: UnauthorizedRecoveryStep,
    expected_account_id: Option<String>,
    mode: UnauthorizedRecoveryMode,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnauthorizedRecoveryStepResult {
    auth_state_changed: Option<bool>,
}

impl UnauthorizedRecoveryStepResult {
    pub fn auth_state_changed(&self) -> Option<bool> {
        self.auth_state_changed
    }
}

impl UnauthorizedRecovery {
    fn new(manager: Arc<AuthManager>) -> Self {
        let cached_auth = manager.auth_cached();
        let expected_account_id = cached_auth.as_ref().and_then(CodexAuth::get_account_id);
        let mode = if manager.has_external_auth() {
            UnauthorizedRecoveryMode::External
        } else {
            UnauthorizedRecoveryMode::Managed
        };
        let step = match mode {
            UnauthorizedRecoveryMode::Managed => UnauthorizedRecoveryStep::Reload,
            UnauthorizedRecoveryMode::External => UnauthorizedRecoveryStep::ExternalRefresh,
        };
        Self {
            manager,
            step,
            expected_account_id,
            mode,
        }
    }

    pub fn has_next(&self) -> bool {
        if self.manager.has_external_api_key_auth() {
            return !matches!(self.step, UnauthorizedRecoveryStep::Done);
        }

        if !self
            .manager
            .auth_cached()
            .as_ref()
            .is_some_and(CodexAuth::supports_unauthorized_recovery)
        {
            return false;
        }

        if self.mode == UnauthorizedRecoveryMode::External && !self.manager.has_external_auth() {
            return false;
        }

        !matches!(self.step, UnauthorizedRecoveryStep::Done)
    }

    pub fn unavailable_reason(&self) -> &'static str {
        if self.manager.has_external_api_key_auth() {
            return if matches!(self.step, UnauthorizedRecoveryStep::Done) {
                "recovery_exhausted"
            } else {
                "ready"
            };
        }

        if self
            .manager
            .auth_cached()
            .as_ref()
            .is_some_and(CodexAuth::is_personal_access_token_auth)
        {
            return "not_refreshable_auth";
        }

        if !self
            .manager
            .auth_cached()
            .as_ref()
            .is_some_and(CodexAuth::supports_unauthorized_recovery)
        {
            return "not_chatgpt_auth";
        }

        if self.mode == UnauthorizedRecoveryMode::External && !self.manager.has_external_auth() {
            return "no_external_auth";
        }

        if matches!(self.step, UnauthorizedRecoveryStep::Done) {
            return "recovery_exhausted";
        }

        "ready"
    }

    pub fn mode_name(&self) -> &'static str {
        match self.mode {
            UnauthorizedRecoveryMode::Managed => "managed",
            UnauthorizedRecoveryMode::External => "external",
        }
    }

    pub fn step_name(&self) -> &'static str {
        match self.step {
            UnauthorizedRecoveryStep::Reload => "reload",
            UnauthorizedRecoveryStep::RefreshToken => "refresh_token",
            UnauthorizedRecoveryStep::ExternalRefresh => "external_refresh",
            UnauthorizedRecoveryStep::Done => "done",
        }
    }

    pub async fn next(&mut self) -> Result<UnauthorizedRecoveryStepResult, RefreshTokenError> {
        if !self.has_next() {
            return Err(RefreshTokenError::Permanent(RefreshTokenFailedError::new(
                RefreshTokenFailedReason::Other,
                "No more recovery steps available.",
            )));
        }

        match self.step {
            UnauthorizedRecoveryStep::Reload => {
                match self
                    .manager
                    .reload_if_account_id_matches(self.expected_account_id.as_deref())
                    .await
                {
                    ReloadOutcome::ReloadedChanged => {
                        self.step = UnauthorizedRecoveryStep::RefreshToken;
                        return Ok(UnauthorizedRecoveryStepResult {
                            auth_state_changed: Some(true),
                        });
                    }
                    ReloadOutcome::ReloadedNoChange => {
                        self.step = UnauthorizedRecoveryStep::RefreshToken;
                        return Ok(UnauthorizedRecoveryStepResult {
                            auth_state_changed: Some(false),
                        });
                    }
                    ReloadOutcome::Skipped => {
                        self.step = UnauthorizedRecoveryStep::Done;
                        return Err(RefreshTokenError::Permanent(RefreshTokenFailedError::new(
                            RefreshTokenFailedReason::Other,
                            REFRESH_TOKEN_ACCOUNT_MISMATCH_MESSAGE.to_string(),
                        )));
                    }
                }
            }
            UnauthorizedRecoveryStep::RefreshToken => {
                self.manager.refresh_token_from_authority().await?;
                self.step = UnauthorizedRecoveryStep::Done;
                return Ok(UnauthorizedRecoveryStepResult {
                    auth_state_changed: Some(true),
                });
            }
            UnauthorizedRecoveryStep::ExternalRefresh => {
                self.manager.refresh_token_from_authority().await?;
                self.step = UnauthorizedRecoveryStep::Done;
                return Ok(UnauthorizedRecoveryStepResult {
                    auth_state_changed: Some(true),
                });
            }
            UnauthorizedRecoveryStep::Done => {}
        }
        Ok(UnauthorizedRecoveryStepResult {
            auth_state_changed: None,
        })
    }
}

/// Central manager providing a single source of truth for auth.json derived
/// authentication data. It loads once (or on preference change) and then
/// hands out cloned `CodexAuth` values so the rest of the program has a
/// consistent snapshot.
///
/// External modifications to `auth.json` will NOT be observed until
/// `reload()` is called explicitly. This matches the design goal of avoiding
/// different parts of the program seeing inconsistent auth data mid‑run.
pub struct AuthManager {
    codex_home: PathBuf,
    inner: RwLock<CachedAuth>,
    auth_change_tx: watch::Sender<u64>,
    enable_codex_api_key_env: bool,
    auth_credentials_store_mode: AuthCredentialsStoreMode,
    keyring_backend_kind: AuthKeyringBackendKind,
    forced_chatgpt_workspace_id: RwLock<Option<Vec<String>>>,
    chatgpt_base_url: Option<String>,
    agent_identity_authapi_base_url: Option<String>,
    refresh_lock: Semaphore,
    agent_identity_lock: Semaphore,
    agent_identity_bootstrap_cooldown: Mutex<AgentIdentityBootstrapCooldown>,
    external_auth: RwLock<Option<Arc<dyn ExternalAuth>>>,
    auth_route_config: AuthRouteConfig,
}

/// Configuration view required to construct a shared [`AuthManager`].
///
/// Implementations should return the auth-related config values for the
/// already-resolved runtime configuration. The primary implementation is
/// `codex_core::config::Config`, but this trait keeps `codex-login` independent
/// from `codex-core`.
pub trait AuthManagerConfig {
    /// Returns the Codex home directory used for auth storage.
    fn codex_home(&self) -> PathBuf;

    /// Returns the CLI auth credential storage mode for auth loading.
    fn cli_auth_credentials_store_mode(&self) -> AuthCredentialsStoreMode;

    /// Returns the backend to use when CLI auth keyring storage is selected.
    fn auth_keyring_backend_kind(&self) -> AuthKeyringBackendKind;

    /// Returns the workspace IDs that ChatGPT auth should be restricted to, if any.
    fn forced_chatgpt_workspace_id(&self) -> Option<Vec<String>>;

    /// Returns the ChatGPT backend base URL used for first-party backend authorization.
    fn chatgpt_base_url(&self) -> String;

    /// Returns route-selection settings for auth-owned clients.
    fn auth_route_config(&self) -> AuthRouteConfig;
}

impl Debug for AuthManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthManager")
            .field("codex_home", &self.codex_home)
            .field("inner", &self.inner)
            .field("enable_codex_api_key_env", &self.enable_codex_api_key_env)
            .field(
                "auth_credentials_store_mode",
                &self.auth_credentials_store_mode,
            )
            .field("keyring_backend_kind", &self.keyring_backend_kind)
            .field(
                "forced_chatgpt_workspace_id",
                &self.forced_chatgpt_workspace_id,
            )
            .field("chatgpt_base_url", &self.chatgpt_base_url)
            .field("auth_route_config", &self.auth_route_config)
            .field("has_external_auth", &self.has_external_auth())
            .finish_non_exhaustive()
    }
}

fn default_agent_identity_authapi_base_url() -> Option<String> {
    agent_identity_authapi_base_url(/*chatgpt_base_url*/ None).ok()
}

impl AuthManager {
    /// Create a new manager loading the initial auth using the provided
    /// preferred auth method. Errors loading auth are swallowed; `auth()` will
    /// simply return `None` in that case so callers can treat it as an
    /// unauthenticated state.
    pub async fn new(
        codex_home: PathBuf,
        enable_codex_api_key_env: bool,
        auth_credentials_store_mode: AuthCredentialsStoreMode,
        forced_chatgpt_workspace_id: Option<Vec<String>>,
        chatgpt_base_url: Option<String>,
        keyring_backend_kind: AuthKeyringBackendKind,
        auth_route_config: AuthRouteConfig,
    ) -> Self {
        let agent_identity_authapi_base_url =
            agent_identity_authapi_base_url(chatgpt_base_url.as_deref()).ok();
        let managed_auth = load_auth(
            &codex_home,
            enable_codex_api_key_env,
            auth_credentials_store_mode,
            forced_chatgpt_workspace_id.as_deref(),
            chatgpt_base_url.as_deref(),
            keyring_backend_kind,
            agent_identity_authapi_base_url.as_deref(),
            &auth_route_config,
        )
        .await
        .ok()
        .flatten();
        let (auth_change_tx, _auth_change_rx) = watch::channel(0);
        Self {
            codex_home,
            inner: RwLock::new(CachedAuth {
                auth: managed_auth,
                permanent_refresh_failure: None,
            }),
            auth_change_tx,
            enable_codex_api_key_env,
            auth_credentials_store_mode,
            keyring_backend_kind,
            forced_chatgpt_workspace_id: RwLock::new(forced_chatgpt_workspace_id),
            chatgpt_base_url,
            agent_identity_authapi_base_url,
            refresh_lock: Semaphore::new(/*permits*/ 1),
            agent_identity_lock: Semaphore::new(/*permits*/ 1),
            agent_identity_bootstrap_cooldown: Mutex::default(),
            external_auth: RwLock::new(None),
            auth_route_config,
        }
    }

    /// Create an AuthManager with a specific CodexAuth, for testing only.
    pub fn from_auth_for_testing(auth: CodexAuth) -> Arc<Self> {
        let cached = CachedAuth {
            auth: Some(auth),
            permanent_refresh_failure: None,
        };
        let (auth_change_tx, _auth_change_rx) = watch::channel(0);

        Arc::new(Self {
            codex_home: PathBuf::from("non-existent"),
            inner: RwLock::new(cached),
            auth_change_tx,
            enable_codex_api_key_env: false,
            auth_credentials_store_mode: AuthCredentialsStoreMode::File,
            keyring_backend_kind: AuthKeyringBackendKind::default(),
            forced_chatgpt_workspace_id: RwLock::new(None),
            chatgpt_base_url: None,
            agent_identity_authapi_base_url: default_agent_identity_authapi_base_url(),
            refresh_lock: Semaphore::new(/*permits*/ 1),
            agent_identity_lock: Semaphore::new(/*permits*/ 1),
            agent_identity_bootstrap_cooldown: Mutex::default(),
            external_auth: RwLock::new(None),
            auth_route_config: crate::test_support::transport_default_auth_route_config(),
        })
    }

    /// Create an AuthManager with a specific CodexAuth and codex home, for testing only.
    pub fn from_auth_for_testing_with_home(auth: CodexAuth, codex_home: PathBuf) -> Arc<Self> {
        let cached = CachedAuth {
            auth: Some(auth),
            permanent_refresh_failure: None,
        };
        let (auth_change_tx, _auth_change_rx) = watch::channel(0);
        Arc::new(Self {
            codex_home,
            inner: RwLock::new(cached),
            auth_change_tx,
            enable_codex_api_key_env: false,
            auth_credentials_store_mode: AuthCredentialsStoreMode::File,
            keyring_backend_kind: AuthKeyringBackendKind::default(),
            forced_chatgpt_workspace_id: RwLock::new(None),
            chatgpt_base_url: None,
            agent_identity_authapi_base_url: default_agent_identity_authapi_base_url(),
            refresh_lock: Semaphore::new(/*permits*/ 1),
            agent_identity_lock: Semaphore::new(/*permits*/ 1),
            agent_identity_bootstrap_cooldown: Mutex::default(),
            external_auth: RwLock::new(None),
            auth_route_config: crate::test_support::transport_default_auth_route_config(),
        })
    }

    /// Create an AuthManager with a specific CodexAuth and Agent Identity AuthAPI base URL, for testing only.
    #[doc(hidden)]
    pub fn from_auth_for_testing_with_agent_identity_authapi_base_url(
        auth: CodexAuth,
        agent_identity_authapi_base_url: String,
    ) -> Arc<Self> {
        let cached = CachedAuth {
            auth: Some(auth),
            permanent_refresh_failure: None,
        };
        let (auth_change_tx, _auth_change_rx) = watch::channel(0);
        Arc::new(Self {
            codex_home: PathBuf::from("non-existent"),
            inner: RwLock::new(cached),
            auth_change_tx,
            enable_codex_api_key_env: false,
            auth_credentials_store_mode: AuthCredentialsStoreMode::File,
            keyring_backend_kind: AuthKeyringBackendKind::default(),
            forced_chatgpt_workspace_id: RwLock::new(None),
            chatgpt_base_url: None,
            agent_identity_authapi_base_url: Some(
                agent_identity_authapi_base_url
                    .trim_end_matches('/')
                    .to_string(),
            ),
            refresh_lock: Semaphore::new(/*permits*/ 1),
            agent_identity_lock: Semaphore::new(/*permits*/ 1),
            agent_identity_bootstrap_cooldown: Mutex::default(),
            external_auth: RwLock::new(None),
            auth_route_config: crate::test_support::transport_default_auth_route_config(),
        })
    }

    pub fn external_bearer_only(config: ModelProviderAuthInfo) -> Arc<Self> {
        let (auth_change_tx, _auth_change_rx) = watch::channel(0);
        Arc::new(Self {
            codex_home: PathBuf::from("non-existent"),
            inner: RwLock::new(CachedAuth {
                auth: None,
                permanent_refresh_failure: None,
            }),
            auth_change_tx,
            enable_codex_api_key_env: false,
            auth_credentials_store_mode: AuthCredentialsStoreMode::File,
            keyring_backend_kind: AuthKeyringBackendKind::default(),
            forced_chatgpt_workspace_id: RwLock::new(None),
            chatgpt_base_url: None,
            agent_identity_authapi_base_url: default_agent_identity_authapi_base_url(),
            refresh_lock: Semaphore::new(/*permits*/ 1),
            agent_identity_lock: Semaphore::new(/*permits*/ 1),
            agent_identity_bootstrap_cooldown: Mutex::default(),
            external_auth: RwLock::new(Some(
                Arc::new(BearerTokenRefresher::new(config)) as Arc<dyn ExternalAuth>
            )),
            // External bearer auth refreshes by running the provider's command and never makes
            // auth-owned HTTP requests, so this route is intentionally inert.
            auth_route_config: AuthRouteConfig::from_http_client_factory(HttpClientFactory::new(
                OutboundProxyPolicy::ReqwestDefault,
            )),
        })
    }

    /// Current cached auth (clone) without attempting a refresh.
    pub fn auth_cached(&self) -> Option<CodexAuth> {
        self.inner
            .read()
            .ok()
            .and_then(|cached| cached.auth.clone())
    }

    /// Subscribes to cached auth changes that can affect request recovery.
    pub fn auth_change_receiver(&self) -> watch::Receiver<u64> {
        self.auth_change_tx.subscribe()
    }

    pub fn refresh_failure_for_auth(&self, auth: &CodexAuth) -> Option<RefreshTokenFailedError> {
        self.inner.read().ok().and_then(|cached| {
            cached
                .permanent_refresh_failure
                .as_ref()
                .filter(|failure| Self::auths_equal_for_refresh(Some(auth), Some(&failure.auth)))
                .map(|failure| failure.error.clone())
        })
    }

    /// Current cached auth (clone). May be `None` if not logged in or load failed.
    /// For managed ChatGPT auth that needs a proactive refresh, first performs
    /// a guarded reload and then refreshes only if the on-disk auth is unchanged.
    #[instrument(level = "trace", skip_all)]
    pub async fn auth(&self) -> Option<CodexAuth> {
        if self.has_external_auth() {
            self.reload().await;
            return self.auth_cached();
        }

        let auth = self.auth_cached()?;
        if Self::should_refresh_proactively(&auth)
            && let Err(err) = self.refresh_token().await
        {
            tracing::error!("Failed to refresh token: {}", err);
            return Some(auth);
        }
        self.auth_cached()
    }

    pub async fn agent_identity_auth(
        &self,
        policy: AgentIdentityAuthPolicy,
        session_source: SessionSource,
    ) -> std::io::Result<Option<AgentIdentityAuth>> {
        let Some(auth) = self.auth().await else {
            return Ok(None);
        };
        if policy == AgentIdentityAuthPolicy::ChatGptAuth && matches!(auth, CodexAuth::Chatgpt(_)) {
            let _bootstrap_permit = self
                .agent_identity_lock
                .acquire()
                .await
                .map_err(std::io::Error::other)?;
            let forced_chatgpt_workspace_id = self.forced_chatgpt_workspace_id();
            let cooldown_key = ManagedChatGptAgentIdentityBinding::from_auth(
                &auth,
                forced_chatgpt_workspace_id.clone(),
            )
            .and_then(|binding| {
                self.agent_identity_authapi_base_url
                    .as_ref()
                    .map(|base_url| (binding.account_id, base_url.clone()))
            });
            if let Some((account_id, authapi_base_url)) = cooldown_key.as_ref()
                && let Ok(mut cooldown) = self.agent_identity_bootstrap_cooldown.lock()
                && let Some(error) =
                    cooldown.error_for(account_id, authapi_base_url, Instant::now())
            {
                tracing::warn!("agent identity bootstrap retry suppressed during shared cooldown");
                return Err(std::io::Error::other(error));
            }

            let result = auth
                .agent_identity_auth(
                    policy,
                    self.agent_identity_authapi_base_url.as_deref(),
                    forced_chatgpt_workspace_id,
                    &self.auth_route_config,
                    session_source,
                )
                .await;
            if let Ok(mut cooldown) = self.agent_identity_bootstrap_cooldown.lock() {
                if let (Err(err), Some((account_id, authapi_base_url))) = (&result, cooldown_key)
                    && let Some(error) = AgentIdentityAuthError::bootstrap_unavailable(err).cloned()
                {
                    cooldown.record_failure(account_id, authapi_base_url, error, Instant::now());
                } else {
                    cooldown.clear();
                }
            }
            return result;
        }
        auth.agent_identity_auth(
            policy,
            self.agent_identity_authapi_base_url.as_deref(),
            self.forced_chatgpt_workspace_id(),
            &self.auth_route_config,
            session_source,
        )
        .await
    }

    /// Reloads auth from the active source. Returns whether the auth value changed.
    pub async fn reload(&self) -> bool {
        tracing::info!("Reloading auth");
        let new_auth = self.load_auth().await;
        self.set_cached_auth(new_auth)
    }

    async fn reload_if_account_id_matches(
        &self,
        expected_account_id: Option<&str>,
    ) -> ReloadOutcome {
        let expected_account_id = match expected_account_id {
            Some(account_id) => account_id,
            None => {
                tracing::info!("Skipping auth reload because no account id is available.");
                return ReloadOutcome::Skipped;
            }
        };

        let new_auth = self.load_auth().await;
        let new_account_id = new_auth.as_ref().and_then(CodexAuth::get_account_id);

        if new_account_id.as_deref() != Some(expected_account_id) {
            let found_account_id = new_account_id.as_deref().unwrap_or("unknown");
            tracing::info!(
                "Skipping auth reload due to account id mismatch (expected: {expected_account_id}, found: {found_account_id})"
            );
            return ReloadOutcome::Skipped;
        }

        tracing::info!("Reloading auth for account {expected_account_id}");
        let cached_before_reload = self.auth_cached();
        let auth_changed =
            !Self::auths_equal_for_refresh(cached_before_reload.as_ref(), new_auth.as_ref());
        self.set_cached_auth(new_auth);
        if auth_changed {
            ReloadOutcome::ReloadedChanged
        } else {
            ReloadOutcome::ReloadedNoChange
        }
    }

    fn auths_equal_for_refresh(a: Option<&CodexAuth>, b: Option<&CodexAuth>) -> bool {
        match (a, b) {
            (None, None) => true,
            (Some(a), Some(b)) => match (a.api_auth_mode(), b.api_auth_mode()) {
                (AuthMode::ApiKey, AuthMode::ApiKey) => a.api_key() == b.api_key(),
                (AuthMode::Chatgpt, AuthMode::Chatgpt)
                | (AuthMode::ChatgptAuthTokens, AuthMode::ChatgptAuthTokens) => {
                    a.get_current_auth_json() == b.get_current_auth_json()
                }
                (AuthMode::Headers, AuthMode::Headers) => a == b,
                (AuthMode::AgentIdentity, AuthMode::AgentIdentity) => match (a, b) {
                    (CodexAuth::AgentIdentity(a), CodexAuth::AgentIdentity(b)) => {
                        a.record() == b.record()
                    }
                    _ => false,
                },
                (AuthMode::PersonalAccessToken, AuthMode::PersonalAccessToken) => a == b,
                (AuthMode::BedrockApiKey, AuthMode::BedrockApiKey) => a == b,
                _ => false,
            },
            _ => false,
        }
    }

    fn auths_equal(a: Option<&CodexAuth>, b: Option<&CodexAuth>) -> bool {
        match (a, b) {
            (None, None) => true,
            (Some(a), Some(b)) => a == b,
            _ => false,
        }
    }

    /// Records a permanent refresh failure only if the failed refresh was
    /// attempted against the auth snapshot that is still cached.
    fn record_permanent_refresh_failure_if_unchanged(
        &self,
        attempted_auth: &CodexAuth,
        error: &RefreshTokenFailedError,
    ) {
        if let Ok(mut guard) = self.inner.write() {
            let current_auth_matches =
                Self::auths_equal_for_refresh(Some(attempted_auth), guard.auth.as_ref());
            if current_auth_matches {
                guard.permanent_refresh_failure = Some(AuthScopedRefreshFailure {
                    auth: attempted_auth.clone(),
                    error: error.clone(),
                });
            }
        }
    }

    async fn load_auth(&self) -> Option<CodexAuth> {
        if let Some(external_auth) = self.external_auth() {
            return match self.resolve_external_auth(&external_auth).await {
                Ok(auth) => Some(auth),
                Err(err) => {
                    tracing::error!("Failed to resolve external auth: {err}");
                    None
                }
            };
        }

        let forced_chatgpt_workspace_id = self.forced_chatgpt_workspace_id();
        load_auth(
            &self.codex_home,
            self.enable_codex_api_key_env,
            self.auth_credentials_store_mode,
            forced_chatgpt_workspace_id.as_deref(),
            self.chatgpt_base_url.as_deref(),
            self.keyring_backend_kind,
            self.agent_identity_authapi_base_url.as_deref(),
            &self.auth_route_config,
        )
        .await
        .ok()
        .flatten()
    }

    fn set_cached_auth(&self, new_auth: Option<CodexAuth>) -> bool {
        if let Ok(mut guard) = self.inner.write() {
            let previous = guard.auth.as_ref();
            let changed = !AuthManager::auths_equal(previous, new_auth.as_ref());
            let auth_changed_for_refresh =
                !Self::auths_equal_for_refresh(previous, new_auth.as_ref());
            if auth_changed_for_refresh {
                guard.permanent_refresh_failure = None;
            }
            tracing::info!("Reloaded auth, changed: {changed}");
            guard.auth = new_auth;
            if auth_changed_for_refresh {
                self.auth_change_tx.send_modify(|revision| *revision += 1);
            }
            changed
        } else {
            false
        }
    }

    pub async fn set_external_auth(
        &self,
        external_auth: Arc<dyn ExternalAuth>,
    ) -> Result<(), RefreshTokenError> {
        let auth = self.resolve_external_auth(&external_auth).await?;
        *self.external_auth.write().map_err(|_| {
            RefreshTokenError::Transient(std::io::Error::other("external auth lock is poisoned"))
        })? = Some(external_auth);
        self.commit_external_auth(auth)
    }

    pub fn clear_external_auth(&self) {
        if let Ok(mut external_auth) = self.external_auth.write()
            && external_auth.take().is_some()
        {
            self.set_cached_auth(/*new_auth*/ None);
        }
    }

    pub fn set_forced_chatgpt_workspace_id(&self, workspace_id: Option<Vec<String>>) {
        if let Ok(mut guard) = self.forced_chatgpt_workspace_id.write()
            && *guard != workspace_id
        {
            *guard = workspace_id;
        }
    }

    pub fn forced_chatgpt_workspace_id(&self) -> Option<Vec<String>> {
        self.forced_chatgpt_workspace_id
            .read()
            .ok()
            .and_then(|guard| guard.clone())
    }

    pub fn has_external_auth(&self) -> bool {
        self.external_auth().is_some()
    }

    pub fn is_external_chatgpt_auth_active(&self) -> bool {
        self.auth_cached()
            .as_ref()
            .is_some_and(CodexAuth::is_external_chatgpt_tokens)
    }

    pub fn codex_api_key_env_enabled(&self) -> bool {
        self.enable_codex_api_key_env
    }

    /// Convenience constructor returning an `Arc` wrapper.
    pub async fn shared(
        codex_home: PathBuf,
        enable_codex_api_key_env: bool,
        auth_credentials_store_mode: AuthCredentialsStoreMode,
        forced_chatgpt_workspace_id: Option<Vec<String>>,
        chatgpt_base_url: Option<String>,
        keyring_backend_kind: AuthKeyringBackendKind,
        auth_route_config: AuthRouteConfig,
    ) -> Arc<Self> {
        Arc::new(
            Self::new(
                codex_home,
                enable_codex_api_key_env,
                auth_credentials_store_mode,
                forced_chatgpt_workspace_id,
                chatgpt_base_url,
                keyring_backend_kind,
                auth_route_config,
            )
            .await,
        )
    }

    /// Convenience constructor returning an `Arc` wrapper from resolved config.
    pub async fn shared_from_config(
        config: &impl AuthManagerConfig,
        enable_codex_api_key_env: bool,
    ) -> Arc<Self> {
        Self::shared(
            config.codex_home(),
            enable_codex_api_key_env,
            config.cli_auth_credentials_store_mode(),
            config.forced_chatgpt_workspace_id(),
            Some(config.chatgpt_base_url()),
            config.auth_keyring_backend_kind(),
            config.auth_route_config(),
        )
        .await
    }

    pub fn unauthorized_recovery(self: &Arc<Self>) -> UnauthorizedRecovery {
        UnauthorizedRecovery::new(Arc::clone(self))
    }

    fn external_auth(&self) -> Option<Arc<dyn ExternalAuth>> {
        self.external_auth
            .read()
            .ok()
            .and_then(|external_auth| external_auth.as_ref().map(Arc::clone))
    }

    fn has_external_api_key_auth(&self) -> bool {
        self.has_external_auth()
            && self
                .auth_cached()
                .as_ref()
                .is_some_and(CodexAuth::is_api_key_auth)
    }

    async fn resolve_external_auth(
        &self,
        external_auth: &Arc<dyn ExternalAuth>,
    ) -> Result<CodexAuth, RefreshTokenError> {
        let auth = external_auth
            .resolve()
            .await
            .map_err(RefreshTokenError::Transient)?;
        self.validate_external_auth(&auth)?;
        Ok(auth)
    }

    /// Attempt to refresh the token by first performing a guarded reload from
    /// the active auth source. If the loaded token differs from the cached token,
    /// we can assume that the source already refreshed it. Otherwise, ask the
    /// token authority to refresh.
    pub async fn refresh_token(&self) -> Result<(), RefreshTokenError> {
        let _refresh_guard = self.refresh_lock.acquire().await.map_err(|_| {
            RefreshTokenError::Permanent(RefreshTokenFailedError::new(
                RefreshTokenFailedReason::Other,
                REFRESH_TOKEN_UNKNOWN_MESSAGE.to_string(),
            ))
        })?;
        let auth_before_reload = self.auth_cached();
        if auth_before_reload
            .as_ref()
            .is_some_and(|auth| auth.is_api_key_auth() || auth.is_personal_access_token_auth())
        {
            return Ok(());
        }
        let expected_account_id = auth_before_reload
            .as_ref()
            .and_then(CodexAuth::get_account_id);

        match self
            .reload_if_account_id_matches(expected_account_id.as_deref())
            .await
        {
            ReloadOutcome::ReloadedChanged => {
                tracing::info!("Skipping token refresh because auth changed after guarded reload.");
                Ok(())
            }
            ReloadOutcome::ReloadedNoChange => self.refresh_token_from_authority_impl().await,
            ReloadOutcome::Skipped => {
                Err(RefreshTokenError::Permanent(RefreshTokenFailedError::new(
                    RefreshTokenFailedReason::Other,
                    REFRESH_TOKEN_ACCOUNT_MISMATCH_MESSAGE.to_string(),
                )))
            }
        }
    }

    /// Attempt to refresh the current auth token from the authority that issued
    /// it and update the shared cache. If the token refresh fails, returns the
    /// error to the caller.
    pub async fn refresh_token_from_authority(&self) -> Result<(), RefreshTokenError> {
        let _refresh_guard = self.refresh_lock.acquire().await.map_err(|_| {
            RefreshTokenError::Permanent(RefreshTokenFailedError::new(
                RefreshTokenFailedReason::Other,
                REFRESH_TOKEN_UNKNOWN_MESSAGE.to_string(),
            ))
        })?;
        self.refresh_token_from_authority_impl().await
    }

    async fn refresh_token_from_authority_impl(&self) -> Result<(), RefreshTokenError> {
        tracing::info!("Refreshing token");

        let auth = match self.auth_cached() {
            Some(auth) => auth,
            None => return Ok(()),
        };
        if let Some(error) = self.refresh_failure_for_auth(&auth) {
            return Err(RefreshTokenError::Permanent(error));
        }

        let attempted_auth = auth.clone();
        let result = if self.has_external_auth() {
            self.refresh_external_auth(ExternalAuthRefreshReason::Unauthorized)
                .await
        } else {
            match auth {
                CodexAuth::Chatgpt(chatgpt_auth) => {
                    let token_data = chatgpt_auth.current_token_data().ok_or_else(|| {
                        RefreshTokenError::Transient(std::io::Error::other(
                            "Token data is not available.",
                        ))
                    })?;
                    self.refresh_and_persist_chatgpt_token(&chatgpt_auth, token_data.refresh_token)
                        .await
                }
                CodexAuth::ApiKey(_)
                | CodexAuth::ChatgptAuthTokens(_)
                | CodexAuth::Headers(_)
                | CodexAuth::AgentIdentity(_)
                | CodexAuth::PersonalAccessToken(_)
                | CodexAuth::BedrockApiKey(_) => Ok(()),
            }
        };
        if let Err(RefreshTokenError::Permanent(error)) = &result {
            self.record_permanent_refresh_failure_if_unchanged(&attempted_auth, error);
        }
        result
    }

    /// Log out by deleting the on‑disk auth.json (if present). Returns Ok(true)
    /// if a file was removed, Ok(false) if no auth file existed. On success,
    /// reloads the in‑memory auth cache so callers immediately observe the
    /// unauthenticated state.
    pub async fn logout(&self) -> std::io::Result<bool> {
        let removed = logout_all_stores(
            &self.codex_home,
            self.auth_credentials_store_mode,
            self.keyring_backend_kind,
        )?;
        // Always reload to clear any cached auth (even if file absent).
        self.clear_external_auth();
        self.reload().await;
        Ok(removed)
    }

    pub async fn logout_with_revoke(&self) -> std::io::Result<bool> {
        let auth_dot_json = self
            .auth_cached()
            .and_then(|auth| auth.get_current_auth_json());
        if let Err(err) = revoke_auth_tokens(auth_dot_json.as_ref(), &self.auth_route_config).await
        {
            tracing::warn!("failed to revoke auth tokens during logout: {err}");
        }
        let result = logout_all_stores(
            &self.codex_home,
            self.auth_credentials_store_mode,
            self.keyring_backend_kind,
        )?;
        // Always reload to clear any cached auth (even if file absent).
        self.clear_external_auth();
        self.reload().await;
        Ok(result)
    }

    /// Returns the precise kind of credentials backing the current authentication.
    pub fn get_api_auth_mode(&self) -> Option<AuthMode> {
        self.auth_cached().as_ref().map(CodexAuth::api_auth_mode)
    }

    /// Returns the effective backend auth mode for the current authentication.
    pub fn auth_mode(&self) -> Option<AuthMode> {
        self.auth_cached().as_ref().map(CodexAuth::auth_mode)
    }

    pub fn current_auth_uses_codex_backend(&self) -> bool {
        self.get_api_auth_mode()
            .is_some_and(AuthMode::uses_codex_backend)
    }

    fn should_refresh_proactively(auth: &CodexAuth) -> bool {
        let chatgpt_auth = match auth {
            CodexAuth::Chatgpt(chatgpt_auth) => chatgpt_auth,
            _ => return false,
        };

        let auth_dot_json = match chatgpt_auth.current_auth_json() {
            Some(auth_dot_json) => auth_dot_json,
            None => return false,
        };
        if let Some(tokens) = auth_dot_json.tokens.as_ref()
            && let Ok(Some(expires_at)) = parse_jwt_expiration(&tokens.access_token)
        {
            return expires_at
                <= Utc::now()
                    + chrono::Duration::minutes(CHATGPT_ACCESS_TOKEN_REFRESH_WINDOW_MINUTES);
        }
        let last_refresh = match auth_dot_json.last_refresh {
            Some(last_refresh) => last_refresh,
            None => return false,
        };
        last_refresh < Utc::now() - chrono::Duration::days(TOKEN_REFRESH_INTERVAL)
    }

    async fn refresh_external_auth(
        &self,
        reason: ExternalAuthRefreshReason,
    ) -> Result<(), RefreshTokenError> {
        let Some(external_auth) = self.external_auth() else {
            return Err(RefreshTokenError::Transient(std::io::Error::other(
                "external auth is not configured",
            )));
        };
        let previous_account_id = self
            .auth_cached()
            .as_ref()
            .and_then(CodexAuth::get_account_id);
        let context = ExternalAuthRefreshContext {
            reason,
            previous_account_id,
        };

        let refreshed = external_auth
            .refresh(context)
            .await
            .map_err(RefreshTokenError::Transient)?;
        self.validate_external_auth(&refreshed)?;
        self.commit_external_auth(refreshed)?;
        Ok(())
    }

    fn commit_external_auth(&self, auth: CodexAuth) -> Result<(), RefreshTokenError> {
        if auth.is_external_chatgpt_tokens() {
            let auth_dot_json = auth.get_current_auth_json().ok_or_else(|| {
                RefreshTokenError::Transient(std::io::Error::other(
                    "external ChatGPT auth tokens are missing auth state",
                ))
            })?;
            // App/connectors paths still construct independent AuthManagers from Config. Mirror
            // external ChatGPT auth into the process-local store so those managers see it too.
            save_auth(
                &self.codex_home,
                &auth_dot_json,
                AuthCredentialsStoreMode::Ephemeral,
                AuthKeyringBackendKind::default(),
            )
            .map_err(RefreshTokenError::Transient)?;
        }

        self.set_cached_auth(Some(auth));
        Ok(())
    }

    fn validate_external_auth(&self, auth: &CodexAuth) -> Result<(), RefreshTokenError> {
        if let Some(account_id) = auth.get_account_id()
            && let Some(expected_workspace_ids) = self.forced_chatgpt_workspace_id()
            && !expected_workspace_ids.contains(&account_id)
        {
            return Err(RefreshTokenError::Transient(std::io::Error::other(
                format!(
                    "external auth returned workspace {account_id:?}, expected one of {expected_workspace_ids:?}"
                ),
            )));
        }
        Ok(())
    }

    // Refreshes ChatGPT OAuth tokens, persists the updated auth state, and
    // reloads the in-memory cache so callers immediately observe new tokens.
    async fn refresh_and_persist_chatgpt_token(
        &self,
        auth: &ChatgptAuth,
        refresh_token: String,
    ) -> Result<(), RefreshTokenError> {
        let refresh_response = request_chatgpt_token_refresh(refresh_token, auth.client()).await?;

        persist_tokens(
            auth.storage(),
            refresh_response.id_token,
            refresh_response.access_token,
            refresh_response.refresh_token,
        )
        .map_err(RefreshTokenError::from)?;
        self.reload().await;

        Ok(())
    }
}

#[cfg(test)]
#[path = "auth_tests.rs"]
mod tests;
