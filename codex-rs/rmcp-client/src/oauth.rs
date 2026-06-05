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
use anyhow::anyhow;
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
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;
use tracing::warn;

use codex_keyring_store::DefaultKeyringStore;
use codex_keyring_store::KeyringStore;
use rmcp::transport::auth::AuthError;
use rmcp::transport::auth::AuthorizationManager;
use rmcp::transport::auth::CredentialStore;
use rmcp::transport::auth::InMemoryCredentialStore;
use rmcp::transport::auth::StoredCredentials;
use tokio::sync::Mutex;

use codex_utils_home_dir::find_codex_home;

const KEYRING_SERVICE: &str = "Codex MCP Credentials";
const FALLBACK_LOCK_PREFIX: &str = "codex-mcp-oauth-fallback";
const FILE_OAUTH_REFRESH_LOCK_PREFIX: &str = "codex-mcp-oauth-refresh-file";
const KEYRING_OAUTH_REFRESH_LOCK_PREFIX: &str = "codex-mcp-oauth-refresh";
const MISSING_REFRESH_TOKEN_ERROR: &str = "No refresh token available";
const OAUTH_SERVER_ERROR_PREFIX: &str = "Server returned error response: ";
const REFRESH_SKEW_MILLIS: u64 = 30_000;
pub const OAUTH_REFRESH_REAUTHENTICATION_REQUIRED_ERROR: &str =
    "OAuth refresh token was rejected; reauthentication required";

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
    credential_store: InMemoryCredentialStore,
    store_mode: OAuthCredentialsStoreMode,
    last_credentials: Mutex<Option<StoredOAuthTokens>>,
}

impl OAuthPersistor {
    pub(crate) fn new(
        server_name: String,
        url: String,
        authorization_manager: Arc<Mutex<AuthorizationManager>>,
        credential_store: InMemoryCredentialStore,
        store_mode: OAuthCredentialsStoreMode,
        initial_credentials: Option<StoredOAuthTokens>,
    ) -> Self {
        Self {
            inner: Arc::new(OAuthPersistorInner {
                server_name,
                url,
                authorization_manager,
                credential_store,
                store_mode,
                last_credentials: Mutex::new(initial_credentials),
            }),
        }
    }

    /// Persists the latest stored credentials if they have changed.
    /// Deletes the credentials if they are no longer present.
    #[expect(
        clippy::await_holding_invalid_type,
        reason = "AuthorizationManager async access must be serialized through its mutex"
    )]
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

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "AuthorizationManager async access must be serialized through its mutex"
    )]
    pub(crate) async fn refresh_if_needed(&self) -> Result<()> {
        let initial_expires_at = {
            let guard = self.inner.last_credentials.lock().await;
            guard.as_ref().and_then(|tokens| tokens.expires_at)
        };

        if !token_needs_refresh(initial_expires_at) {
            return Ok(());
        }

        // A different process may have rotated the one-time refresh token.
        // Reload its durable result while all refreshers share the same lock.
        let (_refresh_locks, stored) = lock_and_load_oauth_tokens(
            &self.inner.server_name,
            &self.inner.url,
            self.inner.store_mode,
        )
        .await?;

        if let Some(stored) = stored {
            let credentials_changed = {
                let last_credentials = self.inner.last_credentials.lock().await;
                last_credentials.as_ref() != Some(&stored)
            };
            if credentials_changed {
                let token_response = stored.token_response.0.clone();
                let granted_scopes = token_response
                    .scopes()
                    .map(|scopes| {
                        scopes
                            .iter()
                            .map(|scope| scope.as_ref().to_string())
                            .collect()
                    })
                    .unwrap_or_default();
                let token_received_at = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                self.inner
                    .credential_store
                    .save(StoredCredentials::new(
                        stored.client_id.clone(),
                        Some(token_response),
                        granted_scopes,
                        Some(token_received_at),
                    ))
                    .await
                    .context("failed to reload persisted OAuth credentials")?;
                *self.inner.last_credentials.lock().await = Some(stored);
            }
        }

        let reloaded_expires_at = {
            let guard = self.inner.last_credentials.lock().await;
            guard.as_ref().and_then(|tokens| tokens.expires_at)
        };
        if !token_needs_refresh(reloaded_expires_at) {
            return Ok(());
        }

        {
            let manager = self.inner.authorization_manager.clone();
            let guard = manager.lock().await;
            match guard.refresh_token().await {
                Ok(_) => {}
                Err(error) if refresh_requires_reauthentication(&error) => {
                    return Err(anyhow!(
                        "Auth required: {OAUTH_REFRESH_REAUTHENTICATION_REQUIRED_ERROR}"
                    ));
                }
                Err(error) => {
                    return Err(error).with_context(|| {
                        format!(
                            "failed to refresh OAuth tokens for server {}",
                            self.inner.server_name
                        )
                    });
                }
            }
        }

        self.persist_if_needed().await
    }
}

