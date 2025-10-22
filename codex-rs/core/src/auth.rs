use chrono::DateTime;
use chrono::Utc;
use keyring::Entry;
use serde::Deserialize;
use serde::Serialize;
#[cfg(test)]
use serial_test::serial;
use sha2::Digest;
use sha2::Sha256;
use std::env;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::Read;
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use tracing::warn;

use codex_app_server_protocol::AuthMode;
use codex_protocol::config_types::ForcedLoginMethod;

use crate::config::Config;
use crate::token_data::PlanType;
use crate::token_data::TokenData;
use crate::token_data::parse_id_token;

/// Determine where Codex should store CLI auth credentials.
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuthCredentialsStoreMode {
    #[default]
    /// Persist credentials in CODEX_HOME/auth.json.
    File,
    /// Use keyring when available; otherwise, fall back to a file in CODEX_HOME.
    Auto,
    /// Persist credentials in the keyring. Fail if unavailable.
    Keyring,
}

const KEYRING_SERVICE: &str = "Codex CLI Auth";

trait KeyringStore: Send + Sync {
    fn load(&self, service: &str, account: &str) -> Result<Option<String>, keyring::Error>;
    fn save(&self, service: &str, account: &str, value: &str) -> Result<(), keyring::Error>;
    fn delete(&self, service: &str, account: &str) -> Result<bool, keyring::Error>;
}

#[derive(Default)]
struct DefaultKeyringStore;

impl KeyringStore for DefaultKeyringStore {
    fn load(&self, service: &str, account: &str) -> Result<Option<String>, keyring::Error> {
        let entry = Entry::new(service, account)?;
        match entry.get_password() {
            Ok(password) => Ok(Some(password)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(error),
        }
    }

    fn save(&self, service: &str, account: &str, value: &str) -> Result<(), keyring::Error> {
        let entry = Entry::new(service, account)?;
        entry.set_password(value)
    }

    fn delete(&self, service: &str, account: &str) -> Result<bool, keyring::Error> {
        let entry = Entry::new(service, account)?;
        match entry.delete_credential() {
            Ok(()) => Ok(true),
            Err(keyring::Error::NoEntry) => Ok(false),
            Err(error) => Err(error),
        }
    }
}

fn compute_store_key(codex_home: &Path) -> std::io::Result<String> {
    let canonical = codex_home
        .canonicalize()
        .unwrap_or_else(|_| codex_home.to_path_buf());
    let path_str = canonical.to_string_lossy();
    let mut hasher = Sha256::new();
    hasher.update(path_str.as_bytes());
    let digest = hasher.finalize();
    let hex = format!("{digest:x}");
    let truncated = hex.get(..16).unwrap_or(&hex);
    Ok(format!("cli|{truncated}"))
}

#[derive(Clone)]
struct AuthStorage {
    codex_home: PathBuf,
    mode: AuthCredentialsStoreMode,
    keyring_store: Arc<dyn KeyringStore>,
}

impl AuthStorage {
    fn new(codex_home: PathBuf, mode: AuthCredentialsStoreMode) -> Self {
        let store: Arc<dyn KeyringStore> = Arc::new(DefaultKeyringStore);
        Self::with_keyring_store(codex_home, mode, store)
    }

    fn with_keyring_store(
        codex_home: PathBuf,
        mode: AuthCredentialsStoreMode,
        keyring_store: Arc<dyn KeyringStore>,
    ) -> Self {
        Self {
            codex_home,
            mode,
            keyring_store,
        }
    }

    fn load(&self) -> std::io::Result<Option<AuthDotJson>> {
        let key = compute_store_key(&self.codex_home)?;
        match self.mode {
            AuthCredentialsStoreMode::File => self.load_from_file(),
            AuthCredentialsStoreMode::Keyring => {
                self.load_from_keyring(&key).map_err(std::io::Error::other)
            }
            AuthCredentialsStoreMode::Auto => match self.load_from_keyring(&key) {
                Ok(Some(auth)) => Ok(Some(auth)),
                Ok(None) => self.load_from_file(),
                Err(message) => {
                    warn!("{message}");
                    self.load_from_file()
                }
            },
        }
    }

    fn save(&self, auth: &AuthDotJson) -> std::io::Result<()> {
        let key = compute_store_key(&self.codex_home)?;

        let serialized = serde_json::to_string(auth)
            .map_err(|err| std::io::Error::other(format!("failed to serialize auth: {err}")))?;

        match self.mode {
            AuthCredentialsStoreMode::File => self.save_to_file(auth),
            AuthCredentialsStoreMode::Keyring => {
                self.save_to_keyring(&key, &serialized)
                    .map_err(std::io::Error::other)?;
                if let Err(err) = self.delete_file_if_exists() {
                    warn!("failed to remove CLI auth fallback file: {err}");
                }
                Ok(())
            }
            AuthCredentialsStoreMode::Auto => match self.save_to_keyring(&key, &serialized) {
                Ok(()) => {
                    if let Err(err) = self.delete_file_if_exists() {
                        warn!("failed to remove CLI auth fallback file: {err}");
                    }
                    Ok(())
                }
                Err(message) => {
                    warn!("{message}");
                    self.save_to_file(auth)
                }
            },
        }
    }

