//! Lifecycle-local persistence and serialized refresh transactions for MCP OAuth credentials.

use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use anyhow::Context;
use anyhow::Result;
use codex_keyring_store::DefaultKeyringStore;
use codex_keyring_store::KeyringStore;
use oauth2::AccessToken;
use oauth2::TokenResponse;
use rmcp::transport::auth::AuthorizationManager;
use rmcp::transport::auth::CredentialStore as _;
use rmcp::transport::auth::InMemoryCredentialStore;
use rmcp::transport::auth::OAuthTokenResponse;
use rmcp::transport::auth::StoredCredentials;
use tokio::sync::Mutex;
use tokio::time::timeout;
use tracing::debug;
use tracing::warn;

use super::ResolvedOAuthCredentialStore;
use super::StoredOAuthTokens;
use super::WrappedOAuthTokenResponse;
use super::compute_expires_at_millis;
use super::load_oauth_tokens_from_store;
use super::refresh_lock::RefreshCredentialLock;
use super::save_oauth_tokens_to_file;
use super::save_oauth_tokens_with_keyring;
use super::token_needs_refresh;

const REFRESH_REQUEST_TIMEOUT: Duration = Duration::from_secs(45);

#[derive(Clone)]
pub(crate) struct OAuthPersistor {
    inner: Arc<OAuthPersistorInner>,
}

struct OAuthPersistorInner {
    server_name: String,
    url: String,
    authorization_manager: Arc<Mutex<AuthorizationManager>>,
    credential_store: ResolvedOAuthCredentialStore,
    current_credentials: Mutex<Option<StoredOAuthTokens>>,
}

impl OAuthPersistor {
    pub(crate) fn new(
        server_name: String,
        url: String,
        authorization_manager: Arc<Mutex<AuthorizationManager>>,
        credential_store: ResolvedOAuthCredentialStore,
        initial_credentials: Option<StoredOAuthTokens>,
    ) -> Self {
        Self {
            inner: Arc::new(OAuthPersistorInner {
                server_name,
                url,
                authorization_manager,
                credential_store,
                current_credentials: Mutex::new(initial_credentials),
            }),
        }
    }

    pub(crate) async fn refresh_if_needed(&self) -> Result<()> {
        self.refresh_if_needed_with_keyring_store(&DefaultKeyringStore)
            .await
    }

    #[expect(dead_code, reason = "wired by part 4 of this stack")]
    pub(crate) async fn refresh_after_unauthorized(
        &self,
        rejected_access_token: AccessToken,
    ) -> Result<()> {
        self.refresh_after_unauthorized_with_keyring_store(
            &DefaultKeyringStore,
            rejected_access_token,
        )
        .await
    }

