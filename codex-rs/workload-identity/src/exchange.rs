use std::fmt;
use std::time::Duration;
use std::time::Instant;

use codex_http_client::build_reqwest_client_with_custom_ca;
use reqwest::Client;
use reqwest::ClientBuilder;
use reqwest::StatusCode;
use reqwest::redirect::Policy;
use serde::Deserialize;
use tokio::sync::Mutex;
use tokio::sync::Semaphore;
use tokio::time::sleep;
use url::Host;
use url::Url;

use crate::WorkloadIdentityConfig;
use crate::WorkloadIdentityError;

const DEFAULT_TOKEN_URL: &str = "https://auth.openai.com/oauth/token";
pub(crate) const JWT_BEARER_GRANT_TYPE: &str = "urn:ietf:params:oauth:grant-type:jwt-bearer";
const MAX_ACCESS_TOKEN_LIFETIME: Duration = Duration::from_secs(60 * 60);
const MAX_RESPONSE_BYTES: usize = 1024 * 1024;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const RETRY_BASE_DELAY: Duration = Duration::from_millis(100);
const RETRY_LIMIT: u32 = 2;
const TOKEN_URL_OVERRIDE_ENV_VAR: &str = "CODEX_WIF_TOKEN_URL_OVERRIDE";

/// Exchanges assertions and retains only the current short-lived token in memory.
pub struct WorkloadIdentityExchange {
    cache: Mutex<CacheState>,
    client: Client,
    config: WorkloadIdentityConfig,
    exchange_gate: Semaphore,
    token_url: Url,
}

impl WorkloadIdentityExchange {
    pub fn new(config: WorkloadIdentityConfig) -> Result<Self, WorkloadIdentityError> {
        let (token_url, is_loopback_override) = token_url_from_environment()?;
        let builder = Client::builder()
            .redirect(Policy::none())
            .timeout(REQUEST_TIMEOUT);
        let builder = if is_loopback_override {
            builder.no_proxy()
        } else {
            builder
        };
        Self::with_client_builder(config, token_url, builder)
    }

    pub(crate) fn with_client_builder(
        config: WorkloadIdentityConfig,
        token_url: Url,
        builder: ClientBuilder,
    ) -> Result<Self, WorkloadIdentityError> {
        let client = build_reqwest_client_with_custom_ca(builder)
            .map_err(|_| WorkloadIdentityError::HttpClientConfiguration)?;
        Ok(Self {
            cache: Mutex::new(CacheState::default()),
            client,
            config,
            exchange_gate: Semaphore::new(1),
            token_url,
        })
    }

    /// Returns a cached token when possible and otherwise performs an exchange.
    pub async fn resolve(&self) -> Result<WorkloadIdentityToken, WorkloadIdentityError> {
        self.exchange(ExchangeMode::Resolve).await
    }

    /// Forces one fresh exchange after a downstream service rejects the current token.
    pub async fn refresh(&self) -> Result<WorkloadIdentityToken, WorkloadIdentityError> {
        self.exchange(ExchangeMode::Refresh).await
    }

    async fn exchange(
        &self,
        mode: ExchangeMode,
    ) -> Result<WorkloadIdentityToken, WorkloadIdentityError> {
        let now = Instant::now();
        let observed_generation = {
            let state = self.cache.lock().await;
            if mode == ExchangeMode::Resolve
                && let Some(cached) = &state.cached
                && cached.refresh_at > now
            {
                return Ok(cached.token.clone());
            }
            state.generation
        };
        let _permit = self
            .exchange_gate
            .acquire()
            .await
            .map_err(|_| WorkloadIdentityError::ExchangeUnavailable)?;
        let now = Instant::now();
        let (fallback, next_generation) = {
            let state = self.cache.lock().await;
            if state.generation != observed_generation
                && let Some(cached) = &state.cached
            {
                return Ok(cached.token.clone());
            }
            if mode == ExchangeMode::Resolve
                && let Some(cached) = &state.cached
                && cached.refresh_at > now
            {
                return Ok(cached.token.clone());
            }
            let fallback = match mode {
                ExchangeMode::Refresh => None,
                ExchangeMode::Resolve => state
                    .cached
                    .as_ref()
                    .filter(|cached| cached.mandatory_refresh_at > now)
                    .map(|cached| cached.token.clone()),
            };
            (fallback, state.generation.saturating_add(1))
        };
        let token = match self.exchange_uncached().await {
            Ok(token) => token,
            Err(error) => {
                let Some(fallback) = fallback else {
                    return Err(error);
                };
                let mut state = self.cache.lock().await;
                if let Some(cached) = state.cached.as_mut() {
                    cached.refresh_at =
                        std::cmp::min(now + Duration::from_secs(30), cached.mandatory_refresh_at);
                }
                state.generation = next_generation;
                return Ok(fallback);
            }
        };
        let mut state = self.cache.lock().await;
        state.generation = next_generation;
        state.cached = Some(CachedToken::new(token.clone(), now));
        Ok(token)
    }

