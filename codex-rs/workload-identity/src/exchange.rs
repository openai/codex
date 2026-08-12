use std::fmt;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;

use codex_http_client::ClientRouteClass;
use codex_http_client::HttpClient;
use codex_http_client::HttpClientBuilder;
use codex_http_client::HttpClientFactory;
use serde::Deserialize;
use tokio::sync::Mutex;
use url::Host;
use url::Url;

use crate::WorkloadIdentityConfig;
use crate::WorkloadIdentityError;
use crate::assertion::read_assertion;

const ACCESS_TOKEN_TYPE: &str = "urn:ietf:params:oauth:token-type:access_token";
const JWT_BEARER_GRANT_TYPE: &str = "urn:ietf:params:oauth:grant-type:jwt-bearer";
const MAX_ACCESS_TOKEN_LIFETIME: Duration = Duration::from_secs(60 * 60);
const MAX_RESPONSE_BYTES: usize = 1024 * 1024;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const TRANSIENT_FAILURE_RETRY_DELAY: Duration = Duration::from_secs(30);

/// Exchanges assertions and retains only the current short-lived access token in memory.
pub struct WorkloadIdentityExchange {
    client: HttpClient,
    completed_attempts: AtomicU64,
    config: WorkloadIdentityConfig,
    state: Mutex<CacheState>,
    token_url: Url,
}

impl WorkloadIdentityExchange {
    /// Creates an exchange against a token URL selected by the trusted caller.
    ///
    /// HTTP is accepted only for loopback development servers. The supplied factory governs
    /// production proxy and custom-CA policy; loopback requests connect directly.
    pub fn new(
        config: WorkloadIdentityConfig,
        token_url: Url,
        http_client_factory: HttpClientFactory,
    ) -> Result<Self, WorkloadIdentityError> {
        let is_loopback = validate_token_url(&token_url)?;
        let builder = HttpClientBuilder::new()
            .without_redirects()
            .without_request_logging();
        let client = if is_loopback {
            builder
                .build_direct()
                .map_err(|_| WorkloadIdentityError::HttpClientConfiguration)
        } else {
            builder
                .build_respecting_outbound_proxy_policy(
                    &http_client_factory,
                    token_url.as_str(),
                    ClientRouteClass::Auth,
                )
                .map_err(|_| WorkloadIdentityError::HttpClientConfiguration)
        }?;
        Ok(Self {
            client,
            completed_attempts: AtomicU64::new(0),
            config,
            state: Mutex::new(CacheState::default()),
            token_url,
        })
    }