    fn delete(&self) -> std::io::Result<bool> {
        let key = compute_store_key(&self.codex_home)?;
        let keyring_removed = match self.keyring_store.delete(KEYRING_SERVICE, &key) {
            Ok(removed) => removed,
            Err(keyring::Error::NoEntry) => false,
            Err(err) => {
                if matches!(
                    self.mode,
                    AuthCredentialsStoreMode::Auto | AuthCredentialsStoreMode::Keyring
                ) {
                    return Err(std::io::Error::other(format!(
                        "failed to delete auth from keyring: {err}"
                    )));
                }
                warn!("failed to delete auth from keyring: {err}");
                false
            }
        };

        let file_removed = match self.delete_file_if_exists() {
            Ok(removed) => removed,
            Err(err) => {
                return Err(err);
            }
        };

        Ok(keyring_removed || file_removed)
    }

    fn load_from_keyring(&self, key: &str) -> Result<Option<AuthDotJson>, String> {
        match self.keyring_store.load(KEYRING_SERVICE, key) {
            Ok(Some(serialized)) => serde_json::from_str(&serialized)
                .map(Some)
                .map_err(|err| format!("failed to deserialize CLI auth from keyring: {err}")),
            Ok(None) => Ok(None),
            Err(err) => Err(format!("failed to read CLI auth from keyring: {err}")),
        }
    }

    fn load_from_file(&self) -> std::io::Result<Option<AuthDotJson>> {
        let auth_file = get_auth_file(&self.codex_home);
        let auth_dot_json = match try_read_auth_json(&auth_file) {
            Ok(auth) => auth,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(err),
        };
        Ok(Some(auth_dot_json))
    }

    fn save_to_file(&self, auth: &AuthDotJson) -> std::io::Result<()> {
        write_auth_json(&get_auth_file(&self.codex_home), auth)
    }

    fn save_to_keyring(&self, key: &str, value: &str) -> Result<(), String> {
        self.keyring_store
            .save(KEYRING_SERVICE, key, value)
            .map_err(|err| format!("failed to write CLI auth to keyring: {err}"))
    }

    fn delete_file_if_exists(&self) -> std::io::Result<bool> {
        let auth_file = get_auth_file(&self.codex_home);
        match std::fs::remove_file(&auth_file) {
            Ok(()) => Ok(true),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(err) => Err(err),
        }
    }
}

impl std::fmt::Debug for AuthStorage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthStorage")
            .field("codex_home", &self.codex_home)
            .field("mode", &self.mode)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone)]
pub struct CodexAuth {
    pub mode: AuthMode,

    pub(crate) api_key: Option<String>,
    pub(crate) auth_dot_json: Arc<Mutex<Option<AuthDotJson>>>,
    storage: Arc<AuthStorage>,
    pub(crate) client: reqwest::Client,
}

impl PartialEq for CodexAuth {
    fn eq(&self, other: &Self) -> bool {
        self.mode == other.mode
    }
}

impl CodexAuth {
    pub async fn refresh_token(&self) -> Result<String, std::io::Error> {
        let token_data = self
            .get_current_token_data()
            .ok_or(std::io::Error::other("Token data is not available."))?;
        let token = token_data.refresh_token;

        let refresh_response = try_refresh_token(token, &self.client)
            .await
            .map_err(std::io::Error::other)?;

        let updated = update_tokens(
            &self.storage,
            refresh_response.id_token,
            refresh_response.access_token,
            refresh_response.refresh_token,
        )
        .await?;

        if let Ok(mut auth_lock) = self.auth_dot_json.lock() {
            *auth_lock = Some(updated.clone());
        }

        let access = match updated.tokens {
            Some(t) => t.access_token,
            None => {
                return Err(std::io::Error::other(
                    "Token data is not available after refresh.",
                ));
            }
        };
        Ok(access)
    }

    /// Loads the available auth information using an explicit credential store mode.
    pub fn from_codex_home(
        codex_home: &Path,
        store_mode: AuthCredentialsStoreMode,
    ) -> std::io::Result<Option<CodexAuth>> {
        load_auth(codex_home, false, store_mode)
    }

    pub async fn get_token_data(&self) -> Result<TokenData, std::io::Error> {
        let auth_dot_json: Option<AuthDotJson> = self.get_current_auth_json();
        match auth_dot_json {
            Some(AuthDotJson {
                tokens: Some(mut tokens),
                last_refresh: Some(last_refresh),
                ..
            }) => {
                if last_refresh < Utc::now() - chrono::Duration::days(28) {
                    let refresh_response = tokio::time::timeout(
                        Duration::from_secs(60),
                        try_refresh_token(tokens.refresh_token.clone(), &self.client),
                    )
                    .await
                    .map_err(|_| {
                        std::io::Error::other("timed out while refreshing OpenAI API key")
                    })?
                    .map_err(std::io::Error::other)?;

                    let updated_auth_dot_json = update_tokens(
                        &self.storage,
                        refresh_response.id_token,
                        refresh_response.access_token,
                        refresh_response.refresh_token,
                    )
                    .await?;

                    tokens = updated_auth_dot_json
                        .tokens
                        .clone()
                        .ok_or(std::io::Error::other(
                            "Token data is not available after refresh.",
                        ))?;

                    #[expect(clippy::unwrap_used)]
                    let mut auth_lock = self.auth_dot_json.lock().unwrap();
                    *auth_lock = Some(updated_auth_dot_json);
                }

                Ok(tokens)
            }
            _ => Err(std::io::Error::other("Token data is not available.")),
        }
    }