    pub(super) async fn refresh_if_needed_with_keyring_store<K: KeyringStore + Clone + 'static>(
        &self,
        keyring_store: &K,
    ) -> Result<()> {
        self.refresh_if_needed_with_keyring_store_and_timeout(
            keyring_store,
            REFRESH_REQUEST_TIMEOUT,
        )
        .await
    }

    pub(super) async fn refresh_if_needed_with_keyring_store_and_timeout<
        K: KeyringStore + Clone + 'static,
    >(
        &self,
        keyring_store: &K,
        refresh_request_timeout: Duration,
    ) -> Result<()> {
        let expires_at = {
            let guard = self.inner.current_credentials.lock().await;
            guard.as_ref().and_then(|tokens| tokens.expires_at)
        };

        if !token_needs_refresh(expires_at) {
            return Ok(());
        }

        self.run_owned_refresh_transaction(
            keyring_store.clone(),
            RefreshReason::Expiry,
            refresh_request_timeout,
        )
        .await
    }

    pub(super) async fn refresh_after_unauthorized_with_keyring_store<
        K: KeyringStore + Clone + 'static,
    >(
        &self,
        keyring_store: &K,
        rejected_access_token: AccessToken,
    ) -> Result<()> {
        self.run_owned_refresh_transaction(
            keyring_store.clone(),
            RefreshReason::Unauthorized {
                rejected_access_token,
            },
            REFRESH_REQUEST_TIMEOUT,
        )
        .await
    }

    async fn run_owned_refresh_transaction<K: KeyringStore + Clone + 'static>(
        &self,
        keyring_store: K,
        reason: RefreshReason,
        refresh_request_timeout: Duration,
    ) -> Result<()> {
        let persistor = self.clone();
        let server_name = self.inner.server_name.clone();
        // Once a provider request can consume a rotating refresh token, dropping the caller's
        // future must not also drop the refresh-and-persist transaction. Dropping this JoinHandle
        // detaches the task, so it continues while the provider request remains bounded by
        // `refresh_request_timeout` and the credential lock remains bounded by its own timeout.
        //
        // A provider timeout deliberately leaves the outcome unknown, releases the lock, and
        // permits a later serialized retry. Some providers accept the previous token during a
        // grace period; otherwise that retry surfaces reauthorization. We accept that residual
        // token-family-revocation risk rather than holding the lock indefinitely.
        let refresh_reason = reason.as_str();
        tokio::spawn(async move {
            let result = persistor
                .refresh_transaction(&keyring_store, reason, refresh_request_timeout)
                .await;

            // Keep this summary inside the owned task so caller cancellation cannot suppress it.
            if let Err(error) = &result {
                warn!(
                    server_name = %persistor.inner.server_name,
                    refresh_reason,
                    error = %error,
                    "MCP OAuth refresh transaction failed"
                );
            }

            result
        })
        .await
        .with_context(|| format!("OAuth refresh task failed for server {server_name}"))?
    }

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "AuthorizationManager async access must be serialized through its mutex"
    )]
    #[tracing::instrument(
        level = "debug",
        skip_all,
        fields(
            server_name = %self.inner.server_name,
            refresh_reason = reason.as_str(),
        ),
        err
    )]
    async fn refresh_transaction<K: KeyringStore + Clone + 'static>(
        &self,
        keyring_store: &K,
        reason: RefreshReason,
        refresh_request_timeout: Duration,
    ) -> Result<()> {
        let transaction_started_at = Instant::now();
        let lock_started_at = Instant::now();
        debug!("waiting for the MCP OAuth credential transaction lock");
        let _lock =
            RefreshCredentialLock::acquire_for_server(&self.inner.server_name, &self.inner.url)
                .await?;
        debug!(
            lock_wait_ms = lock_started_at.elapsed().as_millis(),
            "acquired the MCP OAuth credential transaction lock"
        );

        // The refresh transaction must stay on the store that supplied its snapshot. Falling back
        // here could replay an older rotating refresh token from the other store. We assume store
        // availability is stable for this client lifecycle and surface violations of that
        // assumption instead of switching stores.
        debug!("rereading authoritative MCP OAuth credentials");
        let latest = load_oauth_tokens_from_store(
            keyring_store,
            &self.inner.server_name,
            &self.inner.url,
            self.inner.credential_store,
        )?;

        // The pre-lock snapshot only decides whether a refresh transaction might be needed. Once
        // the lock is held, this reread is authoritative: adopt it before deciding whether to
        // refresh so this process never sends a refresh token superseded by another process.
        let Some(latest) = latest else {
            self.clear_manager_credentials().await;
            *self.inner.current_credentials.lock().await = None;
            anyhow::bail!(
                "OAuth tokens for server {} were removed before refresh; authorization required",
                self.inner.server_name
            );
        };

        let latest_access_token = latest.token_response.0.access_token().secret();
        // Expiry refresh can adopt any reread that is now healthy. A 401 belongs to the access
        // token sent with that specific request, not to this client's mutable current snapshot.
        // If a delayed request rejected A after another request refreshed A to B, adopt B and let
        // the caller retry instead of rotating B again.
        let should_adopt = !token_needs_refresh(latest.expires_at)
            && match &reason {
                RefreshReason::Expiry => true,
                RefreshReason::Unauthorized {
                    rejected_access_token,
                } => rejected_access_token.secret() != latest_access_token,
            };
        if should_adopt {
            debug!("adopting newer MCP OAuth credentials without contacting the provider");
            self.adopt_credentials(latest).await?;
            return Ok(());
        }

        let manager = self.inner.authorization_manager.clone();
        let mut guard = manager.lock().await;
        if let Err(error) =
            install_tokens_in_manager_guard(&mut guard, &latest, CredentialExposure::Refresh).await
        {
            install_tokens_in_manager_guard(&mut guard, &latest, CredentialExposure::Request)
                .await
                .context("failed to restore request-only OAuth credentials")?;
            return Err(error).context("failed to stage OAuth credentials for refresh");
        }
        // The provider request has its own bound. The independently owned task prevents caller
        // startup and operation deadlines from canceling this future after the provider may have
        // rotated the refresh token.
        let provider_started_at = Instant::now();
        debug!(
            timeout_ms = refresh_request_timeout.as_millis(),
            "requesting refreshed MCP OAuth credentials from the provider"
        );
        let refresh_result = match timeout(refresh_request_timeout, guard.refresh_token()).await {
            Ok(Ok(token_response)) => {
                debug!(
                    provider_elapsed_ms = provider_started_at.elapsed().as_millis(),
                    "received refreshed MCP OAuth credentials from the provider"
                );
                Ok(refreshed_tokens(token_response, &latest, &self.inner))
            }
            Ok(Err(error)) => {
                warn!(
                    provider_elapsed_ms = provider_started_at.elapsed().as_millis(),
                    error = %error,
                    "MCP OAuth provider refresh failed"
                );
                Err(error).with_context(|| {
                    format!(
                        "failed to refresh OAuth tokens for server {}",
                        self.inner.server_name
                    )
                })
            }
            Err(_) => {
                warn!(
                    provider_elapsed_ms = provider_started_at.elapsed().as_millis(),
                    timeout_ms = refresh_request_timeout.as_millis(),
                    "MCP OAuth provider refresh timed out; the outcome is unknown and a later serialized retry is permitted"
                );
                Err(anyhow::anyhow!(
                    "timed out after {refresh_request_timeout:?} refreshing OAuth tokens for server {}",
                    self.inner.server_name
                ))
            }
        };
        let request_tokens = refresh_result.as_ref().unwrap_or(&latest);
        if let Err(error) =
            install_tokens_in_manager_guard(&mut guard, request_tokens, CredentialExposure::Request)
                .await
        {
            return Err(error).context("failed to restore request-only OAuth credentials");
        }
        let refreshed = refresh_result?;
        // Once the provider rotates a refresh token, the owned task must attempt persistence even
        // if the caller's deadline expires in the meantime. Refresh persistence stays on the
        // source resolved at client startup. In particular, a keyring failure must surface instead
        // of writing the rotated token to fallback File.
        //
        // A refreshed token becomes authoritative only after this write succeeds. If it fails, we
        // deliberately restore the previous in-process credentials and return the error rather
        // than serving an unpersisted token whose eventual loss would be difficult to correlate
        // with this transaction. A later refresh may require reauthorization if the provider
        // already consumed the previous token; that is the accepted fail-closed behavior.
        // TODO: If persistence failures are common in practice, add an explicit bounded retry or
        // reconciliation policy here. Do not silently continue with unpersisted credentials.
        debug!("persisting refreshed MCP OAuth credentials to the resolved store");
        let persistence_started_at = Instant::now();
        let persistence_result = match self.inner.credential_store {
            ResolvedOAuthCredentialStore::File => save_oauth_tokens_to_file(&refreshed),
            ResolvedOAuthCredentialStore::Keyring(keyring_backend_kind) => {
                save_oauth_tokens_with_keyring(
                    keyring_store,
                    keyring_backend_kind,
                    &self.inner.server_name,
                    &refreshed,
                )
            }
        };
        if let Err(error) = persistence_result {
            warn!(
                persistence_elapsed_ms = persistence_started_at.elapsed().as_millis(),
                transaction_elapsed_ms = transaction_started_at.elapsed().as_millis(),
                error = %error,
                "failed to persist refreshed MCP OAuth credentials; returning the error and restoring the previous in-process credentials"
            );
            install_tokens_in_manager_guard(&mut guard, &latest, CredentialExposure::Request)
                .await
                .context(
                    "failed to restore request-only OAuth credentials after refresh persistence failed",
                )?;
            return Err(error);
        }
        *self.inner.current_credentials.lock().await = Some(refreshed);
        drop(guard);
        debug!(
            persistence_elapsed_ms = persistence_started_at.elapsed().as_millis(),
            transaction_elapsed_ms = transaction_started_at.elapsed().as_millis(),
            "persisted refreshed MCP OAuth credentials and completed the transaction"
        );
        Ok(())
    }

    async fn adopt_credentials(&self, tokens: StoredOAuthTokens) -> Result<()> {
        install_tokens_in_manager(&self.inner.authorization_manager, &tokens).await?;
        *self.inner.current_credentials.lock().await = Some(tokens);
        Ok(())
    }

    async fn clear_manager_credentials(&self) {
        let manager = self.inner.authorization_manager.clone();
        let mut guard = manager.lock().await;
        guard.set_credential_store(InMemoryCredentialStore::new());
    }
}

