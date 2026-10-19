//! Authenticates proxy requests using a locked API key or renewable OpenAI credentials.
//! Federated exchanges are serialized, fail closed on renewal errors, and never replay requests.
//! Token clients never follow redirects or emit credential-bearing URL diagnostics.

use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use anyhow::Result;
use anyhow::anyhow;
use bytes::Bytes;
use codex_http_client::ClientRouteClass;
use codex_http_client::HttpClient;
use codex_http_client::HttpClientBuilder;
use codex_http_client::HttpClientFactory;
use codex_http_client::HttpResponse;
use codex_http_client::OutboundProxyPolicy;
use codex_http_client::RouteAwareClientPool;
use http::HeaderValue;
use http::header::CONTENT_TYPE;
use serde::Deserialize;
use serde::Serialize;
use serde::de::DeserializeOwned;
use tokio::runtime::Runtime;
use url::Url;
use zeroize::Zeroizing;

const OPENAI_TOKEN_URL: &str = "https://auth.openai.com/oauth/token";
pub(crate) const MAX_TOKEN_BYTES: usize = 16 * 1024;
const MAX_RESPONSE_BYTES: usize = 64 * 1024;
// Bound the complete serialized token and provider metadata, including JSON escaping.
const MAX_EXCHANGE_REQUEST_BYTES: usize = 64 * 1024;
pub(crate) const TOKEN_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_FEDERATED_TOKEN_LIFETIME: Duration = Duration::from_secs(3600);
const INITIAL_RENEWAL_BACKOFF: Duration = Duration::from_secs(1);
const MAX_RENEWAL_BACKOFF: Duration = Duration::from_secs(30);

/// Supplies a fresh JWT whenever an OpenAI credential needs to be issued.
/// Acquisition stays asynchronous so callers can compose it with the exchange.
/// Implementations must not include tokens, credential-bearing URLs, or upstream
/// response bodies in errors. The exchange deliberately has no issuer-specific config.
pub(crate) trait SubjectTokenSource: Send {
    fn subject_token<'a>(&'a mut self, client: &'a TokenClient) -> SubjectTokenFuture<'a>;
}

// Keep the source usable as a trait object while acquisition runs asynchronously.
pub(crate) type SubjectTokenFuture<'a> =
    Pin<Box<dyn Future<Output = Result<Zeroizing<String>>> + Send + 'a>>;

/// Credential requests must not follow redirects or log URLs or response headers.
/// The private field prevents other modules from wrapping arbitrary HTTP clients.
pub(crate) struct TokenClient(HttpClient);

impl TokenClient {
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
    Federated { state: Box<Mutex<FederatedAuth>> },
}

impl fmt::Debug for Auth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ApiKey(_) => f.debug_tuple("ApiKey").field(&"[REDACTED]").finish(),
            Self::Federated { .. } => f
                .debug_struct("Federated")
                .field("state", &"[REDACTED]")
                .finish(),
        }
    }
}

/// The sensitive header and, for federation, the rejection state of this issuance.
/// In-flight requests share only their own issuance's flag, never a replacement's.
#[derive(Clone, Debug)]
pub(crate) struct AuthHeader {
    pub(crate) value: HeaderValue,
    rejected: Option<Arc<AtomicBool>>,
}

impl AuthHeader {
    pub(crate) fn invalidate(&self) {
        if let Some(rejected) = &self.rejected {
            // This flag carries no other state; credentials remain mutex-protected.
            rejected.store(true, Ordering::Relaxed);
        }
    }
}

