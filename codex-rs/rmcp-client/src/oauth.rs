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
use oauth2::AccessToken;
use oauth2::EmptyExtraTokenFields;
use oauth2::RefreshToken;
use oauth2::Scope;
use oauth2::TokenResponse;
use oauth2::basic::BasicTokenType;
use rmcp::transport::auth::CredentialStore;
use rmcp::transport::auth::InMemoryCredentialStore;
use rmcp::transport::auth::OAuthTokenResponse;
use rmcp::transport::auth::StoredCredentials;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use serde_json::map::Map as JsonMap;
use sha2::Digest;
use sha2::Sha256;
use std::collections::BTreeMap;
use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;
use tracing::warn;

use codex_keyring_store::DefaultKeyringStore;
use codex_keyring_store::KeyringStore;
use rmcp::transport::auth::AuthorizationManager;
use tokio::sync::Mutex;

use codex_utils_home_dir::find_codex_home;

const KEYRING_SERVICE: &str = "Codex MCP Credentials";
const REFRESH_SKEW_MILLIS: u64 = 30_000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StoredOAuthTokens {
    pub server_name: String,
    pub url: String,
    pub client_id: String,
    pub token_response: WrappedOAuthTokenResponse,
    #[serde(default)]
    pub expires_at: Option<u64>,
}

/// Determine where Codex should store and read MCP credentials.
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum OAuthCredentialsStoreMode {
    /// `Keyring` when available; otherwise, `File`.
    /// Credentials stored in the keyring will only be readable by Codex unless the user explicitly grants access via OS-level keyring access.
    #[default]
    Auto,
    /// CODEX_HOME/.credentials.json
    /// This file will be readable to Codex and other applications running as the same user.
    File,
    /// Keyring when available, otherwise fail.
    Keyring,
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

pub(crate) fn load_oauth_tokens(
    server_name: &str,
    url: &str,
    store_mode: OAuthCredentialsStoreMode,
) -> Result<Option<StoredOAuthTokens>> {
    let keyring_store = DefaultKeyringStore;
    match store_mode {
        OAuthCredentialsStoreMode::Auto => {
            load_oauth_tokens_from_keyring_with_fallback_to_file(&keyring_store, server_name, url)
        }
        OAuthCredentialsStoreMode::File => load_oauth_tokens_from_file(server_name, url),
        OAuthCredentialsStoreMode::Keyring => {
            load_oauth_tokens_from_keyring(&keyring_store, server_name, url)
                .with_context(|| "failed to read OAuth tokens from keyring".to_string())
        }
    }
}

pub(crate) fn has_oauth_tokens(
    server_name: &str,
    url: &str,
    store_mode: OAuthCredentialsStoreMode,
) -> Result<bool> {
    Ok(load_oauth_tokens(server_name, url, store_mode)?.is_some())
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
            tokens.token_response.0.set_expires_in(None);
        }
    }
}

fn load_oauth_tokens_from_keyring_with_fallback_to_file<K: KeyringStore>(
    keyring_store: &K,
    server_name: &str,
    url: &str,
) -> Result<Option<StoredOAuthTokens>> {
    match load_oauth_tokens_from_keyring(keyring_store, server_name, url) {
        Ok(Some(tokens)) => Ok(Some(tokens)),
        Ok(None) => load_oauth_tokens_from_file(server_name, url),
        Err(error) => {
            warn!("failed to read OAuth tokens from keyring: {error}");
            load_oauth_tokens_from_file(server_name, url)
                .with_context(|| format!("failed to read OAuth tokens from keyring: {error}"))
        }
    }
}

