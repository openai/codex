use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::Weak;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use codex_http_client::HttpClientFactory;
use codex_protocol::shell_environment::OPENAI_FEDERATION_RULE_ID_ENV_VAR;
use codex_protocol::shell_environment::OPENAI_IDENTITY_TOKEN_FILE_ENV_VAR;
use codex_protocol::shell_environment::OPENAI_IDENTITY_TOKEN_SOURCE_ENV_VAR;
use codex_protocol::shell_environment::OPENAI_SPIFFE_AUDIENCE_ENV_VAR;
use codex_protocol::shell_environment::OPENAI_SPIFFE_ID_ENV_VAR;
use codex_protocol::shell_environment::OPENAI_WORKLOAD_IDENTITY_CONTEXT_ENV_VAR;
use codex_protocol::shell_environment::SPIFFE_ENDPOINT_SOCKET_ENV_VAR;
use codex_workload_identity::AssertionSource;
use codex_workload_identity::SpiffeWorkloadApi;
use codex_workload_identity::WorkloadIdentityConfig;
use codex_workload_identity::WorkloadIdentityError;
use codex_workload_identity::WorkloadIdentityExchange;
use codex_workload_identity::WorkloadIdentityMetadata;
use codex_workload_identity::WorkloadIdentityToken;
use thiserror::Error;
use url::Url;

use super::ApiKeyAuth;
use super::AuthConfig;
use super::CodexAuth;
use super::ExternalAuth;
use super::ExternalAuthFuture;
use super::ExternalAuthRefreshContext;
use super::RefreshTokenError;
use super::RefreshTokenFailedError;
use super::RefreshTokenFailedReason;
use crate::AuthRouteConfig;

const PROD_TOKEN_URL: &str = "https://auth.openai.com/oauth/token";
const STAGING_TOKEN_URL: &str = "https://auth.api.openai.org/oauth/token";

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

#[derive(Clone)]
struct WorkloadIdentitySessionConfig {
    assertion_source: AssertionSource,
    environment: WorkloadIdentityEnvironment,
    federation_rule_id: String,
    http_client_factory: HttpClientFactory,
    token_url: Url,
    workload_identity_context: Option<String>,
}

impl std::fmt::Debug for WorkloadIdentitySessionConfig {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("WorkloadIdentitySessionConfig")
            .field("assertion_source", &self.assertion_source)
            .field("environment", &self.environment)
            .field("federation_rule_id", &self.federation_rule_id)
            .field("http_client_factory", &self.http_client_factory)
            .field("token_url", &self.token_url)
            .field(
                "workload_identity_context",
                &self
                    .workload_identity_context
                    .as_ref()
                    .map(|_| "[redacted]"),
            )
            .finish()
    }
}

impl WorkloadIdentitySessionConfig {
    fn fingerprint(&self) -> WorkloadIdentityFingerprint {
        WorkloadIdentityFingerprint {
            assertion_source: self.assertion_source.clone(),
            environment: self.environment,
            federation_rule_id: self.federation_rule_id.trim().to_string(),
            token_url: self.token_url.to_string(),
            workload_identity_context: self.workload_identity_context.clone(),
        }
    }

    fn into_exchange(self) -> Result<WorkloadIdentityExchange, WorkloadIdentityError> {
        let config = WorkloadIdentityConfig::new(
            self.federation_rule_id,
            self.assertion_source,
            self.workload_identity_context,
        )?;
        WorkloadIdentityExchange::new(config, self.token_url, self.http_client_factory)
    }
}