impl Auth {
    #[cfg_attr(not(test), allow(dead_code, reason = "used by a later stack layer"))]
    pub(crate) fn federated(
        source: impl SubjectTokenSource + 'static,
        provider: String,
        service_account: String,
    ) -> Result<Self> {
        let client = TokenClient::new();
        // Only credential acquisition needs an async runtime. The proxy's
        // blocking request threads drive it while holding the credential mutex.
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| anyhow!("could not start workload identity HTTP runtime"))?;
        Ok(Self::Federated {
            state: Box::new(Mutex::new(FederatedAuth {
                client,
                runtime,
                token_url: Url::parse(OPENAI_TOKEN_URL)?,
                source: Box::new(source),
                provider,
                service_account,
                cached: None,
                renewal_failure: None,
            })),
        })
    }

    pub(crate) fn redirect_policy(&self) -> reqwest::redirect::Policy {
        match self {
            Self::ApiKey(_) => reqwest::redirect::Policy::default(),
            // A federated credential must never be sent to a redirect target.
            Self::Federated { .. } => reqwest::redirect::Policy::none(),
        }
    }

    /// Returns a sensitive authorization header.
    ///
    /// API keys are validated when read. Federated authentication can fail if the
    /// credential cache cannot be locked, or if token acquisition, exchange, or
    /// validation fails.
    pub(crate) fn header(&self) -> Result<AuthHeader> {
        match self {
            Self::ApiKey(value) => {
                // Keep the existing locked, static API key allocation.
                let mut header = HeaderValue::from_static(value);
                header.set_sensitive(true);
                Ok(AuthHeader {
                    value: header,
                    rejected: None,
                })
            }
            Self::Federated { state } => state
                .lock()
                .map_err(|_| anyhow!("workload identity credential cache unavailable"))?
                .header(Instant::now),
        }
    }
}

pub(crate) struct FederatedAuth {
    client: TokenClient,
    runtime: Runtime,
    token_url: Url,
    source: Box<dyn SubjectTokenSource>,
    provider: String,
    service_account: String,
    cached: Option<CachedToken>,
    renewal_failure: Option<RenewalFailure>,
}

struct CachedToken {
    header: AuthHeader,
    renew_at: Instant,
    expires_at: SystemTime,
}

struct RenewalFailure {
    retry_at: Instant,
    delay: Duration,
}

impl FederatedAuth {
    fn header(&mut self, clock: impl Fn() -> Instant) -> Result<AuthHeader> {
        let now = clock();
        if let Some(cached) = &self.cached
            && now < cached.renew_at
            // Monotonic time can stop while a host is suspended.
            && SystemTime::now() < cached.expires_at
            && cached
                .header
                .rejected
                .as_ref()
                .is_none_or(|rejected| !rejected.load(Ordering::Relaxed))
        {
            return Ok(cached.header.clone());
        }
        if let Some(failure) = &self.renewal_failure
            && now < failure.retry_at
        {
            // Keep neither the failed token nor an error that could hold source
            // credentials. The initiating request reports the original safe error.
            return Err(anyhow!(
                "workload identity renewal temporarily unavailable; retry later"
            ));
        }
        // One exchange at a time (under Auth's mutex), including during startup.
        // On failure, fail closed instead of reusing an expired/revoked token.
        self.cached = None;
        match self.exchange() {
            Ok(cached) => {
                let header = cached.header.clone();
                self.cached = Some(cached);
                self.renewal_failure = None;
                Ok(header)
            }
            Err(err) => {
                // Waiters share this failure instead of each retrying the token
                // endpoints. Later requests may retry after a bounded delay.
                let delay = self
                    .renewal_failure
                    .as_ref()
                    .map(|failure| (failure.delay * 2).min(MAX_RENEWAL_BACKOFF))
                    .unwrap_or(INITIAL_RENEWAL_BACKOFF);
                self.renewal_failure = Some(RenewalFailure {
                    retry_at: clock() + delay,
                    delay,
                });
                Err(err)
            }
        }
    }

