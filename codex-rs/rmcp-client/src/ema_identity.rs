//! Shared enterprise IdP discovery and stored refresh-token resolution.
//! Login and token exchange require published metadata bound to the configured issuer.

use std::sync::Arc;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use anyhow::bail;
use codex_exec_server::HttpClient;
use codex_keyring_store::DefaultKeyringStore;
use codex_keyring_store::KeyringStore;
use oauth2::TokenResponse;
use rmcp::transport::AuthorizationManager;
use rmcp::transport::auth::AuthorizationMetadata;
use rmcp::transport::auth::OAuthHttpClient;
use tracing::instrument::WithSubscriber;

use crate::EmaCredentialLease;
use crate::ema_auth_policy::advertised_capability;
use crate::ema_auth_policy::ema_reauthentication_required;
use crate::ema_auth_policy::validate_ema_oauth_endpoint;
use crate::ema_auth_policy::validate_ema_public_client_auth;
use crate::ema_claims::ID_JAG_TOKEN_TYPE;
use crate::ema_exchange::TOKEN_EXCHANGE_GRANT_TYPE;
use crate::http_client_adapter::StreamableHttpRedirectMode;
use crate::oauth::RefreshCredentialLock;
use crate::oauth::StoredOAuthTokens;
use crate::oauth::stored_oidc_identity;
use crate::oauth_http_client::OAuthHttpClientAdapter;
use crate::utils::build_default_headers;

pub struct EmaIdpIdentityRequest<'a> {
    pub issuer: &'a str,
    pub client_id: &'a str,
    pub credentials: EmaCredentialLease,
    pub http_client: Arc<dyn HttpClient>,
    pub redirect_mode: StreamableHttpRedirectMode,
}

/// An opaque IdP refresh token paired with the credentials admitted for its exchange.
pub struct EmaIdpIdentity<L = EmaCredentialLease> {
    pub(crate) token_endpoint: String,
    pub(crate) refresh_token: String,
    pub(crate) credentials: L,
}

impl<L> EmaIdpIdentity<L> {
    /// Preserve an opaque credential lease when selecting an EMA credential source.
    /// The mapping must retain its owned refresh guard until the IdP exchange finishes.
    pub fn map_credentials<U>(self, map: impl FnOnce(L) -> U) -> EmaIdpIdentity<U> {
        EmaIdpIdentity {
            token_endpoint: self.token_endpoint,
            refresh_token: self.refresh_token,
            credentials: map(self.credentials),
        }
    }
}

/// Resolve a refresh token issued by the account's validated OAuth authority.
/// The caller supplies trusted persisted issuer/client provenance and an ID/refresh-token
/// pair from one admitted account record. Discovery may run before acquiring a refresh
/// guard; before exchange, attach a lease that still matches that record with map_credentials.
/// ID-token claims corroborate that record; this does not verify a JWT signature or
/// authenticate an expired ID token. Only the refresh token is exchanged with the IdP.
pub async fn resolve_ema_account_idp_identity<L: Send>(
    issuer: &str,
    client_id: &str,
    id_token: &str,
    refresh_token: String,
    credentials: L,
    http_client: Arc<dyn HttpClient>,
    redirect_mode: StreamableHttpRedirectMode,
) -> Result<EmaIdpIdentity<L>> {
    if refresh_token.trim().is_empty() {
        return Err(ema_reauthentication_required(
            "Codex account has no refresh token; sign in again",
        ));
    }
    crate::ema_claims::oidc_identity(id_token, issuer, client_id)?;
    let token_endpoint = resolve_ema_idp_token_endpoint(issuer, http_client, redirect_mode).await?;
    Ok(EmaIdpIdentity {
        token_endpoint,
        refresh_token,
        credentials,
    })
}

/// Validate account ID-token bindings without treating an expired ID token as an expired grant.
pub fn account_oidc_identity_is_usable(id_token: &str, issuer: &str, client_id: &str) -> bool {
    crate::ema_claims::oidc_identity(id_token, issuer, client_id).is_ok()
}

/// Only an explanatory legacy-login hint, never authority for RT exchange.
pub fn account_oidc_issuer_hint(id_token: &str, issuer: &str) -> bool {
    crate::ema_claims::oidc_issuer_hint(id_token, issuer)
}

/// Returns whether stored credentials contain a refresh token bound to the configured login.
pub fn stored_ema_identity_is_usable(
    tokens: &StoredOAuthTokens,
    issuer: &str,
    client_id: &str,
) -> bool {
    tokens.url == issuer
        && tokens.bound_issuer() == Some(issuer)
        && tokens.client_id == client_id
        && tokens.has_refresh_token()
        && stored_oidc_identity(tokens).is_ok()
}

