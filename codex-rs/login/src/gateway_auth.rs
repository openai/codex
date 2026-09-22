//! Owns gateway credentials, token refresh, and policy-controlled browser authorization.
//! Credential I/O runs off the async worker and writes retain the store lock through cancellation.
//! Rotated credentials survive caller cancellation and remain pending until persistence succeeds.

use std::fmt;
use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use crate::oauth::AuthorizationCodeGrant;
use crate::oauth::AuthorizationRequest;
use crate::oauth::ErrorBodyLimit;
use crate::oauth::OAuthClient;
use crate::oauth::OAuthError;
use crate::oauth::RefreshTokenGrant;
use crate::oauth::TokenEncoding;
use crate::oauth::TokenEndpoint;
use crate::oauth::build_authorization_url;
use crate::oauth::generate_pkce;
use crate::oauth::generate_state;
use chrono::Utc;
use codex_http_client::ClientRouteClass;
use codex_http_client::HttpClient;
use codex_http_client::HttpClientBuilder;
use codex_http_client::HttpClientFactory;
use codex_keyring_store::KeyringStore;
use http::StatusCode;
use sha2::Digest;
use sha2::Sha256;
use tokio::sync::Mutex;
use url::Host;
use url::Url;

#[path = "gateway_auth_callback.rs"]
mod callback;
#[path = "gateway_auth_login.rs"]
mod login;
#[path = "gateway_auth_storage.rs"]
mod storage;
#[path = "gateway_auth_token.rs"]
mod token;

pub use login::GatewayAuthStatus;
pub use login::GatewayAuthStatusChange;
pub use login::GatewayLoginControl;
pub use login::subscribe_gateway_auth_status;

use callback::CallbackListener;
use storage::GatewayAuthStorage;
use token::StoredToken;
use token::TokenResponse;

const REFRESH_SKEW_SECONDS: i64 = 30;
const HTTP_TIMEOUT: Duration = Duration::from_secs(/*secs*/ 20);

/// Gateway authentication states that require user action rather than request retries.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum GatewayAuthError {
    #[error("Gateway sign-in required. Choose Sign in or Reconnect, then retry your request")]
    LoginRequired,
    #[error("Gateway authentication is in progress; retry after sign-in")]
    LoginInProgress,
}

/// Public-client OAuth settings for a model provider.
#[derive(Clone, PartialEq, Eq)]
pub struct GatewayAuthConfig {
    pub authorization_url: String,
    pub token_url: String,
    pub client_id: String,
    pub resource: Option<String>,
    pub scopes: Vec<String>,
    pub redirect_port: Option<u16>,
}

/// Resolves and persists an OAuth access token for a model provider.
#[derive(Clone)]
pub struct GatewayAuthManager {
    control: Arc<GatewayLoginControl>,
    state: Arc<GatewayAuthState>,
}

enum RefreshPolicy {
    WhenExpired,
    AfterRejection(String),
}

impl RefreshPolicy {
    fn can_reuse(&self, token: &StoredToken) -> bool {
        token_is_usable(token)
            && match self {
                Self::WhenExpired => true,
                Self::AfterRejection(rejected_access_token) => {
                    token.access_token != *rejected_access_token
                }
            }
    }
}

enum RefreshOutcome {
    AccessToken(String),
    LoginRequired,
}

struct GatewayAuthState {
    config: GatewayAuthConfig,
    codex_home: PathBuf,
    storage: GatewayAuthStorage,
    http_client: HttpClient,
    cached_token: Arc<Mutex<GatewayAuthCache>>,
    // Reserve explicit login independently of background credential reads and refreshes.
    login_attempt: Arc<Mutex<()>>,
    status: tokio::sync::watch::Sender<Option<login::GatewayAuthStatusSnapshot>>,
    status_tx: tokio::sync::broadcast::Sender<GatewayAuthStatusChange>,
}

