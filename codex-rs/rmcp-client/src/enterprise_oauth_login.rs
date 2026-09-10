//! Enterprise OIDC policy and staged, keyring-only credential commits.
//! Browser completion never persists a grant; its owner revalidates authority under the commit lock.

use std::future::Future;
use std::net::IpAddr;
use std::net::Ipv4Addr;
use std::sync::Arc;

use anyhow::Result;
use anyhow::anyhow;
use anyhow::bail;
use codex_config::types::AuthKeyringBackendKind;
use codex_config::types::OAuthCredentialsStoreMode;
use codex_exec_server::HttpClient;
use http::Method;
use http::header::CONTENT_LENGTH;
use http::header::CONTENT_TYPE;
use rmcp::transport::AuthorizationManager;
use rmcp::transport::auth::AuthorizationMetadata;
use rmcp::transport::auth::OAuthHttpClient;
use rmcp::transport::auth::OAuthHttpClientFuture;
use rmcp::transport::auth::OAuthHttpRequest;
use tracing::instrument::WithSubscriber;
use url::Host;
use url::Url;

use crate::StoredOAuthTokens;
use crate::ema_auth_policy::validate_ema_oauth_endpoint;
use crate::ema_claims::validate_oidc_identity_assertion;
use crate::ema_identity::resolve_ema_idp_authorization_manager;
use crate::http_client_adapter::StreamableHttpRedirectMode;
use crate::oauth::EnterpriseOAuthGeneration;
use crate::oauth::EnterpriseOAuthGenerationFile;
use crate::oauth::RefreshCredentialLock;
use crate::oauth::delete_oauth_tokens_with_lock_held;
use crate::oauth::save_oauth_tokens_with_lock_held;
use crate::oauth::validate_authorization_server_endpoints;
use crate::oauth_client_registration::McpOAuthClientRegistration;
use crate::perform_oauth_login::OAuthHttpContext;
use crate::perform_oauth_login::OAuthLoginPurpose;
use crate::perform_oauth_login::OauthLoginFlow;

/// An exclusive credential mutation guard. Hold it through primary account logout
/// so a competing process cannot commit between deleting the grant and signing out.
/// Coordination, like ordinary OAuth refresh, is scoped to the same CODEX_HOME.
pub struct EnterpriseOAuthCredentialGuard {
    credential_name: String,
    issuer: String,
    keyring_backend: AuthKeyringBackendKind,
    generation_file: EnterpriseOAuthGenerationFile,
    _lock: RefreshCredentialLock,
}

impl EnterpriseOAuthCredentialGuard {
    pub async fn acquire(
        credential_name: &str,
        issuer: &str,
        keyring_backend: AuthKeyringBackendKind,
    ) -> Result<Self> {
        let lock = RefreshCredentialLock::acquire_for_server(credential_name, issuer)
            .with_subscriber(tracing::subscriber::NoSubscriber::default())
            .await
            .map_err(|_| anyhow!("failed to lock enterprise credentials"))?;
        let generation_file = EnterpriseOAuthGenerationFile::open(credential_name, issuer, &lock)
            .map_err(|_| anyhow!("failed to open enterprise login generation"))?;
        Ok(Self {
            credential_name: credential_name.to_owned(),
            issuer: issuer.to_owned(),
            keyring_backend,
            generation_file,
            _lock: lock,
        })
    }

    /// Invalidate pending logins and delete the grant, if present. A false return
    /// means no grant was stored; earlier login attempts are still invalidated.
    pub fn delete_tokens(&self) -> Result<bool> {
        // Invalidate attempts that have not stored a grant yet, including other processes.
        // Persist before deletion so no successful logout can admit an earlier login.
        self.generation_file
            .replace()
            .map_err(|_| anyhow!("failed to invalidate pending enterprise sign-ins"))?;
        // Underlying keyring diagnostics include the account-scoped key. Suppress
        // them and discard their error chain, not merely the outer error message.
        tracing::subscriber::with_default(tracing::subscriber::NoSubscriber::default(), || {
            delete_oauth_tokens_with_lock_held(
                &self._lock,
                &self.credential_name,
                &self.issuer,
                OAuthCredentialsStoreMode::Keyring,
                self.keyring_backend,
            )
        })
        .map_err(|_| anyhow!("failed to delete enterprise credentials"))
    }
}

/// Invalidate earlier enterprise logins across processes, then delete any stored grant.
pub async fn delete_enterprise_oauth_tokens(
    credential_name: &str,
    issuer: &str,
    keyring_backend_kind: AuthKeyringBackendKind,
) -> Result<bool> {
    EnterpriseOAuthCredentialGuard::acquire(credential_name, issuer, keyring_backend_kind)
        .await?
        .delete_tokens()
}

