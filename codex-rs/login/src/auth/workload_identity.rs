use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::Weak;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use codex_http_client::HttpClientFactory;
use codex_protocol::config_types::ForcedLoginMethod;
use codex_protocol::shell_environment::OPENAI_FEDERATION_RULE_ID_ENV_VAR;
use codex_protocol::shell_environment::OPENAI_IDENTITY_TOKEN_FILE_ENV_VAR;
use codex_workload_identity::WorkloadIdentityConfig;
use codex_workload_identity::WorkloadIdentityError;
use codex_workload_identity::WorkloadIdentityExchange;
use codex_workload_identity::WorkloadIdentityToken;
use thiserror::Error;
use url::Url;

use super::AuthConfig;
use super::CodexAuth;
use super::ExternalAuth;
use super::ExternalAuthFuture;
use super::ExternalAuthRefreshContext;
use super::RefreshTokenError;
use super::RefreshTokenFailedError;
use super::RefreshTokenFailedReason;
use super::read_codex_access_token_from_env;
use super::read_codex_api_key_from_env;
use crate::AuthRouteConfig;

const PROD_TOKEN_URL: &str = "https://auth.openai.com/oauth/token";
const STAGING_TOKEN_URL: &str = "https://auth.api.openai.org/oauth/token";
// copybara:strip-for-public begin
const PRIVATE_STAGING_BACKEND_ROOT: &str =
    "https://codex-backend.gateway.unified-0s.internal.api.openai.org";
// copybara:strip-for-public end

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WorkloadIdentityEnvironment {
    Production,
    Staging,
}

impl WorkloadIdentityEnvironment {
    fn token_url(self) -> Result<Url, WorkloadIdentitySessionError> {
        Url::parse(match self {
            Self::Production => PROD_TOKEN_URL,
            Self::Staging => STAGING_TOKEN_URL,
        })
        .map_err(|_| invalid_config("workload identity token endpoint is invalid"))
    }
}

#[derive(Clone, Debug)]
struct WorkloadIdentitySessionConfig {
    assertion_file: PathBuf,
    environment: WorkloadIdentityEnvironment,
    federation_rule_id: String,
    http_client_factory: HttpClientFactory,
    token_url: Url,
}

impl WorkloadIdentitySessionConfig {
    fn fingerprint(&self) -> WorkloadIdentityFingerprint {
        WorkloadIdentityFingerprint {
            assertion_file: self.assertion_file.clone(),
            environment: self.environment,
            federation_rule_id: self.federation_rule_id.trim().to_string(),
            token_url: self.token_url.to_string(),
        }
    }

    fn into_exchange(self) -> Result<WorkloadIdentityExchange, WorkloadIdentityError> {
        WorkloadIdentityExchange::new(
            WorkloadIdentityConfig::new(self.federation_rule_id, self.assertion_file)?,
            self.token_url,
            self.http_client_factory,
        )
    }
}

#[derive(Clone, PartialEq, Eq)]
struct WorkloadIdentityFingerprint {
    assertion_file: PathBuf,
    environment: WorkloadIdentityEnvironment,
    federation_rule_id: String,
    token_url: String,
}