    /// Returns a cached token when possible and otherwise performs one shared exchange.
    #[expect(
        clippy::await_holding_invalid_type,
        reason = "the mutex intentionally provides single-flight exchange ownership"
    )]
    pub async fn resolve(&self) -> Result<WorkloadIdentityToken, WorkloadIdentityError> {
        let observed_attempts = self.completed_attempts.load(Ordering::Acquire);
        let mut state = self.state.lock().await;
        let now = Instant::now();
        if let Some(cached) = &state.cached
            && cached.refresh_at > now
            && let Some(token) = cached.token_at(now)
        {
            return Ok(token);
        }
        if self.completed_attempts.load(Ordering::Acquire) != observed_attempts {
            if let Some(error) = state.last_attempt_error.clone() {
                return Err(error);
            }
            if let Some(token) = state
                .cached
                .as_ref()
                .and_then(|cached| cached.token_at(now))
            {
                return Ok(token);
            }
        }

        let valid_from = Instant::now();
        let result = match self.exchange_uncached().await {
            Ok(token) => state.store(token, valid_from, Instant::now()),
            Err(error) if error.is_transient() => {
                let now = Instant::now();
                match state.cached.as_mut() {
                    Some(cached) if cached.expires_at > now => {
                        cached.refresh_at =
                            std::cmp::min(now + TRANSIENT_FAILURE_RETRY_DELAY, cached.expires_at);
                        cached
                            .token_at(now)
                            .ok_or(WorkloadIdentityError::InvalidExchangeResponse)
                    }
                    _ => Err(error),
                }
            }
            Err(error) => Err(error),
        };
        self.complete_attempt(&mut state, result.as_ref().err());
        result
    }

    /// Exchanges after a downstream service rejects `observed_token_version`.
    ///
    /// Concurrent callers that rejected the same token share the first caller's result.
    #[expect(
        clippy::await_holding_invalid_type,
        reason = "the mutex intentionally provides single-flight exchange ownership"
    )]
    pub async fn refresh(
        &self,
        observed_token_version: u64,
    ) -> Result<WorkloadIdentityToken, WorkloadIdentityError> {
        let observed_attempts = self.completed_attempts.load(Ordering::Acquire);
        let mut state = self.state.lock().await;
        if state.token_generation != observed_token_version
            && let Some(token) = state
                .cached
                .as_ref()
                .and_then(|cached| cached.token_at(Instant::now()))
        {
            return Ok(token);
        }
        if self.completed_attempts.load(Ordering::Acquire) != observed_attempts
            && let Some(error) = state.last_attempt_error.clone()
        {
            return Err(error);
        }

        state.cached = None;
        let valid_from = Instant::now();
        let result = self
            .exchange_uncached()
            .await
            .and_then(|token| state.store(token, valid_from, Instant::now()));
        self.complete_attempt(&mut state, result.as_ref().err());
        result
    }

    /// Drops the cached token only when it is still the token the caller rejected.
    pub async fn invalidate_if_current(&self, observed_token_version: u64) {
        let mut state = self.state.lock().await;
        if state.token_generation == observed_token_version {
            state.cached = None;
            state.last_attempt_error = None;
        }
    }

    fn complete_attempt(&self, state: &mut CacheState, error: Option<&WorkloadIdentityError>) {
        state.last_attempt_error = error.cloned();
        self.completed_attempts.fetch_add(1, Ordering::Release);
    }

    async fn exchange_uncached(&self) -> Result<WorkloadIdentityToken, WorkloadIdentityError> {
        let assertion = read_assertion(&self.config.assertion_file).await?;
        let body = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("grant_type", JWT_BEARER_GRANT_TYPE)
            .append_pair("assertion", &assertion)
            .append_pair("federation_rule_id", &self.config.federation_rule_id)
            .finish();
        let response = self
            .client
            .post(self.token_url.as_str())
            .header("content-type", "application/x-www-form-urlencoded")
            .body(body)
            .timeout(REQUEST_TIMEOUT)
            .send()
            .await
            .map_err(|_| WorkloadIdentityError::ExchangeUnavailable)?;
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

        let mut response = response;
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| WorkloadIdentityError::ExchangeUnavailable)?
        {
            if chunk.len() > MAX_RESPONSE_BYTES.saturating_sub(bytes.len()) {
                return Err(WorkloadIdentityError::InvalidExchangeResponse);
            }
            bytes.extend_from_slice(&chunk);
        }
        serde_json::from_slice::<TokenExchangeResponse>(&bytes)
            .map_err(|_| WorkloadIdentityError::InvalidExchangeResponse)?
            .into_token()
    }
}

fn validate_token_url(url: &Url) -> Result<bool, WorkloadIdentityError> {
    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(WorkloadIdentityError::InvalidTokenUrl);
    }
    let is_loopback = match url.host().ok_or(WorkloadIdentityError::InvalidTokenUrl)? {
        Host::Domain(domain) => domain.eq_ignore_ascii_case("localhost"),
        Host::Ipv4(address) => address.is_loopback(),
        Host::Ipv6(address) => address.is_loopback(),
    };
    if url.scheme() != "https" && !(url.scheme() == "http" && is_loopback) {
        return Err(WorkloadIdentityError::InvalidTokenUrl);
    }
    Ok(is_loopback)
}

#[derive(Default)]
struct CacheState {
    cached: Option<CachedToken>,
    last_attempt_error: Option<WorkloadIdentityError>,
    token_generation: u64,
}