#[derive(Clone, PartialEq, Eq)]
struct WorkloadIdentityFingerprint {
    assertion_source: AssertionSource,
    environment: WorkloadIdentityEnvironment,
    federation_rule_id: String,
    token_url: String,
    workload_identity_context: Option<String>,
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

/// Returns whether workload identity was selected through process configuration.
///
/// OpenAI workload identity markers select this mode; an ambient SPIFFE socket alone does not.
/// Partial configuration fails validation rather than falling back to another credential source.
pub fn is_workload_identity_selected() -> bool {
    ProcessEnvironment::read().has_marker()
}

fn resolve_config(
    chatgpt_base_url: &str,
    environment: ProcessEnvironment,
    auth_route_config: AuthRouteConfig,
) -> Result<Option<WorkloadIdentitySessionConfig>, WorkloadIdentitySessionError> {
    if !environment.has_marker() {
        return Ok(None);
    }

    let federation_rule_id = required_unicode(
        environment.federation_rule_id,
        OPENAI_FEDERATION_RULE_ID_ENV_VAR,
    )?;
    let source = optional_unicode(
        environment.identity_token_source,
        OPENAI_IDENTITY_TOKEN_SOURCE_ENV_VAR,
    )?;
    let assertion_source = match source.as_deref() {
        None | Some("file") => {
            if environment.spiffe_audience.is_some() || environment.spiffe_id.is_some() {
                return Err(invalid_config(
                    "SPIFFE settings require OPENAI_IDENTITY_TOKEN_SOURCE=spiffe",
                ));
            }
            AssertionSource::File(required_path(
                environment.identity_token_file,
                OPENAI_IDENTITY_TOKEN_FILE_ENV_VAR,
            )?)
        }
        Some("spiffe") => {
            if environment.identity_token_file.is_some() {
                return Err(invalid_config(
                    "SPIFFE authentication cannot also set OPENAI_IDENTITY_TOKEN_FILE",
                ));
            }
            let endpoint =
                required_unicode(environment.spiffe_endpoint, SPIFFE_ENDPOINT_SOCKET_ENV_VAR)?;
            let audience =
                required_unicode(environment.spiffe_audience, OPENAI_SPIFFE_AUDIENCE_ENV_VAR)?;
            let spiffe_id = optional_unicode(environment.spiffe_id, OPENAI_SPIFFE_ID_ENV_VAR)?;
            AssertionSource::Spiffe(SpiffeWorkloadApi::new(&endpoint, audience, spiffe_id)?)
        }
        Some(_) => {
            return Err(invalid_config(
                "OPENAI_IDENTITY_TOKEN_SOURCE must be file or spiffe",
            ));
        }
    };
    let workload_identity_context = optional_unicode(
        environment.workload_identity_context,
        OPENAI_WORKLOAD_IDENTITY_CONTEXT_ENV_VAR,
    )?;
    let auth_environment = classify_auth_environment(chatgpt_base_url)?;

    Ok(Some(WorkloadIdentitySessionConfig {
        assertion_source,
        environment: auth_environment,
        federation_rule_id,
        http_client_factory: auth_route_config
            .authentication_factory(auth_environment.token_url()?.as_str()),
        token_url: auth_environment.token_url()?,
        workload_identity_context,
    }))
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

fn optional_unicode(
    value: Option<OsString>,
    variable: &'static str,
) -> Result<Option<String>, WorkloadIdentitySessionError> {
    value
        .map(|value| {
            value.into_string().map_err(|_| {
                invalid_config(format!("workload identity variable {variable} is invalid"))
            })
        })
        .transpose()
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
    identity_token_source: Option<OsString>,
    spiffe_endpoint: Option<OsString>,
    spiffe_audience: Option<OsString>,
    spiffe_id: Option<OsString>,
    workload_identity_context: Option<OsString>,
}

impl ProcessEnvironment {
    fn read() -> Self {
        Self {
            federation_rule_id: std::env::var_os(OPENAI_FEDERATION_RULE_ID_ENV_VAR),
            identity_token_file: std::env::var_os(OPENAI_IDENTITY_TOKEN_FILE_ENV_VAR),
            identity_token_source: std::env::var_os(OPENAI_IDENTITY_TOKEN_SOURCE_ENV_VAR),
            spiffe_endpoint: std::env::var_os(SPIFFE_ENDPOINT_SOCKET_ENV_VAR),
            spiffe_audience: std::env::var_os(OPENAI_SPIFFE_AUDIENCE_ENV_VAR),
            spiffe_id: std::env::var_os(OPENAI_SPIFFE_ID_ENV_VAR),
            workload_identity_context: std::env::var_os(OPENAI_WORKLOAD_IDENTITY_CONTEXT_ENV_VAR),
        }
    }

    fn has_marker(&self) -> bool {
        self.federation_rule_id.is_some()
            || self.identity_token_file.is_some()
            || self.identity_token_source.is_some()
            || self.spiffe_audience.is_some()
            || self.spiffe_id.is_some()
    }
}

#[derive(Clone, PartialEq, Eq)]
enum WorkloadIdentitySubject {
    Chatgpt {
        account_id: String,
        account_user_id: String,
        user_id: String,
    },
    Api {
        organization_id: String,
        project_id: String,
        service_account_id: String,
    },
}

impl From<&WorkloadIdentityMetadata> for WorkloadIdentitySubject {
    fn from(metadata: &WorkloadIdentityMetadata) -> Self {
        match metadata {
            WorkloadIdentityMetadata::Chatgpt(metadata) => Self::Chatgpt {
                account_id: metadata.chatgpt_account_id.clone(),
                account_user_id: metadata.chatgpt_account_user_id.clone(),
                user_id: metadata.user_id.clone(),
            },
            WorkloadIdentityMetadata::Api(metadata) => Self::Api {
                organization_id: metadata.api_organization_id.clone(),
                project_id: metadata.api_project_id.clone(),
                service_account_id: metadata.user_id.clone(),
            },
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
        metadata: &WorkloadIdentityMetadata,
        previous_account_id: Option<&str>,
    ) -> Result<(), WorkloadIdentityError> {
        let account_id = match metadata {
            WorkloadIdentityMetadata::Chatgpt(metadata) => {
                Some(metadata.chatgpt_account_id.as_str())
            }
            WorkloadIdentityMetadata::Api(_) => None,
        };
        if previous_account_id.is_some() && previous_account_id != account_id {
            return Err(WorkloadIdentityError::InvalidExchangeResponse);
        }
        let subject = WorkloadIdentitySubject::from(metadata);
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
    ) -> Result<Option<Self>, WorkloadIdentitySessionError> {
        let registry = process_registry();
        resolve_config(
            auth_config
                .chatgpt_base_url
                .as_deref()
                .unwrap_or("https://chatgpt.com/backend-api"),
            ProcessEnvironment::read(),
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
        let auth = match &token.metadata {
            WorkloadIdentityMetadata::Api(metadata) => {
                if !super::source::has_session_claim(&token.access_token) {
                    return Err(std::io::Error::other(
                        WorkloadIdentityError::InvalidExchangeResponse,
                    ));
                }
                CodexAuth::ApiKey(ApiKeyAuth {
                    api_key: token.access_token.clone(),
                    workload_identity: Some(metadata.clone()),
                })
            }
            WorkloadIdentityMetadata::Chatgpt(metadata) => {
                let auth = CodexAuth::from_external_chatgpt_tokens(
                    &token.access_token,
                    &metadata.chatgpt_account_id,
                    metadata.chatgpt_plan_type.as_deref(),
                )
                .map_err(|_| {
                    std::io::Error::other(WorkloadIdentityError::InvalidExchangeResponse)
                })?;
                if auth.get_chatgpt_user_id().as_deref() != Some(metadata.user_id.as_str()) {
                    return Err(std::io::Error::other(
                        WorkloadIdentityError::InvalidExchangeResponse,
                    ));
                }
                auth
            }
        };
        self.session
            .accept_subject(&token.metadata, previous_account_id)
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
        if let Some(WorkloadIdentityError::Policy(denied)) = error
            .get_ref()
            .and_then(|source| source.downcast_ref::<WorkloadIdentityError>())
        {
            return RefreshTokenError::Policy(*denied);
        }
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