/// A browser login with no detached persistence worker. Dropping this handle or
/// its wait future unblocks the callback listener and cannot write credentials.
pub struct EnterpriseOAuthLoginHandle {
    flow: OauthLoginFlow,
    keyring_backend: AuthKeyringBackendKind,
    generation: EnterpriseOAuthGeneration,
}

impl EnterpriseOAuthLoginHandle {
    pub fn authorization_url(&self) -> String {
        self.flow.authorization_url()
    }

    pub async fn wait(self) -> Result<EnterpriseOAuthCredentials> {
        let stored = self
            .flow
            .complete(/*emit_browser_url*/ false)
            .with_subscriber(tracing::subscriber::NoSubscriber::default())
            .await
            .map_err(|_| anyhow!("enterprise IdP authorization failed"))?;
        validate_enterprise_credentials(&stored)?;
        Ok(EnterpriseOAuthCredentials {
            stored,
            keyring_backend: self.keyring_backend,
            generation: self.generation,
        })
    }
}

/// A validated grant that is not yet stored. It intentionally does not expose tokens.
pub struct EnterpriseOAuthCredentials {
    stored: StoredOAuthTokens,
    keyring_backend: AuthKeyringBackendKind,
    generation: EnterpriseOAuthGeneration,
}

impl EnterpriseOAuthCredentials {
    /// Revalidate the current attempt, account and configuration under the same
    /// exclusive lock used by logout, then persist without an intervening await.
    /// Return the authority proof so the caller can retire its attempt while still
    /// holding the commit gate, before notifying clients or refreshing runtimes.
    pub async fn commit_if<F, Fut, T>(self, is_current: F) -> Result<T>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Option<T>>,
    {
        let guard = EnterpriseOAuthCredentialGuard::acquire(
            &self.stored.server_name,
            &self.stored.url,
            self.keyring_backend,
        )
        .await?;
        let generation = guard
            .generation_file
            .current()
            .map_err(|_| anyhow!("failed to read enterprise login generation"))?;
        if generation.as_ref() != Some(&self.generation) {
            bail!("enterprise sign-in was invalidated by logout");
        }
        let Some(authority) = is_current().await else {
            bail!("enterprise sign-in no longer matches the active account or configuration");
        };
        tracing::subscriber::with_default(tracing::subscriber::NoSubscriber::default(), || {
            save_oauth_tokens_with_lock_held(
                &guard._lock,
                &self.stored.server_name,
                &self.stored,
                OAuthCredentialsStoreMode::Keyring,
                self.keyring_backend,
            )
        })
        .map_err(|_| anyhow!("failed to store enterprise credentials"))?;
        Ok(authority)
    }
}

/// Start a host-registered enterprise login without launching the browser or
/// persisting a credential. The caller owns both the attempt and its later commit.
pub async fn perform_enterprise_oauth_login_return_url(
    request: EnterpriseOAuthLoginRequest<'_>,
) -> Result<EnterpriseOAuthLoginHandle> {
    // Capture before discovery or browser setup, then release the lock while the user signs in.
    let generation = {
        let guard = EnterpriseOAuthCredentialGuard::acquire(
            request.credential_name,
            request.issuer,
            request.keyring_backend_kind,
        )
        .await?;
        match guard.generation_file.current() {
            Ok(Some(generation)) => generation,
            Ok(None) => guard
                .generation_file
                .replace()
                .map_err(|_| anyhow!("failed to initialize enterprise login generation"))?,
            Err(_) => bail!("failed to read enterprise login generation"),
        }
    };
    let flow = OauthLoginFlow::new(
        request.credential_name,
        request.issuer,
        OAuthCredentialsStoreMode::Keyring,
        request.keyring_backend_kind,
        OAuthHttpContext {
            http_headers: None,
            env_http_headers: None,
            http_client: request.http_client,
            redirect_mode: request.redirect_mode,
        },
        &["openid".to_string(), "offline_access".to_string()],
        Some(request.client_id),
        OAuthLoginPurpose::EnterpriseIdp,
        McpOAuthClientRegistration::Auto,
        /*oauth_resource*/ None,
        /*launch_browser*/ false,
        request.callback_port,
        request.callback_url,
        /*global_callback_url*/ None,
        request.timeout_secs,
    )
    .with_subscriber(tracing::subscriber::NoSubscriber::default())
    .await
    .map_err(|_| anyhow!("failed to start enterprise IdP authorization"))?;
    Ok(EnterpriseOAuthLoginHandle {
        flow,
        keyring_backend: request.keyring_backend_kind,
        generation,
    })
}

