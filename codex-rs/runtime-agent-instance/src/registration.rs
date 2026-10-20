//! Direct composite bootstrap. Credentials are sent only in the body, never through redirects.

use crate::RuntimeRegistration;
use crate::RuntimeRegistrationResponse;
use codex_http_client::ClientRouteClass;
use codex_http_client::HttpClientFactory;
use codex_http_client::HttpError;
use codex_http_client::NetworkPolicyDenied;
use codex_http_client::RouteAwareClientPool;
use codex_http_client::RouteAwareRequestError;
use http::StatusCode;
use serde::Deserialize;
use std::time::Duration;
use url::Url;

pub const DEFAULT_REGISTRATION_URL: &str = "https://auth.openai.com/agent/register";
const MAX_BODY_BYTES: usize = 64 * 1024;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Makes one bounded attempt. Callers reuse the pending registration when retrying.
pub struct RuntimeRegistrationClient {
    http: RouteAwareClientPool,
    url: Url,
}

impl RuntimeRegistrationClient {
    pub fn new(factory: HttpClientFactory, url: Url) -> Result<Self, RegistrationError> {
        let loopback = url.host_str().is_some_and(|host| {
            host == "localhost"
                || host == "[::1]"
                || host
                    .parse::<std::net::IpAddr>()
                    .is_ok_and(|ip| ip.is_loopback())
        });
        if !(url.scheme() == "https" || (url.scheme() == "http" && loopback))
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err(RegistrationError::InvalidEndpoint);
        }
        Ok(Self {
            http: RouteAwareClientPool::with_chatgpt_cloudflare_cookies_without_redirects_or_request_logging(
                factory,
                ClientRouteClass::Auth,
            ),
            url,
        })
    }

    pub async fn register(
        &self,
        registration: &RuntimeRegistration,
    ) -> Result<RuntimeRegistrationResponse, RegistrationError> {
        let body = serde_json::to_vec(&registration.request)
            .map_err(|_| RegistrationError::InvalidRequest)?;
        if body.len() > MAX_BODY_BYTES {
            return Err(RegistrationError::InvalidRequest);
        }
        let mut response = self
            .http
            .post(self.url.as_str())
            .header("Content-Type", "application/json")
            .header(
                "Idempotency-Key",
                registration.initialization_id.to_string(),
            )
            .timeout(REQUEST_TIMEOUT)
            .body(body)
            .send()
            .await
            .map_err(|error| match error {
                RouteAwareRequestError::Policy(denied) => RegistrationError::Policy(denied),
                _ => RegistrationError::Transport,
            })?;
        let status = response.status();
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|error| match error {
            HttpError::Policy(denied) => RegistrationError::Policy(denied),
            _ => RegistrationError::Transport,
        })? {
            if chunk.len() > MAX_BODY_BYTES - body.len() {
                return Err(RegistrationError::InvalidResponse);
            }
            body.extend_from_slice(&chunk);
        }
        if status != StatusCode::OK {
            let code = serde_json::from_slice::<ErrorEnvelope>(&body)
                .ok()
                .map(|e| e.error.code);
            return Err(RegistrationError::Rejected { status, code });
        }
        let info: crate::RuntimeRegistrationResponse =
            serde_json::from_slice(&body).map_err(|_| RegistrationError::InvalidResponse)?;
        if info.runtime_agent_instance_id.is_empty() || info.agent_host_id.is_empty() {
            return Err(RegistrationError::InvalidResponse);
        }
        Ok(info)
    }
}

/// Errors intentionally omit raw transport details and server messages, which can contain secrets.
#[derive(Clone, Debug, thiserror::Error)]
pub enum RegistrationError {
    #[error(
        "runtime registration requires a secure endpoint without credentials or query parameters"
    )]
    InvalidEndpoint,
    #[error("invalid runtime registration request")]
    InvalidRequest,
    #[error(transparent)]
    Policy(#[from] NetworkPolicyDenied),
    #[error("runtime registration transport failed")]
    Transport,
    #[error("invalid runtime registration response")]
    InvalidResponse,
    #[error("runtime registration rejected ({status}, {code:?})")]
    Rejected {
        status: StatusCode,
        code: Option<RegistrationErrorCode>,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RegistrationErrorCode {
    TokenExpired,
    RuntimeActorInvalid,
    RuntimeInstanceRevoked,
    RuntimeActorSourceInactive,
    RuntimeInputInvalid,
    RuntimeIdempotencyKeyInvalid,
    RuntimeActorConflict,
    RuntimeHostConflict,
    RuntimeRegistrationConflict,
    RuntimeInputTooLarge,
    RuntimeRateLimited,
    RuntimeVerificationUnavailable,
    NotFound,
    #[serde(other)]
    Unknown,
}

#[derive(Deserialize)]
struct ErrorEnvelope {
    error: ErrorDetail,
}

#[derive(Deserialize)]
struct ErrorDetail {
    code: RegistrationErrorCode,
}

#[cfg(test)]
#[path = "registration_tests.rs"]
mod tests;
