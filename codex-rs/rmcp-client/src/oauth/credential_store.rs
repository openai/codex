//! Adapts Codex's pinned credential storage to RMCP-owned OAuth refreshes.
//!
//! Attach this store only after `AuthorizationManager::initialize_from_store` completes
//! against an in-memory store. Initialization may save tokenless client credentials, which
//! this refresh adapter does not support.
//!
//! Ordinary token reads use the cached credentials. Refresh-guard acquisition rereads
//! the pinned store before RMCP exchanges the token and saves the result. Codex
//! preparation rechecks freshness under that guard before asking RMCP to refresh.
//! Saves and clears require an active guard; no store operation reacquires the lock.
//! The runtime snapshot advances only for credentials compatible with this connection,
//! so replacement logins still cause a rebuild.
//! Synchronous store operations run off the Tokio workers so caller deadlines remain pollable.
//! Blocking mutations retain the caller's transaction guard even if their await is cancelled.

use std::sync::Arc;
use std::sync::Weak;

use anyhow::Context;
use anyhow::Result;
use codex_config::McpServerOAuthConfig;
use codex_keyring_store::DefaultKeyringStore;
use codex_keyring_store::KeyringStore;
use futures::future::BoxFuture;
use oauth2::Scope;
use oauth2::TokenResponse;
use rmcp::transport::auth::AuthError;
use rmcp::transport::auth::AuthorizationManager;
use rmcp::transport::auth::CredentialRefreshGuard;
use rmcp::transport::auth::CredentialStore;
use rmcp::transport::auth::StoredCredentials;
use tokio::sync::Mutex;
use tracing::warn;

use crate::oauth_client_credentials::OAuthClientCredentials;
use crate::oauth_http_client::PROACTIVE_REFRESH_TIMEOUT;

use super::RefreshCredentialLock;
use super::ResolvedOAuthCredentialStore;
use super::StoredOAuthTokens;
use super::WrappedOAuthTokenResponse;
use super::normalized_oauth_credentials;
use super::refresh_expires_in_from_timestamp;
use super::refresh_transaction::REFRESH_REQUEST_TIMEOUT;
use super::token_needs_refresh;
use super::validate_refresh_token_issuer;

#[derive(Clone)]
pub(crate) struct OAuthCredentialStore<K = DefaultKeyringStore> {
    inner: Arc<OAuthCredentialStoreInner<K>>,
    held_refresh_guard: Option<Arc<RefreshCredentialLock>>,
}

struct OAuthCredentialStoreInner<K> {
    server_name: String,
    url: String,
    client_id: String,
    issuer: Option<String>,
    store: ResolvedOAuthCredentialStore,
    keyring: K,
    oauth_config: Option<McpServerOAuthConfig>,
    last_credentials: Mutex<Option<StoredOAuthTokens>>,
    refresh_guard: Mutex<Weak<RefreshCredentialLock>>,
}