    pub async fn get_token(&self) -> Result<String, std::io::Error> {
        match self.mode {
            AuthMode::ApiKey => Ok(self.api_key.clone().unwrap_or_default()),
            AuthMode::ChatGPT => {
                let id_token = self.get_token_data().await?.access_token;
                Ok(id_token)
            }
        }
    }

    pub fn get_account_id(&self) -> Option<String> {
        self.get_current_token_data().and_then(|t| t.account_id)
    }

    pub fn get_account_email(&self) -> Option<String> {
        self.get_current_token_data().and_then(|t| t.id_token.email)
    }

    pub(crate) fn get_plan_type(&self) -> Option<PlanType> {
        self.get_current_token_data()
            .and_then(|t| t.id_token.chatgpt_plan_type)
    }

    fn get_current_auth_json(&self) -> Option<AuthDotJson> {
        #[expect(clippy::unwrap_used)]
        self.auth_dot_json.lock().unwrap().clone()
    }

    fn get_current_token_data(&self) -> Option<TokenData> {
        self.get_current_auth_json().and_then(|t| t.tokens)
    }

    /// Consider this private to integration tests.
    pub fn create_dummy_chatgpt_auth_for_testing() -> Self {
        let auth_dot_json = AuthDotJson {
            openai_api_key: None,
            tokens: Some(TokenData {
                id_token: Default::default(),
                access_token: "Access Token".to_string(),
                refresh_token: "test".to_string(),
                account_id: Some("account_id".to_string()),
            }),
            last_refresh: Some(Utc::now()),
        };

        let auth_dot_json = Arc::new(Mutex::new(Some(auth_dot_json)));
        Self {
            api_key: None,
            mode: AuthMode::ChatGPT,
            auth_dot_json,
            storage: Arc::new(AuthStorage::new(
                PathBuf::new(),
                AuthCredentialsStoreMode::File,
            )),
            client: crate::default_client::create_client(),
        }
    }

    fn from_api_key_with_client(api_key: &str, client: reqwest::Client) -> Self {
        Self {
            api_key: Some(api_key.to_owned()),
            mode: AuthMode::ApiKey,
            auth_dot_json: Arc::new(Mutex::new(None)),
            storage: Arc::new(AuthStorage::new(
                PathBuf::new(),
                AuthCredentialsStoreMode::File,
            )),
            client,
        }
    }

    pub fn from_api_key(api_key: &str) -> Self {
        Self::from_api_key_with_client(api_key, crate::default_client::create_client())
    }
}

pub const OPENAI_API_KEY_ENV_VAR: &str = "OPENAI_API_KEY";
pub const CODEX_API_KEY_ENV_VAR: &str = "CODEX_API_KEY";

