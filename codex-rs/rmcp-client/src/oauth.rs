//! This file handles all logic related to managing MCP OAuth credentials.
//! All credentials are stored using the keyring crate which uses os-specific keyring services.
//! https://crates.io/crates/keyring
//! macOS: macOS keychain.
//! Windows: Windows Credential Manager
//! Linux: DBus-based Secret Service, the kernel keyutils, and a combo of the two
//! FreeBSD, OpenBSD: DBus-based Secret Service
//!
//! For Linux, we use linux-native-async-persistent which uses both keyutils and async-secret-service (see below) for storage.
//! See the docs for the keyutils_persistent module for a full explanation of why both are used. Because this store uses the
//! async-secret-service, you must specify the additional features required by that store
//!
//! async-secret-service provides access to the DBus-based Secret Service storage on Linux, FreeBSD, and OpenBSD. This is an asynchronous
//! keystore that always encrypts secrets when they are transferred across the bus. If DBus isn't installed the keystore will fall back to the json
//! file because we don't use the "vendored" feature.
//!
//! If the keyring is not available or fails, we fall back to CODEX_HOME/.credentials.json which is consistent with other coding CLI agents.

use anyhow::Context;
use anyhow::Error;
use anyhow::Result;
use codex_config::types::AuthKeyringBackendKind;
use codex_config::types::OAuthCredentialsStoreMode;
use codex_secrets::LocalSecretsNamespace;
use codex_secrets::SecretName;
use codex_secrets::SecretScope;
use codex_secrets::SecretsBackendKind;
use codex_secrets::SecretsManager;
use oauth2::AccessToken;
use oauth2::RefreshToken;
use oauth2::Scope;
use oauth2::TokenResponse;
use oauth2::basic::BasicTokenType;
use rmcp::transport::auth::OAuthTokenResponse;
use rmcp::transport::auth::VendorExtraTokenFields;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use serde_json::map::Map as JsonMap;
use sha2::Digest;
use sha2::Sha256;
use std::collections::BTreeMap;
use std::fs;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;
use tracing::warn;

use codex_keyring_store::DefaultKeyringStore;
use codex_keyring_store::KeyringStore;
use rmcp::transport::auth::AuthorizationManager;
use rmcp::transport::auth::CredentialStore as _;
use rmcp::transport::auth::InMemoryCredentialStore;
use rmcp::transport::auth::StoredCredentials;
use tokio::sync::Mutex;
use tokio::time::sleep;
use tokio::time::timeout;

use codex_utils_home_dir::find_codex_home;

const KEYRING_SERVICE: &str = "Codex MCP Credentials";
const MCP_OAUTH_SECRET_PREFIX: &str = "MCP_OAUTH";
const REFRESH_SKEW_MILLIS: u64 = 30_000;
const REFRESH_LOCK_DIR: &str = "mcp-oauth-refresh-locks";
const LOCK_ACQUIRE_TIMEOUT: Duration = Duration::from_secs(60);
const LOCK_RETRY_SLEEP: Duration = Duration::from_millis(500);
const REFRESH_REQUEST_TIMEOUT: Duration = Duration::from_secs(45);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StoredOAuthTokens {
    pub server_name: String,
    pub url: String,
    pub client_id: String,
    pub token_response: WrappedOAuthTokenResponse,
    #[serde(default)]
    pub expires_at: Option<u64>,
}

/// Wrap OAuthTokenResponse to allow for partial equality comparison.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WrappedOAuthTokenResponse(pub OAuthTokenResponse);

impl PartialEq for WrappedOAuthTokenResponse {
    fn eq(&self, other: &Self) -> bool {
        match (serde_json::to_string(self), serde_json::to_string(other)) {
            (Ok(s1), Ok(s2)) => s1 == s2,
            _ => false,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum StoredOAuthTokenStatus {
    Missing,
    Usable,
    AuthorizationRequired,
}

pub(crate) fn load_oauth_tokens(
    server_name: &str,
    url: &str,
    store_mode: OAuthCredentialsStoreMode,
    keyring_backend_kind: AuthKeyringBackendKind,
) -> Result<Option<StoredOAuthTokens>> {
    let keyring_store = DefaultKeyringStore;
    load_oauth_tokens_with_keyring_store(
        &keyring_store,
        server_name,
        url,
        store_mode,
        keyring_backend_kind,
    )
}

fn load_oauth_tokens_with_keyring_store<K: KeyringStore + Clone + 'static>(
    keyring_store: &K,
    server_name: &str,
    url: &str,
    store_mode: OAuthCredentialsStoreMode,
    keyring_backend_kind: AuthKeyringBackendKind,
) -> Result<Option<StoredOAuthTokens>> {
    match store_mode {
        OAuthCredentialsStoreMode::Auto => load_oauth_tokens_from_keyring_with_fallback_to_file(
            keyring_store,
            keyring_backend_kind,
            server_name,
            url,
        ),
        OAuthCredentialsStoreMode::File => load_oauth_tokens_from_file(server_name, url),
        OAuthCredentialsStoreMode::Keyring => {
            load_oauth_tokens_from_keyring(keyring_store, keyring_backend_kind, server_name, url)
                .with_context(|| "failed to read OAuth tokens from keyring".to_string())
        }
    }
}

pub(crate) fn oauth_token_status(
    server_name: &str,
    url: &str,
    store_mode: OAuthCredentialsStoreMode,
    keyring_backend_kind: AuthKeyringBackendKind,
) -> Result<StoredOAuthTokenStatus> {
    Ok(
        match load_oauth_tokens(server_name, url, store_mode, keyring_backend_kind)?.as_ref() {
            None => StoredOAuthTokenStatus::Missing,
            Some(tokens) if oauth_tokens_are_usable(tokens) => StoredOAuthTokenStatus::Usable,
            Some(_) => StoredOAuthTokenStatus::AuthorizationRequired,
        },
    )
}

fn oauth_tokens_are_usable(tokens: &StoredOAuthTokens) -> bool {
    if tokens.client_id.trim().is_empty() {
        return false;
    }

    let token_response = &tokens.token_response.0;
    if token_needs_refresh(tokens.expires_at) {
        return token_response
            .refresh_token()
            .is_some_and(|token| !token.secret().trim().is_empty());
    }

    !token_response.access_token().secret().trim().is_empty()
}

fn refresh_expires_in_from_timestamp(tokens: &mut StoredOAuthTokens) {
    let Some(expires_at) = tokens.expires_at else {
        return;
    };

    match expires_in_from_timestamp(expires_at) {
        Some(seconds) => {
            let duration = Duration::from_secs(seconds);
            tokens.token_response.0.set_expires_in(Some(&duration));
        }
        None => {
            // RMCP treats a missing expiry as unknown and uses the access token
            // as-is. Treat a known-expired timestamp as an explicit zero so
            // startup refreshes the token before the first request.
            tokens
                .token_response
                .0
                .set_expires_in(Some(&Duration::ZERO));
        }
    }
}

fn load_oauth_tokens_from_keyring_with_fallback_to_file<K: KeyringStore + Clone + 'static>(
    keyring_store: &K,
    keyring_backend_kind: AuthKeyringBackendKind,
    server_name: &str,
    url: &str,
) -> Result<Option<StoredOAuthTokens>> {
    match load_oauth_tokens_from_keyring(keyring_store, keyring_backend_kind, server_name, url) {
        Ok(Some(tokens)) => Ok(Some(tokens)),
        Ok(None) => load_oauth_tokens_from_file(server_name, url),
        Err(error) => {
            warn!("failed to read OAuth tokens from keyring: {error}");
            load_oauth_tokens_from_file(server_name, url)
                .with_context(|| format!("failed to read OAuth tokens from keyring: {error}"))
        }
    }
}

fn load_oauth_tokens_from_keyring<K: KeyringStore + Clone + 'static>(
    keyring_store: &K,
    keyring_backend_kind: AuthKeyringBackendKind,
    server_name: &str,
    url: &str,
) -> Result<Option<StoredOAuthTokens>> {
    match keyring_backend_kind {
        AuthKeyringBackendKind::Direct => {
            load_oauth_tokens_from_direct_keyring(keyring_store, server_name, url)
        }
        AuthKeyringBackendKind::Secrets => {
            load_oauth_tokens_from_secrets_keyring(keyring_store, server_name, url)
        }
    }
}

fn load_oauth_tokens_from_direct_keyring<K: KeyringStore>(
    keyring_store: &K,
    server_name: &str,
    url: &str,
) -> Result<Option<StoredOAuthTokens>> {
    let key = compute_store_key(server_name, url)?;
    match keyring_store.load(KEYRING_SERVICE, &key) {
        Ok(Some(serialized)) => {
            let mut tokens: StoredOAuthTokens = serde_json::from_str(&serialized)
                .context("failed to deserialize OAuth tokens from keyring")?;
            refresh_expires_in_from_timestamp(&mut tokens);
            Ok(Some(tokens))
        }
        Ok(None) => Ok(None),
        Err(error) => Err(Error::new(error.into_error())),
    }
}

fn load_oauth_tokens_from_secrets_keyring<K: KeyringStore + Clone + 'static>(
    keyring_store: &K,
    server_name: &str,
    url: &str,
) -> Result<Option<StoredOAuthTokens>> {
    let _store_lock = OAuthStoreLock::acquire(OAuthStore::Secrets)?;
    let codex_home = find_codex_home()?;
    let manager = SecretsManager::new_with_keyring_store_and_namespace(
        codex_home.to_path_buf(),
        SecretsBackendKind::Local,
        Arc::new(keyring_store.clone()),
        LocalSecretsNamespace::McpOAuth,
    );
    let secret_name = compute_secret_name(server_name, url)?;
    match manager
        .get(&SecretScope::Global, &secret_name)
        .context("failed to load MCP OAuth tokens from encrypted storage")?
    {
        Some(serialized) => {
            let mut tokens: StoredOAuthTokens = serde_json::from_str(&serialized)
                .context("failed to deserialize OAuth tokens from encrypted storage")?;
            refresh_expires_in_from_timestamp(&mut tokens);
            Ok(Some(tokens))
        }
        None => Ok(None),
    }
}

pub fn save_oauth_tokens(
    server_name: &str,
    tokens: &StoredOAuthTokens,
    store_mode: OAuthCredentialsStoreMode,
    keyring_backend_kind: AuthKeyringBackendKind,
) -> Result<()> {
    let keyring_store = DefaultKeyringStore;
    save_oauth_tokens_with_keyring_store(
        &keyring_store,
        server_name,
        tokens,
        store_mode,
        keyring_backend_kind,
    )
}

