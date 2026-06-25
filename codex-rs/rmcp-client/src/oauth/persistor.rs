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
    credential_state: Mutex<CredentialState>,
}

struct CredentialState {
    current: Option<StoredOAuthTokens>,
    // A successful provider response becomes authoritative for this client before the fallible
    // durable write. We intentionally do not retry that write in this stack: healthy in-memory
    // credentials may keep serving this process. If they later need refresh, retain the durable
    // snapshot that preceded the failed write: an unchanged snapshot is stale and fails closed,
    // while a genuinely changed login, logout, or concurrent refresh remains authoritative.
    // The persistence warning is the signal for deciding whether a bounded retry policy is
    // warranted in a follow-up.
    unpersisted_refresh: Option<UnpersistedRefresh>,
}

#[derive(Clone)]
struct UnpersistedRefresh {
    previously_persisted: Option<StoredOAuthTokens>,
}

fn durable_credentials_match_snapshot(
    latest: &Option<StoredOAuthTokens>,
    snapshot: &Option<StoredOAuthTokens>,
) -> bool {
    match (latest, snapshot) {
        (Some(latest), Some(snapshot)) => {
            // `expires_in` is reconstructed from the durable `expires_at` timestamp on every
            // load, so elapsed time alone must not look like a concurrent credential change.
            let mut comparable_latest = latest.clone();
            comparable_latest
                .token_response
                .0
                .set_expires_in(snapshot.token_response.0.expires_in().as_ref());
            comparable_latest == *snapshot
        }
        (None, None) => true,
        (Some(_), None) | (None, Some(_)) => false,
    }
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
                credential_state: Mutex::new(CredentialState {
                    current: initial_credentials,
                    unpersisted_refresh: None,
                }),
            }),
        }
    }

    /// Persists credentials that RMCP may still refresh before the ownership switch lands.
    ///
    /// The next stack layer removes this compatibility path atomically with request-only RMCP
    /// credentials. Keeping it here makes this intermediate tip preserve the existing contract.
    pub(crate) async fn persist_if_needed(&self) -> Result<()> {
        self.persist_if_needed_locked_with_keyring_store(&DefaultKeyringStore)
            .await
    }

    pub(super) async fn persist_if_needed_locked_with_keyring_store<
        K: KeyringStore + Clone + 'static,
    >(
        &self,
        keyring_store: &K,
    ) -> Result<()> {
        let (snapshot, unpersisted_refresh) = {
            let state = self.inner.credential_state.lock().await;
            (state.current.clone(), state.unpersisted_refresh.clone())
        };
        let (client_id, current_credentials) = self.manager_credentials().await?;
        let manager_changed = match (&snapshot, current_credentials.as_ref()) {
            (Some(previous), Some(current)) => {
                previous.client_id != client_id
                    || previous.token_response != WrappedOAuthTokenResponse(current.clone())
            }
            (None, None) => false,
            (Some(_), None) | (None, Some(_)) => true,
        };
        if !manager_changed {
            return Ok(());
        }

        let _lock =
            RefreshCredentialLock::acquire_for_server(&self.inner.server_name, &self.inner.url)
                .await?;
        let latest = self.load_resolved_credentials(keyring_store)?;

        let latest_matches_snapshot = durable_credentials_match_snapshot(&latest, &snapshot)
            || unpersisted_refresh.as_ref().is_some_and(|pending| {
                // The failed write never changed durable authority. If storage still contains the
                // last known durable snapshot, persist the manager's newer result instead of
                // adopting and replaying that stale snapshot.
                durable_credentials_match_snapshot(&latest, &pending.previously_persisted)
            });

        if !latest_matches_snapshot {
            // A completed login or logout is authoritative over tokens refreshed inside an RMCP
            // operation. Adopt that state instead of allowing delayed post-operation persistence
            // to overwrite the login or resurrect the logout.
            match latest {
                Some(latest) => self.adopt_credentials(latest).await?,
                None => {
                    self.clear_manager_credentials().await;
                    let mut state = self.inner.credential_state.lock().await;
                    state.current = None;
                    state.unpersisted_refresh = None;
                }
            }
            return Ok(());
        }

        self.persist_if_needed_with_keyring_store(keyring_store)
            .await
    }

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "AuthorizationManager async access must be serialized through its mutex"
    )]
    async fn manager_credentials(&self) -> Result<(String, Option<OAuthTokenResponse>)> {
        let manager = self.inner.authorization_manager.clone();
        let guard = manager.lock().await;
        guard.get_credentials().await.map_err(Into::into)
    }

    fn load_resolved_credentials<K: KeyringStore + Clone + 'static>(
        &self,
        keyring_store: &K,
    ) -> Result<Option<StoredOAuthTokens>> {
        match self.inner.credential_store {
            ResolvedOAuthCredentialStore::File => {
                load_oauth_tokens_from_file(&self.inner.server_name, &self.inner.url)
                    .context("failed to reread OAuth tokens from resolved file storage")
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
                )
            }
        }
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
                let mut state = self.inner.credential_state.lock().await;
                let new_token_response = WrappedOAuthTokenResponse(credentials.clone());
                let same_token = state
                    .current
                    .as_ref()
                    .map(|prev| prev.token_response == new_token_response)
                    .unwrap_or(false);
                let expires_at = if same_token {
                    state.current.as_ref().and_then(|prev| prev.expires_at)
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
                if state.current.as_ref() != Some(&stored) {
                    // A provider may already have consumed the old rotating refresh token. Make B
                    // authoritative in this process before the fallible save.
                    // Preserve the last snapshot known to be durable across repeated in-memory
                    // changes. Using the immediately preceding in-memory token here could make an
                    // unchanged stale store look like an external login or refresh.
                    let previously_persisted = state
                        .unpersisted_refresh
                        .take()
                        .map(|pending| pending.previously_persisted)
                        .unwrap_or_else(|| state.current.clone());
                    state.current = Some(stored.clone());
                    state.unpersisted_refresh = Some(UnpersistedRefresh {
                        previously_persisted,
                    });
                    debug!("persisting refreshed MCP OAuth credentials to the resolved store");
                    let persistence_started_at = Instant::now();
                    let persistence_result = match self.inner.credential_store {
                        ResolvedOAuthCredentialStore::File => save_oauth_tokens_to_file(&stored),
                        ResolvedOAuthCredentialStore::Keyring(keyring_backend_kind) => {
                            save_oauth_tokens_with_keyring(
                                keyring_store,
                                keyring_backend_kind,
                                &self.inner.server_name,
                                &stored,
                            )
                        }
                    };
                    if let Err(error) = persistence_result {
                        warn!(
                            persistence_elapsed_ms = persistence_started_at.elapsed().as_millis(),
                            error = %error,
                            "failed to persist refreshed MCP OAuth credentials; retaining them as the in-process authority without retrying persistence"
                        );
                        return Err(error);
                    }
                    state.unpersisted_refresh = None;
                    debug!(
                        persistence_elapsed_ms = persistence_started_at.elapsed().as_millis(),
                        "persisted refreshed MCP OAuth credentials"
                    );
                }
            }
            None => {
                let mut state = self.inner.credential_state.lock().await;
                if state.current.take().is_some()
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
                state.unpersisted_refresh = None;
            }
        }

        Ok(())
    }

    pub(crate) async fn refresh_if_needed(&self) -> Result<()> {
        self.refresh_if_needed_with_keyring_store(&DefaultKeyringStore)
            .await
    }

    pub(crate) async fn refresh_after_unauthorized(&self) -> Result<()> {
        self.refresh_after_unauthorized_with_keyring_store(&DefaultKeyringStore)
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
            let state = self.inner.credential_state.lock().await;
            state.current.as_ref().and_then(|tokens| tokens.expires_at)
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
    ) -> Result<()> {
        self.run_owned_refresh_transaction(
            keyring_store.clone(),
            RefreshReason::Unauthorized,
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
        tokio::spawn(async move {
            persistor
                .refresh_transaction(&keyring_store, reason, refresh_request_timeout)
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
        let local_access_token = {
            let state = self.inner.credential_state.lock().await;
            state
                .current
                .as_ref()
                .map(|tokens| tokens.token_response.0.access_token().secret().to_string())
        };
        debug!("waiting for the MCP OAuth credential transaction lock");
        let _lock =
            RefreshCredentialLock::acquire_for_server(&self.inner.server_name, &self.inner.url)
                .await?;
        debug!(
            lock_wait_ms = lock_started_at.elapsed().as_millis(),
            "acquired the MCP OAuth credential transaction lock"
        );

        let unpersisted_refresh = {
            let state = self.inner.credential_state.lock().await;
            if let Some(unpersisted_refresh) = state.unpersisted_refresh.as_ref() {
                if matches!(reason, RefreshReason::Expiry)
                    && state
                        .current
                        .as_ref()
                        .is_some_and(|tokens| !token_needs_refresh(tokens.expires_at))
                {
                    debug!(
                        "using the memory-authoritative MCP OAuth credentials from a refresh whose persistence failed"
                    );
                    return Ok(());
                }
                Some(unpersisted_refresh.clone())
            } else {
                None
            }
        };
        // The refresh transaction must stay on the store that supplied its snapshot. Falling back
        // here could replay an older rotating refresh token from the other store. We assume store
        // availability is stable for this client lifecycle and surface violations of that
        // assumption instead of switching stores.
        let latest = self.load_resolved_credentials(keyring_store)?;

        if let Some(unpersisted_refresh) = unpersisted_refresh {
            if durable_credentials_match_snapshot(
                &latest,
                &unpersisted_refresh.previously_persisted,
            ) {
                anyhow::bail!(
                    "refusing to refresh MCP OAuth credentials for server {} because the previous refresh succeeded but its credentials were not persisted",
                    self.inner.server_name
                );
            }
            debug!(
                "the resolved store changed after refresh persistence failed; adopting the serialized login, logout, or concurrent refresh"
            );
        }

        // The pre-lock snapshot only decides whether a refresh transaction might be needed. Once
        // the lock is held, this reread is authoritative: adopt it before deciding whether to
        // refresh so this process never sends a refresh token superseded by another process.
        let Some(latest) = latest else {
            self.clear_manager_credentials().await;
            let mut state = self.inner.credential_state.lock().await;
            state.current = None;
            state.unpersisted_refresh = None;
            anyhow::bail!(
                "OAuth tokens for server {} were removed before refresh; authorization required",
                self.inner.server_name
            );
        };

        let latest_access_token = latest.token_response.0.access_token().secret();
        // Expiry refresh can adopt any reread that is now healthy. A 401 is different: an
        // unexpired token is still rejected, so adopt only when another actor has already changed
        // the access token; otherwise force one serialized provider refresh.
        let should_adopt = !token_needs_refresh(latest.expires_at)
            && match reason {
                RefreshReason::Expiry => true,
                RefreshReason::Unauthorized => {
                    local_access_token.as_deref() != Some(latest_access_token)
                }
            };
        if should_adopt {
            self.adopt_credentials(latest).await?;
            return Ok(());
        }

        let manager = self.inner.authorization_manager.clone();
        let mut guard = manager.lock().await;
        install_tokens_in_manager_guard(&mut guard, &latest)
            .await
            .context("failed to stage OAuth credentials for refresh")?;
        let provider_started_at = Instant::now();
        debug!(
            timeout_ms = refresh_request_timeout.as_millis(),
            "requesting refreshed MCP OAuth credentials from the provider"
        );
        let refreshed = match timeout(refresh_request_timeout, guard.refresh_token()).await {
            Ok(Ok(token_response)) => {
                debug!(
                    provider_elapsed_ms = provider_started_at.elapsed().as_millis(),
                    "received refreshed MCP OAuth credentials from the provider"
                );
                refreshed_tokens(token_response, &latest, &self.inner)
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
        };
        install_tokens_in_manager_guard(&mut guard, &refreshed)
            .await
            .context("failed to install refreshed OAuth credentials")?;
        drop(guard);

        {
            let mut state = self.inner.credential_state.lock().await;
            state.current = Some(refreshed.clone());
            state.unpersisted_refresh = Some(UnpersistedRefresh {
                previously_persisted: Some(latest.clone()),
            });
        }
        debug!(
            "installed refreshed MCP OAuth credentials in memory and marked persistence pending"
        );
        // Once the provider rotates a refresh token, persistence must complete even if the caller's
        // deadline expires in the meantime. Returning early here would lose the only usable token.
        // Refresh persistence stays on the source resolved at client startup. In particular, a
        // keyring failure must surface instead of writing the rotated token to fallback File.
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
                "failed to persist refreshed MCP OAuth credentials; retaining them as the in-process authority without retrying persistence"
            );
            return Err(error);
        }
        let mut state = self.inner.credential_state.lock().await;
        state.unpersisted_refresh = None;
        debug!(
            persistence_elapsed_ms = persistence_started_at.elapsed().as_millis(),
            transaction_elapsed_ms = transaction_started_at.elapsed().as_millis(),
            "persisted refreshed MCP OAuth credentials and completed the transaction"
        );
        Ok(())
    }

    async fn adopt_credentials(&self, tokens: StoredOAuthTokens) -> Result<()> {
        install_tokens_in_manager(&self.inner.authorization_manager, &tokens).await?;
        let mut state = self.inner.credential_state.lock().await;
        state.current = Some(tokens);
        state.unpersisted_refresh = None;
        Ok(())
    }

    async fn clear_manager_credentials(&self) {
        let manager = self.inner.authorization_manager.clone();
        let mut guard = manager.lock().await;
        guard.set_credential_store(InMemoryCredentialStore::new());
    }
}

#[derive(Clone, Copy)]
enum RefreshReason {
    Expiry,
    Unauthorized,
}

impl RefreshReason {
    fn as_str(self) -> &'static str {
        match self {
            Self::Expiry => "expiry",
            Self::Unauthorized => "unauthorized",
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
    install_tokens_in_manager_guard(&mut guard, tokens).await
}

async fn install_tokens_in_manager_guard(
    authorization_manager: &mut AuthorizationManager,
    tokens: &StoredOAuthTokens,
) -> Result<()> {
    let store = InMemoryCredentialStore::new();
    store
        .save(stored_credentials_from_tokens(tokens))
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