pub fn read_openai_api_key_from_env() -> Option<String> {
    env::var(OPENAI_API_KEY_ENV_VAR)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub fn read_codex_api_key_from_env() -> Option<String> {
    env::var(CODEX_API_KEY_ENV_VAR)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub fn get_auth_file(codex_home: &Path) -> PathBuf {
    codex_home.join("auth.json")
}

/// Delete stored credentials using the specified backend.
pub fn logout(codex_home: &Path, store_mode: AuthCredentialsStoreMode) -> std::io::Result<bool> {
    let storage = AuthStorage::new(codex_home.to_path_buf(), store_mode);
    storage.delete()
}

/// Writes auth credentials using the specified credential store backend.
pub fn login_with_api_key(
    codex_home: &Path,
    api_key: &str,
    store_mode: AuthCredentialsStoreMode,
) -> std::io::Result<()> {
    let auth_dot_json = AuthDotJson {
        openai_api_key: Some(api_key.to_string()),
        tokens: None,
        last_refresh: None,
    };
    let storage = AuthStorage::new(codex_home.to_path_buf(), store_mode);
    storage.save(&auth_dot_json)
}

/// Persist the provided auth payload using the specified backend.
pub fn save_auth(
    codex_home: &Path,
    auth: &AuthDotJson,
    store_mode: AuthCredentialsStoreMode,
) -> std::io::Result<()> {
    let storage = AuthStorage::new(codex_home.to_path_buf(), store_mode);
    storage.save(auth)
}

pub async fn enforce_login_restrictions(config: &Config) -> std::io::Result<()> {
    let Some(auth) = load_auth(&config.codex_home, true)? else {
        return Ok(());
    };

    if let Some(required_method) = config.forced_login_method {
        let method_violation = match (required_method, auth.mode) {
            (ForcedLoginMethod::Api, AuthMode::ApiKey) => None,
            (ForcedLoginMethod::Chatgpt, AuthMode::ChatGPT) => None,
            (ForcedLoginMethod::Api, AuthMode::ChatGPT) => Some(
                "API key login is required, but ChatGPT is currently being used. Logging out."
                    .to_string(),
            ),
            (ForcedLoginMethod::Chatgpt, AuthMode::ApiKey) => Some(
                "ChatGPT login is required, but an API key is currently being used. Logging out."
                    .to_string(),
            ),
        };

        if let Some(message) = method_violation {
            return logout_with_message(&config.codex_home, message);
        }
    }

    if let Some(expected_account_id) = config.forced_chatgpt_workspace_id.as_deref() {
        if auth.mode != AuthMode::ChatGPT {
            return Ok(());
        }

        let token_data = match auth.get_token_data().await {
            Ok(data) => data,
            Err(err) => {
                return logout_with_message(
                    &config.codex_home,
                    format!(
                        "Failed to load ChatGPT credentials while enforcing workspace restrictions: {err}. Logging out."
                    ),
                );
            }
        };

        // workspace is the external identifier for account id.
        let chatgpt_account_id = token_data.id_token.chatgpt_account_id.as_deref();
        if chatgpt_account_id != Some(expected_account_id) {
            let message = match chatgpt_account_id {
                Some(actual) => format!(
                    "Login is restricted to workspace {expected_account_id}, but current credentials belong to {actual}. Logging out."
                ),
                None => format!(
                    "Login is restricted to workspace {expected_account_id}, but current credentials lack a workspace identifier. Logging out."
                ),
            };
            return logout_with_message(&config.codex_home, message);
        }
    }

    Ok(())
}

fn logout_with_message(codex_home: &Path, message: String) -> std::io::Result<()> {
    match logout(codex_home) {
        Ok(_) => Err(std::io::Error::other(message)),
        Err(err) => Err(std::io::Error::other(format!(
            "{message}. Failed to remove auth.json: {err}"
        ))),
    }
}

fn load_auth(
    codex_home: &Path,
    enable_codex_api_key_env: bool,
    store_mode: AuthCredentialsStoreMode,
) -> std::io::Result<Option<CodexAuth>> {
    if enable_codex_api_key_env && let Some(api_key) = read_codex_api_key_from_env() {
        let client = crate::default_client::create_client();
        return Ok(Some(CodexAuth::from_api_key_with_client(
            api_key.as_str(),
            client,
        )));
    }

    let storage = AuthStorage::new(codex_home.to_path_buf(), store_mode);
    let auth = match storage.load()? {
        Some(auth) => auth,
        None => return Ok(None),
    };

    let client = crate::default_client::create_client();

    let AuthDotJson {
        openai_api_key: auth_json_api_key,
        tokens,
        last_refresh,
    } = auth;

    // Prefer AuthMode.ApiKey if it's set in the auth.json.
    if let Some(api_key) = &auth_json_api_key {
        return Ok(Some(CodexAuth::from_api_key_with_client(api_key, client)));
    }

    Ok(Some(CodexAuth {
        api_key: None,
        mode: AuthMode::ChatGPT,
        storage: Arc::new(storage),
        auth_dot_json: Arc::new(Mutex::new(Some(AuthDotJson {
            openai_api_key: None,
            tokens,
            last_refresh,
        }))),
        client,
    }))
}

/// Attempt to read and refresh the `auth.json` file in the given `CODEX_HOME` directory.
/// Returns the full AuthDotJson structure after refreshing if necessary.
pub fn try_read_auth_json(auth_file: &Path) -> std::io::Result<AuthDotJson> {
    let mut file = File::open(auth_file)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    let auth_dot_json: AuthDotJson = serde_json::from_str(&contents)?;

    Ok(auth_dot_json)
}

pub fn write_auth_json(auth_file: &Path, auth_dot_json: &AuthDotJson) -> std::io::Result<()> {
    if let Some(parent) = auth_file.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json_data = serde_json::to_string_pretty(auth_dot_json)?;
    let mut options = OpenOptions::new();
    options.truncate(true).write(true).create(true);
    #[cfg(unix)]
    {
        options.mode(0o600);
    }
    let mut file = options.open(auth_file)?;
    file.write_all(json_data.as_bytes())?;
    file.flush()?;
    Ok(())
}

async fn update_tokens(
    storage: &AuthStorage,
    id_token: String,
    access_token: Option<String>,
    refresh_token: Option<String>,
) -> std::io::Result<AuthDotJson> {
    let mut auth_dot_json = storage
        .load()?
        .ok_or(std::io::Error::other("Token data is not available."))?;

    let tokens = auth_dot_json.tokens.get_or_insert_with(TokenData::default);
    tokens.id_token = parse_id_token(&id_token).map_err(std::io::Error::other)?;
    if let Some(access_token) = access_token {
        tokens.access_token = access_token;
    }
    if let Some(refresh_token) = refresh_token {
        tokens.refresh_token = refresh_token;
    }
    auth_dot_json.last_refresh = Some(Utc::now());
    storage.save(&auth_dot_json)?;
    Ok(auth_dot_json)
}

async fn try_refresh_token(
    refresh_token: String,
    client: &reqwest::Client,
) -> std::io::Result<RefreshResponse> {
    let refresh_request = RefreshRequest {
        client_id: CLIENT_ID,
        grant_type: "refresh_token",
        refresh_token,
        scope: "openid profile email",
    };

    // Use shared client factory to include standard headers
    let response = client
        .post("https://auth.openai.com/oauth/token")
        .header("Content-Type", "application/json")
        .json(&refresh_request)
        .send()
        .await
        .map_err(std::io::Error::other)?;

    if response.status().is_success() {
        let refresh_response = response
            .json::<RefreshResponse>()
            .await
            .map_err(std::io::Error::other)?;
        Ok(refresh_response)
    } else {
        Err(std::io::Error::other(format!(
            "Failed to refresh token: {}",
            response.status()
        )))
    }
}

#[derive(Serialize)]
struct RefreshRequest {
    client_id: &'static str,
    grant_type: &'static str,
    refresh_token: String,
    scope: &'static str,
}

#[derive(Deserialize, Clone)]
struct RefreshResponse {
    id_token: String,
    access_token: Option<String>,
    refresh_token: Option<String>,
}

/// Expected structure for $CODEX_HOME/auth.json.
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct AuthDotJson {
    #[serde(rename = "OPENAI_API_KEY")]
    pub openai_api_key: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens: Option<TokenData>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_refresh: Option<DateTime<Utc>>,
}

// Shared constant for token refresh (client id used for oauth token refresh flow)
pub const CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";

use std::sync::RwLock;

/// Internal cached auth state.
#[derive(Clone, Debug)]
struct CachedAuth {
    auth: Option<CodexAuth>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::config::ConfigOverrides;
    use crate::config::ConfigToml;
    use crate::token_data::IdTokenInfo;
    use crate::token_data::KnownPlan;
    use crate::token_data::PlanType;
    use base64::Engine;
    use codex_protocol::config_types::ForcedLoginMethod;
    use pretty_assertions::assert_eq;
    use serde::Serialize;
    use serde_json::json;
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::sync::Mutex;
    use tempfile::tempdir;

    #[derive(Default, Clone)]
    struct MockKeyringStore {
        entries: Arc<Mutex<HashMap<String, String>>>,
    }

    impl MockKeyringStore {
        fn get(&self, account: &str) -> Option<String> {
            self.entries.lock().unwrap().get(account).cloned()
        }
    }

    impl KeyringStore for MockKeyringStore {
        fn load(&self, _service: &str, account: &str) -> Result<Option<String>, keyring::Error> {
            Ok(self.get(account))
        }

        fn save(&self, _service: &str, account: &str, value: &str) -> Result<(), keyring::Error> {
            let mut guard = self.entries.lock().unwrap();
            guard.insert(account.to_string(), value.to_string());
            Ok(())
        }

        fn delete(&self, _service: &str, account: &str) -> Result<bool, keyring::Error> {
            let mut guard = self.entries.lock().unwrap();
            Ok(guard.remove(account).is_some())
        }
    }

    const LAST_REFRESH: &str = "2025-08-06T20:41:36.232376Z";

    #[tokio::test]
    async fn roundtrip_auth_dot_json() {
        let codex_home = tempdir().unwrap();
        let _ = write_auth_file(
            AuthFileParams {
                openai_api_key: None,
                chatgpt_plan_type: "pro".to_string(),
                chatgpt_account_id: None,
            },
            codex_home.path(),
        )
        .expect("failed to write auth file");

        let file = get_auth_file(codex_home.path());
        let auth_dot_json = try_read_auth_json(&file).unwrap();
        write_auth_json(&file, &auth_dot_json).unwrap();

        let same_auth_dot_json = try_read_auth_json(&file).unwrap();
        assert_eq!(auth_dot_json, same_auth_dot_json);
    }

    #[test]
    fn login_with_api_key_overwrites_existing_auth_json() {
        let dir = tempdir().unwrap();
        let auth_path = dir.path().join("auth.json");
        let stale_auth = json!({
            "OPENAI_API_KEY": "sk-old",
            "tokens": {
                "id_token": "stale.header.payload",
                "access_token": "stale-access",
                "refresh_token": "stale-refresh",
                "account_id": "stale-acc"
            }
        });
        std::fs::write(
            &auth_path,
            serde_json::to_string_pretty(&stale_auth).unwrap(),
        )
        .unwrap();

        super::login_with_api_key(dir.path(), "sk-new", AuthCredentialsStoreMode::File)
            .expect("login_with_api_key should succeed");

        let auth = super::try_read_auth_json(&auth_path).expect("auth.json should parse");
        assert_eq!(auth.openai_api_key.as_deref(), Some("sk-new"));
        assert!(auth.tokens.is_none(), "tokens should be cleared");
    }

    #[test]
    fn missing_auth_json_returns_none() {
        let dir = tempdir().unwrap();
        let auth = CodexAuth::from_codex_home(dir.path()).expect("call should succeed");
        assert_eq!(auth, None);
    }

    #[tokio::test]
    #[serial(codex_api_key)]
    async fn pro_account_with_no_api_key_uses_chatgpt_auth() {
        let codex_home = tempdir().unwrap();
        let fake_jwt = write_auth_file(
            AuthFileParams {
                openai_api_key: None,
                chatgpt_plan_type: "pro".to_string(),
                chatgpt_account_id: None,
            },
            codex_home.path(),
        )
        .expect("failed to write auth file");

        let CodexAuth {
            api_key,
            mode,
            auth_dot_json,
            ..
        } = super::load_auth(codex_home.path(), false, AuthCredentialsStoreMode::File)
            .unwrap()
            .unwrap();
        assert_eq!(None, api_key);
        assert_eq!(AuthMode::ChatGPT, mode);

        let guard = auth_dot_json.lock().unwrap();
        let auth_dot_json = guard.as_ref().expect("AuthDotJson should exist");
        let last_refresh = auth_dot_json
            .last_refresh
            .expect("last_refresh should be recorded");

        assert_eq!(
            &AuthDotJson {
                openai_api_key: None,
                tokens: Some(TokenData {
                    id_token: IdTokenInfo {
                        email: Some("user@example.com".to_string()),
                        chatgpt_plan_type: Some(PlanType::Known(KnownPlan::Pro)),
                        chatgpt_account_id: None,
                        raw_jwt: fake_jwt,
                    },
                    access_token: "test-access-token".to_string(),
                    refresh_token: "test-refresh-token".to_string(),
                    account_id: None,
                }),
                last_refresh: Some(last_refresh),
            },
            auth_dot_json
        );
    }

    #[tokio::test]
    #[serial(codex_api_key)]
    async fn loads_api_key_from_auth_json() {
        let dir = tempdir().unwrap();
        let auth_file = dir.path().join("auth.json");
        std::fs::write(
            auth_file,
            r#"{"OPENAI_API_KEY":"sk-test-key","tokens":null,"last_refresh":null}"#,
        )
        .unwrap();

        let auth = super::load_auth(dir.path(), false, AuthCredentialsStoreMode::File)
            .unwrap()
            .unwrap();
        assert_eq!(auth.mode, AuthMode::ApiKey);
        assert_eq!(auth.api_key, Some("sk-test-key".to_string()));

        assert!(auth.get_token_data().await.is_err());
    }

    #[test]
    fn logout_removes_auth_file() -> Result<(), std::io::Error> {
        let dir = tempdir()?;
        let auth_dot_json = AuthDotJson {
            openai_api_key: Some("sk-test-key".to_string()),
            tokens: None,
            last_refresh: None,
        };
        write_auth_json(&get_auth_file(dir.path()), &auth_dot_json)?;
        assert!(dir.path().join("auth.json").exists());
        let removed = logout(dir.path(), AuthCredentialsStoreMode::File)?;
        assert!(removed);
        assert!(!dir.path().join("auth.json").exists());
        Ok(())
    }

    #[test]
    fn keyring_mode_saves_and_removes_file() -> std::io::Result<()> {
        let codex_home = tempdir()?;
        let keyring = Arc::new(MockKeyringStore::default());
        let storage = AuthStorage::with_keyring_store(
            codex_home.path().to_path_buf(),
            AuthCredentialsStoreMode::Keyring,
            keyring.clone(),
        );
        let auth = AuthDotJson {
            openai_api_key: Some("sk-key".to_string()),
            tokens: None,
            last_refresh: None,
        };

        storage.save(&auth)?;
        let account = compute_store_key(codex_home.path())?;
        let stored = keyring.get(&account).expect("keyring entry expected");
        let parsed: AuthDotJson = serde_json::from_str(&stored)?;
        assert_eq!(parsed.openai_api_key, auth.openai_api_key);
        assert!(!codex_home.path().join("auth.json").exists());
        Ok(())
    }

    #[test]
    fn auto_mode_loads_from_file_if_keyring_missing() -> std::io::Result<()> {
        let codex_home = tempdir()?;
        let auth = AuthDotJson {
            openai_api_key: Some("sk-auto".to_string()),
            tokens: None,
            last_refresh: None,
        };
        write_auth_json(&get_auth_file(codex_home.path()), &auth)?;
        let storage = AuthStorage::with_keyring_store(
            codex_home.path().to_path_buf(),
            AuthCredentialsStoreMode::Auto,
            Arc::new(MockKeyringStore::default()),
        );

        let loaded = storage.load()?.expect("should load from file");
        assert_eq!(loaded.openai_api_key, auth.openai_api_key);
        Ok(())
    }

    #[test]
    fn keyring_delete_removes_entries() -> std::io::Result<()> {
        let codex_home = tempdir()?;
        let keyring = Arc::new(MockKeyringStore::default());
        let storage = AuthStorage::with_keyring_store(
            codex_home.path().to_path_buf(),
            AuthCredentialsStoreMode::Keyring,
            keyring.clone(),
        );
        let auth = AuthDotJson {
            openai_api_key: Some("sk-del".to_string()),
            tokens: None,
            last_refresh: None,
        };
        storage.save(&auth)?;
        assert!(storage.delete()?);
        let account = compute_store_key(codex_home.path())?;
        assert!(keyring.get(&account).is_none());
        Ok(())
    }

    struct AuthFileParams {
        openai_api_key: Option<String>,
        chatgpt_plan_type: String,
        chatgpt_account_id: Option<String>,
    }

    fn write_auth_file(params: AuthFileParams, codex_home: &Path) -> std::io::Result<String> {
        let auth_file = get_auth_file(codex_home);
        // Create a minimal valid JWT for the id_token field.
        #[derive(Serialize)]
        struct Header {
            alg: &'static str,
            typ: &'static str,
        }
        let header = Header {
            alg: "none",
            typ: "JWT",
        };
        let mut auth_payload = serde_json::json!({
            "chatgpt_plan_type": params.chatgpt_plan_type,
            "chatgpt_user_id": "user-12345",
            "user_id": "user-12345",
        });

        if let Some(chatgpt_account_id) = params.chatgpt_account_id {
            let org_value = serde_json::Value::String(chatgpt_account_id);
            auth_payload["chatgpt_account_id"] = org_value;
        }

        let payload = serde_json::json!({
            "email": "user@example.com",
            "email_verified": true,
            "https://api.openai.com/auth": auth_payload,
        });
        let b64 = |b: &[u8]| base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b);
        let header_b64 = b64(&serde_json::to_vec(&header)?);
        let payload_b64 = b64(&serde_json::to_vec(&payload)?);
        let signature_b64 = b64(b"sig");
        let fake_jwt = format!("{header_b64}.{payload_b64}.{signature_b64}");

        let auth_json_data = json!({
            "OPENAI_API_KEY": params.openai_api_key,
            "tokens": {
                "id_token": fake_jwt,
                "access_token": "test-access-token",
                "refresh_token": "test-refresh-token"
            },
            "last_refresh": Utc::now(),
        });
        let auth_json = serde_json::to_string_pretty(&auth_json_data)?;
        std::fs::write(auth_file, auth_json)?;
        Ok(fake_jwt)
    }

    fn build_config(
        codex_home: &Path,
        forced_login_method: Option<ForcedLoginMethod>,
        forced_chatgpt_workspace_id: Option<String>,
    ) -> Config {
        let mut config = Config::load_from_base_config_with_overrides(
            ConfigToml::default(),
            ConfigOverrides::default(),
            codex_home.to_path_buf(),
        )
        .expect("config should load");
        config.forced_login_method = forced_login_method;
        config.forced_chatgpt_workspace_id = forced_chatgpt_workspace_id;
        config
    }

    /// Use sparingly.
    /// TODO (gpeal): replace this with an injectable env var provider.
    #[cfg(test)]
    struct EnvVarGuard {
        key: &'static str,
        original: Option<std::ffi::OsString>,
    }

    #[cfg(test)]
    impl EnvVarGuard {
        fn set(key: &'static str, value: &str) -> Self {
            let original = env::var_os(key);
            unsafe {
                env::set_var(key, value);
            }
            Self { key, original }
        }
    }

    #[cfg(test)]
    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            unsafe {
                match &self.original {
                    Some(value) => env::set_var(self.key, value),
                    None => env::remove_var(self.key),
                }
            }
        }
    }

    #[tokio::test]
    async fn enforce_login_restrictions_logs_out_for_method_mismatch() {
        let codex_home = tempdir().unwrap();
        login_with_api_key(codex_home.path(), "sk-test").expect("seed api key");

        let config = build_config(codex_home.path(), Some(ForcedLoginMethod::Chatgpt), None);

        let err = super::enforce_login_restrictions(&config)
            .await
            .expect_err("expected method mismatch to error");
        assert!(err.to_string().contains("ChatGPT login is required"));
        assert!(
            !codex_home.path().join("auth.json").exists(),
            "auth.json should be removed on mismatch"
        );
    }

    #[tokio::test]
    #[serial(codex_api_key)]
    async fn enforce_login_restrictions_logs_out_for_workspace_mismatch() {
        let codex_home = tempdir().unwrap();
        let _jwt = write_auth_file(
            AuthFileParams {
                openai_api_key: None,
                chatgpt_plan_type: "pro".to_string(),
                chatgpt_account_id: Some("org_another_org".to_string()),
            },
            codex_home.path(),
        )
        .expect("failed to write auth file");

        let config = build_config(codex_home.path(), None, Some("org_mine".to_string()));

        let err = super::enforce_login_restrictions(&config)
            .await
            .expect_err("expected workspace mismatch to error");
        assert!(err.to_string().contains("workspace org_mine"));
        assert!(
            !codex_home.path().join("auth.json").exists(),
            "auth.json should be removed on mismatch"
        );
    }

    #[tokio::test]
    #[serial(codex_api_key)]
    async fn enforce_login_restrictions_allows_matching_workspace() {
        let codex_home = tempdir().unwrap();
        let _jwt = write_auth_file(
            AuthFileParams {
                openai_api_key: None,
                chatgpt_plan_type: "pro".to_string(),
                chatgpt_account_id: Some("org_mine".to_string()),
            },
            codex_home.path(),
        )
        .expect("failed to write auth file");

        let config = build_config(codex_home.path(), None, Some("org_mine".to_string()));

        super::enforce_login_restrictions(&config)
            .await
            .expect("matching workspace should succeed");
        assert!(
            codex_home.path().join("auth.json").exists(),
            "auth.json should remain when restrictions pass"
        );
    }

    #[tokio::test]
    async fn enforce_login_restrictions_allows_api_key_if_login_method_not_set_but_forced_chatgpt_workspace_id_is_set()
     {
        let codex_home = tempdir().unwrap();
        login_with_api_key(codex_home.path(), "sk-test").expect("seed api key");

        let config = build_config(codex_home.path(), None, Some("org_mine".to_string()));

        super::enforce_login_restrictions(&config)
            .await
            .expect("matching workspace should succeed");
        assert!(
            codex_home.path().join("auth.json").exists(),
            "auth.json should remain when restrictions pass"
        );
    }

    #[tokio::test]
    #[serial(codex_api_key)]
    async fn enforce_login_restrictions_blocks_env_api_key_when_chatgpt_required() {
        let _guard = EnvVarGuard::set(CODEX_API_KEY_ENV_VAR, "sk-env");
        let codex_home = tempdir().unwrap();

        let config = build_config(codex_home.path(), Some(ForcedLoginMethod::Chatgpt), None);

        let err = super::enforce_login_restrictions(&config)
            .await
            .expect_err("environment API key should not satisfy forced ChatGPT login");
        assert!(
            err.to_string()
                .contains("ChatGPT login is required, but an API key is currently being used.")
        );
    }
}