pub(crate) async fn resolve_ema_idp_authorization_manager(
    issuer: &str,
    http_client: Arc<dyn OAuthHttpClient>,
) -> Result<(AuthorizationManager, AuthorizationMetadata)> {
    validate_ema_oauth_endpoint(issuer, "enterprise IdP issuer")?;
    let mut manager = AuthorizationManager::new_with_oauth_http_client(issuer, http_client)
        .await
        .context("failed to create enterprise IdP metadata discovery client")?;
    manager.set_allow_missing_issuer(false);
    let resolution = manager
        .resolve_metadata()
        .await
        .context("failed to discover enterprise IdP authorization metadata")?;
    if !resolution.source.is_discovered() {
        bail!("enterprise IdP must publish authorization metadata");
    }
    let metadata = resolution.metadata;
    if metadata.issuer.as_deref() != Some(issuer) {
        bail!("enterprise IdP authorization metadata issuer does not match configuration");
    }
    validate_ema_oauth_endpoint(&metadata.token_endpoint, "enterprise IdP token endpoint")?;
    validate_ema_public_client_auth(
        metadata
            .additional_fields
            .get("token_endpoint_auth_methods_supported"),
        "enterprise IdP",
    )?;
    Ok((manager, metadata))
}

/// Resolve a stored refresh-token subject against the configured enterprise IdP metadata.
pub async fn resolve_ema_idp_identity(
    request: EmaIdpIdentityRequest<'_>,
) -> Result<EmaIdpIdentity> {
    resolve_ema_idp_identity_in(request, &DefaultKeyringStore).await
}

async fn resolve_ema_idp_identity_in<K: KeyringStore + Clone + 'static>(
    request: EmaIdpIdentityRequest<'_>,
    keyring_store: &K,
) -> Result<EmaIdpIdentity> {
    if request.issuer.trim().is_empty() || request.client_id.trim().is_empty() {
        bail!("ema_auth requires a non-empty enterprise IdP issuer and client ID");
    }
    let credentials = request.credentials.credentials.credentials();
    if credentials.url != request.issuer
        || credentials.bound_issuer() != Some(request.issuer)
        || credentials.client_id != request.client_id
    {
        bail!("stored enterprise IdP credentials do not match the configured issuer and client");
    }
    stored_oidc_identity(credentials)?;
    let token_endpoint =
        resolve_ema_idp_token_endpoint(request.issuer, request.http_client, request.redirect_mode)
            .await?;
    let credentials = request.credentials;
    let credential_lock = RefreshCredentialLock::acquire_for_server(
        &credentials.credentials.credentials().server_name,
        request.issuer,
    )
    .with_subscriber(tracing::subscriber::NoSubscriber::default())
    .await
    .map_err(|_| anyhow!("failed to lock enterprise credentials"))?;
    let keyring_store = keyring_store.clone();
    // The worker retains the short read lock if its caller stops waiting for the reread.
    tokio::task::spawn_blocking(move || {
        let _credential_lock = credential_lock;
        let latest = credentials
            .credentials
            .load_ema_credentials(&keyring_store)?;
        let refresh_token = latest
            .token_response
            .0
            .refresh_token()
            .filter(|token| !token.secret().trim().is_empty())
            .ok_or_else(|| {
                ema_reauthentication_required(
                    "enterprise IdP session has no refresh token; sign in again",
                )
            })?;
        Ok(EmaIdpIdentity {
            token_endpoint,
            refresh_token: refresh_token.secret().to_string(),
            credentials,
        })
    })
    .await
    .map_err(|_| anyhow!("enterprise IdP credential reread task failed"))?
}

async fn resolve_ema_idp_token_endpoint(
    issuer: &str,
    http_client: Arc<dyn HttpClient>,
    redirect_mode: StreamableHttpRedirectMode,
) -> Result<String> {
    let client = OAuthHttpClientAdapter::new_with_redirect_mode(
        http_client,
        build_default_headers(/*http_headers*/ None, /*env_http_headers*/ None)?,
        issuer,
        /*has_configured_headers*/ false,
        redirect_mode,
    )?;
    let (_, metadata) = resolve_ema_idp_authorization_manager(issuer, Arc::new(client)).await?;
    for (name, expected) in [
        (
            "identity_chaining_requested_token_types_supported",
            ID_JAG_TOKEN_TYPE,
        ),
        ("grant_types_supported", TOKEN_EXCHANGE_GRANT_TYPE),
    ] {
        if advertised_capability(metadata.additional_fields.get(name), expected, name)?
            == Some(false)
        {
            bail!(
                "enterprise IdP does not advertise the required ID-JAG token exchange capability"
            );
        }
    }
    Ok(metadata.token_endpoint)
}

#[cfg(test)]
#[path = "ema_identity_tests.rs"]
mod tests;
