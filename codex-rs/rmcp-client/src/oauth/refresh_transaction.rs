//! Serialized read-refresh-write transactions for MCP OAuth credentials.

use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use anyhow::Context;
use anyhow::Result;
use codex_keyring_store::DefaultKeyringStore;
use codex_keyring_store::KeyringStore;
use oauth2::TokenResponse;
use rmcp::transport::auth::AuthError;
use rmcp::transport::auth::AuthorizationManager;
use rmcp::transport::auth::CredentialStore as _;
use rmcp::transport::auth::InMemoryCredentialStore;
use rmcp::transport::auth::OAuthTokenResponse;
use rmcp::transport::auth::StoredCredentials;
use tokio::time::timeout;
use tracing::debug;
use tracing::warn;

use super::OAuthPersistor;
use super::OAuthPersistorInner;
use super::StoredOAuthTokens;
use super::WrappedOAuthTokenResponse;
use super::compute_expires_at_millis;
use super::refresh_lock::RefreshCredentialLock;
use super::token_needs_refresh;

const REFRESH_REQUEST_TIMEOUT: Duration = Duration::from_secs(45);

impl OAuthPersistor {
    pub(crate) async fn refresh_if_needed(&self) -> Result<()> {
        self.refresh_if_needed_in(&DefaultKeyringStore, REFRESH_REQUEST_TIMEOUT)
            .await
    }