pub(crate) async fn save_oauth_tokens_locked(
    server_name: &str,
    tokens: &StoredOAuthTokens,
    store_mode: OAuthCredentialsStoreMode,
    keyring_backend_kind: AuthKeyringBackendKind,
) -> Result<()> {
    let keyring_store = DefaultKeyringStore;
    save_oauth_tokens_locked_with_keyring_store(
        &keyring_store,
        server_name,
        tokens,
        store_mode,
        keyring_backend_kind,
    )
    .await
}

async fn save_oauth_tokens_locked_with_keyring_store<K: KeyringStore + Clone + 'static>(
    keyring_store: &K,
    server_name: &str,
    tokens: &StoredOAuthTokens,
    store_mode: OAuthCredentialsStoreMode,
    keyring_backend_kind: AuthKeyringBackendKind,
) -> Result<()> {
    let _lock = RefreshCredentialLock::acquire_for_server(server_name, &tokens.url).await?;
    save_oauth_tokens_with_keyring_store(
        keyring_store,
        server_name,
        tokens,
        store_mode,
        keyring_backend_kind,
    )
}

fn save_oauth_tokens_with_keyring_store<K: KeyringStore + Clone + 'static>(
    keyring_store: &K,
    server_name: &str,
    tokens: &StoredOAuthTokens,
    store_mode: OAuthCredentialsStoreMode,
    keyring_backend_kind: AuthKeyringBackendKind,
) -> Result<()> {
    match store_mode {
        OAuthCredentialsStoreMode::Auto => save_oauth_tokens_with_keyring_with_fallback_to_file(
            keyring_store,
            keyring_backend_kind,
            server_name,
            tokens,
        ),
        OAuthCredentialsStoreMode::File => save_oauth_tokens_to_file(tokens),
        OAuthCredentialsStoreMode::Keyring => {
            save_oauth_tokens_with_keyring(keyring_store, keyring_backend_kind, server_name, tokens)
        }
    }
}

fn save_oauth_tokens_with_keyring<K: KeyringStore + Clone + 'static>(
    keyring_store: &K,
    keyring_backend_kind: AuthKeyringBackendKind,
    server_name: &str,
    tokens: &StoredOAuthTokens,
) -> Result<()> {
    match keyring_backend_kind {
        AuthKeyringBackendKind::Direct => {
            save_oauth_tokens_to_direct_keyring(keyring_store, server_name, tokens)
        }
        AuthKeyringBackendKind::Secrets => {
            save_oauth_tokens_to_secrets_keyring(keyring_store, server_name, tokens)
        }
    }
}

fn save_oauth_tokens_to_direct_keyring<K: KeyringStore>(
    keyring_store: &K,
    server_name: &str,
    tokens: &StoredOAuthTokens,
) -> Result<()> {
    let serialized = serde_json::to_string(tokens).context("failed to serialize OAuth tokens")?;

    let key = compute_store_key(server_name, &tokens.url)?;
    match keyring_store.save(KEYRING_SERVICE, &key, &serialized) {
        Ok(()) => {
            if let Err(error) = delete_oauth_tokens_from_file(&key) {
                warn!("failed to remove OAuth tokens from fallback storage: {error:?}");
            }
            Ok(())
        }
        Err(error) => {
            let message = format!(
                "failed to write OAuth tokens to keyring: {}",
                error.message()
            );
            warn!("{message}");
            Err(Error::new(error.into_error()).context(message))
        }
    }
}

fn save_oauth_tokens_to_secrets_keyring<K: KeyringStore + Clone + 'static>(
    keyring_store: &K,
    server_name: &str,
    tokens: &StoredOAuthTokens,
) -> Result<()> {
    let serialized = serde_json::to_string(tokens).context("failed to serialize OAuth tokens")?;
    {
        let _store_lock = OAuthStoreLock::acquire(OAuthStore::Secrets)?;
        save_oauth_tokens_to_secrets_keyring_unlocked(
            keyring_store,
            server_name,
            tokens,
            &serialized,
        )?;
    }

    let key = compute_store_key(server_name, &tokens.url)?;
    if let Err(error) = delete_oauth_tokens_from_file(&key) {
        warn!("failed to remove OAuth tokens from fallback storage: {error:?}");
    }
    Ok(())
}

fn save_oauth_tokens_to_secrets_keyring_unlocked<K: KeyringStore + Clone + 'static>(
    keyring_store: &K,
    server_name: &str,
    tokens: &StoredOAuthTokens,
    serialized: &str,
) -> Result<()> {
    let codex_home = find_codex_home()?;
    let manager = SecretsManager::new_with_keyring_store_and_namespace(
        codex_home.to_path_buf(),
        SecretsBackendKind::Local,
        Arc::new(keyring_store.clone()),
        LocalSecretsNamespace::McpOAuth,
    );
    let secret_name = compute_secret_name(server_name, &tokens.url)?;
    manager
        .set(&SecretScope::Global, &secret_name, serialized)
        .context("failed to write OAuth tokens to encrypted storage")
}

fn save_oauth_tokens_with_keyring_with_fallback_to_file<K: KeyringStore + Clone + 'static>(
    keyring_store: &K,
    keyring_backend_kind: AuthKeyringBackendKind,
    server_name: &str,
    tokens: &StoredOAuthTokens,
) -> Result<()> {
    match save_oauth_tokens_with_keyring(keyring_store, keyring_backend_kind, server_name, tokens) {
        Ok(()) => Ok(()),
        Err(error) => {
            let message = error.to_string();
            warn!("falling back to file storage for OAuth tokens: {message}");
            save_oauth_tokens_to_file(tokens)
                .with_context(|| format!("failed to write OAuth tokens to keyring: {message}"))
        }
    }
}

pub fn delete_oauth_tokens(
    server_name: &str,
    url: &str,
    store_mode: OAuthCredentialsStoreMode,
    keyring_backend_kind: AuthKeyringBackendKind,
) -> Result<bool> {
    let keyring_store = DefaultKeyringStore;
    delete_oauth_tokens_from_keyring_and_file(
        &keyring_store,
        store_mode,
        keyring_backend_kind,
        server_name,
        url,
    )
}

pub async fn delete_oauth_tokens_locked(
    server_name: &str,
    url: &str,
    store_mode: OAuthCredentialsStoreMode,
    keyring_backend_kind: AuthKeyringBackendKind,
) -> Result<bool> {
    let keyring_store = DefaultKeyringStore;
    delete_oauth_tokens_locked_with_keyring_store(
        &keyring_store,
        store_mode,
        keyring_backend_kind,
        server_name,
        url,
    )
    .await
}

async fn delete_oauth_tokens_locked_with_keyring_store<K: KeyringStore + Clone + 'static>(
    keyring_store: &K,
    store_mode: OAuthCredentialsStoreMode,
    keyring_backend_kind: AuthKeyringBackendKind,
    server_name: &str,
    url: &str,
) -> Result<bool> {
    let _lock = RefreshCredentialLock::acquire_for_server(server_name, url).await?;
    delete_oauth_tokens_from_keyring_and_file(
        keyring_store,
        store_mode,
        keyring_backend_kind,
        server_name,
        url,
    )
}

fn delete_oauth_tokens_from_keyring_and_file<K: KeyringStore + Clone + 'static>(
    keyring_store: &K,
    store_mode: OAuthCredentialsStoreMode,
    keyring_backend_kind: AuthKeyringBackendKind,
    server_name: &str,
    url: &str,
) -> Result<bool> {
    let key = compute_store_key(server_name, url)?;
    let keyring_result =
        delete_oauth_tokens_from_keyring(keyring_store, keyring_backend_kind, server_name, url);
    let keyring_removed = match keyring_result {
        Ok(removed) => removed,
        Err(error) => {
            let message = error.to_string();
            warn!("failed to delete OAuth tokens from keyring: {message}");
            match store_mode {
                OAuthCredentialsStoreMode::Auto | OAuthCredentialsStoreMode::Keyring => {
                    return Err(error).context("failed to delete OAuth tokens from keyring");
                }
                OAuthCredentialsStoreMode::File => false,
            }
        }
    };

    let file_removed = delete_oauth_tokens_from_file(&key)?;
    Ok(keyring_removed || file_removed)
}

fn delete_oauth_tokens_from_keyring<K: KeyringStore + Clone + 'static>(
    keyring_store: &K,
    keyring_backend_kind: AuthKeyringBackendKind,
    server_name: &str,
    url: &str,
) -> Result<bool> {
    match keyring_backend_kind {
        AuthKeyringBackendKind::Direct => {
            delete_oauth_tokens_from_direct_keyring(keyring_store, server_name, url)
        }
        AuthKeyringBackendKind::Secrets => {
            let direct_removed =
                delete_oauth_tokens_from_direct_keyring(keyring_store, server_name, url)?;
            let secrets_removed =
                delete_oauth_tokens_from_secrets_keyring(keyring_store, server_name, url)?;
            Ok(direct_removed || secrets_removed)
        }
    }
}

fn delete_oauth_tokens_from_direct_keyring<K: KeyringStore>(
    keyring_store: &K,
    server_name: &str,
    url: &str,
) -> Result<bool> {
    let key = compute_store_key(server_name, url)?;
    keyring_store
        .delete(KEYRING_SERVICE, &key)
        .map_err(|error| Error::new(error.into_error()))
}

fn delete_oauth_tokens_from_secrets_keyring<K: KeyringStore + Clone + 'static>(
    keyring_store: &K,
    server_name: &str,
    url: &str,
) -> Result<bool> {
    let _store_lock = OAuthStoreLock::acquire(OAuthStore::Secrets)?;
    let codex_home = find_codex_home()?;
    let manager = SecretsManager::new_with_keyring_store_and_namespace(
        codex_home.to_path_buf(),
        SecretsBackendKind::Local,
        Arc::new(keyring_store.clone()),
        LocalSecretsNamespace::McpOAuth,
    );
    let secret_name = compute_secret_name(server_name, url)?;
    let secrets_removed = manager
        .delete(&SecretScope::Global, &secret_name)
        .context("failed to delete OAuth tokens from encrypted storage")?;
    Ok(secrets_removed)
}

#[derive(Clone)]
pub(crate) struct OAuthPersistor {
    inner: Arc<OAuthPersistorInner>,
}