impl CacheState {
    fn store(
        &mut self,
        mut token: WorkloadIdentityToken,
        valid_from: Instant,
        now: Instant,
    ) -> Result<WorkloadIdentityToken, WorkloadIdentityError> {
        let token_generation = self.token_generation.saturating_add(1);
        token.version = token_generation;
        let cached = CachedToken::new(token, valid_from);
        let token = cached
            .token_at(now)
            .ok_or(WorkloadIdentityError::InvalidExchangeResponse)?;
        self.cached = Some(cached);
        self.token_generation = token_generation;
        Ok(token)
    }
}

struct CachedToken {
    expires_at: Instant,
    refresh_at: Instant,
    token: WorkloadIdentityToken,
}

impl CachedToken {
    fn new(token: WorkloadIdentityToken, valid_from: Instant) -> Self {
        let lifetime = Duration::from_secs(token.expires_in);
        let refresh_margin = std::cmp::min(Duration::from_secs(120), lifetime / 2);
        Self {
            expires_at: valid_from + lifetime,
            refresh_at: valid_from + lifetime.saturating_sub(refresh_margin),
            token,
        }
    }

    fn token_at(&self, now: Instant) -> Option<WorkloadIdentityToken> {
        let remaining = self.expires_at.checked_duration_since(now)?;
        if remaining.is_zero() {
            return None;
        }
        let mut token = self.token.clone();
        token.expires_in = remaining
            .as_secs()
            .saturating_add(u64::from(remaining.subsec_nanos() != 0));
        Some(token)
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct WorkloadIdentityToken {
    pub access_token: String,
    pub chatgpt_account_id: String,
    pub chatgpt_account_user_id: String,
    pub chatgpt_plan_type: Option<String>,
    pub expires_in: u64,
    pub scope: String,
    pub user_id: String,
    version: u64,
}

impl WorkloadIdentityToken {
    pub fn version(&self) -> u64 {
        self.version
    }
}

impl fmt::Debug for WorkloadIdentityToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WorkloadIdentityToken")
            .field("access_token", &"[redacted]")
            .field("expires_in", &self.expires_in)
            .field("version", &self.version)
            .finish_non_exhaustive()
    }
}

#[derive(Deserialize)]
struct TokenExchangeResponse {
    access_token: String,
    chatgpt_account_id: String,
    chatgpt_account_user_id: String,
    chatgpt_plan_type: Option<String>,
    expires_in: u64,
    issued_token_type: String,
    scope: String,
    token_type: String,
    user_id: String,
}

impl TokenExchangeResponse {
    fn into_token(self) -> Result<WorkloadIdentityToken, WorkloadIdentityError> {
        let lifetime = Duration::from_secs(self.expires_in);
        if self.access_token.trim().is_empty()
            || self.issued_token_type != ACCESS_TOKEN_TYPE
            || !self.token_type.eq_ignore_ascii_case("bearer")
            || lifetime.is_zero()
            || lifetime > MAX_ACCESS_TOKEN_LIFETIME
            || self.scope.trim().is_empty()
            || self.chatgpt_account_id.trim().is_empty()
            || self.chatgpt_account_user_id.trim().is_empty()
            || self.user_id.trim().is_empty()
            || self
                .chatgpt_plan_type
                .as_deref()
                .is_some_and(|plan_type| plan_type.trim().is_empty())
        {
            return Err(WorkloadIdentityError::InvalidExchangeResponse);
        }
        Ok(WorkloadIdentityToken {
            access_token: self.access_token,
            chatgpt_account_id: self.chatgpt_account_id,
            chatgpt_account_user_id: self.chatgpt_account_user_id,
            chatgpt_plan_type: self.chatgpt_plan_type,
            expires_in: self.expires_in,
            scope: self.scope,
            user_id: self.user_id,
            version: 0,
        })
    }
}

#[cfg(test)]
#[path = "workload_identity_tests.rs"]
mod tests;
