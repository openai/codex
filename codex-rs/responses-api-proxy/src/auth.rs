//! Supplies sensitive request headers without copying the locked API key.
//! Token clients never follow redirects or emit credential-bearing URL diagnostics.

use std::fmt;
use std::future::Future;
use std::pin::Pin;

use anyhow::Result;
use anyhow::anyhow;
use codex_http_client::ClientRouteClass;
use codex_http_client::HttpClient;
use codex_http_client::HttpClientBuilder;
use codex_http_client::HttpClientFactory;
use codex_http_client::HttpResponse;
use codex_http_client::OutboundProxyPolicy;
use codex_http_client::RouteAwareClientPool;
use http::HeaderValue;
use serde::de::DeserializeOwned;
use std::time::Duration;
use zeroize::Zeroizing;

#[cfg_attr(not(test), allow(dead_code, reason = "used by a later stack layer"))]
pub(crate) const MAX_TOKEN_BYTES: usize = 16 * 1024;
#[cfg_attr(not(test), allow(dead_code, reason = "used by a later stack layer"))]
const MAX_RESPONSE_BYTES: usize = 64 * 1024;
#[cfg_attr(not(test), allow(dead_code, reason = "used by a later stack layer"))]
pub(crate) const TOKEN_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Supplies a fresh JWT whenever an OpenAI credential needs to be issued.
/// Acquisition stays asynchronous so callers can compose it with the exchange.
/// Implementations must not include tokens, credential-bearing URLs, or upstream
/// response bodies in errors. The exchange deliberately has no issuer-specific config.
#[cfg_attr(not(test), allow(dead_code, reason = "used by a later stack layer"))]
pub(crate) trait SubjectTokenSource: Send {
    fn subject_token<'a>(&'a mut self, client: &'a TokenClient) -> SubjectTokenFuture<'a>;
}

// Keep the source usable as a trait object while acquisition runs asynchronously.
pub(crate) type SubjectTokenFuture<'a> =
    Pin<Box<dyn Future<Output = Result<Zeroizing<String>>> + Send + 'a>>;

/// Credential requests must not follow redirects or log URLs or response headers.
/// The private field prevents other modules from wrapping arbitrary HTTP clients.
#[cfg_attr(not(test), allow(dead_code, reason = "used by a later stack layer"))]
pub(crate) struct TokenClient(HttpClient);

#[cfg_attr(not(test), allow(dead_code, reason = "used by a later stack layer"))]
impl TokenClient {
    #[allow(
        dead_code,
        reason = "federation creates the routed client in the next stack layer"
    )]
    pub(crate) fn new() -> Self {
        // The standalone proxy does not load Codex's session config. For its
        // opt-in auth flow, resolve system and environment proxy settings per URL.
        let factory = HttpClientFactory::new(OutboundProxyPolicy::RespectSystemProxy);
        Self(
            RouteAwareClientPool::with_builder(factory, ClientRouteClass::Auth, Self::builder())
                .into_client(),
        )
    }

    fn builder() -> HttpClientBuilder {
        HttpClientBuilder::new()
            .connect_timeout(Duration::from_secs(10))
            .without_redirects()
            .without_request_logging()
    }

    pub(crate) fn http(&self) -> &HttpClient {
        &self.0
    }
}

pub(crate) enum Auth {
    ApiKey(&'static str),
}

impl fmt::Debug for Auth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ApiKey(_) => f.debug_tuple("ApiKey").field(&"[REDACTED]").finish(),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct AuthHeader {
    pub(crate) value: HeaderValue,
}

impl Auth {
    pub(crate) fn redirect_policy(&self) -> reqwest::redirect::Policy {
        match self {
            Self::ApiKey(_) => reqwest::redirect::Policy::default(),
        }
    }

    /// Returns a sensitive authorization header. API keys are validated when read,
    /// so this currently always succeeds. The fallible return type allows later
    /// credential providers to report acquisition failures.
    pub(crate) fn header(&self) -> Result<AuthHeader> {
        match self {
            Self::ApiKey(value) => {
                // Keep the existing locked, static API key allocation.
                let mut header = HeaderValue::from_static(value);
                header.set_sensitive(true);
                Ok(AuthHeader { value: header })
            }
        }
    }
}

#[cfg_attr(not(test), allow(dead_code, reason = "used by a later stack layer"))]
pub(crate) async fn read_response<T: DeserializeOwned>(
    mut response: HttpResponse,
    source: &str,
) -> Result<T> {
    if !response.status().is_success() {
        // Never read or log the error body; it may echo a credential. Redirects are
        // failures too. HTTP errors can contain the URL, so don't attach them.
        return Err(anyhow!(
            "{source} token request rejected (HTTP {})",
            response.status().as_u16()
        ));
    }
    // Keep secret-bearing response bytes in one allocation, including on errors.
    let mut body = Zeroizing::new(Vec::with_capacity(MAX_RESPONSE_BYTES));
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| anyhow!("{source} returned an unreadable token response"))?
    {
        if chunk.len() > MAX_RESPONSE_BYTES - body.len() {
            return Err(anyhow!("{source} returned an oversized token response"));
        }
        body.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&body)
        .map_err(|_| anyhow!("{source} returned an invalid token response"))
}

#[cfg(test)]
#[path = "auth_tests.rs"]
pub(crate) mod tests;