#[derive(Debug, Error)]
pub(super) enum WorkloadIdentitySessionError {
    #[error(transparent)]
    Exchange(#[from] WorkloadIdentityError),
    #[error("a different workload identity configuration is already active in this process")]
    ConflictingConfiguration,
    #[error("the workload identity process-session registry is unavailable")]
    RegistryUnavailable,
    #[error("{0}")]
    InvalidConfiguration(String),
}

pub fn is_workload_identity_selected(
    auth_config: &AuthConfig,
    enable_codex_api_key_env: bool,
) -> bool {
    !has_explicit_process_auth(auth_config, enable_codex_api_key_env)
        && ProcessEnvironment::read().has_marker()
}

fn resolve_config(
    chatgpt_base_url: &str,
    environment: ProcessEnvironment,
    has_explicit_process_auth: bool,
    chatgpt_login_allowed: bool,
    auth_route_config: AuthRouteConfig,
) -> Result<Option<WorkloadIdentitySessionConfig>, WorkloadIdentitySessionError> {
    if has_explicit_process_auth || !environment.has_marker() {
        return Ok(None);
    }
    if !chatgpt_login_allowed {
        return Err(invalid_config(
            "workload identity requires a login policy that permits ChatGPT authentication",
        ));
    }

    let federation_rule_id = required_unicode(
        environment.federation_rule_id,
        OPENAI_FEDERATION_RULE_ID_ENV_VAR,
    )?;
    let assertion_file = required_path(
        environment.identity_token_file,
        OPENAI_IDENTITY_TOKEN_FILE_ENV_VAR,
    )?;
    let auth_environment = classify_auth_environment(chatgpt_base_url)?;

    Ok(Some(WorkloadIdentitySessionConfig {
        assertion_file,
        environment: auth_environment,
        federation_rule_id,
        http_client_factory: auth_route_config.http_client_factory().clone(),
        token_url: auth_environment.token_url()?,
    }))
}

fn has_explicit_process_auth(auth_config: &AuthConfig, enable_codex_api_key_env: bool) -> bool {
    (enable_codex_api_key_env
        && auth_config.is_login_method_allowed(ForcedLoginMethod::Api)
        && read_codex_api_key_from_env().is_some())
        || (auth_config.is_login_method_allowed(ForcedLoginMethod::Chatgpt)
            && read_codex_access_token_from_env().is_some())
}

fn required_path(
    value: Option<OsString>,
    variable: &'static str,
) -> Result<PathBuf, WorkloadIdentitySessionError> {
    let path = PathBuf::from(
        value.ok_or_else(|| invalid_config(format!("workload identity requires {variable}")))?,
    );
    if !path.is_absolute() {
        return Err(invalid_config(format!(
            "{variable} must be an absolute path"
        )));
    }
    Ok(path)
}

fn required_unicode(
    value: Option<OsString>,
    variable: &'static str,
) -> Result<String, WorkloadIdentitySessionError> {
    let value = value
        .ok_or_else(|| invalid_config(format!("workload identity requires {variable}")))?
        .into_string()
        .map_err(|_| invalid_config(format!("workload identity variable {variable} is invalid")))?;
    let value = value.trim();
    if value.is_empty() {
        return Err(invalid_config(format!(
            "workload identity variable {variable} is invalid"
        )));
    }
    Ok(value.to_string())
}

fn classify_auth_environment(
    base_url: &str,
) -> Result<WorkloadIdentityEnvironment, WorkloadIdentitySessionError> {
    match base_url.trim().trim_end_matches('/') {
        "https://chatgpt.com"
        | "https://chatgpt.com/backend-api"
        | "https://chatgpt.com/codex"
        | "https://chatgpt.com/backend-api/codex"
        | "https://chat.openai.com"
        | "https://chat.openai.com/backend-api"
        | "https://chat.openai.com/codex"
        | "https://chat.openai.com/backend-api/codex" => {
            Ok(WorkloadIdentityEnvironment::Production)
        }
        "https://chatgpt-staging.com"
        | "https://chatgpt-staging.com/backend-api"
        | "https://chatgpt-staging.com/codex"
        | "https://chatgpt-staging.com/backend-api/codex" => {
            Ok(WorkloadIdentityEnvironment::Staging)
        }
        // copybara:strip-for-public begin
        PRIVATE_STAGING_BACKEND_ROOT => Ok(WorkloadIdentityEnvironment::Staging),
        // copybara:strip-for-public end
        _ => Err(invalid_config(
            "workload identity auth supports only trusted production and staging app routing",
        )),
    }
}

fn invalid_config(message: impl Into<String>) -> WorkloadIdentitySessionError {
    WorkloadIdentitySessionError::InvalidConfiguration(message.into())
}

#[derive(Default)]
struct ProcessEnvironment {
    federation_rule_id: Option<OsString>,
    identity_token_file: Option<OsString>,
}

impl ProcessEnvironment {
    fn read() -> Self {
        Self {
            federation_rule_id: std::env::var_os(OPENAI_FEDERATION_RULE_ID_ENV_VAR),
            identity_token_file: std::env::var_os(OPENAI_IDENTITY_TOKEN_FILE_ENV_VAR),
        }
    }