#[derive(Clone, Default, PartialEq, Eq)]
struct GatewayAuthCache {
    token: Option<StoredToken>,
    // Retain rotated credentials across a failed save. The prior persisted token remains
    // available for comparison so retrying cannot overwrite a newer external login.
    pending: Option<StoredToken>,
    // A failed recovery requires replacement credentials, even before local expiry.
    login_required: Option<StoredToken>,
}

impl GatewayAuthManager {
    /// Creates independent gateway credentials in their encrypted store using the caller's HTTP policy.
    /// Token grants never follow redirects or include primary-provider credentials or request logs.
    pub fn new(
        config: GatewayAuthConfig,
        codex_home: PathBuf,
        http_client_factory: &HttpClientFactory,
        keyring: Arc<dyn KeyringStore>,
    ) -> io::Result<Self> {
        let http_client = HttpClientBuilder::new()
            .without_redirects()
            .without_request_logging()
            .build_respecting_outbound_proxy_policy(
                http_client_factory,
                &config.token_url,
                ClientRouteClass::Auth,
            )
            .map_err(|_| io::Error::other("failed to create provider OAuth HTTP client"))?;
        Ok(Self {
            control: GatewayLoginControl::for_host(&codex_home, http_client_factory),
            state: Arc::new(GatewayAuthState {
                config,
                status_tx: login::status_sender_for_host(&codex_home, http_client_factory),
                storage: GatewayAuthStorage::new(codex_home.clone(), keyring),
                codex_home,
                http_client,
                cached_token: Arc::new(Mutex::new(GatewayAuthCache::default())),
                login_attempt: Arc::new(Mutex::new(())),
                status: tokio::sync::watch::channel(/*init*/ None).0,
            }),
        })
    }

    /// Returns a usable token, refreshing or starting browser login according to the host policy.
    pub async fn resolve_access_token(&self) -> io::Result<String> {
        self.resolve_with_browser(RefreshPolicy::WhenExpired, login::open_browser)
            .await
    }

    /// Recovers after a request rejects `rejected_access_token`, reusing a usable replacement
    /// from storage or refreshing/authorizing according to the host policy.
    /// Pass the token used by the failed request; callers must bound retries and decide
    /// whether the request is safe to replay.
    pub async fn refresh_access_token(&self, rejected_access_token: &str) -> io::Result<String> {
        self.resolve_with_browser(
            RefreshPolicy::AfterRejection(rejected_access_token.to_owned()),
            login::open_browser,
        )
        .await
    }

