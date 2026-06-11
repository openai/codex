use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;
use std::time::Instant;

use crate::CredentialSourceConfig;
use crate::WorkloadIdentityConfig;
#[cfg(feature = "azure")]
use crate::azure::AzureSubjectTokenSource;
use reqwest::StatusCode;
use serde::Deserialize;
use serde::Serialize;
use thiserror::Error;
use tokio::sync::Semaphore;

const TOKEN_EXCHANGE_GRANT_TYPE: &str = "urn:ietf:params:oauth:grant-type:token-exchange";
const JWT_TOKEN_TYPE: &str = "urn:ietf:params:oauth:token-type:jwt";
const ACCESS_TOKEN_TYPE: &str = "urn:ietf:params:oauth:token-type:access_token";
const MAX_REFRESH_LEAD: Duration = Duration::from_secs(5 * 60);
const TOKEN_EXCHANGE_TIMEOUT: Duration = Duration::from_secs(10);
const FAILED_EXCHANGE_RETRY_DELAY: Duration = Duration::from_secs(5);

#[derive(Clone, PartialEq, Eq)]
pub struct WorkloadIdentityAccessToken {
    pub access_token: String,
    pub chatgpt_account_id: String,
    pub chatgpt_plan_type: Option<String>,
}

impl std::fmt::Debug for WorkloadIdentityAccessToken {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("WorkloadIdentityAccessToken")
            .field("access_token", &"[REDACTED]")
            .field("chatgpt_account_id", &self.chatgpt_account_id)
            .field("chatgpt_plan_type", &self.chatgpt_plan_type)
            .finish()
    }
}

#[derive(Debug, Error)]
pub enum WorkloadIdentityError {
    #[error("AZURE_FEDERATED_TOKEN_FILE is not set and no Azure token_file was configured")]
    MissingAzureFederatedTokenFile,
    #[error("failed to read workload identity subject token from {path}: {source}")]
    ReadSubjectToken {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("workload identity subject token file is empty")]
    EmptySubjectToken,
    #[error("the configured credential source is not enabled in this Codex build")]
    CredentialSourceNotEnabled,
    #[error("workload identity token exchange request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("workload identity token exchange was rejected with HTTP {status}: {message}")]
    Rejected { status: StatusCode, message: String },
    #[error("workload identity token exchange returned an empty access token")]
    EmptyAccessToken,
    #[error("workload identity token exchange returned an empty ChatGPT account ID")]
    EmptyAccountId,
    #[error("workload identity token exchange returned a token with no usable lifetime")]
    InvalidLifetime,
    #[error("workload identity token exchange returned an unexpected token type")]
    UnexpectedTokenType,
    #[error("workload identity token exchange is unavailable")]
    ExchangeUnavailable,
    #[error("{0}")]
    RecentFailure(String),
}

enum SubjectTokenSource {
    #[cfg(feature = "azure")]
    Azure(AzureSubjectTokenSource),
    #[cfg(not(feature = "azure"))]
    Disabled,
}

impl SubjectTokenSource {
    fn from_config(config: CredentialSourceConfig) -> Self {
        match config {
            CredentialSourceConfig::Azure { token_file } => {
                #[cfg(feature = "azure")]
                {
                    Self::Azure(AzureSubjectTokenSource::new(token_file))
                }
                #[cfg(not(feature = "azure"))]
                {
                    let _ = token_file;
                    Self::Disabled
                }
            }
        }
    }

    async fn subject_token(&self) -> Result<String, WorkloadIdentityError> {
        match self {
            #[cfg(feature = "azure")]
            Self::Azure(source) => source.subject_token().await,
            #[cfg(not(feature = "azure"))]
            Self::Disabled => Err(WorkloadIdentityError::CredentialSourceNotEnabled),
        }
    }
}

struct CachedAccessToken {
    token: WorkloadIdentityAccessToken,
    refresh_at: Instant,
}

struct CachedExchangeFailure {
    message: String,
    retry_at: Instant,
}

#[derive(Default)]
struct CacheState {
    token: Option<CachedAccessToken>,
    failure: Option<CachedExchangeFailure>,
}

pub struct WorkloadIdentityClient {
    identity_provider_id: String,
    identity_provider_mapping_id: String,
    token_url: String,
    client_id: String,
    source: SubjectTokenSource,
    http: reqwest::Client,
    cache: Mutex<CacheState>,
    exchange_lock: Semaphore,
}

impl WorkloadIdentityClient {
    pub fn new(
        config: WorkloadIdentityConfig,
        client_id: impl Into<String>,
        http: reqwest::Client,
    ) -> Self {
        Self {
            identity_provider_id: config.identity_provider_id,
            identity_provider_mapping_id: config.identity_provider_mapping_id,
            token_url: config.token_url,
            client_id: client_id.into(),
            source: SubjectTokenSource::from_config(config.credential_source),
            http,
            cache: Mutex::new(CacheState::default()),
            exchange_lock: Semaphore::new(/*permits*/ 1),
        }
    }