pub(crate) fn enterprise_callback_settings(
    issuer: &str,
    client_id: Option<&str>,
    callback_url: Option<&str>,
    callback_port: Option<u16>,
) -> Result<(IpAddr, Option<u16>)> {
    validate_ema_oauth_endpoint(issuer, "enterprise IdP issuer")?;
    if client_id.is_none_or(|client_id| client_id.trim().is_empty()) {
        bail!("enterprise IdP login requires its registered client ID");
    }
    let (ip, registered_port) = if let Some(callback_url) = callback_url {
        validate_ema_oauth_endpoint(callback_url, "enterprise IdP callback URL")?;
        let callback = Url::parse(callback_url)?;
        let ip = match (callback.scheme(), callback.host()) {
            ("http", Some(Host::Domain("localhost"))) => Ipv4Addr::LOCALHOST.into(),
            ("http", Some(Host::Ipv4(ip))) if ip.is_loopback() => ip.into(),
            ("http", Some(Host::Ipv6(ip))) if ip.is_loopback() => ip.into(),
            _ => bail!("enterprise IdP callback URL must use an HTTP loopback address"),
        };
        (ip, callback.port())
    } else {
        (Ipv4Addr::LOCALHOST.into(), None)
    };
    if callback_port
        .zip(registered_port)
        .is_some_and(|(configured, registered)| configured != registered)
    {
        bail!("enterprise IdP callback URL and listener specify different ports");
    }
    Ok((ip, callback_port.or(registered_port)))
}

fn validate_enterprise_credentials(stored: &StoredOAuthTokens) -> Result<()> {
    if !stored.has_refresh_token() {
        bail!("enterprise IdP login did not return a refresh token");
    }
    let assertion = stored
        .token_response
        .0
        .extra_fields()
        .0
        .get("id_token")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| anyhow!("enterprise IdP login did not return an OIDC identity assertion"))?;
    validate_oidc_identity_assertion(
        assertion,
        stored.issuer.as_deref().unwrap_or(&stored.url),
        &stored.client_id,
    )
    .map_err(|_| anyhow!("enterprise IdP returned an invalid OIDC identity assertion"))
}

pub(crate) async fn resolve_enterprise_authorization_manager(
    issuer: &str,
    http_client: Arc<dyn OAuthHttpClient>,
) -> Result<(AuthorizationManager, AuthorizationMetadata)> {
    let (manager, metadata) = resolve_ema_idp_authorization_manager(
        issuer,
        Arc::new(EnterpriseOAuthHttpClient(http_client)),
    )
    .await?;
    validate_authorization_server_endpoints(&metadata)?;
    validate_ema_oauth_endpoint(
        &metadata.authorization_endpoint,
        "enterprise IdP authorization endpoint",
    )?;
    Ok((manager, metadata))
}

pub(crate) fn enterprise_authorization_url(auth_url: &str) -> Result<String> {
    let mut url = Url::parse(auth_url)?;
    let query = url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs(
            url.query_pairs()
                .filter(|(key, _)| key != "resource" && key != "prompt"),
        )
        .append_pair("prompt", "consent")
        .finish();
    url.set_query(Some(&query));
    Ok(url.to_string())
}

/// rmcp supplies a resource indicator for MCP OAuth, but the independent OIDC
/// login must not request the IdP issuer as a protected-resource audience.
struct EnterpriseOAuthHttpClient(Arc<dyn OAuthHttpClient>);

impl OAuthHttpClient for EnterpriseOAuthHttpClient {
    fn execute(&self, mut request: OAuthHttpRequest) -> OAuthHttpClientFuture<'_> {
        if request.request.method() == Method::POST
            && request
                .request
                .headers()
                .get(CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                .is_some_and(|value| {
                    value.split(';').next().is_some_and(|mime| {
                        mime.trim()
                            .eq_ignore_ascii_case("application/x-www-form-urlencoded")
                    })
                })
            && url::form_urlencoded::parse(request.request.body())
                .any(|(key, value)| key == "grant_type" && value == "authorization_code")
        {
            let body = url::form_urlencoded::Serializer::new(String::new())
                .extend_pairs(
                    url::form_urlencoded::parse(request.request.body())
                        .filter(|(key, _)| key != "resource"),
                )
                .finish();
            *request.request.body_mut() = body.into_bytes();
            request.request.headers_mut().remove(CONTENT_LENGTH);
        }
        self.0.execute(request)
    }
}

/// Enterprise registration and host-owned login settings. The flow always uses
/// OpenID Connect, strict issuer validation, and keyring-only credential storage.
pub struct EnterpriseOAuthLoginRequest<'a> {
    pub credential_name: &'a str,
    pub issuer: &'a str,
    pub client_id: &'a str,
    pub keyring_backend_kind: AuthKeyringBackendKind,
    pub callback_port: Option<u16>,
    pub callback_url: Option<&'a str>,
    pub timeout_secs: Option<i64>,
    pub http_client: Arc<dyn HttpClient>,
    pub redirect_mode: StreamableHttpRedirectMode,
}

#[cfg(test)]
#[path = "enterprise_oauth_login_tests.rs"]
mod tests;
