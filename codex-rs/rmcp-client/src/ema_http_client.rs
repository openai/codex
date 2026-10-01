//! Renewable, endpoint-confined EMA bearers. An unauthorized MCP operation is never replayed.
//! Authentication shares the request deadline, and a rejected bearer cannot evict a newer one.

use std::future::Future;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;
use std::sync::PoisonError;
use std::time::Duration;

use codex_exec_server::ExecServerError;
use codex_exec_server::HttpClient;
use codex_exec_server::HttpHeader;
use codex_exec_server::HttpRedirectPolicy;
use codex_exec_server::HttpRequestParams;
use codex_exec_server::HttpRequestResponse;
use codex_exec_server::HttpResponseBodyStream;
use futures::future::BoxFuture;
use tokio::sync::OwnedRwLockReadGuard;
use tokio::sync::RwLock;
use tokio::sync::Semaphore;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;
use url::Url;

use crate::EmaAccessToken;
use crate::is_authentication_required_error;
use crate::validate_ema_auth_resource;

/// Supplies resource tokens for one immutable endpoint and pinned enterprise login.
/// Credential ownership is checked when renewing a token, not for cached bearer requests.
pub trait EmaTokenSource: Send + Sync {
    fn endpoint(&self) -> &Url;
    /// Acquires and validates credentials for renewal. Implementations recheck ownership
    /// and a saved grant's generation after exchange before publishing the bearer.
    fn access_token(&self) -> impl Future<Output = anyhow::Result<EmaAccessToken>> + Send;
}

/// Revocable authority held while one EMA connection awaits HTTP responses.
#[derive(Clone, Default)]
pub struct EmaRequestAuthority {
    revoked: CancellationToken,
    responses: Arc<RwLock<()>>,
}

impl EmaRequestAuthority {
    pub fn revoke(&self) {
        self.revoked.cancel();
    }

    pub fn is_revoked(&self) -> bool {
        self.revoked.is_cancelled()
    }

    /// Wait until response futures admitted before revocation complete or are cancelled.
    pub async fn quiesce(&self) {
        let _responses = self.responses.clone().write_owned().await;
    }

    async fn acquire(&self) -> Result<OwnedRwLockReadGuard<()>, ExecServerError> {
        let response = self.responses.clone().read_owned().await;
        if self.is_revoked() {
            return Err(revoked_authority_error());
        }
        Ok(response)
    }

    async fn cancelled(&self) {
        self.revoked.cancelled().await;
    }
}

/// Adds a renewable resource bearer to the existing runtime-selected HTTP capability.
pub struct EmaAuthenticatedHttpClient<S> {
    inner: Arc<dyn HttpClient>,
    source: S,
    authority: EmaRequestAuthority,
    token: Mutex<Option<CachedToken>>,
    renewal: Semaphore,
}

struct CachedToken {
    value: String,
    refresh_at: Instant,
}

impl CachedToken {
    fn new(token: EmaAccessToken) -> Self {
        // Bound reuse even when the authorization server omits an expiry.
        let lifetime = token
            .expires_in
            .unwrap_or(Duration::from_secs(300))
            .min(Duration::from_secs(300));
        let skew = Duration::from_secs(30).min(lifetime / 2);
        Self {
            value: token.access_token,
            refresh_at: token.received_at + lifetime.saturating_sub(skew),
        }
    }
}

impl<S: EmaTokenSource> EmaAuthenticatedHttpClient<S> {
    pub fn new(
        inner: Arc<dyn HttpClient>,
        source: S,
        access_token: EmaAccessToken,
        authority: EmaRequestAuthority,
    ) -> Self {
        Self {
            inner,
            source,
            authority,
            token: Mutex::new(Some(CachedToken::new(access_token))),
            renewal: Semaphore::new(/*permits*/ 1),
        }
    }