    pub async fn resolve(&self) -> Result<WorkloadIdentityAccessToken, WorkloadIdentityError> {
        if let Some(cached) = self.cached_result() {
            return cached;
        }
        let _permit = self
            .exchange_lock
            .acquire()
            .await
            .map_err(|_| WorkloadIdentityError::ExchangeUnavailable)?;
        if let Some(cached) = self.cached_result() {
            return cached;
        }
        self.exchange_and_record().await
    }

    pub async fn refresh(&self) -> Result<WorkloadIdentityAccessToken, WorkloadIdentityError> {
        let _permit = self
            .exchange_lock
            .acquire()
            .await
            .map_err(|_| WorkloadIdentityError::ExchangeUnavailable)?;
        self.exchange_and_record().await
    }

    fn cached_result(&self) -> Option<Result<WorkloadIdentityAccessToken, WorkloadIdentityError>> {
        let cache = self
            .cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let now = Instant::now();
        if let Some(failure) = cache.failure.as_ref()
            && now < failure.retry_at
        {
            return Some(Err(WorkloadIdentityError::RecentFailure(
                failure.message.clone(),
            )));
        }
        cache
            .token
            .as_ref()
            .filter(|cached| now < cached.refresh_at)
            .map(|cached| Ok(cached.token.clone()))
    }

    async fn exchange_and_record(
        &self,
    ) -> Result<WorkloadIdentityAccessToken, WorkloadIdentityError> {
        let result = self.exchange_and_cache().await;
        if let Err(error) = &result {
            self.cache
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .failure = Some(CachedExchangeFailure {
                message: error.to_string(),
                retry_at: Instant::now() + FAILED_EXCHANGE_RETRY_DELAY,
            });
        }
        result
    }

    async fn exchange_and_cache(
        &self,
    ) -> Result<WorkloadIdentityAccessToken, WorkloadIdentityError> {
        let subject_token = self.source.subject_token().await?;
        let response = self
            .http
            .post(&self.token_url)
            .timeout(TOKEN_EXCHANGE_TIMEOUT)
            .json(&TokenExchangeRequest {
                grant_type: TOKEN_EXCHANGE_GRANT_TYPE,
                requested_token_type: ACCESS_TOKEN_TYPE,
                subject_token: &subject_token,
                subject_token_type: JWT_TOKEN_TYPE,
                identity_provider_id: &self.identity_provider_id,
                identity_provider_mapping_id: &self.identity_provider_mapping_id,
                client_id: &self.client_id,
            })
            .send()
            .await?;
        let status = response.status();
        if !status.is_success() {
            let message = response
                .json::<TokenExchangeErrorResponse>()
                .await
                .ok()
                .and_then(|response| response.error)
                .unwrap_or_else(|| "token endpoint rejected the request".to_string());
            return Err(WorkloadIdentityError::Rejected { status, message });
        }
        let response: TokenExchangeResponse = response.json().await?;
        if !response.token_type.eq_ignore_ascii_case("bearer")
            || response.issued_token_type != ACCESS_TOKEN_TYPE
        {
            return Err(WorkloadIdentityError::UnexpectedTokenType);
        }
        if response.access_token.trim().is_empty() {
            return Err(WorkloadIdentityError::EmptyAccessToken);
        }
        if response.chatgpt_account_id.trim().is_empty() {
            return Err(WorkloadIdentityError::EmptyAccountId);
        }
        if response.expires_in == 0 {
            return Err(WorkloadIdentityError::InvalidLifetime);
        }

        let token = WorkloadIdentityAccessToken {
            access_token: response.access_token,
            chatgpt_account_id: response.chatgpt_account_id,
            chatgpt_plan_type: response.chatgpt_plan_type,
        };
        let lifetime = Duration::from_secs(response.expires_in);
        let refresh_lead = MAX_REFRESH_LEAD.min(Duration::from_secs(
            response.expires_in.saturating_div(10).max(1),
        ));
        let refresh_at = Instant::now()
            .checked_add(lifetime.saturating_sub(refresh_lead))
            .ok_or(WorkloadIdentityError::InvalidLifetime)?;
        *self
            .cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = CacheState {
            failure: None,
            token: Some(CachedAccessToken {
                token: token.clone(),
                refresh_at,
            }),
        };
        Ok(token)
    }
}

#[derive(Serialize)]
struct TokenExchangeRequest<'a> {
    grant_type: &'static str,
    requested_token_type: &'static str,
    subject_token: &'a str,
    subject_token_type: &'static str,
    identity_provider_id: &'a str,
    identity_provider_mapping_id: &'a str,
    client_id: &'a str,
}

#[derive(Deserialize)]
struct TokenExchangeResponse {
    access_token: String,
    issued_token_type: String,
    token_type: String,
    expires_in: u64,
    chatgpt_account_id: String,
    #[serde(default)]
    chatgpt_plan_type: Option<String>,
}

#[derive(Deserialize)]
struct TokenExchangeErrorResponse {
    #[serde(default)]
    error: Option<String>,
}

#[cfg(all(test, feature = "azure"))]
#[path = "client_tests.rs"]
mod tests;