    async fn resolve_with_browser(
        &self,
        policy: RefreshPolicy,
        open_browser: impl FnOnce(&Url),
    ) -> io::Result<String> {
        validate_config(&self.state.config)?;
        let mut cached = self.lock_cached_token().await?;
        let terminal_status = self.state.status.borrow().as_ref().is_some_and(|snapshot| {
            matches!(
                snapshot.status,
                GatewayAuthStatus::NotReady | GatewayAuthStatus::Failed { .. }
            )
        });
        // A cached token may predate the failed login. Only persisted replacements can clear it.
        if cached.pending.is_none()
            && (cached.token.is_none() || terminal_status || cached.login_required.is_some())
        {
            cached.token = self.load_token_async().await?;
        }
        if cached.pending.is_none()
            && cached.login_required.is_some()
            && cached.token == cached.login_required
        {
            if !self
                .control
                .explicit_login
                .load(std::sync::atomic::Ordering::Relaxed)
            {
                return self.authorize_with_browser(&mut cached, open_browser).await;
            }
            return Err(self.login_required_error(cached.token.as_ref()));
        }
        if cached.pending.is_none()
            && matches!(policy, RefreshPolicy::WhenExpired)
            && let Some(token) = cached.token.as_ref()
            && token_is_usable(token)
        {
            let terminal_matches = self.state.status.borrow().as_ref().is_some_and(|snapshot| {
                matches!(
                    snapshot.status,
                    GatewayAuthStatus::NotReady | GatewayAuthStatus::Failed { .. }
                ) && snapshot.credential.as_ref() == Some(token)
            });
            if !terminal_matches {
                self.publish_status(GatewayAuthStatus::Succeeded, Some(token));
            }
            let access_token = token.access_token.clone();
            cached.login_required = None;
            return Ok(access_token);
        }
        // The provider may rotate its token before the HTTP response arrives. Keep the
        // refresh, persistence, and cache update alive if the caller drops this future.
        let manager = self.clone();
        let (mut cached, result) = tokio::spawn(async move {
            let result = match manager.refresh(&mut cached, &policy).await {
                Err(error) => Err(error),
                Ok(RefreshOutcome::AccessToken(access_token)) => {
                    cached.login_required = None;
                    manager.publish_status(GatewayAuthStatus::Succeeded, cached.token.as_ref());
                    Ok(access_token)
                }
                Ok(RefreshOutcome::LoginRequired) => {
                    cached.login_required = cached.token.clone();
                    Err(manager.login_required_error(cached.token.as_ref()))
                }
            };
            (cached, result)
        })
        .await
        .map_err(|_| io::Error::other("provider OAuth refresh task failed"))?;
        if result.as_ref().is_err_and(|error| {
            error
                .get_ref()
                .and_then(|source| source.downcast_ref::<GatewayAuthError>())
                == Some(&GatewayAuthError::LoginRequired)
        }) && !self
            .control
            .explicit_login
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            return self.authorize_with_browser(&mut cached, open_browser).await;
        }
        result
    }

    async fn persist_pending(
        &self,
        cached: &mut GatewayAuthCache,
        credential_lock: &Arc<std::fs::File>,
    ) -> io::Result<String> {
        let token = cached
            .pending
            .as_ref()
            .ok_or_else(|| io::Error::other("provider OAuth credentials are missing"))?;
        let manager = self.clone();
        let saved = token.clone();
        let credential_lock = Arc::clone(credential_lock);
        tokio::task::spawn_blocking(move || {
            // A canceled login may drop its async guard while this write is still running.
            // Keep the cross-process lock until encryption and the atomic write finish.
            let _credential_lock = credential_lock;
            manager.save_token(&saved)
        })
        .await
        .map_err(|_| io::Error::other("provider OAuth credential save task failed"))??;
        let access_token = token.access_token.clone();
        cached.token = cached.pending.take();
        cached.login_required = None;
        Ok(access_token)
    }

    async fn refresh(
        &self,
        cached: &mut GatewayAuthCache,
        policy: &RefreshPolicy,
    ) -> io::Result<RefreshOutcome> {
        let credential_lock = Arc::new(storage::lock_credentials(&self.state.codex_home).await?);
        // Recovery always rereads under the cross-process lock before choosing a token.
        // Even a replacement from storage must differ from the token rejected by this request.
        for _ in 0..2 {
            let stored = self.load_token_async().await?;
            if stored != cached.token {
                cached.token = stored;
                cached.pending = None;
            }
            if cached.pending.is_some() {
                self.persist_pending(cached, &credential_lock).await?;
            }
            if let Some(token) = cached.token.as_ref()
                && policy.can_reuse(token)
            {
                return Ok(RefreshOutcome::AccessToken(token.access_token.clone()));
            }
            let Some(refresh_token) = cached
                .token
                .as_ref()
                .and_then(|token| token.refresh_token.as_deref())
            else {
                break;
            };
            match self
                .oauth()
                .refresh::<TokenResponse>(RefreshTokenGrant {
                    refresh_token,
                    resource: self.state.config.resource.as_deref(),
                })
                .await
            {
                Ok(response) => {
                    cached.pending = Some(response.into_stored(Some(refresh_token))?);
                    return self
                        .persist_pending(cached, &credential_lock)
                        .await
                        .map(RefreshOutcome::AccessToken);
                }
                Err(OAuthError::Rejected(rejection))
                    if rejection.status == StatusCode::BAD_REQUEST
                        && matches!(
                            rejection.detail.error_code(),
                            Some(
                                "invalid_grant" | "unauthorized_client" | "unsupported_grant_type"
                            )
                        ) =>
                {
                    // Some public clients receive refresh tokens despite being unable to use
                    // that grant. Require browser login after rejection without disabling refresh.
                    // Also recover updates from clients that predate the credential lock.
                    if self.load_token_async().await? == cached.token {
                        break;
                    }
                }
                Err(error) => {
                    return Err(token::endpoint_error(
                        error,
                        &self.state.config,
                        "refresh_token",
                        /*redirect_uri*/ None,
                    ));
                }
            }
        }
        let stored = self.load_token_async().await?;
        if stored != cached.token {
            cached.token = stored;
            cached.pending = None;
            if let Some(token) = cached.token.as_ref()
                && policy.can_reuse(token)
            {
                return Ok(RefreshOutcome::AccessToken(token.access_token.clone()));
            }
        }
        Ok(RefreshOutcome::LoginRequired)
    }

    fn credential_id(&self) -> String {
        let config = &self.state.config;
        let mut digest = Sha256::new();
        digest.update(self.state.codex_home.to_string_lossy().as_bytes());
        digest.update([0]);
        for value in [
            config.authorization_url.as_str(),
            config.token_url.as_str(),
            config.client_id.as_str(),
            config.resource.as_deref().unwrap_or_default(),
        ] {
            digest.update(value.as_bytes());
            digest.update([0]);
        }
        for scope in &config.scopes {
            digest.update(scope.as_bytes());
            digest.update([0]);
        }
        format!("provider-oauth|{:x}", digest.finalize())
    }

    async fn load_token_async(&self) -> io::Result<Option<StoredToken>> {
        let manager = self.clone();
        tokio::task::spawn_blocking(move || manager.load_token())
            .await
            .map_err(|_| io::Error::other("provider OAuth credential load task failed"))?
    }

    fn load_token(&self) -> io::Result<Option<StoredToken>> {
        self.state
            .storage
            .load(&self.credential_id())?
            .map(|value| {
                serde_json::from_str(&value)
                    .map_err(|_| io::Error::other("stored provider OAuth credentials are invalid"))
            })
            .transpose()
    }

    fn save_token(&self, token: &StoredToken) -> io::Result<()> {
        let value = serde_json::to_string(token)
            .map_err(|_| io::Error::other("failed to encode provider OAuth credentials"))?;
        self.state.storage.save(&self.credential_id(), &value)
    }

    async fn authorize_with_browser(
        &self,
        cached: &mut GatewayAuthCache,
        open_browser: impl FnOnce(&Url),
    ) -> io::Result<String> {
        let pkce = generate_pkce();
        let state = generate_state();
        let mut listener = CallbackListener::new(self.state.config.redirect_port, state.clone())?;
        let redirect_uri = listener.redirect_uri().to_string();
        let scope = self.state.config.scopes.join(" ");
        let authorization_url = build_authorization_url(AuthorizationRequest {
            endpoint: &self.state.config.authorization_url,
            client_id: &self.state.config.client_id,
            redirect_uri: &redirect_uri,
            scope: (!scope.is_empty()).then_some(scope.as_str()),
            resource: self.state.config.resource.as_deref(),
            pkce: &pkce,
            state: &state,
            extra_parameters: &[],
        })
        .map_err(|_| io::Error::other("invalid provider OAuth authorization endpoint"))?;

        open_browser(&authorization_url);

        let code = listener.wait().await?;
        drop(listener);

        // Wait for user interaction without the store lock, then serialize issuance and
        // persistence with refreshes. Use the current stored token as the failed-save baseline.
        let credential_lock = Arc::new(storage::lock_credentials(&self.state.codex_home).await?);
        let stored = self.load_token_async().await?;
        if stored != cached.token {
            cached.token = stored;
            cached.pending = None;
        }
        let response = self
            .oauth()
            .exchange_code::<TokenResponse>(AuthorizationCodeGrant {
                code: &code,
                redirect_uri: &redirect_uri,
                pkce: &pkce,
                resource: self.state.config.resource.as_deref(),
            })
            .await
            .map_err(|error| {
                token::endpoint_error(
                    error,
                    &self.state.config,
                    "authorization_code",
                    Some(&redirect_uri),
                )
            })?;
        cached.pending = Some(response.into_stored(/*previous_refresh_token*/ None)?);
        let access_token = self.persist_pending(cached, &credential_lock).await?;
        self.publish_status(GatewayAuthStatus::Succeeded, cached.token.as_ref());
        Ok(access_token)
    }

    fn oauth(&self) -> OAuthClient<'_> {
        OAuthClient::new(
            &self.state.http_client,
            TokenEndpoint {
                url: &self.state.config.token_url,
                client_id: &self.state.config.client_id,
                encoding: TokenEncoding::Form,
                timeout: Some(HTTP_TIMEOUT),
                error_body_limit: ErrorBodyLimit::Bytes(8 * 1024),
            },
        )
    }
}