    fn exchange(&mut self) -> Result<CachedToken> {
        let (started, exchange): (Instant, ExchangeResponse) = self.runtime.block_on(async {
            let subject = self.source.subject_token(&self.client).await?;
            if subject.is_empty() || subject.len() > MAX_TOKEN_BYTES {
                return Err(anyhow!(
                    "workload identity source returned an invalid subject token"
                ));
            }
            let request = ExchangeRequest {
                grant_type: "urn:ietf:params:oauth:grant-type:token-exchange",
                subject_token_type: "urn:ietf:params:oauth:token-type:jwt",
                subject_token: subject.as_str(),
                identity_provider_id: &self.provider,
                service_account_id: &self.service_account,
            };
            // Serialize through a fixed slice so secret-bearing storage cannot grow.
            // The HTTP body owns this buffer, including any request clones used for retries.
            let mut body = Zeroizing::new(vec![0u8; MAX_EXCHANGE_REQUEST_BYTES]);
            let written = {
                let mut writer = std::io::Cursor::new(body.as_mut_slice());
                serde_json::to_writer(&mut writer, &request).map_err(|_| {
                    anyhow!(
                        "OpenAI workload identity exchange request exceeds {MAX_EXCHANGE_REQUEST_BYTES} bytes"
                    )
                })?;
                writer.position() as usize
            };
            body.truncate(written);
            // Start the lifetime clock before the OpenAI exchange, after obtaining
            // the subject. expires_in is measured from issuance, not receipt.
            let started = Instant::now();
            let response = self
                .client
                .http()
                .post(self.token_url.clone())
                .timeout(TOKEN_REQUEST_TIMEOUT)
                .header(CONTENT_TYPE, HeaderValue::from_static("application/json"))
                .body(Bytes::from_owner(body))
                .send()
                .await
                .map_err(|_| anyhow!("OpenAI workload identity token exchange failed"))?;
            let exchange = read_response(response, "OpenAI workload identity").await?;
            Ok((started, exchange))
        })?;
        if !exchange.token_type.eq_ignore_ascii_case("bearer")
            || exchange.issued_token_type != "urn:ietf:params:oauth:token-type:access_token"
        {
            return Err(anyhow!(
                "OpenAI returned an unsupported workload identity token type"
            ));
        }
        if exchange.expires_in > MAX_FEDERATED_TOKEN_LIFETIME.as_secs() {
            return Err(anyhow!("OpenAI returned an invalid token lifetime"));
        }
        let header = bearer_header(exchange.access_token.as_bytes())?;
        let expires_at = UNIX_EPOCH
            .checked_add(Duration::from_secs(exchange.expires_at))
            .ok_or_else(|| anyhow!("OpenAI returned an expired workload identity token"))?;
        let now = SystemTime::now();
        let remaining_absolute = expires_at
            .duration_since(now)
            .map_err(|_| anyhow!("OpenAI returned an expired workload identity token"))?;
        let remaining_relative = Duration::from_secs(exchange.expires_in)
            .checked_sub(started.elapsed())
            .ok_or_else(|| anyhow!("OpenAI returned an expired workload identity token"))?;
        let remaining = remaining_absolute.min(remaining_relative);
        if remaining.is_zero() {
            return Err(anyhow!(
                "OpenAI returned an expired workload identity token"
            ));
        }
        // Bound the wall-clock deadline too, even when the issuer's clock is
        // ahead. Otherwise a host suspend could outlast the shorter relative TTL.
        let expires_at = now
            .checked_add(remaining)
            .ok_or_else(|| anyhow!("OpenAI returned an invalid token lifetime"))?;
        // Proactive renewal without a fixed skew that rejects short-lived subject tokens.
        let skew = (remaining / 10).min(Duration::from_secs(60));
        let renew_at = Instant::now()
            .checked_add(remaining - skew)
            .ok_or_else(|| anyhow!("OpenAI returned an invalid token lifetime"))?;
        Ok(CachedToken {
            header: AuthHeader {
                value: header,
                rejected: Some(Arc::new(AtomicBool::new(/*v*/ false))),
            },
            renew_at,
            expires_at,
        })
    }
}

#[derive(Serialize)]
struct ExchangeRequest<'a> {
    grant_type: &'static str,
    subject_token_type: &'static str,
    subject_token: &'a str,
    identity_provider_id: &'a str,
    service_account_id: &'a str,
}

#[derive(Deserialize)]
struct ExchangeResponse {
    access_token: Zeroizing<String>,
    token_type: String,
    issued_token_type: String,
    expires_in: u64,
    expires_at: u64,
}

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

// RFC 6750 bearer token characters, rather than the API key's narrower alphabet.
// Accept JWT periods, but never control bytes, whitespace, or header injection.
fn bearer_header(token: &[u8]) -> Result<HeaderValue> {
    if token.is_empty()
        || token.len() > MAX_TOKEN_BYTES
        || !token.iter().all(|byte| {
            byte.is_ascii_alphanumeric()
                || matches!(byte, b'-' | b'.' | b'_' | b'~' | b'+' | b'/' | b'=')
        })
    {
        return Err(anyhow!("invalid workload identity bearer credential"));
    }
    let mut bytes = Zeroizing::new(Vec::with_capacity(7 + token.len()));
    bytes.extend_from_slice(b"Bearer ");
    bytes.extend_from_slice(token);
    let mut header = HeaderValue::from_bytes(&bytes)
        .map_err(|_| anyhow!("invalid workload identity bearer credential"))?;
    header.set_sensitive(true);
    Ok(header)
}

#[cfg(test)]
#[path = "auth_tests.rs"]
pub(crate) mod tests;