async fn lock_and_load_oauth_tokens(
    server_name: &str,
    url: &str,
    store_mode: OAuthCredentialsStoreMode,
) -> Result<(Vec<fs::File>, Option<StoredOAuthTokens>)> {
    let keyring_store = DefaultKeyringStore;
    lock_and_load_oauth_tokens_with_keyring(&keyring_store, server_name, url, store_mode).await
}

async fn lock_and_load_oauth_tokens_with_keyring<K: KeyringStore>(
    keyring_store: &K,
    server_name: &str,
    url: &str,
    store_mode: OAuthCredentialsStoreMode,
) -> Result<(Vec<fs::File>, Option<StoredOAuthTokens>)> {
    match store_mode {
        OAuthCredentialsStoreMode::Auto => {
            let keyring_lock =
                acquire_oauth_refresh_lock(&keyring_oauth_refresh_lock_path(server_name, url)?)
                    .await?;
            match load_oauth_tokens_from_keyring(keyring_store, server_name, url) {
                Ok(Some(tokens)) => Ok((vec![keyring_lock], Some(tokens))),
                Ok(None) => {
                    let file_lock = acquire_oauth_refresh_lock(&file_oauth_refresh_lock_path(
                        server_name,
                        url,
                    )?)
                    .await?;
                    let tokens = load_oauth_tokens_from_file(server_name, url)?;
                    Ok((vec![keyring_lock, file_lock], tokens))
                }
                Err(error) => {
                    warn!("failed to read OAuth tokens from keyring: {error}");
                    let file_lock = acquire_oauth_refresh_lock(&file_oauth_refresh_lock_path(
                        server_name,
                        url,
                    )?)
                    .await?;
                    let tokens =
                        load_oauth_tokens_from_file(server_name, url).with_context(|| {
                            format!("failed to read OAuth tokens from keyring: {error}")
                        })?;
                    Ok((vec![keyring_lock, file_lock], tokens))
                }
            }
        }
        OAuthCredentialsStoreMode::File => {
            let lock = acquire_oauth_refresh_lock(&file_oauth_refresh_lock_path(server_name, url)?)
                .await?;
            let tokens = load_oauth_tokens_from_file(server_name, url)?;
            Ok((vec![lock], tokens))
        }
        OAuthCredentialsStoreMode::Keyring => {
            let lock =
                acquire_oauth_refresh_lock(&keyring_oauth_refresh_lock_path(server_name, url)?)
                    .await?;
            let tokens = load_oauth_tokens_from_keyring(keyring_store, server_name, url)
                .with_context(|| "failed to read OAuth tokens from keyring".to_string())?;
            Ok((vec![lock], tokens))
        }
    }
}

fn oauth_refresh_lock_id(server_name: &str, url: &str) -> Result<String> {
    let account = compute_store_key(server_name, url)?;
    sha_256_prefix(&Value::String(format!("{KEYRING_SERVICE}:{account}")))
}

fn file_oauth_refresh_lock_path(server_name: &str, url: &str) -> Result<PathBuf> {
    let lock_id = oauth_refresh_lock_id(server_name, url)?;
    external_oauth_lock_path(FILE_OAUTH_REFRESH_LOCK_PREFIX, &lock_id)
}

fn keyring_oauth_refresh_lock_path(server_name: &str, url: &str) -> Result<PathBuf> {
    let lock_id = oauth_refresh_lock_id(server_name, url)?;
    external_oauth_lock_path(KEYRING_OAUTH_REFRESH_LOCK_PREFIX, &lock_id)
}