enum RefreshReason {
    Expiry,
    Unauthorized { rejected_access_token: AccessToken },
}

impl RefreshReason {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Expiry => "expiry",
            Self::Unauthorized { .. } => "unauthorized",
        }
    }
}

#[expect(
    clippy::await_holding_invalid_type,
    reason = "AuthorizationManager async access must be serialized through its mutex"
)]
async fn install_tokens_in_manager(
    authorization_manager: &Arc<Mutex<AuthorizationManager>>,
    tokens: &StoredOAuthTokens,
) -> Result<()> {
    let manager = authorization_manager.clone();
    let mut guard = manager.lock().await;
    install_tokens_in_manager_guard(&mut guard, tokens, CredentialExposure::Request).await
}

async fn install_tokens_in_manager_guard(
    authorization_manager: &mut AuthorizationManager,
    tokens: &StoredOAuthTokens,
    exposure: CredentialExposure,
) -> Result<()> {
    let store = InMemoryCredentialStore::new();
    store
        .save(stored_credentials_from_tokens(tokens, exposure))
        .await
        .context("failed to stage OAuth tokens for authorization manager")?;

    authorization_manager.set_credential_store(store);
    // TODO(stevenlee): RMCP's `initialize_from_store` updates the credential store and client ID
    // but not its private `current_scopes`. Credential adoption can therefore leave scope-upgrade
    // state stale until RMCP exposes an adoption API that synchronizes both.
    authorization_manager
        .initialize_from_store()
        .await
        .context("failed to adopt refreshed OAuth tokens")?;
    Ok(())
}