    fn has_marker(&self) -> bool {
        self.federation_rule_id.is_some() || self.identity_token_file.is_some()
    }
}

#[derive(Clone, PartialEq, Eq)]
struct WorkloadIdentitySubject {
    account_id: String,
    account_user_id: String,
    user_id: String,
}

impl From<&WorkloadIdentityToken> for WorkloadIdentitySubject {
    fn from(token: &WorkloadIdentityToken) -> Self {
        Self {
            account_id: token.chatgpt_account_id.clone(),
            account_user_id: token.chatgpt_account_user_id.clone(),
            user_id: token.user_id.clone(),
        }
    }
}

struct WorkloadIdentitySession {
    exchange: WorkloadIdentityExchange,
    subject: Mutex<Option<WorkloadIdentitySubject>>,
}

impl WorkloadIdentitySession {
    fn new(config: WorkloadIdentitySessionConfig) -> Result<Self, WorkloadIdentitySessionError> {
        Ok(Self {
            exchange: config.into_exchange()?,
            subject: Mutex::new(None),
        })
    }

    fn accept_subject(
        &self,
        token: &WorkloadIdentityToken,
        previous_account_id: Option<&str>,
    ) -> Result<(), WorkloadIdentityError> {
        if previous_account_id.is_some_and(|account_id| account_id != token.chatgpt_account_id) {
            return Err(WorkloadIdentityError::InvalidExchangeResponse);
        }
        let subject = WorkloadIdentitySubject::from(token);
        let mut current = self
            .subject
            .lock()
            .map_err(|_| WorkloadIdentityError::InvalidExchangeResponse)?;
        match current.as_ref() {
            Some(current) if current != &subject => {
                Err(WorkloadIdentityError::InvalidExchangeResponse)
            }
            Some(_) => Ok(()),
            None => {
                *current = Some(subject);
                Ok(())
            }
        }
    }
}

#[derive(Default)]
struct WorkloadIdentitySessionRegistry {
    entry: Mutex<Option<WorkloadIdentitySessionEntry>>,
}

struct WorkloadIdentitySessionEntry {
    fingerprint: WorkloadIdentityFingerprint,
    session: Weak<WorkloadIdentitySession>,
}

impl WorkloadIdentitySessionRegistry {
    fn has_active_session(&self) -> Result<bool, WorkloadIdentitySessionError> {
        let entry = self
            .entry
            .lock()
            .map_err(|_| WorkloadIdentitySessionError::RegistryUnavailable)?;
        Ok(entry
            .as_ref()
            .and_then(|active| active.session.upgrade())
            .is_some())
    }

    fn session(
        &self,
        config: WorkloadIdentitySessionConfig,
    ) -> Result<Arc<WorkloadIdentitySession>, WorkloadIdentitySessionError> {
        let fingerprint = config.fingerprint();
        let mut entry = self
            .entry
            .lock()
            .map_err(|_| WorkloadIdentitySessionError::RegistryUnavailable)?;
        if let Some(active) = entry.as_ref()
            && let Some(session) = active.session.upgrade()
        {
            if active.fingerprint == fingerprint {
                return Ok(session);
            }
            return Err(WorkloadIdentitySessionError::ConflictingConfiguration);
        }

        let session = Arc::new(WorkloadIdentitySession::new(config)?);
        *entry = Some(WorkloadIdentitySessionEntry {
            fingerprint,
            session: Arc::downgrade(&session),
        });
        Ok(session)
    }
}

fn process_registry() -> &'static WorkloadIdentitySessionRegistry {
    static REGISTRY: OnceLock<WorkloadIdentitySessionRegistry> = OnceLock::new();
    REGISTRY.get_or_init(WorkloadIdentitySessionRegistry::default)
}