fn external_oauth_lock_path(prefix: &str, lock_id: &str) -> Result<PathBuf> {
    let user_namespace = os_user_namespace()?;
    Ok(os_shared_temp_dir()?.join(format!("{prefix}-{user_namespace}-{lock_id}.lock")))
}

#[cfg(unix)]
fn os_shared_temp_dir() -> Result<PathBuf> {
    Ok(PathBuf::from("/tmp"))
}

#[cfg(unix)]
fn os_user_namespace() -> Result<String> {
    // SAFETY: getuid has no preconditions and returns the real UID of this process.
    Ok(format!("uid-{}", unsafe { libc::getuid() }))
}

#[cfg(windows)]
fn os_shared_temp_dir() -> Result<PathBuf> {
    use std::ffi::OsString;
    use std::io;
    use std::os::windows::ffi::OsStringExt;
    use windows_sys::Win32::System::SystemInformation::GetSystemWindowsDirectoryW;

    let mut buffer = vec![0_u16; 32_768];
    // SAFETY: buffer is writable for the length passed to GetSystemWindowsDirectoryW.
    let length = unsafe {
        GetSystemWindowsDirectoryW(
            buffer.as_mut_ptr(),
            u32::try_from(buffer.len()).expect("Windows path buffer length fits in u32"),
        )
    };
    if length == 0 {
        return Err(io::Error::last_os_error())
            .context("failed to resolve the system Windows directory");
    }
    let length = usize::try_from(length).context("Windows directory length did not fit usize")?;
    if length >= buffer.len() {
        return Err(anyhow!(
            "system Windows directory exceeded the fixed path buffer"
        ));
    }

    Ok(PathBuf::from(OsString::from_wide(&buffer[..length])).join("Temp"))
}

#[cfg(windows)]
fn os_user_namespace() -> Result<String> {
    use std::io;
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::Security::GetLengthSid;
    use windows_sys::Win32::Security::GetTokenInformation;
    use windows_sys::Win32::Security::TOKEN_QUERY;
    use windows_sys::Win32::Security::TOKEN_USER;
    use windows_sys::Win32::Security::TokenUser;
    use windows_sys::Win32::System::Threading::GetCurrentProcess;
    use windows_sys::Win32::System::Threading::OpenProcessToken;

    struct OwnedHandle(HANDLE);

    impl Drop for OwnedHandle {
        fn drop(&mut self) {
            if self.0 != 0 {
                // SAFETY: the handle was returned by OpenProcessToken and is owned here.
                unsafe {
                    CloseHandle(self.0);
                }
            }
        }
    }

    let mut token = 0;
    // SAFETY: token points to writable storage for the returned process-token handle.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
        return Err(io::Error::last_os_error()).context("failed to open the current process token");
    }
    let token = OwnedHandle(token);

    let mut required = 0;
    // SAFETY: the null-buffer call obtains the required TOKEN_USER buffer length.
    unsafe {
        GetTokenInformation(token.0, TokenUser, std::ptr::null_mut(), 0, &mut required);
    }
    if required == 0 {
        return Err(io::Error::last_os_error())
            .context("failed to size the current process user SID");
    }

    let mut buffer = vec![0_u8; required as usize];
    // SAFETY: buffer is writable for required bytes and receives a TOKEN_USER value.
    if unsafe {
        GetTokenInformation(
            token.0,
            TokenUser,
            buffer.as_mut_ptr().cast(),
            required,
            &mut required,
        )
    } == 0
    {
        return Err(io::Error::last_os_error())
            .context("failed to read the current process user SID");
    }

    // SAFETY: GetTokenInformation initialized the buffer with TOKEN_USER. The
    // unaligned read avoids assuming Vec<u8> has TOKEN_USER alignment.
    let token_user = unsafe { std::ptr::read_unaligned(buffer.as_ptr().cast::<TOKEN_USER>()) };
    // SAFETY: token_user.User.Sid points into buffer and remains valid here.
    let sid_length = unsafe { GetLengthSid(token_user.User.Sid) };
    if sid_length == 0 {
        return Err(io::Error::last_os_error()).context("failed to size the current user SID");
    }
    // SAFETY: GetLengthSid returned the byte length of the valid SID in buffer.
    let sid = unsafe {
        std::slice::from_raw_parts(token_user.User.Sid.cast::<u8>(), sid_length as usize)
    };
    Ok(format!("sid-{}", sha_256_bytes_prefix(sid)))
}