/// Central manager providing a single source of truth for auth.json derived
/// authentication data. It loads once (or on preference change) and then
/// hands out cloned `CodexAuth` values so the rest of the program has a
/// consistent snapshot.
///
/// External modifications to `auth.json` will NOT be observed until
/// `reload()` is called explicitly. This matches the design goal of avoiding
/// different parts of the program seeing inconsistent auth data mid‑run.
#[derive(Debug)]
pub struct AuthManager {
    codex_home: PathBuf,
    inner: RwLock<CachedAuth>,
    enable_codex_api_key_env: bool,
    store_mode: AuthCredentialsStoreMode,
}

impl AuthManager {
    /// Create a new manager loading the initial auth using the provided
    /// preferred auth method. Errors loading auth are swallowed; `auth()` will
    /// simply return `None` in that case so callers can treat it as an
    /// unauthenticated state.
    pub fn new(
        codex_home: PathBuf,
        enable_codex_api_key_env: bool,
        store_mode: AuthCredentialsStoreMode,
    ) -> Self {
        let auth = load_auth(&codex_home, enable_codex_api_key_env, store_mode)
            .ok()
            .flatten();
        Self {
            codex_home,
            inner: RwLock::new(CachedAuth { auth }),
            enable_codex_api_key_env,
            store_mode,
        }
    }

