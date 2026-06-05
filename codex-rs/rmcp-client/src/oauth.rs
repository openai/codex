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
use codex_config::types::OAuthCredentialsStoreMode;
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
use std::fs::OpenOptions;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::OnceLock;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;
use tracing::warn;

use codex_keyring_store::DefaultKeyringStore;
use codex_keyring_store::KeyringStore;
use rmcp::transport::auth::AuthorizationManager;
use rmcp::transport::auth::InMemoryCredentialStore;
use rmcp::transport::auth::OAuthState;
use tokio::sync::Mutex;
use tokio::sync::OwnedMutexGuard;

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
            match load_oauth_tokens_from_file(server_name, url) {
                Ok(Some(tokens)) => Ok(Some(tokens)),
                Ok(None) => Err(error).context(
                    "failed to read OAuth tokens from keyring and no fallback credentials exist",
                ),
                Err(file_error) => Err(file_error)
                    .with_context(|| format!("failed to read OAuth tokens from keyring: {error}")),
            }
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
    let _lock = acquire_oauth_server_lock(server_name, &tokens.url)?;
    save_oauth_tokens_locked(server_name, tokens, store_mode)
}

pub(crate) async fn save_oauth_tokens_async(
    server_name: &str,
    tokens: &StoredOAuthTokens,
    store_mode: OAuthCredentialsStoreMode,
) -> Result<()> {
    let _lock = acquire_oauth_server_lock_async(server_name, &tokens.url).await?;
    save_oauth_tokens_locked(server_name, tokens, store_mode)
}

fn save_oauth_tokens_locked(
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
    let _lock = acquire_oauth_server_lock(server_name, url)?;
    delete_oauth_tokens_locked(server_name, url, store_mode)
}

fn delete_oauth_tokens_locked(
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
    oauth_metadata_client: reqwest::Client,
    store_mode: OAuthCredentialsStoreMode,
    last_credentials: Mutex<Option<StoredOAuthTokens>>,
}

enum CredentialReload {
    Unchanged,
    Replaced,
    Removed,
}

pub(crate) async fn authorization_manager_from_tokens(
    url: &str,
    oauth_metadata_client: reqwest::Client,
    tokens: &StoredOAuthTokens,
) -> Result<AuthorizationManager> {
    let mut oauth_state = OAuthState::new(url.to_string(), Some(oauth_metadata_client)).await?;
    oauth_state
        .set_credentials(&tokens.client_id, tokens.token_response.0.clone())
        .await?;

    match oauth_state {
        OAuthState::Authorized(manager) | OAuthState::Unauthorized(manager) => Ok(manager),
        _ => Err(anyhow::anyhow!(
            "unexpected OAuth state while replacing credentials"
        )),
    }
}

impl OAuthPersistor {
    pub(crate) fn new(
        server_name: String,
        url: String,
        authorization_manager: Arc<Mutex<AuthorizationManager>>,
        oauth_metadata_client: reqwest::Client,
        store_mode: OAuthCredentialsStoreMode,
        initial_credentials: Option<StoredOAuthTokens>,
    ) -> Self {
        Self {
            inner: Arc::new(OAuthPersistorInner {
                server_name,
                url,
                authorization_manager,
                oauth_metadata_client,
                store_mode,
                last_credentials: Mutex::new(initial_credentials),
            }),
        }
    }

