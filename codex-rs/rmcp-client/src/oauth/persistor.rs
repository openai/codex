//! Lifecycle-local persistence and serialized refresh transactions for MCP OAuth credentials.

use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use anyhow::Context;
use anyhow::Result;
use codex_config::types::AuthKeyringBackendKind;
use codex_keyring_store::DefaultKeyringStore;
use codex_keyring_store::KeyringStore;
use oauth2::TokenResponse;
use rmcp::transport::auth::AuthorizationManager;
use rmcp::transport::auth::CredentialStore as _;
use rmcp::transport::auth::InMemoryCredentialStore;
use rmcp::transport::auth::StoredCredentials;
use tokio::sync::Mutex;
use tokio::time::timeout;
use tracing::debug;
use tracing::warn;

use super::ResolvedOAuthCredentialStore;
use super::StoredOAuthTokens;
use super::WrappedOAuthTokenResponse;
use super::compute_expires_at_millis;
use super::compute_store_key;
use super::delete_oauth_tokens_from_direct_keyring;
use super::delete_oauth_tokens_from_file;
use super::delete_oauth_tokens_from_secrets_keyring;
use super::load_oauth_tokens_from_file;
use super::load_oauth_tokens_from_keyring;
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
    last_credentials: Mutex<Option<StoredOAuthTokens>>,
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
                last_credentials: Mutex::new(initial_credentials),
            }),
        }
    }

    /// Persists the latest stored credentials if they have changed.
    /// Deletes the credentials if they are no longer present.
    pub(crate) async fn persist_if_needed(&self) -> Result<()> {
        self.persist_if_needed_with_keyring_store(&DefaultKeyringStore)
            .await
    }

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "AuthorizationManager async access must be serialized through its mutex"
    )]
    pub(super) async fn persist_if_needed_with_keyring_store<K: KeyringStore + Clone + 'static>(
        &self,
        keyring_store: &K,
    ) -> Result<()> {
        let (client_id, maybe_credentials) = {
            let manager = self.inner.authorization_manager.clone();
            let guard = manager.lock().await;
            guard.get_credentials().await
        }?;

        match maybe_credentials {
            Some(credentials) => {
                let mut last_credentials = self.inner.last_credentials.lock().await;
                let new_token_response = WrappedOAuthTokenResponse(credentials.clone());
                let same_token = last_credentials
                    .as_ref()
                    .map(|prev| prev.token_response == new_token_response)
                    .unwrap_or(false);
                let expires_at = if same_token {
                    last_credentials.as_ref().and_then(|prev| prev.expires_at)
                } else {
                    compute_expires_at_millis(&credentials)
                };
                let stored = StoredOAuthTokens {
                    server_name: self.inner.server_name.clone(),
                    url: self.inner.url.clone(),
                    client_id,
                    token_response: new_token_response,
                    expires_at,
                };
                if last_credentials.as_ref() != Some(&stored) {
                    match self.inner.credential_store {
                        ResolvedOAuthCredentialStore::File => save_oauth_tokens_to_file(&stored)?,
                        ResolvedOAuthCredentialStore::Keyring(keyring_backend_kind) => {
                            save_oauth_tokens_with_keyring(
                                keyring_store,
                                keyring_backend_kind,
                                &self.inner.server_name,
                                &stored,
                            )?;
                        }
                    }
                    *last_credentials = Some(stored);
                }
            }
            None => {
                let mut last_serialized = self.inner.last_credentials.lock().await;
                if last_serialized.take().is_some()
                    && let Err(error) = match self.inner.credential_store {
                        ResolvedOAuthCredentialStore::File => {
                            let key = compute_store_key(&self.inner.server_name, &self.inner.url)?;
                            delete_oauth_tokens_from_file(&key).map(|_| ())
                        }
                        ResolvedOAuthCredentialStore::Keyring(AuthKeyringBackendKind::Direct) => {
                            delete_oauth_tokens_from_direct_keyring(
                                keyring_store,
                                &self.inner.server_name,
                                &self.inner.url,
                            )
                            .map(|_| ())
                        }
                        ResolvedOAuthCredentialStore::Keyring(AuthKeyringBackendKind::Secrets) => {
                            delete_oauth_tokens_from_secrets_keyring(
                                keyring_store,
                                &self.inner.server_name,
                                &self.inner.url,
                            )
                            .map(|_| ())
                        }
                    }
                {
                    warn!(
                        "failed to remove OAuth tokens for server {}: {error}",
                        self.inner.server_name
                    );
                }
            }
        }

        Ok(())
    }

    pub(crate) async fn refresh_if_needed(&self) -> Result<()> {
        self.refresh_if_needed_with_keyring_store(&DefaultKeyringStore)
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
            let guard = self.inner.last_credentials.lock().await;
            guard.as_ref().and_then(|tokens| tokens.expires_at)
        };

        if !token_needs_refresh(expires_at) {
            return Ok(());
        }

        self.run_owned_refresh_transaction(keyring_store.clone(), refresh_request_timeout)
            .await
    }

    async fn run_owned_refresh_transaction<K: KeyringStore + Clone + 'static>(
        &self,
        keyring_store: K,
        refresh_request_timeout: Duration,
    ) -> Result<()> {
        let persistor = self.clone();
        let server_name = self.inner.server_name.clone();
        // Once the provider may consume a rotating refresh token, dropping the caller's future
        // must not also drop refresh plus persistence. Dropping this JoinHandle detaches the task,
        // which continues under the credential lock until its explicit lock/provider bounds.
        //
        // A provider timeout deliberately leaves the outcome unknown, releases the lock, and
        // permits a later serialized retry. Some providers accept the previous token during a
        // grace period; otherwise that retry surfaces reauthorization. We accept that residual
        // token-family-revocation risk rather than holding the lock indefinitely.
        tokio::spawn(async move {
            persistor
                .refresh_transaction(&keyring_store, refresh_request_timeout)
                .await
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
        fields(server_name = %self.inner.server_name),
        err
    )]
    async fn refresh_transaction<K: KeyringStore + Clone + 'static>(
        &self,
        keyring_store: &K,
        refresh_request_timeout: Duration,
    ) -> Result<()> {
        let transaction_started_at = Instant::now();
        let lock_started_at = Instant::now();
        debug!("waiting for the MCP OAuth credential transaction lock");
        let snapshot = {
            let guard = self.inner.last_credentials.lock().await;
            guard.clone()
        };
        let key = compute_store_key(&self.inner.server_name, &self.inner.url)?;
        let _lock = RefreshCredentialLock::acquire(&key).await?;
        debug!(
            lock_wait_ms = lock_started_at.elapsed().as_millis(),
            "acquired the MCP OAuth credential transaction lock"
        );
        // The refresh transaction must stay on the store that supplied its snapshot. Falling back
        // here could replay an older rotating refresh token from the other store. We assume store
        // availability is stable for this client lifecycle and surface violations of that
        // assumption instead of switching stores.
        let latest = match self.inner.credential_store {
            ResolvedOAuthCredentialStore::File => {
                load_oauth_tokens_from_file(&self.inner.server_name, &self.inner.url)
                    .context("failed to reread OAuth tokens from resolved file storage")?
            }
            ResolvedOAuthCredentialStore::Keyring(keyring_backend_kind) => {
                load_oauth_tokens_from_keyring(
                    keyring_store,
                    keyring_backend_kind,
                    &self.inner.server_name,
                    &self.inner.url,
                )
                .context(
                    "failed to reread OAuth tokens from resolved keyring storage; refusing file fallback",
                )?
            }
        };

        if latest.is_none() && snapshot.is_some() {
            self.clear_manager_credentials().await;
            let mut last_credentials = self.inner.last_credentials.lock().await;
            *last_credentials = None;
            anyhow::bail!(
                "OAuth tokens for server {} were removed before refresh; authorization required",
                self.inner.server_name
            );
        }

        if latest != snapshot {
            if let Some(latest) = latest {
                let needs_refresh = token_needs_refresh(latest.expires_at);
                self.adopt_credentials(latest).await?;
                // `expires_in` is derived from `expires_at` on each load and can drift without a
                // persisted change. Even for a real concurrent update, keep going when the
                // authoritative token is still inside the refresh window.
                if !needs_refresh {
                    return Ok(());
                }
            } else {
                return Ok(());
            }
        }

        {
            let manager = self.inner.authorization_manager.clone();
            let guard = manager.lock().await;
            let provider_started_at = Instant::now();
            debug!(
                timeout_ms = refresh_request_timeout.as_millis(),
                "requesting refreshed MCP OAuth credentials from the provider"
            );
            match timeout(refresh_request_timeout, guard.refresh_token()).await {
                Ok(Ok(_token_response)) => {
                    debug!(
                        provider_elapsed_ms = provider_started_at.elapsed().as_millis(),
                        "received refreshed MCP OAuth credentials from the provider"
                    );
                }
                Ok(Err(error)) => {
                    warn!(
                        provider_elapsed_ms = provider_started_at.elapsed().as_millis(),
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
                        provider_elapsed_ms = provider_started_at.elapsed().as_millis(),
                        timeout_ms = refresh_request_timeout.as_millis(),
                        "MCP OAuth provider refresh timed out; the outcome is unknown and a later serialized retry is permitted"
                    );
                    anyhow::bail!(
                        "timed out after {refresh_request_timeout:?} refreshing OAuth tokens for server {}",
                        self.inner.server_name
                    );
                }
            }
        }

        // Once the provider returns a rotated token, persistence must finish before the credential
        // lock is released. In particular, caller startup deadlines must not cancel this step.
        let result = self
            .persist_if_needed_with_keyring_store(keyring_store)
            .await;
        if result.is_ok() {
            debug!(
                transaction_elapsed_ms = transaction_started_at.elapsed().as_millis(),
                "completed the MCP OAuth refresh transaction"
            );
        }
        result
    }

    async fn adopt_credentials(&self, tokens: StoredOAuthTokens) -> Result<()> {
        install_tokens_in_manager(&self.inner.authorization_manager, &tokens).await?;
        let mut last_credentials = self.inner.last_credentials.lock().await;
        *last_credentials = Some(tokens);
        Ok(())
    }

    async fn clear_manager_credentials(&self) {
        let manager = self.inner.authorization_manager.clone();
        let mut guard = manager.lock().await;
        guard.set_credential_store(InMemoryCredentialStore::new());
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
    let store = InMemoryCredentialStore::new();
    store
        .save(stored_credentials_from_tokens(tokens))
        .await
        .context("failed to stage OAuth tokens for authorization manager")?;

    let manager = authorization_manager.clone();
    let mut guard = manager.lock().await;
    guard.set_credential_store(store);
    // TODO(stevenlee): RMCP's `initialize_from_store` updates the credential store and client ID
    // but not its private `current_scopes`. Credential adoption can therefore leave scope-upgrade
    // state stale until RMCP exposes an adoption API that synchronizes both.
    guard
        .initialize_from_store()
        .await
        .context("failed to adopt refreshed OAuth tokens")?;
    Ok(())
}

fn stored_credentials_from_tokens(tokens: &StoredOAuthTokens) -> StoredCredentials {
    let token_response = tokens.token_response.0.clone();
    let granted_scopes = token_response
        .scopes()
        .map(|scopes| scopes.iter().map(|scope| scope.to_string()).collect())
        .unwrap_or_default();
    let token_received_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_secs());

    StoredCredentials::new(
        tokens.client_id.clone(),
        Some(token_response),
        granted_scopes,
        token_received_at,
    )
}