struct OAuthPersistorInner {
    server_name: String,
    url: String,
    authorization_manager: Arc<Mutex<AuthorizationManager>>,
    store_mode: OAuthCredentialsStoreMode,
    keyring_backend_kind: AuthKeyringBackendKind,
    last_credentials: Mutex<Option<StoredOAuthTokens>>,
}

impl OAuthPersistor {
    pub(crate) fn new(
        server_name: String,
        url: String,
        authorization_manager: Arc<Mutex<AuthorizationManager>>,
        store_mode: OAuthCredentialsStoreMode,
        keyring_backend_kind: AuthKeyringBackendKind,
        initial_credentials: Option<StoredOAuthTokens>,
    ) -> Self {
        Self {
            inner: Arc::new(OAuthPersistorInner {
                server_name,
                url,
                authorization_manager,
                store_mode,
                keyring_backend_kind,
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
    async fn persist_if_needed_with_keyring_store<K: KeyringStore + Clone + 'static>(
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
                    save_oauth_tokens_with_keyring_store(
                        keyring_store,
                        &self.inner.server_name,
                        &stored,
                        self.inner.store_mode,
                        self.inner.keyring_backend_kind,
                    )?;
                    *last_credentials = Some(stored);
                }
            }
            None => {
                let mut last_serialized = self.inner.last_credentials.lock().await;
                if last_serialized.take().is_some()
                    && let Err(error) = delete_oauth_tokens(
                        &self.inner.server_name,
                        &self.inner.url,
                        self.inner.store_mode,
                        self.inner.keyring_backend_kind,
                    )
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

    async fn refresh_if_needed_with_keyring_store<K: KeyringStore + Clone + 'static>(
        &self,
        keyring_store: &K,
    ) -> Result<()> {
        self.refresh_if_needed_with_keyring_store_and_timeout(
            keyring_store,
            REFRESH_REQUEST_TIMEOUT,
        )
        .await
    }

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "AuthorizationManager async access must be serialized through its mutex"
    )]
    async fn refresh_if_needed_with_keyring_store_and_timeout<K: KeyringStore + Clone + 'static>(
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

        let _lock =
            RefreshCredentialLock::acquire_for_server(&self.inner.server_name, &self.inner.url)
                .await?;
        let latest = load_oauth_tokens_with_keyring_store(
            keyring_store,
            &self.inner.server_name,
            &self.inner.url,
            self.inner.store_mode,
            self.inner.keyring_backend_kind,
        )?;

        let Some(latest) = latest else {
            self.clear_manager_credentials().await;
            let mut last_credentials = self.inner.last_credentials.lock().await;
            *last_credentials = None;
            anyhow::bail!(
                "OAuth tokens for server {} were removed before refresh; authorization required",
                self.inner.server_name
            );
        };

        if !token_needs_refresh(latest.expires_at) {
            self.adopt_credentials(latest).await?;
            return Ok(());
        }

        self.adopt_credentials(latest).await?;

        {
            let manager = self.inner.authorization_manager.clone();
            let guard = manager.lock().await;
            match timeout(refresh_request_timeout, guard.refresh_token()).await {
                Ok(result) => {
                    result.with_context(|| {
                        format!(
                            "failed to refresh OAuth tokens for server {}",
                            self.inner.server_name
                        )
                    })?;
                }
                Err(_) => anyhow::bail!(
                    "timed out after {refresh_request_timeout:?} refreshing OAuth tokens for server {}",
                    self.inner.server_name
                ),
            }
        }

        self.persist_if_needed_with_keyring_store(keyring_store)
            .await
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

struct RefreshCredentialLock {
    _file: File,
}

impl RefreshCredentialLock {
    async fn acquire_for_server(server_name: &str, url: &str) -> Result<Self> {
        let key = compute_store_key(server_name, url)?;
        Self::acquire(&key)
            .await
            .with_context(|| format!("failed to acquire OAuth credential lock for {server_name}"))
    }

    async fn acquire(store_key: &str) -> Result<Self> {
        Self::acquire_with_timeout(store_key, LOCK_ACQUIRE_TIMEOUT).await
    }

    async fn acquire_with_timeout(store_key: &str, acquire_timeout: Duration) -> Result<Self> {
        let path = refresh_lock_path(store_key)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .with_context(|| format!("failed to open OAuth refresh lock {}", path.display()))?;

        match timeout(acquire_timeout, async {
            loop {
                match file.try_lock() {
                    Ok(()) => return Ok(()),
                    Err(std::fs::TryLockError::WouldBlock) => {
                        sleep(LOCK_RETRY_SLEEP).await;
                    }
                    Err(error) => return Err(std::io::Error::from(error)),
                }
            }
        })
        .await
        {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                return Err(error).with_context(|| {
                    format!("failed to lock OAuth refresh lock {}", path.display())
                });
            }
            Err(_) => anyhow::bail!(
                "timed out after {acquire_timeout:?} waiting for OAuth refresh lock {}",
                path.display()
            ),
        }

        Ok(Self { _file: file })
    }
}

#[derive(Clone, Copy)]
enum OAuthStore {
    File,
    Secrets,
}

impl OAuthStore {
    fn lock_filename(self) -> &'static str {
        match self {
            Self::File => "file-store.lock",
            Self::Secrets => "secrets-store.lock",
        }
    }

    fn description(self) -> &'static str {
        match self {
            Self::File => "fallback file",
            Self::Secrets => "encrypted secrets",
        }
    }
}

struct OAuthStoreLock {
    _file: File,
}

impl OAuthStoreLock {
    fn acquire(store: OAuthStore) -> Result<Self> {
        Self::acquire_with_timeout(store, LOCK_ACQUIRE_TIMEOUT)
    }

    fn acquire_with_timeout(store: OAuthStore, acquire_timeout: Duration) -> Result<Self> {
        let path = oauth_store_lock_path(store)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .with_context(|| {
                format!(
                    "failed to open MCP OAuth {} store lock {}",
                    store.description(),
                    path.display()
                )
            })?;
        let started = Instant::now();

        loop {
            match file.try_lock() {
                Ok(()) => return Ok(Self { _file: file }),
                Err(std::fs::TryLockError::WouldBlock) if started.elapsed() >= acquire_timeout => {
                    anyhow::bail!(
                        "timed out after {acquire_timeout:?} waiting for MCP OAuth {} store lock {}",
                        store.description(),
                        path.display()
                    );
                }
                Err(std::fs::TryLockError::WouldBlock) => {
                    std::thread::sleep(LOCK_RETRY_SLEEP.min(acquire_timeout));
                }
                Err(error) => {
                    return Err(std::io::Error::from(error)).with_context(|| {
                        format!(
                            "failed to lock MCP OAuth {} store lock {}",
                            store.description(),
                            path.display()
                        )
                    });
                }
            }
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
    let store = InMemoryCredentialStore::new();
    store
        .save(stored_credentials_from_tokens(tokens))
        .await
        .context("failed to stage OAuth tokens for authorization manager")?;

    let manager = authorization_manager.clone();
    let mut guard = manager.lock().await;
    guard.set_credential_store(store);
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

const FALLBACK_FILENAME: &str = ".credentials.json";
const MCP_SERVER_TYPE: &str = "http";

type FallbackFile = BTreeMap<String, FallbackTokenEntry>;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FallbackTokenEntry {
    server_name: String,
    server_url: String,
    client_id: String,
    access_token: String,
    #[serde(default)]
    expires_at: Option<u64>,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    scopes: Vec<String>,
}

fn load_oauth_tokens_from_file(server_name: &str, url: &str) -> Result<Option<StoredOAuthTokens>> {
    let _store_lock = OAuthStoreLock::acquire(OAuthStore::File)?;
    let Some(store) = read_fallback_file_unlocked()? else {
        return Ok(None);
    };

    let key = compute_store_key(server_name, url)?;

    for entry in store.values() {
        let entry_key = compute_store_key(&entry.server_name, &entry.server_url)?;
        if entry_key != key {
            continue;
        }

        let mut token_response = OAuthTokenResponse::new(
            AccessToken::new(entry.access_token.clone()),
            BasicTokenType::Bearer,
            VendorExtraTokenFields::default(),
        );

        if let Some(refresh) = entry.refresh_token.clone() {
            token_response.set_refresh_token(Some(RefreshToken::new(refresh)));
        }

        let scopes = entry.scopes.clone();
        if !scopes.is_empty() {
            token_response.set_scopes(Some(scopes.into_iter().map(Scope::new).collect()));
        }

        let mut stored = StoredOAuthTokens {
            server_name: entry.server_name.clone(),
            url: entry.server_url.clone(),
            client_id: entry.client_id.clone(),
            token_response: WrappedOAuthTokenResponse(token_response),
            expires_at: entry.expires_at,
        };
        refresh_expires_in_from_timestamp(&mut stored);

        return Ok(Some(stored));
    }

    Ok(None)
}

fn save_oauth_tokens_to_file(tokens: &StoredOAuthTokens) -> Result<()> {
    let _store_lock = OAuthStoreLock::acquire(OAuthStore::File)?;
    save_oauth_tokens_to_file_unlocked(tokens)
}

fn save_oauth_tokens_to_file_unlocked(tokens: &StoredOAuthTokens) -> Result<()> {
    let key = compute_store_key(&tokens.server_name, &tokens.url)?;
    let mut store = read_fallback_file_unlocked()?.unwrap_or_default();

    let token_response = &tokens.token_response.0;
    let expires_at = tokens
        .expires_at
        .or_else(|| compute_expires_at_millis(token_response));
    let refresh_token = token_response
        .refresh_token()
        .map(|token| token.secret().to_string());
    let scopes = token_response
        .scopes()
        .map(|s| s.iter().map(|s| s.to_string()).collect())
        .unwrap_or_default();
    let entry = FallbackTokenEntry {
        server_name: tokens.server_name.clone(),
        server_url: tokens.url.clone(),
        client_id: tokens.client_id.clone(),
        access_token: token_response.access_token().secret().to_string(),
        expires_at,
        refresh_token,
        scopes,
    };

    store.insert(key, entry);
    write_fallback_file(&store)
}

fn delete_oauth_tokens_from_file(key: &str) -> Result<bool> {
    let _store_lock = OAuthStoreLock::acquire(OAuthStore::File)?;
    let mut store = match read_fallback_file_unlocked()? {
        Some(store) => store,
        None => return Ok(false),
    };

    let removed = store.remove(key).is_some();

    if removed {
        write_fallback_file(&store)?;
    }

    Ok(removed)
}

pub(crate) fn compute_expires_at_millis(response: &OAuthTokenResponse) -> Option<u64> {
    let expires_in = response.expires_in()?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0));
    let expiry = now.checked_add(expires_in)?;
    let millis = expiry.as_millis();
    if millis > u128::from(u64::MAX) {
        Some(u64::MAX)
    } else {
        Some(millis as u64)
    }
}

fn expires_in_from_timestamp(expires_at: u64) -> Option<u64> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0));
    let now_ms = now.as_millis() as u64;

    if expires_at <= now_ms {
        None
    } else {
        Some((expires_at - now_ms) / 1000)
    }
}