impl fmt::Debug for GatewayAuthConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Configured URLs may contain issuer-specific credentials in arbitrary query keys.
        formatter
            .debug_struct("GatewayAuthConfig")
            .field("client_id", &self.client_id)
            .field("scopes", &self.scopes)
            .field("redirect_port", &self.redirect_port)
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for GatewayAuthManager {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GatewayAuthManager")
            .field("config", &self.state.config)
            .finish_non_exhaustive()
    }
}

fn token_is_usable(token: &StoredToken) -> bool {
    token
        .expires_at
        .is_none_or(|expires_at| expires_at > Utc::now().timestamp() + REFRESH_SKEW_SECONDS)
}

fn validate_config(config: &GatewayAuthConfig) -> io::Result<()> {
    let authorization = validate_oauth_url(
        &config.authorization_url,
        "provider OAuth authorization endpoint",
    )?;
    if authorization.query_pairs().any(|(name, _)| {
        matches!(
            name.as_ref(),
            "response_type"
                | "client_id"
                | "redirect_uri"
                | "state"
                | "scope"
                | "resource"
                | "code_challenge"
                | "code_challenge_method"
        )
    }) {
        return Err(io::Error::other(
            "provider OAuth authorization endpoint cannot include OAuth request parameters",
        ));
    }
    validate_oauth_url(&config.token_url, "provider OAuth token endpoint")?;
    if config.client_id.trim().is_empty() {
        return Err(io::Error::other(
            "provider OAuth client ID must not be empty",
        ));
    }
    if config.redirect_port == Some(0) {
        return Err(io::Error::other(
            "provider OAuth redirect port must not be zero",
        ));
    }
    Ok(())
}

fn validate_oauth_url(value: &str, description: &str) -> io::Result<Url> {
    let url = Url::parse(value).map_err(|_| io::Error::other(format!("invalid {description}")))?;
    if !url.username().is_empty() || url.password().is_some() || url.fragment().is_some() {
        return Err(io::Error::other(format!(
            "{description} cannot include embedded credentials or fragments"
        )));
    }
    let is_loopback = match url.host() {
        Some(Host::Ipv4(address)) => address.is_loopback(),
        Some(Host::Ipv6(address)) => address.is_loopback(),
        Some(Host::Domain(host)) => host.eq_ignore_ascii_case("localhost"),
        None => false,
    };
    if url.scheme() != "https" && !(url.scheme() == "http" && is_loopback) {
        return Err(io::Error::other(format!(
            "{description} must use HTTPS unless it is loopback"
        )));
    }
    Ok(url)
}

#[cfg(test)]
#[path = "gateway_auth_tests.rs"]
mod tests;