#[cfg(not(any(unix, windows)))]
fn os_shared_temp_dir() -> Result<PathBuf> {
    Err(anyhow!(
        "keyring OAuth refresh locking is unsupported on this platform"
    ))
}

#[cfg(not(any(unix, windows)))]
fn os_user_namespace() -> Result<String> {
    Err(anyhow!(
        "keyring OAuth refresh locking is unsupported on this platform"
    ))
}

async fn acquire_oauth_refresh_lock(path: &Path) -> Result<fs::File> {
    let file = open_oauth_lock_file(path)?;
    loop {
        match file.try_lock() {
            Ok(()) => return Ok(file),
            Err(fs::TryLockError::WouldBlock) => {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("failed to acquire OAuth refresh lock at {}", path.display())
                });
            }
        }
    }
}

fn acquire_fallback_write_lock() -> Result<fs::File> {
    let path = fallback_lock_path()?;
    let file = open_oauth_lock_file(&path)?;
    file.lock().with_context(|| {
        format!(
            "failed to acquire OAuth fallback write lock at {}",
            path.display()
        )
    })?;
    Ok(file)
}

fn acquire_fallback_read_lock() -> Result<fs::File> {
    let path = fallback_lock_path()?;
    let file = open_oauth_lock_file(&path)?;
    file.lock_shared().with_context(|| {
        format!(
            "failed to acquire OAuth fallback read lock at {}",
            path.display()
        )
    })?;
    Ok(file)
}

fn fallback_lock_path() -> Result<PathBuf> {
    let codex_home = find_codex_home()?;
    let codex_home_id = sha_256_bytes_prefix(codex_home.as_os_str().to_string_lossy().as_bytes());
    external_oauth_lock_path(FALLBACK_LOCK_PREFIX, &codex_home_id)
}

fn open_oauth_lock_file(path: &Path) -> Result<fs::File> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options
        .open(path)
        .with_context(|| format!("failed to open OAuth lock at {}", path.display()))
}