fn token_needs_refresh(expires_at: Option<u64>) -> bool {
    let Some(expires_at) = expires_at else {
        return false;
    };

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_millis() as u64;

    now.saturating_add(REFRESH_SKEW_MILLIS) >= expires_at
}

fn compute_store_key(server_name: &str, server_url: &str) -> Result<String> {
    let mut payload = JsonMap::new();
    payload.insert(
        "type".to_string(),
        Value::String(MCP_SERVER_TYPE.to_string()),
    );
    payload.insert("url".to_string(), Value::String(server_url.to_string()));
    payload.insert("headers".to_string(), Value::Object(JsonMap::new()));

    let truncated = sha_256_prefix(&Value::Object(payload))?;
    Ok(format!("{server_name}|{truncated}"))
}

/// Derive a valid secret-store name from the MCP OAuth store key.
///
/// `compute_store_key` intentionally includes readable identity components and
/// a pipe separator, but `SecretName` only allows `A-Z`, `0-9`, and `_`.
/// Re-hashing keeps the secret key deterministic while satisfying that
/// restricted alphabet.
fn compute_secret_name(server_name: &str, server_url: &str) -> Result<SecretName> {
    let key = compute_store_key(server_name, server_url)?;
    let mut hasher = Sha256::new();
    hasher.update(key.as_bytes());
    let digest = hasher.finalize();
    let hex = format!("{digest:X}");
    SecretName::new(&format!("{MCP_OAUTH_SECRET_PREFIX}_{}", &hex[..32]))
}

fn fallback_file_path() -> Result<PathBuf> {
    Ok(find_codex_home()?.join(FALLBACK_FILENAME).to_path_buf())
}

fn refresh_lock_path(store_key: &str) -> Result<PathBuf> {
    let mut hasher = Sha256::new();
    hasher.update(store_key.as_bytes());
    let digest = hasher.finalize();
    Ok(find_codex_home()?
        .join(REFRESH_LOCK_DIR)
        .join(format!("{digest:x}.lock"))
        .to_path_buf())
}

fn oauth_store_lock_path(store: OAuthStore) -> Result<PathBuf> {
    Ok(find_codex_home()?
        .join(REFRESH_LOCK_DIR)
        .join(store.lock_filename())
        .to_path_buf())
}

fn read_fallback_file_unlocked() -> Result<Option<FallbackFile>> {
    let path = fallback_file_path()?;
    let contents = match fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(None),
        Err(err) => {
            return Err(err).context(format!(
                "failed to read credentials file at {}",
                path.display()
            ));
        }
    };

    match serde_json::from_str::<FallbackFile>(&contents) {
        Ok(store) => Ok(Some(store)),
        Err(e) => Err(e).context(format!(
            "failed to parse credentials file at {}",
            path.display()
        )),
    }
}

fn write_fallback_file(store: &FallbackFile) -> Result<()> {
    let path = fallback_file_path()?;

    if store.is_empty() {
        if path.exists() {
            fs::remove_file(path)?;
        }
        return Ok(());
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let serialized = serde_json::to_string(store)?;
    fs::write(&path, serialized)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = fs::Permissions::from_mode(0o600);
        fs::set_permissions(&path, perms)?;
    }

    Ok(())
}