    /// Persists the latest stored credentials if they have changed.
    /// Deletes the credentials if they are no longer present.
    pub(crate) async fn persist_if_needed(&self) -> Result<()> {
        let _server_lock =
            acquire_oauth_server_lock_async(&self.inner.server_name, &self.inner.url).await?;
        if !matches!(
            self.reload_persisted_credentials_locked().await?,
            CredentialReload::Unchanged
        ) {
            return Ok(());
        }
        self.persist_if_needed_locked().await
    }

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "AuthorizationManager async access and persisted credential comparison must be serialized"
    )]
    async fn persist_if_needed_locked(&self) -> Result<()> {
        let (client_id, maybe_credentials) = {
            let manager = self.inner.authorization_manager.clone();
            let guard = manager.lock().await;
            guard.get_credentials().await
        }?;

        match maybe_credentials {
            Some(credentials) => {
                let last_credentials = self.inner.last_credentials.lock().await.clone();
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
                if !last_credentials
                    .as_ref()
                    .is_some_and(|previous| same_persisted_generation(previous, &stored))
                {
                    save_oauth_tokens_locked(
                        &self.inner.server_name,
                        &stored,
                        self.inner.store_mode,
                    )?;
                    *self.inner.last_credentials.lock().await = Some(stored);
                }
            }
            None => {
                let mut last_serialized = self.inner.last_credentials.lock().await;
                if last_serialized.take().is_some()
                    && let Err(error) = delete_oauth_tokens_locked(
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

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "the server credential lock must span reload, refresh, and persistence"
    )]
    pub(crate) async fn refresh_if_needed(&self) -> Result<()> {
        let _server_lock =
            acquire_oauth_server_lock_async(&self.inner.server_name, &self.inner.url).await?;
        if matches!(
            self.reload_persisted_credentials_locked().await?,
            CredentialReload::Removed
        ) {
            return Err(anyhow::anyhow!(
                "OAuth credentials were removed for server {}",
                self.inner.server_name
            ));
        }

        let expires_at = {
            let guard = self.inner.last_credentials.lock().await;
            guard.as_ref().and_then(|tokens| tokens.expires_at)
        };

        if !token_needs_refresh(expires_at) {
            return Ok(());
        }

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

        self.persist_if_needed_locked().await
    }

    async fn reload_persisted_credentials_locked(&self) -> Result<CredentialReload> {
        let loaded = load_oauth_tokens(
            &self.inner.server_name,
            &self.inner.url,
            self.inner.store_mode,
        )
        .with_context(|| {
            format!(
                "failed to reload OAuth tokens for server {}",
                self.inner.server_name
            )
        })?;

        let last_credentials = self.inner.last_credentials.lock().await.clone();
        let unchanged = match (&loaded, &last_credentials) {
            (Some(loaded), Some(previous)) => same_persisted_generation(loaded, previous),
            (None, None) => true,
            _ => false,
        };
        if unchanged {
            return Ok(CredentialReload::Unchanged);
        }

        match loaded {
            Some(tokens) => {
                self.replace_manager_credentials(&tokens).await?;
                *self.inner.last_credentials.lock().await = Some(tokens);
                Ok(CredentialReload::Replaced)
            }
            None => {
                self.clear_manager_credentials().await;
                *self.inner.last_credentials.lock().await = None;
                Ok(CredentialReload::Removed)
            }
        }
    }

    async fn replace_manager_credentials(&self, tokens: &StoredOAuthTokens) -> Result<()> {
        let replacement = authorization_manager_from_tokens(
            &self.inner.url,
            self.inner.oauth_metadata_client.clone(),
            tokens,
        )
        .await?;
        let manager = self.inner.authorization_manager.clone();
        let mut guard = manager.lock().await;
        *guard = replacement;
        Ok(())
    }

    async fn clear_manager_credentials(&self) {
        let manager = self.inner.authorization_manager.clone();
        let mut guard = manager.lock().await;
        guard.set_credential_store(InMemoryCredentialStore::new());
    }
}

fn oauth_server_lock_for(server_name: &str, url: &str) -> Arc<Mutex<()>> {
    static OAUTH_SERVER_LOCKS: OnceLock<std::sync::Mutex<BTreeMap<String, Arc<Mutex<()>>>>> =
        OnceLock::new();

    let mut locks = OAUTH_SERVER_LOCKS
        .get_or_init(std::sync::Mutex::default)
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    locks
        .entry(format!("{server_name}\n{url}"))
        .or_insert_with(|| Arc::new(Mutex::new(())))
        .clone()
}

struct OAuthServerLock {
    _file: fs::File,
    _in_process_guard: Option<OwnedMutexGuard<()>>,
}

fn acquire_oauth_server_lock(server_name: &str, url: &str) -> Result<OAuthServerLock> {
    let path = oauth_server_lock_path(server_name, url)?;
    let file = open_oauth_lock_file(&path)?;
    file.lock()
        .with_context(|| format!("failed to lock OAuth credentials at {}", path.display()))?;
    Ok(OAuthServerLock {
        _file: file,
        _in_process_guard: None,
    })
}

async fn acquire_oauth_server_lock_async(server_name: &str, url: &str) -> Result<OAuthServerLock> {
    let in_process_guard = oauth_server_lock_for(server_name, url).lock_owned().await;
    let path = oauth_server_lock_path(server_name, url)?;
    let file = tokio::task::spawn_blocking({
        let path = path.clone();
        move || {
            let file = open_oauth_lock_file(&path)?;
            file.lock().with_context(|| {
                format!("failed to lock OAuth credentials at {}", path.display())
            })?;
            Ok::<_, anyhow::Error>(file)
        }
    })
    .await
    .context("OAuth credential lock task failed")??;
    Ok(OAuthServerLock {
        _file: file,
        _in_process_guard: Some(in_process_guard),
    })
}

struct FallbackStoreLock {
    _in_process_guard: std::sync::MutexGuard<'static, ()>,
    _file: fs::File,
}

fn acquire_fallback_store_lock() -> Result<FallbackStoreLock> {
    static FALLBACK_STORE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    let in_process_guard = FALLBACK_STORE_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let path = fallback_store_lock_path()?;
    let file = open_oauth_lock_file(&path)?;
    file.lock().with_context(|| {
        format!(
            "failed to lock fallback OAuth credentials at {}",
            path.display()
        )
    })?;
    Ok(FallbackStoreLock {
        _in_process_guard: in_process_guard,
        _file: file,
    })
}

fn open_oauth_lock_file(path: &std::path::Path) -> Result<fs::File> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
        secure_oauth_lock_dir(parent)?;
    }
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .with_context(|| format!("failed to open OAuth credential lock at {}", path.display()))
}