impl<K: KeyringStore + Clone + 'static> OAuthCredentialStore<K> {
    pub(crate) fn new(
        tokens: StoredOAuthTokens,
        store: ResolvedOAuthCredentialStore,
        keyring: K,
        oauth_config: Option<McpServerOAuthConfig>,
    ) -> Self {
        Self {
            inner: Arc::new(OAuthCredentialStoreInner {
                server_name: tokens.server_name.clone(),
                url: tokens.url.clone(),
                client_id: tokens.client_id.clone(),
                issuer: tokens.bound_issuer().map(str::to_owned),
                store,
                keyring,
                oauth_config,
                last_credentials: Mutex::new(Some(tokens)),
                refresh_guard: Mutex::new(Weak::new()),
            }),
            held_refresh_guard: None,
        }
    }

    pub(crate) async fn refresh_if_needed(&self, manager: &mut AuthorizationManager) -> Result<()> {
        let guard = self.acquire_transaction_guard().await?;
        let tokens = self
            .inner
            .last_credentials
            .lock()
            .await
            .clone()
            .ok_or(AuthError::AuthorizationRequired)?;
        if !token_needs_refresh(tokens.expires_at) {
            return Ok(());
        }
        let metadata = manager.resolve_metadata().await?.metadata;
        validate_refresh_token_issuer(&metadata, &tokens)?;
        manager.set_metadata(metadata);
        manager.configure_client_id(&tokens.client_id)?;
        OAuthClientCredentials::resolve(self.inner.oauth_config.as_ref())?.configure_for_refresh(
            manager,
            &self.inner.url,
            &tokens.client_id,
        )?;
        // Reuse the guard for RMCP's exchange and save after the locked freshness check.
        manager.set_credential_store(Self {
            inner: Arc::clone(&self.inner),
            held_refresh_guard: Some(guard),
        });
        let result = PROACTIVE_REFRESH_TIMEOUT
            .scope(REFRESH_REQUEST_TIMEOUT, manager.refresh_token())
            .await;
        manager.set_credential_store(self.clone());
        match result {
            Ok(_) => Ok(()),
            Err(AuthError::TokenRefreshRejected(_)) => Err(AuthError::AuthorizationRequired.into()),
            Err(error) => Err(error.into()),
        }
    }

    pub(crate) async fn stored_credentials(&self) -> Option<StoredOAuthTokens> {
        normalized_oauth_credentials(self.inner.last_credentials.lock().await.as_ref())
    }

    pub(crate) async fn acquire_transaction_guard(
        &self,
    ) -> Result<Arc<RefreshCredentialLock>, AuthError> {
        let guard = Arc::new(
            RefreshCredentialLock::acquire_for_server(&self.inner.server_name, &self.inner.url)
                .await
                .map_err(credential_store_error)?,
        );
        let inner = Arc::clone(&self.inner);
        let tokens = tokio::task::spawn_blocking(move || {
            inner
                .store
                .load(&inner.keyring, &inner.server_name, &inner.url)
        })
        .await
        .context("OAuth credential load task failed")
        .map_err(credential_store_error)?
        .map_err(credential_store_error)?
        .ok_or(AuthError::AuthorizationRequired)?;
        self.validate_connection(&tokens)?;
        *self.inner.last_credentials.lock().await = Some(tokens);
        *self.inner.refresh_guard.lock().await = Arc::downgrade(&guard);
        Ok(guard)
    }

    fn validate_connection(&self, tokens: &StoredOAuthTokens) -> Result<(), AuthError> {
        if tokens.client_id != self.inner.client_id
            || tokens.bound_issuer() != self.inner.issuer.as_deref()
            || tokens.has_refresh_token() && tokens.bound_issuer().is_none()
        {
            warn!(
                server_name = %self.inner.server_name,
                "stored OAuth credentials no longer match this connection's client and issuer; authorization must be rebuilt"
            );
            return Err(AuthError::AuthorizationRequired);
        }
        if !tokens.has_refresh_token() && !tokens.access_token_is_usable_without_refresh() {
            return Err(AuthError::TokenExpired);
        }
        Ok(())
    }
}