    /// Create an AuthManager with a specific CodexAuth, for testing only.
    pub fn from_auth_for_testing(auth: CodexAuth) -> Arc<Self> {
        let cached = CachedAuth { auth: Some(auth) };
        Arc::new(Self {
            codex_home: PathBuf::new(),
            inner: RwLock::new(cached),
            enable_codex_api_key_env: false,
            store_mode: AuthCredentialsStoreMode::Auto,
        })
    }

    /// Current cached auth (clone). May be `None` if not logged in or load failed.
    pub fn auth(&self) -> Option<CodexAuth> {
        self.inner.read().ok().and_then(|c| c.auth.clone())
    }

    /// Force a reload of the auth information from auth.json. Returns
    /// whether the auth value changed.
    pub fn reload(&self) -> bool {
        let new_auth = load_auth(
            &self.codex_home,
            self.enable_codex_api_key_env,
            self.store_mode,
        )
        .ok()
        .flatten();
        if let Ok(mut guard) = self.inner.write() {
            let changed = !AuthManager::auths_equal(&guard.auth, &new_auth);
            guard.auth = new_auth;
            changed
        } else {
            false
        }
    }

    fn auths_equal(a: &Option<CodexAuth>, b: &Option<CodexAuth>) -> bool {
        match (a, b) {
            (None, None) => true,
            (Some(a), Some(b)) => a == b,
            _ => false,
        }
    }

    /// Convenience constructor returning an `Arc` wrapper.
    pub fn shared(
        codex_home: PathBuf,
        enable_codex_api_key_env: bool,
        store_mode: AuthCredentialsStoreMode,
    ) -> Arc<Self> {
        Arc::new(Self::new(codex_home, enable_codex_api_key_env, store_mode))
    }

    /// Attempt to refresh the current auth token (if any). On success, reload
    /// the auth state from disk so other components observe refreshed token.
    pub async fn refresh_token(&self) -> std::io::Result<Option<String>> {
        let auth = match self.auth() {
            Some(a) => a,
            None => return Ok(None),
        };
        match auth.refresh_token().await {
            Ok(token) => {
                // Reload to pick up persisted changes.
                self.reload();
                Ok(Some(token))
            }
            Err(e) => Err(e),
        }
    }
}