#[cfg(unix)]
fn secure_oauth_lock_dir(path: &std::path::Path) -> Result<()> {
    use std::os::unix::fs::MetadataExt;
    use std::os::unix::fs::PermissionsExt;

    let metadata = fs::symlink_metadata(path)?;
    let effective_uid = unsafe { libc::geteuid() };
    if !metadata.file_type().is_dir() || metadata.uid() != effective_uid {
        anyhow::bail!(
            "OAuth credential lock directory {} is not owned by the current user",
            path.display()
        );
    }
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

#[cfg(not(unix))]
fn secure_oauth_lock_dir(_path: &std::path::Path) -> Result<()> {
    Ok(())
}

fn oauth_server_lock_path(server_name: &str, url: &str) -> Result<PathBuf> {
    let server_digest = sha_256_prefix(&Value::String(format!("{server_name}\n{url}")))?;
    Ok(oauth_lock_root_path()?.join(format!("server-{server_digest}.lock")))
}

fn fallback_store_lock_path() -> Result<PathBuf> {
    let credentials_path = fallback_file_path()?;
    let store_digest = sha_256_prefix(&Value::String(
        credentials_path.to_string_lossy().into_owned(),
    ))?;
    Ok(oauth_lock_root_path()?.join(format!("fallback-{store_digest}.lock")))
}

#[cfg(unix)]
fn oauth_lock_root_path() -> Result<PathBuf> {
    let effective_uid = unsafe { libc::geteuid() };
    Ok(PathBuf::from("/tmp").join(format!("codex-mcp-oauth-{effective_uid}")))
}

#[cfg(windows)]
fn oauth_lock_root_path() -> Result<PathBuf> {
    Ok(windows_local_app_data_dir()?
        .join("Temp")
        .join("codex-mcp-oauth-locks"))
}

#[cfg(windows)]
fn windows_local_app_data_dir() -> Result<PathBuf> {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use windows_sys::Win32::System::Com::CoTaskMemFree;
    use windows_sys::Win32::UI::Shell::FOLDERID_LocalAppData;
    use windows_sys::Win32::UI::Shell::KF_FLAG_DONT_VERIFY;
    use windows_sys::Win32::UI::Shell::SHGetKnownFolderPath;

    let mut path_ptr = std::ptr::null_mut::<u16>();
    let known_folder_flags =
        u32::try_from(KF_FLAG_DONT_VERIFY).context("KF_FLAG_DONT_VERIFY did not fit in u32")?;
    let hr = unsafe {
        SHGetKnownFolderPath(&FOLDERID_LocalAppData, known_folder_flags, 0, &mut path_ptr)
    };
    if hr != 0 {
        anyhow::bail!("SHGetKnownFolderPath(FOLDERID_LocalAppData) failed with HRESULT {hr:#010x}");
    }
    if path_ptr.is_null() {
        anyhow::bail!("SHGetKnownFolderPath(FOLDERID_LocalAppData) returned a null pointer");
    }

    let path = unsafe {
        let mut len = 0usize;
        while *path_ptr.add(len) != 0 {
            len += 1;
        }
        let wide = std::slice::from_raw_parts(path_ptr, len);
        let path = PathBuf::from(OsString::from_wide(wide));
        CoTaskMemFree(path_ptr.cast());
        path
    };
    Ok(path)
}

#[cfg(not(any(unix, windows)))]
fn oauth_lock_root_path() -> Result<PathBuf> {
    anyhow::bail!("OAuth credential locks are unsupported on this platform")
}

fn same_persisted_generation(left: &StoredOAuthTokens, right: &StoredOAuthTokens) -> bool {
    if left.server_name != right.server_name
        || left.url != right.url
        || left.client_id != right.client_id
        || left.expires_at != right.expires_at
    {
        return false;
    }

    let left_response = &left.token_response.0;
    let right_response = &right.token_response.0;
    left_response.access_token().secret() == right_response.access_token().secret()
        && left_response.token_type() == right_response.token_type()
        && left_response.refresh_token().map(RefreshToken::secret)
            == right_response.refresh_token().map(RefreshToken::secret)
        && left_response.scopes() == right_response.scopes()
        && left_response.extra_fields().0 == right_response.extra_fields().0
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
    let _lock = acquire_fallback_store_lock()?;
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
    let _lock = acquire_fallback_store_lock()?;
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
    let _lock = acquire_fallback_store_lock()?;
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
    Ok(find_codex_home()?.join(FALLBACK_FILENAME).to_path_buf())
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
    use std::ffi::OsString;
    use std::process::Child;
    use std::process::Command;
    use std::sync::Mutex;
    use std::sync::MutexGuard;
    use std::sync::OnceLock;
    use std::sync::PoisonError;
    use std::thread;
    use std::time::Instant;
    use tempfile::tempdir;
    use wiremock::Mock;
    use wiremock::MockServer;
    use wiremock::ResponseTemplate;
    use wiremock::matchers::body_string_contains;
    use wiremock::matchers::method;
    use wiremock::matchers::path;

    use codex_keyring_store::tests::MockKeyringStore;

    const FALLBACK_WRITER_VARIANT_ENV: &str = "CODEX_TEST_FALLBACK_WRITER_VARIANT";
    const FALLBACK_WRITER_ATTEMPT_ENV: &str = "CODEX_TEST_FALLBACK_WRITER_ATTEMPT";
    const FALLBACK_WRITER_DONE_ENV: &str = "CODEX_TEST_FALLBACK_WRITER_DONE";

    struct TempCodexHome {
        _guard: MutexGuard<'static, ()>,
        _dir: tempfile::TempDir,
        original_codex_home: Option<OsString>,
        original_home: Option<OsString>,
        original_tmpdir: Option<OsString>,
        original_userprofile: Option<OsString>,
    }

    impl TempCodexHome {
        fn new() -> Self {
            static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
            let guard = LOCK
                .get_or_init(Mutex::default)
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            let dir = tempdir().expect("create CODEX_HOME temp dir");
            let codex_home = dir.path().join("codex-home");
            let user_home = dir.path().join("user-home");
            fs::create_dir_all(&codex_home).expect("create CODEX_HOME");
            fs::create_dir_all(&user_home).expect("create user home");
            let original_codex_home = std::env::var_os("CODEX_HOME");
            let original_home = std::env::var_os("HOME");
            let original_tmpdir = std::env::var_os("TMPDIR");
            let original_userprofile = std::env::var_os("USERPROFILE");
            unsafe {
                std::env::set_var("CODEX_HOME", codex_home);
                std::env::set_var("HOME", &user_home);
                std::env::set_var("USERPROFILE", user_home);
            }
            Self {
                _guard: guard,
                _dir: dir,
                original_codex_home,
                original_home,
                original_tmpdir,
                original_userprofile,
            }
        }
    }

    impl Drop for TempCodexHome {
        fn drop(&mut self) {
            unsafe {
                restore_env_var("CODEX_HOME", self.original_codex_home.as_ref());
                restore_env_var("HOME", self.original_home.as_ref());
                restore_env_var("TMPDIR", self.original_tmpdir.as_ref());
                restore_env_var("USERPROFILE", self.original_userprofile.as_ref());
            }
        }
    }

    unsafe fn restore_env_var(name: &str, value: Option<&OsString>) {
        match value {
            Some(value) => unsafe { std::env::set_var(name, value) },
            None => unsafe { std::env::remove_var(name) },
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
    fn load_oauth_tokens_preserves_keyring_error_without_fallback() -> Result<()> {
        let _env = TempCodexHome::new();
        let store = MockKeyringStore::default();
        let tokens = sample_tokens();
        let key = super::compute_store_key(&tokens.server_name, &tokens.url)?;
        store.set_error(&key, KeyringError::Invalid("error".into(), "load".into()));

        let error = super::load_oauth_tokens_from_keyring_with_fallback_to_file(
            &store,
            &tokens.server_name,
            &tokens.url,
        )
        .expect_err("missing fallback must not turn a keyring failure into logout");

        assert!(error.to_string().contains(
            "failed to read OAuth tokens from keyring and no fallback credentials exist"
        ));
        Ok(())
    }

    #[tokio::test]
    #[expect(
        clippy::await_holding_invalid_type,
        reason = "the test must inspect and refresh the same authorization manager instance"
    )]
    async fn replacement_updates_current_scopes_used_by_scope_less_refresh() -> Result<()> {
        let server = MockServer::start().await;
        let server_url = format!("{}/mcp", server.uri());
        Mock::given(method("GET"))
            .and(path("/.well-known/oauth-authorization-server/mcp"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "authorization_endpoint": format!("{}/oauth/authorize", server.uri()),
                "token_endpoint": format!("{}/oauth/token", server.uri()),
                "scopes_supported": ["external-scope"],
            })))
            .expect(2)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/oauth/token"))
            .and(body_string_contains("refresh_token=refresh-token"))
            .and(body_string_contains("scope=external-scope"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "refreshed-access-token",
                "token_type": "Bearer",
                "expires_in": 7200,
            })))
            .expect(1)
            .mount(&server)
            .await;

        let mut initial = sample_tokens();
        initial.url = server_url.clone();
        let oauth_metadata_client = reqwest::Client::new();
        let manager = Arc::new(tokio::sync::Mutex::new(
            super::authorization_manager_from_tokens(
                &server_url,
                oauth_metadata_client.clone(),
                &initial,
            )
            .await?,
        ));
        let runtime = OAuthPersistor::new(
            initial.server_name.clone(),
            server_url,
            Arc::clone(&manager),
            oauth_metadata_client,
            OAuthCredentialsStoreMode::File,
            Some(initial.clone()),
        );
        let mut replacement = initial;
        replacement
            .token_response
            .0
            .set_scopes(Some(vec![Scope::new("external-scope".to_string())]));

        runtime.replace_manager_credentials(&replacement).await?;
        let guard = manager.lock().await;
        assert_eq!(
            guard.get_current_scopes().await,
            vec!["external-scope".to_string()]
        );
        guard.refresh_token().await?;
        assert_eq!(
            guard.get_current_scopes().await,
            vec!["external-scope".to_string()]
        );
        drop(guard);
        server.verify().await;
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
    fn persisted_generation_ignores_reload_expires_in_drift() -> Result<()> {
        let _env = TempCodexHome::new();
        let mut tokens = sample_tokens();
        tokens
            .token_response
            .0
            .set_expires_in(Some(&Duration::from_secs(7200)));

        super::save_oauth_tokens_to_file(&tokens)?;
        let loaded = super::load_oauth_tokens_from_file(&tokens.server_name, &tokens.url)?
            .expect("saved tokens should reload");

        assert_ne!(
            loaded.token_response.0.expires_in(),
            tokens.token_response.0.expires_in()
        );
        assert_ne!(loaded, tokens);
        assert!(super::same_persisted_generation(&loaded, &tokens));
        Ok(())
    }

    #[test]
    fn persisted_generation_compares_vendor_fields_without_hashmap_order() {
        let mut left = sample_tokens();
        let mut right = left.clone();
        let mut left_extra_fields = VendorExtraTokenFields::default();
        left_extra_fields
            .0
            .insert("first".to_string(), serde_json::json!({"nested": true}));
        left_extra_fields
            .0
            .insert("second".to_string(), serde_json::json!(["value"]));
        let mut right_extra_fields = VendorExtraTokenFields::default();
        right_extra_fields
            .0
            .insert("second".to_string(), serde_json::json!(["value"]));
        right_extra_fields
            .0
            .insert("first".to_string(), serde_json::json!({"nested": true}));
        left.token_response.0.set_extra_fields(left_extra_fields);
        right.token_response.0.set_extra_fields(right_extra_fields);
        right
            .token_response
            .0
            .set_expires_in(Some(&Duration::from_secs(1)));

        assert!(super::same_persisted_generation(&left, &right));

        let mut changed_extra_fields = right.token_response.0.extra_fields().clone();
        changed_extra_fields
            .0
            .insert("first".to_string(), serde_json::json!({"nested": false}));
        right
            .token_response
            .0
            .set_extra_fields(changed_extra_fields);
        assert!(!super::same_persisted_generation(&left, &right));
    }

    #[cfg(unix)]
    #[test]
    fn read_only_home_and_codex_home_allow_reads_but_not_mutations() -> Result<()> {
        use std::os::unix::fs::PermissionsExt;

        let _env = TempCodexHome::new();
        let tokens = sample_tokens();
        super::save_oauth_tokens_to_file(&tokens)?;
        let codex_home = find_codex_home()?;
        let user_home = PathBuf::from(std::env::var_os("HOME").expect("HOME should be set"));
        let codex_home_permissions = fs::metadata(&codex_home)?.permissions();
        let user_home_permissions = fs::metadata(&user_home)?.permissions();
        fs::set_permissions(&codex_home, fs::Permissions::from_mode(0o500))?;
        fs::set_permissions(&user_home, fs::Permissions::from_mode(0o500))?;

        let loaded = super::load_oauth_tokens_from_file(&tokens.server_name, &tokens.url);
        let key = super::compute_store_key(&tokens.server_name, &tokens.url)?;
        let deletion = super::delete_oauth_tokens_from_file(&key);

        fs::set_permissions(&codex_home, codex_home_permissions)?;
        fs::set_permissions(&user_home, user_home_permissions)?;
        let loaded = loaded?.expect("tokens should load from read-only homes");
        assert_tokens_match_without_expiry(&loaded, &tokens);
        let error = deletion.expect_err("credential deletion should require a writable store");
        assert_eq!(
            error
                .root_cause()
                .downcast_ref::<std::io::Error>()
                .map(std::io::Error::kind),
            Some(ErrorKind::PermissionDenied)
        );
        Ok(())
    }

    #[test]
    fn fallback_store_serializes_cross_process_writes_for_different_servers() -> Result<()> {
        let _env = TempCodexHome::new();
        let first = sample_tokens();
        let mut second = sample_tokens();
        second.server_name = "other-server".to_string();
        second.url = "https://other.example.test".to_string();
        second.token_response.0 = OAuthTokenResponse::new(
            AccessToken::new("other-access-token".to_string()),
            BasicTokenType::Bearer,
            VendorExtraTokenFields::default(),
        );

        let fallback_lock = super::acquire_fallback_store_lock()?;
        let codex_home = find_codex_home()?;
        let markers = tempdir()?;
        let mut writers = ["first", "second"]
            .into_iter()
            .map(|variant| {
                let attempt = markers.path().join(format!("{variant}-attempt"));
                let done = markers.path().join(format!("{variant}-done"));
                let child = Command::new(std::env::current_exe()?)
                    .args(["fallback_store_writer_child", "--ignored", "--nocapture"])
                    .env("CODEX_HOME", codex_home.as_path())
                    .env(FALLBACK_WRITER_VARIANT_ENV, variant)
                    .env(FALLBACK_WRITER_ATTEMPT_ENV, &attempt)
                    .env(FALLBACK_WRITER_DONE_ENV, &done)
                    .spawn()?;
                Ok((child, attempt, done))
            })
            .collect::<Result<Vec<_>>>()?;
        wait_for_paths(
            &writers
                .iter()
                .map(|(_, attempt, _)| attempt.clone())
                .collect::<Vec<_>>(),
            Duration::from_secs(5),
        )?;
        thread::sleep(Duration::from_millis(100));
        for (child, _, done) in &mut writers {
            assert!(
                child.try_wait()?.is_none(),
                "fallback writer should block on the cross-process file lock"
            );
            assert!(
                !done.exists(),
                "fallback writer completed while another process held the file lock"
            );
        }

        drop(fallback_lock);
        for (child, _, _) in &mut writers {
            wait_for_child_success(child, Duration::from_secs(5))?;
        }

        let store = super::read_fallback_file()?.expect("fallback file should load");
        let first_key = super::compute_store_key(&first.server_name, &first.url)?;
        let second_key = super::compute_store_key(&second.server_name, &second.url)?;
        assert_eq!(
            store
                .keys()
                .cloned()
                .collect::<std::collections::BTreeSet<_>>(),
            [first_key, second_key].into_iter().collect()
        );
        Ok(())
    }

    #[test]
    #[ignore = "spawned by fallback_store_serializes_cross_process_writes_for_different_servers"]
    fn fallback_store_writer_child() -> Result<()> {
        let mut tokens = sample_tokens();
        match std::env::var(FALLBACK_WRITER_VARIANT_ENV)?.as_str() {
            "first" => {}
            "second" => {
                tokens.server_name = "other-server".to_string();
                tokens.url = "https://other.example.test".to_string();
                tokens.token_response.0 = OAuthTokenResponse::new(
                    AccessToken::new("other-access-token".to_string()),
                    BasicTokenType::Bearer,
                    VendorExtraTokenFields::default(),
                );
            }
            variant => anyhow::bail!("unexpected fallback writer variant: {variant}"),
        }

        let attempt = PathBuf::from(
            std::env::var_os(FALLBACK_WRITER_ATTEMPT_ENV)
                .context("fallback writer attempt path should be set")?,
        );
        let done = PathBuf::from(
            std::env::var_os(FALLBACK_WRITER_DONE_ENV)
                .context("fallback writer done path should be set")?,
        );
        fs::write(attempt, b"attempting")?;
        super::save_oauth_tokens_to_file(&tokens)?;
        fs::write(done, b"done")?;
        Ok(())
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn server_generation_lock_is_stable_across_read_only_homes_and_tmpdirs() -> Result<()> {
        #[cfg(unix)]
        use std::os::unix::fs::PermissionsExt;

        let _env = TempCodexHome::new();
        #[cfg(unix)]
        let first_home = PathBuf::from(std::env::var_os("HOME").expect("HOME should be set"));
        let first_tmpdir = tempdir()?;
        unsafe {
            std::env::set_var("TMPDIR", first_tmpdir.path());
        }
        let first = super::oauth_server_lock_path("server", "https://example.test")?;
        #[cfg(unix)]
        let expected_root =
            PathBuf::from("/tmp").join(format!("codex-mcp-oauth-{}", unsafe { libc::geteuid() }));
        #[cfg(windows)]
        let expected_root = super::windows_local_app_data_dir()?
            .join("Temp")
            .join("codex-mcp-oauth-locks");
        assert_eq!(first.parent(), Some(expected_root.as_path()));
        let other_codex_home = tempdir()?;
        let other_home = tempdir()?;
        let second_tmpdir = tempdir()?;
        #[cfg(unix)]
        {
            fs::set_permissions(&first_home, fs::Permissions::from_mode(0o500))?;
            fs::set_permissions(other_home.path(), fs::Permissions::from_mode(0o500))?;
        }
        unsafe {
            std::env::set_var("CODEX_HOME", other_codex_home.path());
            std::env::set_var("HOME", other_home.path());
            std::env::set_var("TMPDIR", second_tmpdir.path());
            std::env::set_var("USERPROFILE", other_home.path());
        }
        let second = super::oauth_server_lock_path("server", "https://example.test")?;
        #[cfg(unix)]
        {
            fs::set_permissions(&first_home, fs::Permissions::from_mode(0o700))?;
            fs::set_permissions(other_home.path(), fs::Permissions::from_mode(0o700))?;
        }

        assert_eq!(first, second);
        Ok(())
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
            actual_response.extra_fields().0,
            expected_response.extra_fields().0
        );
        assert_eq!(
            actual_response.expires_in().is_some(),
            expected_response.expires_in().is_some()
        );
    }

    fn wait_for_paths(paths: &[PathBuf], timeout: Duration) -> Result<()> {
        let deadline = Instant::now() + timeout;
        while !paths.iter().all(|path| path.exists()) {
            if Instant::now() >= deadline {
                anyhow::bail!("timed out waiting for fallback writer readiness");
            }
            thread::sleep(Duration::from_millis(10));
        }
        Ok(())
    }

    fn wait_for_child_success(child: &mut Child, timeout: Duration) -> Result<()> {
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(status) = child.try_wait()? {
                anyhow::ensure!(status.success(), "fallback writer failed: {status}");
                return Ok(());
            }
            if Instant::now() >= deadline {
                anyhow::bail!("timed out waiting for fallback writer");
            }
            thread::sleep(Duration::from_millis(10));
        }
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