    fn cached_token(&self) -> MutexGuard<'_, Option<CachedToken>> {
        self.token.lock().unwrap_or_else(PoisonError::into_inner)
    }

    async fn authenticate(
        &self,
        params: &mut HttpRequestParams,
    ) -> Result<String, ExecServerError> {
        let expected = self.source.endpoint();
        let target = Url::parse(&params.url).map_err(|_| {
            ExecServerError::HttpRequest("invalid enterprise MCP destination".to_string())
        })?;
        if expected.origin() != target.origin()
            || expected.path() != target.path()
            || expected.query() != target.query()
            || validate_ema_auth_resource(&params.url, /*resource*/ None).is_err()
        {
            return Err(ExecServerError::HttpRequest(
                "refusing to send enterprise bearer outside the configured MCP endpoint"
                    .to_string(),
            ));
        }
        let current_token = || {
            self.cached_token()
                .as_ref()
                .filter(|token| Instant::now() < token.refresh_at)
                .map(|token| token.value.clone())
        };
        let value = if let Some(value) = current_token() {
            value
        } else {
            let _renewal = self.renewal.acquire().await.map_err(|_| {
                ExecServerError::HttpRequest("enterprise token renewal gate closed".to_string())
            })?;
            if let Some(value) = current_token() {
                value
            } else {
                let renewed = self.source.access_token().await.map_err(auth_error)?;
                let token = CachedToken::new(renewed);
                if Instant::now() >= token.refresh_at {
                    return Err(ExecServerError::HttpRequest(
                        "enterprise MCP bearer expired during credential validation".to_string(),
                    ));
                }
                let value = token.value.clone();
                *self.cached_token() = Some(token);
                value
            }
        };
        params.redirect_policy = HttpRedirectPolicy::Stop;
        params
            .headers
            .retain(|header| !header.name.eq_ignore_ascii_case("authorization"));
        params.headers.push(HttpHeader {
            name: "authorization".to_string(),
            value: format!("Bearer {value}"),
            value_env_var: None,
        });
        Ok(value)
    }

    fn clear_rejected_token(&self, rejected: &str) {
        let mut token = self.cached_token();
        if token.as_ref().is_some_and(|token| token.value == rejected) {
            *token = None;
        }
    }

    fn validate_response(
        &self,
        response: &HttpRequestResponse,
        token: &str,
    ) -> Result<(), ExecServerError> {
        if response.status == 401 {
            self.clear_rejected_token(token);
            return Err(ExecServerError::AuthenticationRequired(
                "enterprise MCP bearer was rejected; reconnect to renew authorization".to_string(),
            ));
        }
        // An outer redirect wrapper may still follow a response even though the
        // inner HTTP capability received Stop. Do not let it replay this operation.
        if (300..400).contains(&response.status) {
            return Err(ExecServerError::HttpRequest(
                "enterprise MCP redirects are not allowed".to_string(),
            ));
        }
        Ok(())
    }
}

fn auth_error(error: anyhow::Error) -> ExecServerError {
    if is_authentication_required_error(&error) {
        ExecServerError::AuthenticationRequired(
            "enterprise MCP authentication is required".to_string(),
        )
    } else {
        ExecServerError::HttpRequest(
            "enterprise MCP token acquisition or credential validation failed".to_string(),
        )
    }
}

impl<S: EmaTokenSource> HttpClient for EmaAuthenticatedHttpClient<S> {
    fn http_request(
        &self,
        mut params: HttpRequestParams,
    ) -> BoxFuture<'_, Result<HttpRequestResponse, ExecServerError>> {
        Box::pin(async move {
            let deadline = params
                .timeout_ms
                .map(|timeout_ms| Instant::now() + Duration::from_millis(timeout_ms));
            let request = async {
                let _authority = self.authority.acquire().await?;
                let token = self.authenticate(&mut params).await?;
                if let Some(deadline) = deadline {
                    params.timeout_ms = Some(remaining_timeout(deadline)?);
                }
                let response = self.inner.http_request(params).await?;
                self.validate_response(&response, &token)?;
                Ok(response)
            };
            tokio::select! {
                biased;
                () = self.authority.cancelled() => Err(revoked_authority_error()),
                () = deadline_elapsed(deadline) => Err(request_timeout_error()),
                result = request => result,
            }
        })
    }

    fn http_request_stream(
        &self,
        mut params: HttpRequestParams,
    ) -> BoxFuture<'_, Result<(HttpRequestResponse, HttpResponseBodyStream), ExecServerError>> {
        Box::pin(async move {
            let deadline = params
                .timeout_ms
                .map(|timeout_ms| Instant::now() + Duration::from_millis(timeout_ms));
            let request = async {
                let _authority = self.authority.acquire().await?;
                let token = self.authenticate(&mut params).await?;
                if let Some(deadline) = deadline {
                    params.timeout_ms = Some(remaining_timeout(deadline)?);
                }
                let response = self.inner.http_request_stream(params).await?;
                self.validate_response(&response.0, &token)?;
                Ok((
                    response.0,
                    response.1.cancel_on(self.authority.revoked.clone()),
                ))
            };
            tokio::select! {
                biased;
                () = self.authority.cancelled() => Err(revoked_authority_error()),
                () = deadline_elapsed(deadline) => Err(request_timeout_error()),
                result = request => result,
            }
        })
    }
}

async fn deadline_elapsed(deadline: Option<Instant>) {
    match deadline {
        Some(deadline) => tokio::time::sleep_until(deadline).await,
        None => std::future::pending().await,
    }
}

fn remaining_timeout(deadline: Instant) -> Result<u64, ExecServerError> {
    let remaining = deadline
        .checked_duration_since(Instant::now())
        .filter(|remaining| !remaining.is_zero())
        .ok_or_else(request_timeout_error)?;
    Ok(u64::try_from(remaining.as_millis())
        .unwrap_or(u64::MAX)
        .max(1))
}

fn request_timeout_error() -> ExecServerError {
    ExecServerError::HttpRequest("enterprise MCP request timed out".to_string())
}

fn revoked_authority_error() -> ExecServerError {
    ExecServerError::AuthenticationRequired(
        "enterprise MCP authorization changed; reconnect before using this server".to_string(),
    )
}

#[cfg(test)]
#[path = "ema_http_client_tests.rs"]
mod tests;