// Spell out RMCP's object-safe boxed-future interface without introducing async-trait here.
impl<K: KeyringStore + Clone + 'static> CredentialStore for OAuthCredentialStore<K> {
    fn load<'life0, 'async_trait>(
        &'life0 self,
    ) -> BoxFuture<'async_trait, Result<Option<StoredCredentials>, AuthError>>
    where
        'life0: 'async_trait,
        Self: 'async_trait,
    {
        Box::pin(async move {
            Ok(self
                .inner
                .last_credentials
                .lock()
                .await
                .as_ref()
                .map(rmcp_credentials))
        })
    }

    fn save<'life0, 'async_trait>(
        &'life0 self,
        credentials: StoredCredentials,
    ) -> BoxFuture<'async_trait, Result<(), AuthError>>
    where
        'life0: 'async_trait,
        Self: 'async_trait,
    {
        Box::pin(async move {
            let mut token_response = credentials
                .token_response
                .ok_or(AuthError::AuthorizationRequired)?;
            // Codex stores granted scopes in the token response rather than a separate field.
            if token_response.scopes().is_none() && !credentials.granted_scopes.is_empty() {
                token_response.set_scopes(Some(
                    credentials
                        .granted_scopes
                        .into_iter()
                        .map(Scope::new)
                        .collect(),
                ));
            }
            // The SDK's receipt time, rather than the time this save completes, owns expiry.
            if credentials.token_received_at.is_none() {
                token_response.set_expires_in(None);
            }
            let expires_at = credentials.token_received_at.and_then(|received_at| {
                token_response.expires_in().map(|expires_in| {
                    received_at
                        .saturating_mul(1000)
                        .saturating_add(u64::try_from(expires_in.as_millis()).unwrap_or(u64::MAX))
                })
            });
            let tokens = StoredOAuthTokens {
                server_name: self.inner.server_name.clone(),
                url: self.inner.url.clone(),
                issuer: credentials.issuer,
                client_id: credentials.client_id,
                token_response: WrappedOAuthTokenResponse(token_response),
                expires_at,
            };
            self.validate_connection(&tokens)?;
            let inner = Arc::clone(&self.inner);
            let guard = self
                .inner
                .refresh_guard
                .lock()
                .await
                .upgrade()
                .context("OAuth credential mutation requires an active refresh guard")
                .map_err(credential_store_error)?;
            let tokens = tokio::task::spawn_blocking(move || -> Result<StoredOAuthTokens> {
                let _guard = guard;
                inner
                    .store
                    .save(&inner.keyring, &inner.server_name, &tokens)?;
                Ok(tokens)
            })
            .await
            .context("OAuth credential save task failed")
            .map_err(credential_store_error)?
            .map_err(credential_store_error)?;
            *self.inner.last_credentials.lock().await = Some(tokens);
            Ok(())
        })
    }

    fn clear<'life0, 'async_trait>(&'life0 self) -> BoxFuture<'async_trait, Result<(), AuthError>>
    where
        'life0: 'async_trait,
        Self: 'async_trait,
    {
        Box::pin(async move {
            let inner = Arc::clone(&self.inner);
            let guard = self
                .inner
                .refresh_guard
                .lock()
                .await
                .upgrade()
                .context("OAuth credential mutation requires an active refresh guard")
                .map_err(credential_store_error)?;
            tokio::task::spawn_blocking(move || -> Result<()> {
                let _guard = guard;
                inner
                    .store
                    .delete(&inner.keyring, &inner.server_name, &inner.url)?;
                Ok(())
            })
            .await
            .context("OAuth credential removal task failed")
            .map_err(credential_store_error)?
            .map_err(credential_store_error)
        })
    }

    fn acquire_refresh_guard<'life0, 'async_trait>(
        &'life0 self,
    ) -> BoxFuture<'async_trait, Result<Option<CredentialRefreshGuard>, AuthError>>
    where
        'life0: 'async_trait,
        Self: 'async_trait,
    {
        Box::pin(async move {
            let guard = match &self.held_refresh_guard {
                Some(guard) => Arc::clone(guard),
                None => self.acquire_transaction_guard().await?,
            };
            Ok(Some(CredentialRefreshGuard::new(guard)))
        })
    }
}

fn credential_store_error(error: anyhow::Error) -> AuthError {
    AuthError::CredentialStoreError(format!("{error:#}"))
}

fn rmcp_credentials(tokens: &StoredOAuthTokens) -> StoredCredentials {
    let mut tokens = tokens.clone();
    refresh_expires_in_from_timestamp(&mut tokens);
    let token_received_at = tokens.expires_at.map(|expires_at| {
        let remaining = tokens.token_response.0.expires_in().unwrap_or_default();
        // Reconstruct the receipt time from the authority's deadline; a second clock read
        // could otherwise extend the grant if this task paused between the two reads.
        expires_at.saturating_sub(u64::try_from(remaining.as_millis()).unwrap_or(u64::MAX)) / 1000
    });
    if token_received_at.is_none() {
        tokens.token_response.0.set_expires_in(None);
    }
    let token_response = tokens.token_response.0;
    let granted_scopes = token_response
        .scopes()
        .map(|scopes| scopes.iter().map(|scope| scope.to_string()).collect())
        .unwrap_or_default();
    StoredCredentials::new(
        tokens.client_id,
        Some(token_response),
        granted_scopes,
        token_received_at,
    )
    .with_issuer(tokens.issuer)
}