fn refresh_requires_reauthentication(error: &AuthError) -> bool {
    match error {
        AuthError::AuthorizationRequired => true,
        AuthError::TokenRefreshFailed(message) => {
            if message == MISSING_REFRESH_TOKEN_ERROR {
                return true;
            }
            message
                .strip_prefix(OAUTH_SERVER_ERROR_PREFIX)
                .is_some_and(|response| {
                    let error_code_end = response.find([':', ' ']).unwrap_or(response.len());
                    &response[..error_code_end] == "invalid_grant"
                })
        }
        _ => false,
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
    let _read_lock = acquire_fallback_read_lock()?;
    load_oauth_tokens_from_file_unlocked(server_name, url)
}

fn load_oauth_tokens_from_file_unlocked(
    server_name: &str,
    url: &str,
) -> Result<Option<StoredOAuthTokens>> {
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
    let _write_lock = acquire_fallback_write_lock()?;
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
    let _read_lock = acquire_fallback_read_lock()?;
    read_fallback_file_unlocked()
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
    Ok(sha_256_bytes_prefix(serialized.as_bytes()))
}

fn sha_256_bytes_prefix(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let hex = format!("{digest:x}");
    let truncated = &hex[..16];
    truncated.to_string()
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

    static CODEX_HOME_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    struct TempCodexHome {
        _guard: MutexGuard<'static, ()>,
        _dir: tempfile::TempDir,
    }

    impl TempCodexHome {
        fn new() -> Self {
            let guard = CODEX_HOME_LOCK
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

        fn unusable() -> Self {
            let guard = CODEX_HOME_LOCK
                .get_or_init(Mutex::default)
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            let dir = tempdir().expect("create CODEX_HOME parent temp dir");
            let file = dir.path().join("not-a-directory");
            fs::write(&file, "not a directory").expect("create unusable CODEX_HOME path");
            unsafe {
                std::env::set_var("CODEX_HOME", file);
            }
            Self {
                _guard: guard,
                _dir: dir,
            }
        }

        fn path(&self) -> &Path {
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
    fn refresh_lock_identity_is_credential_specific_and_home_independent() -> Result<()> {
        let _env = TempCodexHome::new();
        let tokens = sample_tokens();
        let other_url = "https://other.example.test/mcp";

        let keyring_path = keyring_oauth_refresh_lock_path(&tokens.server_name, &tokens.url)?;
        let file_path = file_oauth_refresh_lock_path(&tokens.server_name, &tokens.url)?;
        let shared_temp_dir = os_shared_temp_dir()?;
        let user_namespace = os_user_namespace()?;
        assert_eq!(keyring_path.parent(), Some(shared_temp_dir.as_path()));
        assert_eq!(file_path.parent(), Some(shared_temp_dir.as_path()));
        assert!(
            keyring_path
                .file_name()
                .is_some_and(|name| name.to_string_lossy().contains(&user_namespace))
        );
        assert!(
            file_path
                .file_name()
                .is_some_and(|name| name.to_string_lossy().contains(&user_namespace))
        );
        #[cfg(unix)]
        assert_eq!(shared_temp_dir, PathBuf::from("/tmp"));
        assert_eq!(
            keyring_path,
            keyring_oauth_refresh_lock_path(&tokens.server_name, &tokens.url)?
        );
        assert_ne!(
            keyring_path,
            keyring_oauth_refresh_lock_path(&tokens.server_name, other_url)?
        );
        assert_ne!(
            file_path,
            file_oauth_refresh_lock_path(&tokens.server_name, other_url)?
        );
        Ok(())
    }

    #[tokio::test(flavor = "current_thread")]
    async fn keyring_refresh_locking_works_without_codex_home_for_auto_and_keyring() -> Result<()> {
        let _env = TempCodexHome::unusable();
        let store = MockKeyringStore::default();
        let tokens = sample_tokens();
        let serialized = serde_json::to_string(&tokens)?;
        let key = compute_store_key(&tokens.server_name, &tokens.url)?;
        store.save(KEYRING_SERVICE, &key, &serialized)?;

        let held_lock = acquire_oauth_refresh_lock(&keyring_oauth_refresh_lock_path(
            &tokens.server_name,
            &tokens.url,
        )?)
        .await?;
        let blocked = tokio::time::timeout(
            Duration::from_millis(50),
            lock_and_load_oauth_tokens_with_keyring(
                &store,
                &tokens.server_name,
                &tokens.url,
                OAuthCredentialsStoreMode::Keyring,
            ),
        )
        .await;
        assert!(
            blocked.is_err(),
            "same keyring credential should wait for its refresh lock"
        );
        drop(held_lock);

        for store_mode in [
            OAuthCredentialsStoreMode::Keyring,
            OAuthCredentialsStoreMode::Auto,
        ] {
            let (locks, loaded) = lock_and_load_oauth_tokens_with_keyring(
                &store,
                &tokens.server_name,
                &tokens.url,
                store_mode,
            )
            .await?;
            assert_eq!(locks.len(), 1);
            assert_tokens_match_without_expiry(
                &loaded.expect("keyring credentials should load"),
                &tokens,
            );
        }
        Ok(())
    }

    #[tokio::test(flavor = "current_thread")]
    async fn distinct_keyring_credentials_do_not_block_each_other() -> Result<()> {
        let _env = TempCodexHome::unusable();
        let store = MockKeyringStore::default();
        let first = sample_tokens();
        let mut second = sample_tokens();
        second.url = "https://other.example.test/mcp".to_string();
        for tokens in [&first, &second] {
            let key = compute_store_key(&tokens.server_name, &tokens.url)?;
            store.save(KEYRING_SERVICE, &key, &serde_json::to_string(tokens)?)?;
        }

        let held_lock = acquire_oauth_refresh_lock(&keyring_oauth_refresh_lock_path(
            &first.server_name,
            &first.url,
        )?)
        .await?;
        for store_mode in [
            OAuthCredentialsStoreMode::Keyring,
            OAuthCredentialsStoreMode::Auto,
        ] {
            let result = tokio::time::timeout(
                Duration::from_millis(500),
                lock_and_load_oauth_tokens_with_keyring(
                    &store,
                    &second.server_name,
                    &second.url,
                    store_mode,
                ),
            )
            .await;
            assert!(
                result.is_ok(),
                "a distinct keyring credential should not wait for another lock"
            );
            let (locks, loaded) = result.expect("timeout checked above")?;
            assert_eq!(locks.len(), 1);
            assert_tokens_match_without_expiry(
                &loaded.expect("second keyring credential should load"),
                &second,
            );
        }
        drop(held_lock);
        Ok(())
    }

    #[tokio::test(flavor = "current_thread")]
    async fn refresh_lock_loading_covers_keyring_and_auto_branches() -> Result<()> {
        let _env = TempCodexHome::new();
        let store = MockKeyringStore::default();
        let tokens = sample_tokens();
        let serialized = serde_json::to_string(&tokens)?;
        let key = compute_store_key(&tokens.server_name, &tokens.url)?;
        store.save(KEYRING_SERVICE, &key, &serialized)?;

        let (locks, loaded) = lock_and_load_oauth_tokens_with_keyring(
            &store,
            &tokens.server_name,
            &tokens.url,
            OAuthCredentialsStoreMode::Keyring,
        )
        .await?;
        assert_eq!(locks.len(), 1);
        assert_tokens_match_without_expiry(
            &loaded.expect("keyring credentials should load"),
            &tokens,
        );
        drop(locks);

        let (locks, loaded) = lock_and_load_oauth_tokens_with_keyring(
            &store,
            &tokens.server_name,
            &tokens.url,
            OAuthCredentialsStoreMode::Auto,
        )
        .await?;
        assert_eq!(locks.len(), 1);
        assert_tokens_match_without_expiry(
            &loaded.expect("auto mode should prefer keyring credentials"),
            &tokens,
        );
        drop(locks);

        store.delete(KEYRING_SERVICE, &key)?;
        save_oauth_tokens_to_file(&tokens)?;
        let (locks, loaded) = lock_and_load_oauth_tokens_with_keyring(
            &store,
            &tokens.server_name,
            &tokens.url,
            OAuthCredentialsStoreMode::Auto,
        )
        .await?;
        assert_eq!(locks.len(), 2);
        assert_tokens_match_without_expiry(
            &loaded.expect("auto mode should load fallback file credentials"),
            &tokens,
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn file_and_auto_reads_work_with_read_only_codex_home() -> Result<()> {
        use std::os::unix::fs::PermissionsExt;

        let env = TempCodexHome::new();
        let store = MockKeyringStore::default();
        let tokens = sample_tokens();
        save_oauth_tokens_to_file(&tokens)?;

        let credentials_path = fallback_file_path()?;
        fs::set_permissions(&credentials_path, fs::Permissions::from_mode(0o400))?;
        fs::set_permissions(env.path(), fs::Permissions::from_mode(0o500))?;

        let result = (|| -> Result<(StoredOAuthTokens, StoredOAuthTokens)> {
            let file_tokens = load_oauth_tokens_from_file(&tokens.server_name, &tokens.url)?
                .expect("file credentials should load");
            let auto_tokens = load_oauth_tokens_from_keyring_with_fallback_to_file(
                &store,
                &tokens.server_name,
                &tokens.url,
            )?
            .expect("auto credentials should load from fallback");
            Ok((file_tokens, auto_tokens))
        })();

        fs::set_permissions(env.path(), fs::Permissions::from_mode(0o700))?;
        fs::set_permissions(&credentials_path, fs::Permissions::from_mode(0o600))?;

        let (file_tokens, auto_tokens) = result?;
        assert_tokens_match_without_expiry(&file_tokens, &tokens);
        assert_tokens_match_without_expiry(&auto_tokens, &tokens);
        assert!(!env.path().join(".credentials.json.lock").exists());
        assert_eq!(
            fallback_lock_path()?.parent(),
            Some(os_shared_temp_dir()?.as_path())
        );
        Ok(())
    }

    #[test]
    fn refresh_reauthentication_classification_is_narrow() {
        assert!(refresh_requires_reauthentication(
            &AuthError::AuthorizationRequired
        ));
        assert!(refresh_requires_reauthentication(
            &AuthError::TokenRefreshFailed(MISSING_REFRESH_TOKEN_ERROR.to_string())
        ));
        assert!(refresh_requires_reauthentication(
            &AuthError::TokenRefreshFailed(
                "Server returned error response: invalid_grant: revoked".to_string()
            )
        ));
        assert!(!refresh_requires_reauthentication(
            &AuthError::TokenRefreshFailed(
                "Server returned error response: invalid_request: invalid_grant in description"
                    .to_string()
            )
        ));
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