pub(super) struct WorkloadIdentityExternalAuth {
    observed_token_version: AtomicU64,
    session: Arc<WorkloadIdentitySession>,
}

impl WorkloadIdentityExternalAuth {
    pub(super) fn from_process_config(
        auth_config: &AuthConfig,
        enable_codex_api_key_env: bool,
    ) -> Result<Option<Self>, WorkloadIdentitySessionError> {
        let registry = process_registry();
        let active_session = registry.has_active_session()?;
        resolve_config(
            auth_config
                .chatgpt_base_url
                .as_deref()
                .unwrap_or("https://chatgpt.com/backend-api"),
            ProcessEnvironment::read(),
            has_explicit_process_auth(auth_config, enable_codex_api_key_env) && !active_session,
            auth_config.is_login_method_allowed(ForcedLoginMethod::Chatgpt),
            auth_config.auth_route_config.clone(),
        )?
        .map(|config| Self::from_config_with_registry(config, registry))
        .transpose()
    }

    fn from_config_with_registry(
        config: WorkloadIdentitySessionConfig,
        registry: &WorkloadIdentitySessionRegistry,
    ) -> Result<Self, WorkloadIdentitySessionError> {
        Ok(Self {
            observed_token_version: AtomicU64::new(0),
            session: registry.session(config)?,
        })
    }

    async fn build_validated_auth(
        &self,
        token: WorkloadIdentityToken,
        previous_account_id: Option<&str>,
    ) -> std::io::Result<CodexAuth> {
        let token_version = token.version();
        let result = self.validate_auth(&token, previous_account_id);
        if result.is_err() {
            self.session
                .exchange
                .invalidate_if_current(token_version)
                .await;
        }
        let auth = result?;
        self.observed_token_version
            .store(token_version, Ordering::Release);
        Ok(auth)
    }

    fn validate_auth(
        &self,
        token: &WorkloadIdentityToken,
        previous_account_id: Option<&str>,
    ) -> std::io::Result<CodexAuth> {
        let auth = CodexAuth::from_external_chatgpt_tokens(
            &token.access_token,
            &token.chatgpt_account_id,
            token.chatgpt_plan_type.as_deref(),
        )
        .map_err(|_| std::io::Error::other(WorkloadIdentityError::InvalidExchangeResponse))?;
        if auth.get_chatgpt_user_id().as_deref() != Some(token.user_id.as_str()) {
            return Err(std::io::Error::other(
                WorkloadIdentityError::InvalidExchangeResponse,
            ));
        }
        self.session
            .accept_subject(token, previous_account_id)
            .map_err(std::io::Error::other)?;
        Ok(auth)
    }
}

impl ExternalAuth for WorkloadIdentityExternalAuth {
    fn resolve(&self) -> ExternalAuthFuture<'_, CodexAuth> {
        Box::pin(async move {
            let token = self
                .session
                .exchange
                .resolve()
                .await
                .map_err(std::io::Error::other)?;
            self.build_validated_auth(token, /*previous_account_id*/ None)
                .await
        })
    }

    fn refresh(&self, context: ExternalAuthRefreshContext) -> ExternalAuthFuture<'_, CodexAuth> {
        Box::pin(async move {
            let observed_version = self.observed_token_version.load(Ordering::Acquire);
            let token = self
                .session
                .exchange
                .refresh(observed_version)
                .await
                .map_err(std::io::Error::other)?;
            self.build_validated_auth(token, context.previous_account_id.as_deref())
                .await
        })
    }

    fn classify_error(&self, error: std::io::Error) -> RefreshTokenError {
        if error
            .get_ref()
            .and_then(|source| source.downcast_ref::<WorkloadIdentityError>())
            .is_some_and(WorkloadIdentityError::is_transient)
        {
            return RefreshTokenError::Transient(error);
        }

        RefreshTokenError::Permanent(RefreshTokenFailedError::new(
            RefreshTokenFailedReason::Other,
            error.to_string(),
        ))
    }
}

#[cfg(test)]
#[path = "workload_identity_tests.rs"]
mod tests;