fn load_oauth_tokens_from_keyring<K: KeyringStore>(
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

pub fn save_oauth_tokens(
    server_name: &str,
    tokens: &StoredOAuthTokens,
    store_mode: OAuthCredentialsStoreMode,
) -> Result<()> {
    let keyring_store = DefaultKeyringStore;
    match store_mode {
        OAuthCredentialsStoreMode::Auto => save_oauth_tokens_with_keyring_with_fallback_to_file(
            &keyring_store,
            server_name,
            tokens,
        ),
        OAuthCredentialsStoreMode::File => save_oauth_tokens_to_file(tokens),
        OAuthCredentialsStoreMode::Keyring => {
            save_oauth_tokens_with_keyring(&keyring_store, server_name, tokens)
        }
    }
}

fn save_oauth_tokens_with_keyring<K: KeyringStore>(
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

fn save_oauth_tokens_with_keyring_with_fallback_to_file<K: KeyringStore>(
    keyring_store: &K,
    server_name: &str,
    tokens: &StoredOAuthTokens,
) -> Result<()> {
    match save_oauth_tokens_with_keyring(keyring_store, server_name, tokens) {
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
) -> Result<bool> {
    let keyring_store = DefaultKeyringStore;
    delete_oauth_tokens_from_keyring_and_file(&keyring_store, store_mode, server_name, url)
}

fn delete_oauth_tokens_from_keyring_and_file<K: KeyringStore>(
    keyring_store: &K,
    store_mode: OAuthCredentialsStoreMode,
    server_name: &str,
    url: &str,
) -> Result<bool> {
    let key = compute_store_key(server_name, url)?;
    let keyring_result = keyring_store.delete(KEYRING_SERVICE, &key);
    let keyring_removed = match keyring_result {
        Ok(removed) => removed,
        Err(error) => {
            let message = error.message();
            warn!("failed to delete OAuth tokens from keyring: {message}");
            match store_mode {
                OAuthCredentialsStoreMode::Auto | OAuthCredentialsStoreMode::Keyring => {
                    return Err(error.into_error())
                        .context("failed to delete OAuth tokens from keyring");
                }
                OAuthCredentialsStoreMode::File => false,
            }
        }
    };

    let file_removed = delete_oauth_tokens_from_file(&key)?;
    Ok(keyring_removed || file_removed)
}

#[derive(Clone)]
pub(crate) struct OAuthPersistor {
    inner: Arc<OAuthPersistorInner>,
}

struct OAuthPersistorInner {
    server_name: String,
    url: String,
    authorization_manager: Arc<Mutex<AuthorizationManager>>,
    runtime_credentials: InMemoryCredentialStore,
    store_mode: OAuthCredentialsStoreMode,
    last_credentials: Mutex<Option<StoredOAuthTokens>>,
}

#[derive(Debug, Clone, PartialEq)]
enum GuardedRefreshOutcome {
    NoAction,
    ReloadedChanged(StoredOAuthTokens),
    ReloadedNoChange,
    MissingOrInvalid,
    ReloadFailed,
}

#[derive(Debug, PartialEq)]
enum GuardedRefreshPersistedCredentials {
    Loaded(Option<StoredOAuthTokens>),
    ReloadFailed,
}

impl OAuthPersistor {
    pub(crate) fn new(
        server_name: String,
        url: String,
        authorization_manager: Arc<Mutex<AuthorizationManager>>,
        runtime_credentials: InMemoryCredentialStore,
        store_mode: OAuthCredentialsStoreMode,
        initial_credentials: Option<StoredOAuthTokens>,
    ) -> Self {
        Self {
            inner: Arc::new(OAuthPersistorInner {
                server_name,
                url,
                authorization_manager,
                runtime_credentials,
                store_mode,
                last_credentials: Mutex::new(initial_credentials),
            }),
        }
    }

    /// Persists the latest stored credentials if they have changed.
    /// Deletes the credentials if they are no longer present.
    pub(crate) async fn persist_if_needed(&self) -> Result<()> {
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
                    save_oauth_tokens(&self.inner.server_name, &stored, self.inner.store_mode)?;
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

    /// Guard refreshes against multi-process refresh-token reuse.
    ///
    /// MCP OAuth credentials live in shared storage, but each Codex process also keeps an
    /// in-memory snapshot. Before refreshing, reload the shared credentials and compare them to
    /// the cached copy:
    /// - if the local cache was cleared, reload shared storage first so this process can recover
    ///   when another process logs in and persists fresh credentials;
    /// - if shared storage changed, another process already refreshed, so adopt those credentials
    ///   in the live runtime and skip the local refresh;
    /// - if shared storage is unchanged, this process still owns the refresh and can rotate the
    ///   tokens with the authority;
    /// - if shared storage no longer has credentials, treat that as logged out and clear the live
    ///   runtime instead of sending a stale refresh token.
    pub(crate) async fn refresh_if_needed(&self) -> Result<()> {
        let mut cached_credentials = {
            let guard = self.inner.last_credentials.lock().await;
            guard.clone()
        };

        if cached_credentials.is_none()
            && let Some(credentials) = load_oauth_tokens_when_cache_missing(
                &self.inner.server_name,
                &self.inner.url,
                self.inner.store_mode,
            )
        {
            self.apply_runtime_credentials(Some(credentials.clone()))
                .await?;
            cached_credentials = Some(credentials);
        }

        match self.guarded_refresh_outcome(cached_credentials.as_ref()) {
            GuardedRefreshOutcome::NoAction => Ok(()),
            GuardedRefreshOutcome::ReloadedChanged(credentials) => {
                self.apply_runtime_credentials(Some(credentials)).await
            }
            GuardedRefreshOutcome::ReloadedNoChange => {
                {
                    let manager = self.inner.authorization_manager.clone();
                    let guard = manager.lock().await;
                    guard.refresh_token().await.with_context(|| {
                        format!(
                            "failed to refresh OAuth tokens for server {}",
                            self.inner.server_name
                        )
                    })?;
                }

                self.persist_if_needed().await
            }
            GuardedRefreshOutcome::MissingOrInvalid => self.apply_runtime_credentials(None).await,
            GuardedRefreshOutcome::ReloadFailed => Ok(()),
        }
    }

    fn guarded_refresh_outcome(
        &self,
        cached_credentials: Option<&StoredOAuthTokens>,
    ) -> GuardedRefreshOutcome {
        let Some(cached_credentials) = cached_credentials else {
            return GuardedRefreshOutcome::NoAction;
        };

        if !token_needs_refresh(cached_credentials.expires_at) {
            return GuardedRefreshOutcome::NoAction;
        }

        match load_oauth_tokens_for_guarded_refresh(
            &self.inner.server_name,
            &self.inner.url,
            self.inner.store_mode,
        ) {
            GuardedRefreshPersistedCredentials::Loaded(persisted_credentials) => {
                determine_guarded_refresh_outcome(cached_credentials, persisted_credentials)
            }
            GuardedRefreshPersistedCredentials::ReloadFailed => GuardedRefreshOutcome::ReloadFailed,
        }
    }

    async fn apply_runtime_credentials(
        &self,
        credentials: Option<StoredOAuthTokens>,
    ) -> Result<()> {
        {
            let manager = self.inner.authorization_manager.clone();
            let mut guard = manager.lock().await;

            match credentials.as_ref() {
                Some(credentials) => {
                    self.inner
                        .runtime_credentials
                        .save(StoredCredentials {
                            client_id: credentials.client_id.clone(),
                            token_response: Some(credentials.token_response.0.clone()),
                        })
                        .await?;
                    guard
                        .configure_client_id(&credentials.client_id)
                        .with_context(|| {
                            format!(
                                "failed to reconfigure OAuth client for server {}",
                                self.inner.server_name
                            )
                        })?;
                }
                None => {
                    self.inner.runtime_credentials.clear().await?;
                }
            }
        }

        let mut last_credentials = self.inner.last_credentials.lock().await;
        *last_credentials = credentials;
        Ok(())
    }
}

fn load_oauth_tokens_for_guarded_refresh(
    server_name: &str,
    url: &str,
    store_mode: OAuthCredentialsStoreMode,
) -> GuardedRefreshPersistedCredentials {
    let keyring_store = DefaultKeyringStore;
    match store_mode {
        OAuthCredentialsStoreMode::Auto => {
            load_oauth_tokens_for_guarded_refresh_with_keyring_fallback(
                &keyring_store,
                server_name,
                url,
            )
        }
        OAuthCredentialsStoreMode::File => guarded_refresh_persisted_credentials_from_load_result(
            load_oauth_tokens_from_file(server_name, url),
            server_name,
        ),
        OAuthCredentialsStoreMode::Keyring => {
            guarded_refresh_persisted_credentials_from_load_result(
                load_oauth_tokens_from_keyring(&keyring_store, server_name, url)
                    .with_context(|| "failed to read OAuth tokens from keyring".to_string()),
                server_name,
            )
        }
    }
}

fn load_oauth_tokens_when_cache_missing(
    server_name: &str,
    url: &str,
    store_mode: OAuthCredentialsStoreMode,
) -> Option<StoredOAuthTokens> {
    match load_oauth_tokens_for_guarded_refresh(server_name, url, store_mode) {
        GuardedRefreshPersistedCredentials::Loaded(Some(credentials)) => Some(credentials),
        GuardedRefreshPersistedCredentials::Loaded(None)
        | GuardedRefreshPersistedCredentials::ReloadFailed => None,
    }
}

fn load_oauth_tokens_for_guarded_refresh_with_keyring_fallback<K: KeyringStore>(
    keyring_store: &K,
    server_name: &str,
    url: &str,
) -> GuardedRefreshPersistedCredentials {
    match load_oauth_tokens_from_keyring(keyring_store, server_name, url) {
        Ok(Some(tokens)) => GuardedRefreshPersistedCredentials::Loaded(Some(tokens)),
        Ok(None) => guarded_refresh_persisted_credentials_from_load_result(
            load_oauth_tokens_from_file(server_name, url),
            server_name,
        ),
        Err(error) => {
            warn!("failed to read OAuth tokens from keyring: {error}");
            match load_oauth_tokens_from_file(server_name, url) {
                Ok(Some(tokens)) => GuardedRefreshPersistedCredentials::Loaded(Some(tokens)),
                Ok(None) => {
                    warn!(
                        "failed to reload OAuth tokens for server {server_name}: keyring read failed and no fallback file credentials were available"
                    );
                    GuardedRefreshPersistedCredentials::ReloadFailed
                }
                Err(file_error) => {
                    warn!(
                        "failed to reload OAuth tokens for server {server_name}: keyring read failed ({error}) and fallback file reload failed: {file_error}"
                    );
                    GuardedRefreshPersistedCredentials::ReloadFailed
                }
            }
        }
    }
}

#[cfg(test)]
fn guarded_refresh_outcome_from_load_result(
    cached_credentials: &StoredOAuthTokens,
    persisted_credentials: Result<Option<StoredOAuthTokens>>,
    server_name: &str,
) -> GuardedRefreshOutcome {
    match guarded_refresh_persisted_credentials_from_load_result(persisted_credentials, server_name)
    {
        GuardedRefreshPersistedCredentials::Loaded(persisted_credentials) => {
            determine_guarded_refresh_outcome(cached_credentials, persisted_credentials)
        }
        GuardedRefreshPersistedCredentials::ReloadFailed => GuardedRefreshOutcome::ReloadFailed,
    }
}

fn guarded_refresh_persisted_credentials_from_load_result(
    persisted_credentials: Result<Option<StoredOAuthTokens>>,
    server_name: &str,
) -> GuardedRefreshPersistedCredentials {
    match persisted_credentials {
        Ok(credentials) => GuardedRefreshPersistedCredentials::Loaded(credentials),
        Err(error) => {
            warn!("failed to reload OAuth tokens for server {server_name}: {error}");
            GuardedRefreshPersistedCredentials::ReloadFailed
        }
    }
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
    let Some(store) = read_fallback_file()? else {
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
            EmptyExtraTokenFields {},
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
    let key = compute_store_key(&tokens.server_name, &tokens.url)?;
    let mut store = read_fallback_file()?.unwrap_or_default();

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
    let mut store = match read_fallback_file()? {
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

fn determine_guarded_refresh_outcome(
    cached_credentials: &StoredOAuthTokens,
    persisted_credentials: Option<StoredOAuthTokens>,
) -> GuardedRefreshOutcome {
    match persisted_credentials {
        Some(persisted_credentials)
            if oauth_tokens_equal_for_refresh(
                Some(cached_credentials),
                Some(&persisted_credentials),
            ) =>
        {
            GuardedRefreshOutcome::ReloadedNoChange
        }
        Some(persisted_credentials) => {
            GuardedRefreshOutcome::ReloadedChanged(persisted_credentials)
        }
        None => GuardedRefreshOutcome::MissingOrInvalid,
    }
}

fn oauth_tokens_equal_for_refresh(
    left: Option<&StoredOAuthTokens>,
    right: Option<&StoredOAuthTokens>,
) -> bool {
    match (left, right) {
        (None, None) => true,
        (Some(left), Some(right)) => {
            left.server_name == right.server_name
                && left.url == right.url
                && left.client_id == right.client_id
                && left.expires_at == right.expires_at
                && oauth_token_responses_equal_for_refresh(
                    &left.token_response,
                    &right.token_response,
                )
        }
        _ => false,
    }
}

fn oauth_token_responses_equal_for_refresh(
    left: &WrappedOAuthTokenResponse,
    right: &WrappedOAuthTokenResponse,
) -> bool {
    let left = &left.0;
    let right = &right.0;

    left.access_token().secret() == right.access_token().secret()
        && left.token_type() == right.token_type()
        && left.refresh_token().map(RefreshToken::secret)
            == right.refresh_token().map(RefreshToken::secret)
        && left.scopes() == right.scopes()
        && left.extra_fields() == right.extra_fields()
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

fn fallback_file_path() -> Result<PathBuf> {
    let mut path = find_codex_home()?;
    path.push(FALLBACK_FILENAME);
    Ok(path)
}

fn read_fallback_file() -> Result<Option<FallbackFile>> {
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
    use keyring::Error as KeyringError;
    use pretty_assertions::assert_eq;
    use std::sync::Mutex;
    use std::sync::MutexGuard;
    use std::sync::OnceLock;
    use std::sync::PoisonError;
    use tempfile::tempdir;

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

        let loaded =
            super::load_oauth_tokens_from_keyring(&store, &tokens.server_name, &tokens.url)?
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
            &tokens.server_name,
            &tokens,
        )?;

        let fallback_path = super::fallback_file_path()?;
        assert!(fallback_path.exists(), "fallback file should be created");
        let saved = super::read_fallback_file()?.expect("fallback file should load");
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
    fn refresh_expires_in_from_timestamp_clears_expired_tokens() {
        let mut tokens = sample_tokens();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| Duration::from_secs(0));
        let expired_at = now.as_millis() as u64;
        tokens.expires_at = Some(expired_at.saturating_sub(1000));

        let duration = Duration::from_secs(600);
        tokens.token_response.0.set_expires_in(Some(&duration));

        super::refresh_expires_in_from_timestamp(&mut tokens);

        assert!(tokens.token_response.0.expires_in().is_none());
    }

    #[test]
    fn guarded_refresh_outcome_reloads_when_persisted_credentials_changed() {
        let cached = sample_tokens();
        let mut persisted = sample_tokens();
        persisted
            .token_response
            .0
            .set_refresh_token(Some(RefreshToken::new("rotated-refresh-token".to_string())));
        persisted
            .token_response
            .0
            .set_expires_in(Some(&Duration::from_secs(7200)));
        persisted.expires_at = super::compute_expires_at_millis(&persisted.token_response.0);

        assert_eq!(
            super::determine_guarded_refresh_outcome(&cached, Some(persisted.clone())),
            super::GuardedRefreshOutcome::ReloadedChanged(persisted),
        );
    }

    #[test]
    fn guarded_refresh_outcome_refreshes_when_persisted_credentials_match() {
        let cached = sample_tokens();
        let mut persisted = cached.clone();
        persisted
            .token_response
            .0
            .set_expires_in(Some(&Duration::from_secs(5)));

        assert_eq!(
            super::determine_guarded_refresh_outcome(&cached, Some(persisted)),
            super::GuardedRefreshOutcome::ReloadedNoChange,
        );
    }

    #[test]
    fn guarded_refresh_outcome_clears_when_persisted_credentials_are_missing() {
        assert_eq!(
            super::determine_guarded_refresh_outcome(&sample_tokens(), None),
            super::GuardedRefreshOutcome::MissingOrInvalid,
        );
    }

    #[test]
    fn guarded_refresh_outcome_keeps_state_recoverable_when_reload_fails() {
        let error = anyhow::anyhow!("transient read failure");

        assert_eq!(
            super::guarded_refresh_outcome_from_load_result(
                &sample_tokens(),
                Err(error),
                "test-server",
            ),
            super::GuardedRefreshOutcome::ReloadFailed,
        );
    }

    #[test]
    fn guarded_refresh_auto_load_keeps_state_recoverable_when_keyring_fails_without_file() {
        let _env = TempCodexHome::new();
        let store = MockKeyringStore::default();
        let tokens = sample_tokens();
        let key = super::compute_store_key(&tokens.server_name, &tokens.url)
            .expect("store key should compute");
        store.set_error(&key, KeyringError::Invalid("error".into(), "load".into()));

        assert_eq!(
            super::load_oauth_tokens_for_guarded_refresh_with_keyring_fallback(
                &store,
                &tokens.server_name,
                &tokens.url,
            ),
            super::GuardedRefreshPersistedCredentials::ReloadFailed,
        );
    }

    #[test]
    fn missing_cached_credentials_reload_shared_store_from_file() -> Result<()> {
        let _env = TempCodexHome::new();
        let tokens = sample_tokens();
        let expected = tokens.clone();
        super::save_oauth_tokens_to_file(&tokens)?;

        let loaded = super::load_oauth_tokens_when_cache_missing(
            &tokens.server_name,
            &tokens.url,
            OAuthCredentialsStoreMode::File,
        )
        .expect("tokens should reload from shared file store");
        assert_tokens_match_without_expiry(&loaded, &expected);
        Ok(())
    }

    #[test]
    fn missing_cached_credentials_ignore_reload_failures() {
        let _env = TempCodexHome::new();
        let store = MockKeyringStore::default();
        let tokens = sample_tokens();
        let key = super::compute_store_key(&tokens.server_name, &tokens.url)
            .expect("store key should compute");
        store.set_error(&key, KeyringError::Invalid("error".into(), "load".into()));

        assert_eq!(
            super::load_oauth_tokens_for_guarded_refresh_with_keyring_fallback(
                &store,
                &tokens.server_name,
                &tokens.url,
            ),
            super::GuardedRefreshPersistedCredentials::ReloadFailed,
        );
        assert_eq!(
            super::load_oauth_tokens_when_cache_missing(
                &tokens.server_name,
                &tokens.url,
                OAuthCredentialsStoreMode::Auto,
            ),
            None,
        );
    }

    #[test]
    fn oauth_tokens_equal_for_refresh_ignores_only_expires_in() {
        let left = sample_tokens();
        let mut right = left.clone();
        right
            .token_response
            .0
            .set_expires_in(Some(&Duration::from_secs(5)));

        assert!(super::oauth_tokens_equal_for_refresh(
            Some(&left),
            Some(&right),
        ));

        let mut different_refresh_token = right.clone();
        different_refresh_token
            .token_response
            .0
            .set_refresh_token(Some(RefreshToken::new("different-refresh".to_string())));
        assert!(!super::oauth_tokens_equal_for_refresh(
            Some(&left),
            Some(&different_refresh_token),
        ));

        let mut different_expiry = right;
        different_expiry.expires_at = different_expiry.expires_at.map(|value| value + 1000);
        assert!(!super::oauth_tokens_equal_for_refresh(
            Some(&left),
            Some(&different_expiry),
        ));
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
            actual_response.extra_fields(),
            expected_response.extra_fields()
        );
        assert_eq!(
            actual_response.expires_in().is_some(),
            expected_response.expires_in().is_some()
        );
    }

    fn sample_tokens() -> StoredOAuthTokens {
        let mut response = OAuthTokenResponse::new(
            AccessToken::new("access-token".to_string()),
            BasicTokenType::Bearer,
            EmptyExtraTokenFields {},
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