    /// Injects the credential backend and provider timeout for deterministic failure-path tests.
    pub(super) async fn refresh_if_needed_in<K: KeyringStore + Clone + 'static>(
        &self,
        keyring_store: &K,
        refresh_request_timeout: Duration,
    ) -> Result<()> {
        let expires_at = {
            let guard = self.inner.last_credentials.lock().await;
            guard.as_ref().and_then(|tokens| tokens.expires_at)
        };

        if !token_needs_refresh(expires_at) {
            return Ok(());
        }

        let persistor = self.clone();
        let keyring_store = keyring_store.clone();
        // Once the provider can consume a rotating token, caller cancellation must not cancel
        // persistence. The owned task continues with independently bounded lock and request waits.
        // A provider timeout leaves the outcome unknown and permits a later serialized retry:
        // provider grace may recover, otherwise reauthorization is unavoidable. This residual
        // risk is preferred to holding the credential lock indefinitely.
        let transaction_task = tokio::spawn(async move {
            let result = persistor
                .refresh_transaction(&keyring_store, refresh_request_timeout)
                .await;

            // Keep this summary inside the owned task so caller cancellation cannot suppress it.
            if let Err(error) = &result {
                warn!(
                    server_name = %persistor.inner.server_name,
                    refresh_reason = "expiry",
                    error = %error,
                    "MCP OAuth refresh transaction failed"
                );
            }

            result
        });
        transaction_task.await.with_context(|| {
            format!(
                "OAuth refresh task failed for server {}",
                self.inner.server_name
            )
        })?
    }

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "AuthorizationManager async access must be serialized through its Tokio mutex"
    )]
    #[tracing::instrument(
        level = "debug",
        skip_all,
        fields(
            server_name = %self.inner.server_name,
            refresh_reason = "expiry",
        ),
        err
    )]
    async fn refresh_transaction<K: KeyringStore + Clone + 'static>(
        &self,
        keyring_store: &K,
        refresh_request_timeout: Duration,
    ) -> Result<()> {
        debug!("waiting for the MCP OAuth credential transaction lock");
        let _lock =
            RefreshCredentialLock::acquire_for_server(&self.inner.server_name, &self.inner.url)
                .await?;
        debug!("acquired the MCP OAuth credential transaction lock");

        // Stay on the lifecycle-pinned store. A failure is surfaced rather than falling back and
        // possibly replaying an older rotating refresh token from the other store.
        debug!("rereading authoritative MCP OAuth credentials");
        let latest = self.inner.credential_store.load(
            keyring_store,
            &self.inner.server_name,
            &self.inner.url,
        )?;

        // The pre-lock snapshot is only a hint. This locked reread is authoritative, so adopt a
        // winner from another process rather than refreshing its predecessor.
        let Some(latest) = latest else {
            let manager = self.inner.authorization_manager.clone();
            manager
                .lock()
                .await
                .set_credential_store(InMemoryCredentialStore::new());
            *self.inner.last_credentials.lock().await = None;
            return Err(AuthError::AuthorizationRequired).with_context(|| {
                format!(
                    "OAuth tokens for server {} were removed before refresh; authorization required",
                    self.inner.server_name
                )
            });
        };

        if !token_needs_refresh(latest.expires_at) {
            debug!("adopting newer MCP OAuth credentials without contacting the provider");
            let manager = self.inner.authorization_manager.clone();
            let mut guard = manager.lock().await;
            install_tokens_in_manager_guard(&mut guard, &latest).await?;
            *self.inner.last_credentials.lock().await = Some(latest);
            return Ok(());
        }

        // Preserve RMCP's `AuthorizationRequired` marker only for credentials known to be
        // unrefreshable. Network and provider failures below remain ordinary errors.
        if latest
            .token_response
            .0
            .refresh_token()
            .is_none_or(|refresh_token| refresh_token.secret().trim().is_empty())
        {
            return Err(AuthError::AuthorizationRequired).with_context(|| {
                format!(
                    "OAuth tokens for server {} cannot be refreshed; authorization required",
                    self.inner.server_name
                )
            });
        }

        let manager = self.inner.authorization_manager.clone();
        // The provider uses a separate HTTP client and cannot re-enter `AuthClient`. Retain this
        // async guard so requests cannot observe credentials while they are staged and committed.
        let mut guard = manager.lock().await;
        install_tokens_in_manager_guard(&mut guard, &latest)
            .await
            .context("failed to stage OAuth credentials for refresh")?;
        // The owned task prevents caller deadlines from canceling after possible token rotation;
        // this timeout independently bounds the provider request.
        debug!(
            timeout_ms = refresh_request_timeout.as_millis(),
            "requesting refreshed MCP OAuth credentials from the provider"
        );
        let refreshed = match timeout(refresh_request_timeout, guard.refresh_token()).await {
            Ok(Ok(token_response)) => {
                debug!("received refreshed MCP OAuth credentials from the provider");
                refreshed_tokens(token_response, &latest, &self.inner)
            }
            Ok(Err(error @ AuthError::TokenRefreshFailed(_))) => {
                // RMCP 1.8 collapses definitive OAuth rejection (for example,
                // `invalid_grant`) and transient token-endpoint failures into this string
                // variant. Match RMCP's own request path for now so rejected refresh tokens
                // prompt reauthorization instead of surfacing as generic MCP startup failures.
                // This can also prompt reauthorization after a transient failure.
                // TODO: When RMCP exposes a typed distinction for refresh-token rejection,
                // map only that definitive rejection to `AuthorizationRequired` here.
                warn!(
                    error = %error,
                    "MCP OAuth refresh failed; reauthorization required by RMCP compatibility policy"
                );
                return Err(AuthError::AuthorizationRequired).with_context(|| {
                    format!(
                        "failed to refresh OAuth tokens for server {}: {error}",
                        self.inner.server_name
                    )
                });
            }
            Ok(Err(error)) => {
                warn!(
                    error = %error,
                    "MCP OAuth provider refresh failed"
                );
                return Err(error).with_context(|| {
                    format!(
                        "failed to refresh OAuth tokens for server {}",
                        self.inner.server_name
                    )
                });
            }
            Err(_) => {
                warn!(
                    timeout_ms = refresh_request_timeout.as_millis(),
                    "MCP OAuth provider refresh timed out; the outcome is unknown and a later serialized retry is permitted"
                );
                anyhow::bail!(
                    "timed out after {refresh_request_timeout:?} refreshing OAuth tokens for server {}",
                    self.inner.server_name
                );
            }
        };

        // Persist to the pinned source before exposing the refreshed token. On failure, restore
        // the prior in-process credential and return the error; serving an unpersisted token would
        // hide the root cause until a later process restart. If the provider already consumed the
        // prior token, the next refresh may require reauthorization. That is the deliberate
        // fail-closed policy.
        // TODO: Add a bounded persistence retry only if telemetry shows this is common; never
        // silently switch stores or continue with an unpersisted credential.
        debug!("persisting refreshed MCP OAuth credentials to the resolved store");
        if let Err(error) =
            self.inner
                .credential_store
                .save(keyring_store, &self.inner.server_name, &refreshed)
        {
            warn!(
                error = %error,
                "failed to persist refreshed MCP OAuth credentials; returning the error and restoring the previous in-process credentials"
            );
            install_tokens_in_manager_guard(&mut guard, &latest)
                .await
                .context(
                    "failed to restore previous OAuth credentials after refresh persistence failed",
                )?;
            return Err(error);
        }

        // This layer retains RMCP's legacy persistence hook. Install the same merged response
        // (including carried-forward refresh token/scopes) so that hook cannot overwrite durable
        // credentials with the provider's partial response.
        install_tokens_in_manager_guard(&mut guard, &refreshed)
            .await
            .context(
                "refreshed OAuth tokens were persisted but could not be installed in the authorization manager",
            )?;
        *self.inner.last_credentials.lock().await = Some(refreshed);
        drop(guard);
        debug!("persisted refreshed MCP OAuth credentials and completed the transaction");
        Ok(())
    }
}

async fn install_tokens_in_manager_guard(
    authorization_manager: &mut AuthorizationManager,
    tokens: &StoredOAuthTokens,
) -> Result<()> {
    let store = InMemoryCredentialStore::new();
    let token_response = tokens.token_response.0.clone();
    let granted_scopes = token_response
        .scopes()
        .map(|scopes| scopes.iter().map(|scope| scope.to_string()).collect())
        .unwrap_or_default();
    let token_received_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_secs());
    store
        .save(StoredCredentials::new(
            tokens.client_id.clone(),
            Some(token_response),
            granted_scopes,
            token_received_at,
        ))
        .await
        .context("failed to stage OAuth tokens for authorization manager")?;

    authorization_manager.set_credential_store(store);
    // TODO(stevenlee): Add an RMCP adoption API that atomically updates credentials, client ID,
    // and private `current_scopes`; this path cannot synchronize RMCP's scope-upgrade state.
    authorization_manager
        .initialize_from_store()
        .await
        .context("failed to adopt refreshed OAuth tokens")?;
    Ok(())
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
    StoredOAuthTokens {
        server_name: inner.server_name.clone(),
        url: inner.url.clone(),
        client_id: previous.client_id.clone(),
        expires_at: compute_expires_at_millis(&token_response),
        token_response: WrappedOAuthTokenResponse(token_response),
    }
}