/// Controls which credentials are exposed to RMCP's authorization manager.
///
/// Normal requests receive neither the refresh token nor expiry metadata, so RMCP cannot refresh
/// outside Codex's cross-process transaction. Full credentials are exposed only while that lock is
/// held, and request-only credentials are restored before the transaction returns unless that
/// restoration itself fails.
#[derive(Clone, Copy)]
enum CredentialExposure {
    Request,
    Refresh,
}

fn stored_credentials_from_tokens(
    tokens: &StoredOAuthTokens,
    exposure: CredentialExposure,
) -> StoredCredentials {
    let token_response = match exposure {
        CredentialExposure::Request => request_oauth_token_response(tokens),
        CredentialExposure::Refresh => tokens.token_response.0.clone(),
    };
    let granted_scopes = token_response
        .scopes()
        .map(|scopes| scopes.iter().map(|scope| scope.to_string()).collect())
        .unwrap_or_default();
    let token_received_at = match exposure {
        CredentialExposure::Request => None,
        CredentialExposure::Refresh => SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()
            .map(|duration| duration.as_secs()),
    };

    StoredCredentials::new(
        tokens.client_id.clone(),
        Some(token_response),
        granted_scopes,
        token_received_at,
    )
}

pub(crate) fn request_oauth_token_response(tokens: &StoredOAuthTokens) -> OAuthTokenResponse {
    let mut token_response = tokens.token_response.0.clone();
    token_response.set_refresh_token(None);
    token_response.set_expires_in(None);
    token_response
}

fn refreshed_tokens(
    mut token_response: OAuthTokenResponse,
    previous: &StoredOAuthTokens,
    inner: &OAuthPersistorInner,
) -> StoredOAuthTokens {
    if token_response.refresh_token().is_none() {
        token_response.set_refresh_token(previous.token_response.0.refresh_token().cloned());
    }
    if token_response.scopes().is_none() {
        token_response.set_scopes(previous.token_response.0.scopes().cloned());
    }
    let expires_at = compute_expires_at_millis(&token_response);
    StoredOAuthTokens {
        server_name: inner.server_name.clone(),
        url: inner.url.clone(),
        client_id: previous.client_id.clone(),
        token_response: WrappedOAuthTokenResponse(token_response),
        expires_at,
    }
}