fn sha_256_prefix(value: &Value) -> Result<String> {
    let serialized =
        serde_json::to_string(&value).context("failed to serialize MCP OAuth key payload")?;
    let mut hasher = Sha256::new();
    hasher.update(serialized.as_bytes());
    let digest = hasher.finalize();
    let hex = format!("{digest:x}");
    let truncated = &hex[..16];
    Ok(truncated.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use codex_secrets::compute_keyring_account;
    use keyring::Error as KeyringError;
    use pretty_assertions::assert_eq;
    use rmcp::transport::auth::OAuthState;
    use serde_json::json;
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::sync::MutexGuard;
    use std::sync::OnceLock;
    use std::sync::PoisonError;
    use std::sync::mpsc;
    use tempfile::tempdir;
    use tokio::sync::Mutex as TokioMutex;
    use wiremock::Mock;
    use wiremock::MockServer;
    use wiremock::ResponseTemplate;
    use wiremock::matchers::body_string_contains;
    use wiremock::matchers::method;
    use wiremock::matchers::path;

    use codex_keyring_store::tests::MockKeyringStore;

    struct TempCodexHome {
        _guard: MutexGuard<'static, ()>,
        _dir: tempfile::TempDir,
    }

    impl TempCodexHome {
        fn new() -> Self {
            static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
            let guard = LOCK
                .get_or_init(Mutex::default)
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            let dir = tempdir().expect("create CODEX_HOME temp dir");
            unsafe {
                std::env::set_var("CODEX_HOME", dir.path());
            }
            Self {
                _guard: guard,
                _dir: dir,
            }
        }

        fn path(&self) -> &std::path::Path {
            self._dir.path()
        }
    }

    impl Drop for TempCodexHome {
        fn drop(&mut self) {
            unsafe {
                std::env::remove_var("CODEX_HOME");
            }
        }
    }

    #[test]
    fn load_oauth_tokens_reads_from_keyring_when_available() -> Result<()> {
        let _env = TempCodexHome::new();
        let store = MockKeyringStore::default();
        let tokens = sample_tokens();
        let expected = tokens.clone();
        let serialized = serde_json::to_string(&tokens)?;
        let key = super::compute_store_key(&tokens.server_name, &tokens.url)?;
        store.save(KEYRING_SERVICE, &key, &serialized)?;

        let loaded = super::load_oauth_tokens_from_keyring(
            &store,
            AuthKeyringBackendKind::Direct,
            &tokens.server_name,
            &tokens.url,
        )?
        .expect("tokens should load from keyring");
        assert_tokens_match_without_expiry(&loaded, &expected);
        Ok(())
    }

    #[test]
    fn load_oauth_tokens_falls_back_when_missing_in_keyring() -> Result<()> {
        let _env = TempCodexHome::new();
        let store = MockKeyringStore::default();
        let tokens = sample_tokens();
        let expected = tokens.clone();

        super::save_oauth_tokens_to_file(&tokens)?;

        let loaded = super::load_oauth_tokens_from_keyring_with_fallback_to_file(
            &store,
            AuthKeyringBackendKind::Direct,
            &tokens.server_name,
            &tokens.url,
        )?
        .expect("tokens should load from fallback");
        assert_tokens_match_without_expiry(&loaded, &expected);
        Ok(())
    }

    #[test]
    fn load_oauth_tokens_falls_back_when_keyring_errors() -> Result<()> {
        let _env = TempCodexHome::new();
        let store = MockKeyringStore::default();
        let tokens = sample_tokens();
        let expected = tokens.clone();
        let key = super::compute_store_key(&tokens.server_name, &tokens.url)?;
        store.set_error(&key, KeyringError::Invalid("error".into(), "load".into()));

        super::save_oauth_tokens_to_file(&tokens)?;

        let loaded = super::load_oauth_tokens_from_keyring_with_fallback_to_file(
            &store,
            AuthKeyringBackendKind::Direct,
            &tokens.server_name,
            &tokens.url,
        )?
        .expect("tokens should load from fallback");
        assert_tokens_match_without_expiry(&loaded, &expected);
        Ok(())
    }

    #[test]
    fn save_oauth_tokens_prefers_keyring_when_available() -> Result<()> {
        let _env = TempCodexHome::new();
        let store = MockKeyringStore::default();
        let tokens = sample_tokens();
        let key = super::compute_store_key(&tokens.server_name, &tokens.url)?;

        super::save_oauth_tokens_to_file(&tokens)?;

        super::save_oauth_tokens_with_keyring_with_fallback_to_file(
            &store,
            AuthKeyringBackendKind::Direct,
            &tokens.server_name,
            &tokens,
        )?;

        let fallback_path = super::fallback_file_path()?;
        assert!(!fallback_path.exists(), "fallback file should be removed");
        let stored = store.saved_value(&key).expect("value saved to keyring");
        assert_eq!(serde_json::from_str::<StoredOAuthTokens>(&stored)?, tokens);
        Ok(())
    }

    #[test]
    fn save_oauth_tokens_writes_fallback_when_keyring_fails() -> Result<()> {
        let _env = TempCodexHome::new();
        let store = MockKeyringStore::default();
        let tokens = sample_tokens();
        let key = super::compute_store_key(&tokens.server_name, &tokens.url)?;
        store.set_error(&key, KeyringError::Invalid("error".into(), "save".into()));

        super::save_oauth_tokens_with_keyring_with_fallback_to_file(
            &store,
            AuthKeyringBackendKind::Direct,
            &tokens.server_name,
            &tokens,
        )?;

        let fallback_path = super::fallback_file_path()?;
        assert!(fallback_path.exists(), "fallback file should be created");
        let saved = read_fallback_file()?.expect("fallback file should load");
        let key = super::compute_store_key(&tokens.server_name, &tokens.url)?;
        let entry = saved.get(&key).expect("entry for key");
        assert_eq!(entry.server_name, tokens.server_name);
        assert_eq!(entry.server_url, tokens.url);
        assert_eq!(entry.client_id, tokens.client_id);
        assert_eq!(
            entry.access_token,
            tokens.token_response.0.access_token().secret().as_str()
        );
        assert!(store.saved_value(&key).is_none());
        Ok(())
    }

    #[test]
    fn file_store_lock_preserves_updates_for_different_servers() -> Result<()> {
        let _env = TempCodexHome::new();
        let first = sample_tokens();
        let mut second = sample_tokens();
        second.server_name = "second-server".to_string();
        second.url = "https://second.example.test".to_string();

        let held_lock =
            OAuthStoreLock::acquire_with_timeout(OAuthStore::File, Duration::from_millis(100))?;
        let (started_tx, started_rx) = mpsc::channel();
        let (result_tx, result_rx) = mpsc::channel();
        let second_for_writer = second.clone();
        let writer = std::thread::spawn(move || {
            started_tx.send(()).expect("signal writer start");
            result_tx
                .send(super::save_oauth_tokens_to_file(&second_for_writer))
                .expect("send writer result");
        });

        started_rx.recv_timeout(Duration::from_secs(1))?;
        assert!(matches!(
            result_rx.recv_timeout(Duration::from_millis(100)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ));
        super::save_oauth_tokens_to_file_unlocked(&first)?;
        drop(held_lock);

        result_rx.recv_timeout(Duration::from_secs(10))??;
        writer.join().expect("file store writer should finish");
        let loaded_first = super::load_oauth_tokens_from_file(&first.server_name, &first.url)?
            .expect("first server tokens should remain stored");
        let loaded_second = super::load_oauth_tokens_from_file(&second.server_name, &second.url)?
            .expect("second server tokens should be stored");
        assert_tokens_match_without_expiry(&loaded_first, &first);
        assert_tokens_match_without_expiry(&loaded_second, &second);
        Ok(())
    }

    #[test]
    fn save_oauth_tokens_with_secrets_backend_writes_encrypted_storage() -> Result<()> {
        let env = TempCodexHome::new();
        let store = MockKeyringStore::default();
        let tokens = sample_tokens();
        let key = super::compute_store_key(&tokens.server_name, &tokens.url)?;
        let serialized = serde_json::to_string(&tokens)?;
        store.save(KEYRING_SERVICE, &key, &serialized)?;
        super::save_oauth_tokens_to_file(&tokens)?;

        super::save_oauth_tokens_with_keyring_with_fallback_to_file(
            &store,
            AuthKeyringBackendKind::Secrets,
            &tokens.server_name,
            &tokens,
        )?;

        let manager = SecretsManager::new_with_keyring_store_and_namespace(
            env.path().to_path_buf(),
            SecretsBackendKind::Local,
            Arc::new(store.clone()),
            LocalSecretsNamespace::McpOAuth,
        );
        let secret_name = super::compute_secret_name(&tokens.server_name, &tokens.url)?;
        let stored = manager
            .get(&SecretScope::Global, &secret_name)?
            .expect("tokens should be saved to encrypted storage");
        assert_eq!(serde_json::from_str::<StoredOAuthTokens>(&stored)?, tokens);
        assert_eq!(store.saved_value(&key), Some(serialized));
        assert!(env.path().join("secrets").join("mcp_oauth.age").exists());
        assert!(!env.path().join("secrets").join("local.age").exists());
        assert!(!super::fallback_file_path()?.exists());
        Ok(())
    }

    #[test]
    fn load_oauth_tokens_with_secrets_backend_reads_encrypted_storage() -> Result<()> {
        let _env = TempCodexHome::new();
        let store = MockKeyringStore::default();
        let tokens = sample_tokens();
        let expected = tokens.clone();

        super::save_oauth_tokens_with_keyring(
            &store,
            AuthKeyringBackendKind::Secrets,
            &tokens.server_name,
            &tokens,
        )?;

        let loaded = super::load_oauth_tokens_from_keyring(
            &store,
            AuthKeyringBackendKind::Secrets,
            &tokens.server_name,
            &tokens.url,
        )?
        .expect("tokens should load from encrypted storage");
        assert_tokens_match_without_expiry(&loaded, &expected);
        Ok(())
    }

    #[test]
    fn secrets_store_lock_preserves_updates_for_different_servers() -> Result<()> {
        let _env = TempCodexHome::new();
        let store = MockKeyringStore::default();
        let first = sample_tokens();
        let mut second = sample_tokens();
        second.server_name = "second-server".to_string();
        second.url = "https://second.example.test".to_string();

        let held_lock =
            OAuthStoreLock::acquire_with_timeout(OAuthStore::Secrets, Duration::from_millis(100))?;
        let (started_tx, started_rx) = mpsc::channel();
        let (result_tx, result_rx) = mpsc::channel();
        let store_for_writer = store.clone();
        let second_for_writer = second.clone();
        let writer = std::thread::spawn(move || {
            started_tx.send(()).expect("signal writer start");
            result_tx
                .send(super::save_oauth_tokens_with_keyring(
                    &store_for_writer,
                    AuthKeyringBackendKind::Secrets,
                    &second_for_writer.server_name,
                    &second_for_writer,
                ))
                .expect("send writer result");
        });

        started_rx.recv_timeout(Duration::from_secs(1))?;
        assert!(matches!(
            result_rx.recv_timeout(Duration::from_millis(100)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ));
        let first_serialized = serde_json::to_string(&first)?;
        super::save_oauth_tokens_to_secrets_keyring_unlocked(
            &store,
            &first.server_name,
            &first,
            &first_serialized,
        )?;
        drop(held_lock);

        result_rx.recv_timeout(Duration::from_secs(10))??;
        writer.join().expect("secrets store writer should finish");
        let loaded_first = super::load_oauth_tokens_from_keyring(
            &store,
            AuthKeyringBackendKind::Secrets,
            &first.server_name,
            &first.url,
        )?
        .expect("first server tokens should remain stored");
        let loaded_second = super::load_oauth_tokens_from_keyring(
            &store,
            AuthKeyringBackendKind::Secrets,
            &second.server_name,
            &second.url,
        )?
        .expect("second server tokens should be stored");
        assert_tokens_match_without_expiry(&loaded_first, &first);
        assert_tokens_match_without_expiry(&loaded_second, &second);
        Ok(())
    }

    #[test]
    fn load_oauth_tokens_with_secrets_backend_ignores_direct_entry() -> Result<()> {
        let _env = TempCodexHome::new();
        let store = MockKeyringStore::default();
        let tokens = sample_tokens();
        let key = super::compute_store_key(&tokens.server_name, &tokens.url)?;
        let serialized = serde_json::to_string(&tokens)?;
        store.save(KEYRING_SERVICE, &key, &serialized)?;

        let loaded = super::load_oauth_tokens_from_keyring(
            &store,
            AuthKeyringBackendKind::Secrets,
            &tokens.server_name,
            &tokens.url,
        )?;

        assert!(loaded.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn refresh_transaction_adopts_keyring_update_without_second_provider_refresh()
    -> Result<()> {
        let _env = TempCodexHome::new();
        let server = MockServer::start().await;
        mount_oauth_metadata(&server).await;
        mount_refresh_response(
            &server,
            "refresh-token",
            "refreshed-access-token",
            "rotated-refresh-token",
        )
        .await;

        let store = MockKeyringStore::default();
        let initial_tokens = expired_sample_tokens(&format!("{}/mcp", server.uri()));
        super::save_oauth_tokens_with_keyring_store(
            &store,
            &initial_tokens.server_name,
            &initial_tokens,
            OAuthCredentialsStoreMode::Keyring,
            AuthKeyringBackendKind::Direct,
        )?;

        let winner_manager = authorization_manager_for(&initial_tokens).await?;
        let loser_manager = authorization_manager_for(&initial_tokens).await?;
        let winner = OAuthPersistor::new(
            initial_tokens.server_name.clone(),
            initial_tokens.url.clone(),
            winner_manager,
            OAuthCredentialsStoreMode::Keyring,
            AuthKeyringBackendKind::Direct,
            Some(initial_tokens.clone()),
        );
        let loser = OAuthPersistor::new(
            initial_tokens.server_name.clone(),
            initial_tokens.url.clone(),
            loser_manager.clone(),
            OAuthCredentialsStoreMode::Keyring,
            AuthKeyringBackendKind::Direct,
            Some(initial_tokens.clone()),
        );

        winner.refresh_if_needed_with_keyring_store(&store).await?;
        loser.refresh_if_needed_with_keyring_store(&store).await?;

        server.verify().await;
        let stored = super::load_oauth_tokens_with_keyring_store(
            &store,
            &initial_tokens.server_name,
            &initial_tokens.url,
            OAuthCredentialsStoreMode::Keyring,
            AuthKeyringBackendKind::Direct,
        )?
        .expect("refreshed tokens should be persisted");
        assert_eq!(access_token(&stored), "refreshed-access-token");
        assert_eq!(
            refresh_token(&stored),
            Some("rotated-refresh-token".to_string())
        );

        let loser_tokens = tokens_from_manager(&loser_manager).await?;
        assert_eq!(access_token(&loser_tokens), "refreshed-access-token");
        assert_eq!(
            refresh_token(&loser_tokens),
            Some("rotated-refresh-token".to_string())
        );
        Ok(())
    }

    #[tokio::test]
    async fn refresh_transaction_adopts_valid_reread_without_provider_refresh() -> Result<()> {
        let _env = TempCodexHome::new();
        let server = MockServer::start().await;
        mount_oauth_metadata(&server).await;
        let store = MockKeyringStore::default();
        let mut initial_tokens = expired_sample_tokens(&format!("{}/mcp", server.uri()));
        initial_tokens
            .token_response
            .0
            .set_refresh_token(Some(RefreshToken::new("stale-refresh-token".to_string())));

        let mut latest_tokens = sample_tokens();
        latest_tokens.url.clone_from(&initial_tokens.url);
        latest_tokens
            .token_response
            .0
            .set_access_token(AccessToken::new(
                "already-refreshed-access-token".to_string(),
            ));
        latest_tokens
            .token_response
            .0
            .set_refresh_token(Some(RefreshToken::new(
                "already-rotated-refresh-token".to_string(),
            )));

        super::save_oauth_tokens_with_keyring_store(
            &store,
            &latest_tokens.server_name,
            &latest_tokens,
            OAuthCredentialsStoreMode::Keyring,
            AuthKeyringBackendKind::Direct,
        )?;

        let manager = authorization_manager_for(&initial_tokens).await?;
        let persistor = OAuthPersistor::new(
            initial_tokens.server_name.clone(),
            initial_tokens.url.clone(),
            manager.clone(),
            OAuthCredentialsStoreMode::Keyring,
            AuthKeyringBackendKind::Direct,
            Some(initial_tokens),
        );

        persistor
            .refresh_if_needed_with_keyring_store(&store)
            .await?;

        let manager_tokens = tokens_from_manager(&manager).await?;
        assert_eq!(
            access_token(&manager_tokens),
            "already-refreshed-access-token"
        );
        assert_eq!(
            refresh_token(&manager_tokens),
            Some("already-rotated-refresh-token".to_string())
        );
        Ok(())
    }

    #[tokio::test]
    async fn refresh_transaction_refreshes_when_only_derived_expires_in_drifted() -> Result<()> {
        let _env = TempCodexHome::new();
        let server = MockServer::start().await;
        mount_oauth_metadata(&server).await;
        mount_refresh_response(
            &server,
            "refresh-token",
            "refreshed-after-expiry-drift",
            "rotated-after-expiry-drift",
        )
        .await;

        let store = MockKeyringStore::default();
        let mut initial_tokens = sample_tokens();
        initial_tokens.url = format!("{}/mcp", server.uri());
        initial_tokens.expires_at = Some(now_millis().saturating_add(5_000));
        initial_tokens
            .token_response
            .0
            .set_expires_in(Some(&Duration::from_secs(3600)));
        super::save_oauth_tokens_with_keyring_store(
            &store,
            &initial_tokens.server_name,
            &initial_tokens,
            OAuthCredentialsStoreMode::Keyring,
            AuthKeyringBackendKind::Direct,
        )?;

        let manager = authorization_manager_for(&initial_tokens).await?;
        let persistor = OAuthPersistor::new(
            initial_tokens.server_name.clone(),
            initial_tokens.url.clone(),
            manager,
            OAuthCredentialsStoreMode::Keyring,
            AuthKeyringBackendKind::Direct,
            Some(initial_tokens.clone()),
        );

        persistor
            .refresh_if_needed_with_keyring_store(&store)
            .await?;

        server.verify().await;
        let stored = super::load_oauth_tokens_with_keyring_store(
            &store,
            &initial_tokens.server_name,
            &initial_tokens.url,
            OAuthCredentialsStoreMode::Keyring,
            AuthKeyringBackendKind::Direct,
        )?
        .expect("refreshed tokens should be persisted");
        assert_eq!(access_token(&stored), "refreshed-after-expiry-drift");
        assert_eq!(
            refresh_token(&stored),
            Some("rotated-after-expiry-drift".to_string())
        );
        Ok(())
    }

    #[tokio::test]
    async fn refresh_transaction_uses_latest_refresh_token_when_reread_is_expired() -> Result<()> {
        let _env = TempCodexHome::new();
        let server = MockServer::start().await;
        mount_oauth_metadata(&server).await;
        mount_refresh_response(
            &server,
            "latest-refresh-token",
            "refreshed-from-latest-token",
            "rotated-from-latest-token",
        )
        .await;

        let store = MockKeyringStore::default();
        let mut initial_tokens = expired_sample_tokens(&format!("{}/mcp", server.uri()));
        initial_tokens
            .token_response
            .0
            .set_refresh_token(Some(RefreshToken::new("stale-refresh-token".to_string())));

        let mut latest_tokens = initial_tokens.clone();
        latest_tokens
            .token_response
            .0
            .set_access_token(AccessToken::new("latest-expired-access-token".to_string()));
        latest_tokens
            .token_response
            .0
            .set_refresh_token(Some(RefreshToken::new("latest-refresh-token".to_string())));
        super::save_oauth_tokens_with_keyring_store(
            &store,
            &latest_tokens.server_name,
            &latest_tokens,
            OAuthCredentialsStoreMode::Keyring,
            AuthKeyringBackendKind::Direct,
        )?;

        let manager = authorization_manager_for(&initial_tokens).await?;
        let persistor = OAuthPersistor::new(
            initial_tokens.server_name.clone(),
            initial_tokens.url.clone(),
            manager,
            OAuthCredentialsStoreMode::Keyring,
            AuthKeyringBackendKind::Direct,
            Some(initial_tokens.clone()),
        );

        persistor
            .refresh_if_needed_with_keyring_store(&store)
            .await?;

        server.verify().await;
        let stored = super::load_oauth_tokens_with_keyring_store(
            &store,
            &initial_tokens.server_name,
            &initial_tokens.url,
            OAuthCredentialsStoreMode::Keyring,
            AuthKeyringBackendKind::Direct,
        )?
        .expect("refreshed tokens should be persisted");
        assert_eq!(access_token(&stored), "refreshed-from-latest-token");
        assert_eq!(
            refresh_token(&stored),
            Some("rotated-from-latest-token".to_string())
        );
        Ok(())
    }

    #[tokio::test]
    async fn refresh_lock_acquisition_times_out_without_stealing() -> Result<()> {
        let _env = TempCodexHome::new();
        let store_key = "test-store-key";
        let held_lock =
            RefreshCredentialLock::acquire_with_timeout(store_key, Duration::from_millis(100))
                .await?;

        let error =
            match RefreshCredentialLock::acquire_with_timeout(store_key, Duration::from_millis(50))
                .await
            {
                Ok(_) => panic!("contending lock acquisition should time out"),
                Err(error) => error,
            };
        assert!(
            error
                .to_string()
                .contains("timed out after 50ms waiting for OAuth refresh lock"),
            "unexpected error: {error:#}"
        );

        drop(held_lock);
        let _reacquired =
            RefreshCredentialLock::acquire_with_timeout(store_key, Duration::from_millis(100))
                .await?;
        Ok(())
    }

    #[tokio::test]
    async fn provider_refresh_timeout_releases_lock_without_persisting() -> Result<()> {
        let _env = TempCodexHome::new();
        let server = MockServer::start().await;
        mount_oauth_metadata(&server).await;
        let refresh_started = mount_refresh_response_with_signal(
            &server,
            "refresh-token",
            "late-access-token",
            "late-refresh-token",
            Duration::from_secs(1),
        )
        .await;

        let store = MockKeyringStore::default();
        let initial_tokens = expired_sample_tokens(&format!("{}/mcp", server.uri()));
        super::save_oauth_tokens_with_keyring_store(
            &store,
            &initial_tokens.server_name,
            &initial_tokens,
            OAuthCredentialsStoreMode::Keyring,
            AuthKeyringBackendKind::Direct,
        )?;

        let manager = authorization_manager_for(&initial_tokens).await?;
        let persistor = OAuthPersistor::new(
            initial_tokens.server_name.clone(),
            initial_tokens.url.clone(),
            manager,
            OAuthCredentialsStoreMode::Keyring,
            AuthKeyringBackendKind::Direct,
            Some(initial_tokens.clone()),
        );
        let refresh_task = tokio::spawn({
            let persistor = persistor.clone();
            let store = store.clone();
            async move {
                persistor
                    .refresh_if_needed_with_keyring_store_and_timeout(
                        &store,
                        Duration::from_millis(200),
                    )
                    .await
            }
        });

        wait_for_signal(refresh_started).await?;
        let error = refresh_task
            .await?
            .expect_err("delayed provider response should time out");
        assert_eq!(
            error.to_string(),
            "timed out after 200ms refreshing OAuth tokens for server test-server"
        );

        let store_key = super::compute_store_key(&initial_tokens.server_name, &initial_tokens.url)?;
        let _lock =
            RefreshCredentialLock::acquire_with_timeout(&store_key, Duration::from_millis(100))
                .await?;
        let stored = super::load_oauth_tokens_with_keyring_store(
            &store,
            &initial_tokens.server_name,
            &initial_tokens.url,
            OAuthCredentialsStoreMode::Keyring,
            AuthKeyringBackendKind::Direct,
        )?
        .expect("original tokens should remain persisted");
        assert_eq!(access_token(&stored), "access-token");
        assert_eq!(refresh_token(&stored), Some("refresh-token".to_string()));
        server.verify().await;
        Ok(())
    }

    #[tokio::test]
    async fn locked_login_save_before_refresh_prevents_overwrite() -> Result<()> {
        let _env = TempCodexHome::new();
        let server = MockServer::start().await;
        mount_oauth_metadata(&server).await;

        let store = MockKeyringStore::default();
        let initial_tokens = expired_sample_tokens(&format!("{}/mcp", server.uri()));
        super::save_oauth_tokens_with_keyring_store(
            &store,
            &initial_tokens.server_name,
            &initial_tokens,
            OAuthCredentialsStoreMode::Keyring,
            AuthKeyringBackendKind::Direct,
        )?;

        let mut login_tokens = sample_tokens();
        login_tokens.url.clone_from(&initial_tokens.url);
        login_tokens
            .token_response
            .0
            .set_access_token(AccessToken::new("login-access-token".to_string()));
        login_tokens
            .token_response
            .0
            .set_refresh_token(Some(RefreshToken::new("login-refresh-token".to_string())));
        super::save_oauth_tokens_locked_with_keyring_store(
            &store,
            &login_tokens.server_name,
            &login_tokens,
            OAuthCredentialsStoreMode::Keyring,
            AuthKeyringBackendKind::Direct,
        )
        .await?;

        let manager = authorization_manager_for(&initial_tokens).await?;
        let persistor = OAuthPersistor::new(
            initial_tokens.server_name.clone(),
            initial_tokens.url.clone(),
            manager,
            OAuthCredentialsStoreMode::Keyring,
            AuthKeyringBackendKind::Direct,
            Some(initial_tokens.clone()),
        );

        persistor
            .refresh_if_needed_with_keyring_store(&store)
            .await?;

        let stored = super::load_oauth_tokens_with_keyring_store(
            &store,
            &initial_tokens.server_name,
            &initial_tokens.url,
            OAuthCredentialsStoreMode::Keyring,
            AuthKeyringBackendKind::Direct,
        )?
        .expect("login tokens should remain persisted");
        assert_eq!(access_token(&stored), "login-access-token");
        assert_eq!(
            refresh_token(&stored),
            Some("login-refresh-token".to_string())
        );
        Ok(())
    }

    #[tokio::test]
    async fn locked_login_save_after_refresh_still_wins() -> Result<()> {
        let _env = TempCodexHome::new();
        let server = MockServer::start().await;
        mount_oauth_metadata(&server).await;
        let refresh_started = mount_refresh_response_with_signal(
            &server,
            "refresh-token",
            "refreshed-before-login",
            "rotated-before-login",
            Duration::from_millis(200),
        )
        .await;

        let store = MockKeyringStore::default();
        let initial_tokens = expired_sample_tokens(&format!("{}/mcp", server.uri()));
        super::save_oauth_tokens_with_keyring_store(
            &store,
            &initial_tokens.server_name,
            &initial_tokens,
            OAuthCredentialsStoreMode::Keyring,
            AuthKeyringBackendKind::Direct,
        )?;

        let manager = authorization_manager_for(&initial_tokens).await?;
        let persistor = OAuthPersistor::new(
            initial_tokens.server_name.clone(),
            initial_tokens.url.clone(),
            manager,
            OAuthCredentialsStoreMode::Keyring,
            AuthKeyringBackendKind::Direct,
            Some(initial_tokens.clone()),
        );
        let refresh_task = tokio::spawn({
            let persistor = persistor.clone();
            let store = store.clone();
            async move { persistor.refresh_if_needed_with_keyring_store(&store).await }
        });

        wait_for_signal(refresh_started).await?;

        let mut login_tokens = sample_tokens();
        login_tokens.url.clone_from(&initial_tokens.url);
        login_tokens
            .token_response
            .0
            .set_access_token(AccessToken::new("login-after-refresh-access".to_string()));
        login_tokens
            .token_response
            .0
            .set_refresh_token(Some(RefreshToken::new(
                "login-after-refresh-token".to_string(),
            )));
        super::save_oauth_tokens_locked_with_keyring_store(
            &store,
            &login_tokens.server_name,
            &login_tokens,
            OAuthCredentialsStoreMode::Keyring,
            AuthKeyringBackendKind::Direct,
        )
        .await?;
        refresh_task.await??;

        server.verify().await;
        let stored = super::load_oauth_tokens_with_keyring_store(
            &store,
            &initial_tokens.server_name,
            &initial_tokens.url,
            OAuthCredentialsStoreMode::Keyring,
            AuthKeyringBackendKind::Direct,
        )?
        .expect("login tokens should remain persisted");
        assert_eq!(access_token(&stored), "login-after-refresh-access");
        assert_eq!(
            refresh_token(&stored),
            Some("login-after-refresh-token".to_string())
        );
        Ok(())
    }

    #[tokio::test]
    async fn locked_logout_before_refresh_prevents_resurrection() -> Result<()> {
        let _env = TempCodexHome::new();
        let server = MockServer::start().await;
        mount_oauth_metadata(&server).await;

        let store = MockKeyringStore::default();
        let initial_tokens = expired_sample_tokens(&format!("{}/mcp", server.uri()));
        super::save_oauth_tokens_with_keyring_store(
            &store,
            &initial_tokens.server_name,
            &initial_tokens,
            OAuthCredentialsStoreMode::Keyring,
            AuthKeyringBackendKind::Direct,
        )?;
        let removed = super::delete_oauth_tokens_locked_with_keyring_store(
            &store,
            OAuthCredentialsStoreMode::Keyring,
            AuthKeyringBackendKind::Direct,
            &initial_tokens.server_name,
            &initial_tokens.url,
        )
        .await?;
        assert!(removed);

        let manager = authorization_manager_for(&initial_tokens).await?;
        let persistor = OAuthPersistor::new(
            initial_tokens.server_name.clone(),
            initial_tokens.url.clone(),
            manager,
            OAuthCredentialsStoreMode::Keyring,
            AuthKeyringBackendKind::Direct,
            Some(initial_tokens.clone()),
        );

        let error = persistor
            .refresh_if_needed_with_keyring_store(&store)
            .await
            .expect_err("missing latest tokens should abort refresh");
        assert!(
            error.to_string().contains("authorization required"),
            "unexpected error: {error:#}"
        );
        let stored = super::load_oauth_tokens_with_keyring_store(
            &store,
            &initial_tokens.server_name,
            &initial_tokens.url,
            OAuthCredentialsStoreMode::Keyring,
            AuthKeyringBackendKind::Direct,
        )?;
        assert!(stored.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn locked_logout_after_refresh_still_deletes() -> Result<()> {
        let _env = TempCodexHome::new();
        let server = MockServer::start().await;
        mount_oauth_metadata(&server).await;
        let refresh_started = mount_refresh_response_with_signal(
            &server,
            "refresh-token",
            "refreshed-before-logout",
            "rotated-before-logout",
            Duration::from_millis(200),
        )
        .await;

        let store = MockKeyringStore::default();
        let initial_tokens = expired_sample_tokens(&format!("{}/mcp", server.uri()));
        super::save_oauth_tokens_with_keyring_store(
            &store,
            &initial_tokens.server_name,
            &initial_tokens,
            OAuthCredentialsStoreMode::Keyring,
            AuthKeyringBackendKind::Direct,
        )?;

        let manager = authorization_manager_for(&initial_tokens).await?;
        let persistor = OAuthPersistor::new(
            initial_tokens.server_name.clone(),
            initial_tokens.url.clone(),
            manager,
            OAuthCredentialsStoreMode::Keyring,
            AuthKeyringBackendKind::Direct,
            Some(initial_tokens.clone()),
        );
        let refresh_task = tokio::spawn({
            let persistor = persistor.clone();
            let store = store.clone();
            async move { persistor.refresh_if_needed_with_keyring_store(&store).await }
        });

        wait_for_signal(refresh_started).await?;

        let removed = super::delete_oauth_tokens_locked_with_keyring_store(
            &store,
            OAuthCredentialsStoreMode::Keyring,
            AuthKeyringBackendKind::Direct,
            &initial_tokens.server_name,
            &initial_tokens.url,
        )
        .await?;
        refresh_task.await??;

        server.verify().await;
        assert!(removed);
        let stored = super::load_oauth_tokens_with_keyring_store(
            &store,
            &initial_tokens.server_name,
            &initial_tokens.url,
            OAuthCredentialsStoreMode::Keyring,
            AuthKeyringBackendKind::Direct,
        )?;
        assert!(stored.is_none());
        Ok(())
    }

    #[test]
    fn save_oauth_tokens_with_secrets_backend_falls_back_to_file_when_keyring_fails() -> Result<()>
    {
        let env = TempCodexHome::new();
        let store = MockKeyringStore::default();
        store.set_error(
            &compute_keyring_account(env.path()),
            KeyringError::Invalid("error".into(), "save".into()),
        );
        let tokens = sample_tokens();

        super::save_oauth_tokens_with_keyring_with_fallback_to_file(
            &store,
            AuthKeyringBackendKind::Secrets,
            &tokens.server_name,
            &tokens,
        )?;

        let saved = read_fallback_file()?.expect("fallback file should load");
        let key = super::compute_store_key(&tokens.server_name, &tokens.url)?;
        assert!(saved.contains_key(&key));
        Ok(())
    }

    #[test]
    fn delete_oauth_tokens_with_secrets_backend_removes_secrets_and_file() -> Result<()> {
        let env = TempCodexHome::new();
        let store = MockKeyringStore::default();
        let tokens = sample_tokens();
        let serialized = serde_json::to_string(&tokens)?;
        let key = super::compute_store_key(&tokens.server_name, &tokens.url)?;
        store.save(KEYRING_SERVICE, &key, &serialized)?;
        super::save_oauth_tokens_with_keyring(
            &store,
            AuthKeyringBackendKind::Secrets,
            &tokens.server_name,
            &tokens,
        )?;
        store.save(KEYRING_SERVICE, &key, &serialized)?;
        super::save_oauth_tokens_to_file(&tokens)?;

        let removed = super::delete_oauth_tokens_from_keyring_and_file(
            &store,
            OAuthCredentialsStoreMode::Auto,
            AuthKeyringBackendKind::Secrets,
            &tokens.server_name,
            &tokens.url,
        )?;

        let manager = SecretsManager::new_with_keyring_store_and_namespace(
            env.path().to_path_buf(),
            SecretsBackendKind::Local,
            Arc::new(store.clone()),
            LocalSecretsNamespace::McpOAuth,
        );
        let secret_name = super::compute_secret_name(&tokens.server_name, &tokens.url)?;
        assert!(removed);
        assert!(manager.get(&SecretScope::Global, &secret_name)?.is_none());
        assert!(store.saved_value(&key).is_none());
        assert!(!super::fallback_file_path()?.exists());
        Ok(())
    }

    #[test]
    fn delete_oauth_tokens_removes_all_storage() -> Result<()> {
        let _env = TempCodexHome::new();
        let store = MockKeyringStore::default();
        let tokens = sample_tokens();
        let serialized = serde_json::to_string(&tokens)?;
        let key = super::compute_store_key(&tokens.server_name, &tokens.url)?;
        store.save(KEYRING_SERVICE, &key, &serialized)?;
        super::save_oauth_tokens_to_file(&tokens)?;

        let removed = super::delete_oauth_tokens_from_keyring_and_file(
            &store,
            OAuthCredentialsStoreMode::Auto,
            AuthKeyringBackendKind::Direct,
            &tokens.server_name,
            &tokens.url,
        )?;
        assert!(removed);
        assert!(!store.contains(&key));
        assert!(!super::fallback_file_path()?.exists());
        Ok(())
    }

    #[test]
    fn delete_oauth_tokens_file_mode_removes_keyring_only_entry() -> Result<()> {
        let _env = TempCodexHome::new();
        let store = MockKeyringStore::default();
        let tokens = sample_tokens();
        let serialized = serde_json::to_string(&tokens)?;
        let key = super::compute_store_key(&tokens.server_name, &tokens.url)?;
        store.save(KEYRING_SERVICE, &key, &serialized)?;
        assert!(store.contains(&key));

        let removed = super::delete_oauth_tokens_from_keyring_and_file(
            &store,
            OAuthCredentialsStoreMode::Auto,
            AuthKeyringBackendKind::Direct,
            &tokens.server_name,
            &tokens.url,
        )?;
        assert!(removed);
        assert!(!store.contains(&key));
        assert!(!super::fallback_file_path()?.exists());
        Ok(())
    }

    #[test]
    fn delete_oauth_tokens_propagates_keyring_errors() -> Result<()> {
        let _env = TempCodexHome::new();
        let store = MockKeyringStore::default();
        let tokens = sample_tokens();
        let key = super::compute_store_key(&tokens.server_name, &tokens.url)?;
        store.set_error(&key, KeyringError::Invalid("error".into(), "delete".into()));
        super::save_oauth_tokens_to_file(&tokens).unwrap();

        let result = super::delete_oauth_tokens_from_keyring_and_file(
            &store,
            OAuthCredentialsStoreMode::Auto,
            AuthKeyringBackendKind::Direct,
            &tokens.server_name,
            &tokens.url,
        );
        assert!(result.is_err());
        assert!(super::fallback_file_path().unwrap().exists());
        Ok(())
    }

    #[test]
    fn refresh_expires_in_from_timestamp_restores_future_durations() {
        let mut tokens = sample_tokens();
        let expires_at = tokens.expires_at.expect("expires_at should be set");

        tokens.token_response.0.set_expires_in(None);
        super::refresh_expires_in_from_timestamp(&mut tokens);

        let actual = tokens
            .token_response
            .0
            .expires_in()
            .expect("expires_in should be restored")
            .as_secs();
        let expected = super::expires_in_from_timestamp(expires_at)
            .expect("expires_at should still be in the future");
        let diff = actual.abs_diff(expected);
        assert!(diff <= 1, "expires_in drift too large: diff={diff}");
    }

    #[test]
    fn refresh_expires_in_from_timestamp_marks_expired_tokens() {
        let mut tokens = sample_tokens();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| Duration::from_secs(0));
        let expired_at = now.as_millis() as u64;
        tokens.expires_at = Some(expired_at.saturating_sub(1000));

        let duration = Duration::from_secs(600);
        tokens.token_response.0.set_expires_in(Some(&duration));

        super::refresh_expires_in_from_timestamp(&mut tokens);

        assert_eq!(tokens.token_response.0.expires_in(), Some(Duration::ZERO));
    }

    #[test]
    fn oauth_tokens_are_usable_when_expiry_is_unknown() {
        let mut tokens = sample_tokens();
        tokens.expires_at = None;
        tokens.token_response.0.set_refresh_token(None);

        assert!(super::oauth_tokens_are_usable(&tokens));
    }

    #[test]
    fn oauth_tokens_are_usable_when_unexpired_without_refresh_token() {
        let mut tokens = sample_tokens();
        tokens.token_response.0.set_refresh_token(None);

        assert!(super::oauth_tokens_are_usable(&tokens));
    }

    #[test]
    fn oauth_tokens_are_usable_when_expired_but_refreshable() {
        let mut tokens = sample_tokens();
        tokens.expires_at = Some(0);

        assert!(super::oauth_tokens_are_usable(&tokens));
    }

    #[test]
    fn oauth_tokens_are_not_usable_when_expired_and_unrefreshable() {
        let mut tokens = sample_tokens();
        tokens.expires_at = Some(0);
        tokens.token_response.0.set_refresh_token(None);

        assert!(!super::oauth_tokens_are_usable(&tokens));
    }

    #[test]
    fn oauth_tokens_are_not_usable_when_near_expiry_and_unrefreshable() {
        let mut tokens = sample_tokens();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| Duration::from_secs(0))
            .as_millis() as u64;
        tokens.expires_at = Some(now.saturating_add(REFRESH_SKEW_MILLIS - 1));
        tokens.token_response.0.set_refresh_token(None);

        assert!(!super::oauth_tokens_are_usable(&tokens));
    }

    #[test]
    fn oauth_tokens_are_not_usable_when_client_id_is_blank() {
        let mut tokens = sample_tokens();
        tokens.client_id = " ".to_string();

        assert!(!super::oauth_tokens_are_usable(&tokens));
    }

    #[test]
    fn oauth_tokens_are_not_usable_when_access_token_is_blank() {
        let mut tokens = sample_tokens();
        tokens
            .token_response
            .0
            .set_access_token(AccessToken::new(" ".to_string()));

        assert!(!super::oauth_tokens_are_usable(&tokens));
    }

    #[test]
    fn oauth_tokens_are_not_usable_when_required_refresh_token_is_blank() {
        let mut tokens = sample_tokens();
        tokens.expires_at = Some(0);
        tokens
            .token_response
            .0
            .set_refresh_token(Some(RefreshToken::new(" ".to_string())));

        assert!(!super::oauth_tokens_are_usable(&tokens));
    }

    fn assert_tokens_match_without_expiry(
        actual: &StoredOAuthTokens,
        expected: &StoredOAuthTokens,
    ) {
        assert_eq!(actual.server_name, expected.server_name);
        assert_eq!(actual.url, expected.url);
        assert_eq!(actual.client_id, expected.client_id);
        assert_eq!(actual.expires_at, expected.expires_at);
        assert_token_response_match_without_expiry(
            &actual.token_response,
            &expected.token_response,
        );
    }

    fn read_fallback_file() -> Result<Option<FallbackFile>> {
        let _store_lock = OAuthStoreLock::acquire(OAuthStore::File)?;
        super::read_fallback_file_unlocked()
    }

    fn assert_token_response_match_without_expiry(
        actual: &WrappedOAuthTokenResponse,
        expected: &WrappedOAuthTokenResponse,
    ) {
        let actual_response = &actual.0;
        let expected_response = &expected.0;

        assert_eq!(
            actual_response.access_token().secret(),
            expected_response.access_token().secret()
        );
        assert_eq!(actual_response.token_type(), expected_response.token_type());
        assert_eq!(
            actual_response.refresh_token().map(RefreshToken::secret),
            expected_response.refresh_token().map(RefreshToken::secret),
        );
        assert_eq!(actual_response.scopes(), expected_response.scopes());
        assert_eq!(
            actual_response.extra_fields().0,
            expected_response.extra_fields().0
        );
        assert_eq!(
            actual_response.expires_in().is_some(),
            expected_response.expires_in().is_some()
        );
    }

    async fn authorization_manager_for(
        tokens: &StoredOAuthTokens,
    ) -> Result<Arc<TokioMutex<AuthorizationManager>>> {
        let mut state = OAuthState::new(tokens.url.clone(), Some(reqwest::Client::new())).await?;
        state
            .set_credentials(&tokens.client_id, tokens.token_response.0.clone())
            .await?;
        let manager = match state {
            OAuthState::Authorized(manager) | OAuthState::Unauthorized(manager) => manager,
            OAuthState::Session(_) | OAuthState::AuthorizedHttpClient(_) => {
                anyhow::bail!("unexpected OAuth state")
            }
            _ => anyhow::bail!("unexpected OAuth state"),
        };
        Ok(Arc::new(TokioMutex::new(manager)))
    }

    async fn mount_oauth_metadata(server: &MockServer) {
        Mock::given(method("GET"))
            .and(path("/.well-known/oauth-authorization-server/mcp"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "authorization_endpoint": format!("{}/oauth/authorize", server.uri()),
                "token_endpoint": format!("{}/oauth/token", server.uri()),
                "scopes_supported": ["scope-a", "scope-b"],
            })))
            .mount(server)
            .await;
    }

    async fn mount_refresh_response(
        server: &MockServer,
        request_refresh_token: &str,
        response_access_token: &str,
        response_refresh_token: &str,
    ) {
        Mock::given(method("POST"))
            .and(path("/oauth/token"))
            .and(body_string_contains("grant_type=refresh_token"))
            .and(body_string_contains(format!(
                "refresh_token={request_refresh_token}"
            )))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "access_token": response_access_token,
                "token_type": "Bearer",
                "expires_in": 3600,
                "refresh_token": response_refresh_token,
                "scope": "scope-a scope-b",
            })))
            .expect(1)
            .mount(server)
            .await;
    }

    async fn mount_refresh_response_with_signal(
        server: &MockServer,
        request_refresh_token: &str,
        response_access_token: &str,
        response_refresh_token: &str,
        response_delay: Duration,
    ) -> mpsc::Receiver<()> {
        let (tx, rx) = mpsc::channel();
        let response_access_token = response_access_token.to_string();
        let response_refresh_token = response_refresh_token.to_string();
        Mock::given(method("POST"))
            .and(path("/oauth/token"))
            .and(body_string_contains("grant_type=refresh_token"))
            .and(body_string_contains(format!(
                "refresh_token={request_refresh_token}"
            )))
            .respond_with(move |_request: &wiremock::Request| {
                let _ = tx.send(());
                let access_token = response_access_token.clone();
                let refresh_token = response_refresh_token.clone();
                ResponseTemplate::new(200)
                    .set_delay(response_delay)
                    .set_body_json(json!({
                        "access_token": access_token,
                        "token_type": "Bearer",
                        "expires_in": 3600,
                        "refresh_token": refresh_token,
                        "scope": "scope-a scope-b",
                    }))
            })
            .expect(1)
            .mount(server)
            .await;
        rx
    }

    async fn wait_for_signal(rx: mpsc::Receiver<()>) -> Result<()> {
        tokio::task::spawn_blocking(move || {
            rx.recv_timeout(Duration::from_secs(5))
                .context("timed out waiting for refresh request")
        })
        .await?
    }

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "AuthorizationManager async access must be serialized through its mutex"
    )]
    async fn tokens_from_manager(
        manager: &Arc<TokioMutex<AuthorizationManager>>,
    ) -> Result<StoredOAuthTokens> {
        let guard = manager.lock().await;
        let (client_id, token_response) = guard.get_credentials().await?;
        let token_response = token_response.expect("manager should have token response");
        Ok(StoredOAuthTokens {
            server_name: "test-server".to_string(),
            url: "https://example.test".to_string(),
            client_id,
            token_response: WrappedOAuthTokenResponse(token_response),
            expires_at: None,
        })
    }

    fn access_token(tokens: &StoredOAuthTokens) -> &str {
        tokens.token_response.0.access_token().secret()
    }

    fn refresh_token(tokens: &StoredOAuthTokens) -> Option<String> {
        tokens
            .token_response
            .0
            .refresh_token()
            .map(|token| token.secret().to_string())
    }

    fn expired_sample_tokens(url: &str) -> StoredOAuthTokens {
        let mut tokens = sample_tokens();
        tokens.url = url.to_string();
        tokens.expires_at = Some(0);
        tokens
            .token_response
            .0
            .set_expires_in(Some(&Duration::ZERO));
        tokens
    }

    fn now_millis() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| Duration::from_secs(0))
            .as_millis() as u64
    }

    fn sample_tokens() -> StoredOAuthTokens {
        let mut response = OAuthTokenResponse::new(
            AccessToken::new("access-token".to_string()),
            BasicTokenType::Bearer,
            VendorExtraTokenFields::default(),
        );
        response.set_refresh_token(Some(RefreshToken::new("refresh-token".to_string())));
        response.set_scopes(Some(vec![
            Scope::new("scope-a".to_string()),
            Scope::new("scope-b".to_string()),
        ]));
        let expires_in = Duration::from_secs(3600);
        response.set_expires_in(Some(&expires_in));
        let expires_at = super::compute_expires_at_millis(&response);

        StoredOAuthTokens {
            server_name: "test-server".to_string(),
            url: "https://example.test".to_string(),
            client_id: "client-id".to_string(),
            token_response: WrappedOAuthTokenResponse(response),
            expires_at,
        }
    }
}