    async fn exchange_uncached(&self) -> Result<WorkloadIdentityToken, WorkloadIdentityError> {
        let assertion = self.config.assertion_source.assertion().await?;
        let target = &self.config.target;
        let form = [
            ("grant_type", JWT_BEARER_GRANT_TYPE),
            ("assertion", assertion.as_str()),
            ("federation_rule_id", target.federation_rule_id.as_str()),
            ("tenant_id", target.tenant_id.as_str()),
            ("principal_id", target.principal_id.as_str()),
            ("workspace_id", target.workspace_id.as_str()),
        ];
        let mut attempt = 0;
        let mut response = loop {
            match self
                .client
                .post(self.token_url.clone())
                .form(&form)
                .send()
                .await
            {
                Ok(response) if is_retryable_status(response.status()) && attempt < RETRY_LIMIT => {
                    attempt += 1;
                    sleep(retry_delay(attempt)).await;
                }
                Ok(response) => break response,
                Err(_) if attempt < RETRY_LIMIT => {
                    attempt += 1;
                    sleep(retry_delay(attempt)).await;
                }
                Err(_) => return Err(WorkloadIdentityError::ExchangeUnavailable),
            }
        };
        if !response.status().is_success() {
            return Err(WorkloadIdentityError::ExchangeRejected(
                response.status().as_u16(),
            ));
        }
        if response
            .content_length()
            .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
        {
            return Err(WorkloadIdentityError::InvalidExchangeResponse);
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| WorkloadIdentityError::InvalidExchangeResponse)?
        {
            if chunk.len() > MAX_RESPONSE_BYTES.saturating_sub(bytes.len()) {
                return Err(WorkloadIdentityError::InvalidExchangeResponse);
            }
            bytes.extend_from_slice(&chunk);
        }
        let response: TokenExchangeResponse = serde_json::from_slice(&bytes)
            .map_err(|_| WorkloadIdentityError::InvalidExchangeResponse)?;
        response.into_token(&self.config.target)
    }
}

fn retry_delay(attempt: u32) -> Duration {
    RETRY_BASE_DELAY.saturating_mul(2_u32.saturating_pow(attempt.saturating_sub(1)))
}

fn is_retryable_status(status: StatusCode) -> bool {
    status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error()
}

fn token_url_from_environment() -> Result<(Url, bool), WorkloadIdentityError> {
    let Some(value) = std::env::var_os(TOKEN_URL_OVERRIDE_ENV_VAR) else {
        return Url::parse(DEFAULT_TOKEN_URL)
            .map(|url| (url, false))
            .map_err(|_| WorkloadIdentityError::InvalidTokenUrl);
    };
    let value = value
        .into_string()
        .map_err(|_| WorkloadIdentityError::InvalidTokenUrl)?;
    parse_loopback_token_url(&value).map(|url| (url, true))
}

pub(crate) fn parse_loopback_token_url(value: &str) -> Result<Url, WorkloadIdentityError> {
    let url = Url::parse(value).map_err(|_| WorkloadIdentityError::InvalidTokenUrl)?;
    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(WorkloadIdentityError::InvalidTokenUrl);
    }
    let loopback = match url.host() {
        Some(Host::Domain(domain)) => domain.eq_ignore_ascii_case("localhost"),
        Some(Host::Ipv4(address)) => address.is_loopback(),
        Some(Host::Ipv6(address)) => address.is_loopback(),
        None => false,
    };
    if !matches!(url.scheme(), "http" | "https") || !loopback {
        return Err(WorkloadIdentityError::InvalidTokenUrl);
    }
    Ok(url)
}

#[derive(Default)]
struct CacheState {
    cached: Option<CachedToken>,
    generation: u64,
}

struct CachedToken {
    mandatory_refresh_at: Instant,
    refresh_at: Instant,
    token: WorkloadIdentityToken,
}

impl CachedToken {
    fn new(token: WorkloadIdentityToken, now: Instant) -> Self {
        let lifetime = Duration::from_secs(token.expires_in);
        let advisory_margin = std::cmp::min(Duration::from_secs(120), lifetime / 2);
        let mandatory_margin = std::cmp::min(Duration::from_secs(30), lifetime / 4);
        Self {
            mandatory_refresh_at: now + lifetime.saturating_sub(mandatory_margin),
            refresh_at: now + lifetime.saturating_sub(advisory_margin),
            token,
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct WorkloadIdentityToken {
    pub access_token: String,
    pub chatgpt_account_id: String,
    pub chatgpt_plan_type: Option<String>,
    pub expires_in: u64,
}

impl fmt::Debug for WorkloadIdentityToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WorkloadIdentityToken")
            .field("access_token", &"[redacted]")
            .field("chatgpt_account_id", &self.chatgpt_account_id)
            .field("chatgpt_plan_type", &self.chatgpt_plan_type)
            .field("expires_in", &self.expires_in)
            .finish()
    }
}

#[derive(Deserialize)]
struct TokenExchangeResponse {
    access_token: String,
    chatgpt_account_id: String,
    chatgpt_plan_type: Option<String>,
    expires_in: u64,
    token_type: String,
    user_id: String,
}

impl TokenExchangeResponse {
    fn into_token(
        self,
        expected: &crate::WorkloadIdentityTarget,
    ) -> Result<WorkloadIdentityToken, WorkloadIdentityError> {
        let lifetime = Duration::from_secs(self.expires_in);
        if self.access_token.trim().is_empty()
            || !self.token_type.eq_ignore_ascii_case("bearer")
            || lifetime.is_zero()
            || lifetime > MAX_ACCESS_TOKEN_LIFETIME
            || self.chatgpt_account_id != expected.workspace_id
            || self.user_id != expected.principal_id
        {
            return Err(WorkloadIdentityError::InvalidExchangeResponse);
        }
        Ok(WorkloadIdentityToken {
            access_token: self.access_token,
            chatgpt_account_id: self.chatgpt_account_id,
            chatgpt_plan_type: self.chatgpt_plan_type,
            expires_in: self.expires_in,
        })
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ExchangeMode {
    Resolve,
    Refresh,
}
