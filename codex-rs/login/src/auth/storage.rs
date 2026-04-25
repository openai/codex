use chrono::DateTime;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;
use serde::de::DeserializeOwned;
use sha2::Digest;
use sha2::Sha256;
use std::collections::HashMap;
use std::fmt::Debug;
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
use tracing::warn;

use crate::token_data::TokenData;
use crate::token_data::parse_chatgpt_jwt_claims;
use codex_app_server_protocol::AuthMode;
use codex_config::types::AuthCredentialsStoreMode;
use codex_keyring_store::DefaultKeyringStore;
use codex_keyring_store::KeyringStore;
use codex_protocol::account::PlanType as AccountPlanType;
use once_cell::sync::Lazy;

/// Expected structure for $CODEX_HOME/auth.json.
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct AuthDotJson {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth_mode: Option<AuthMode>,

    #[serde(rename = "OPENAI_API_KEY")]
    pub openai_api_key: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens: Option<TokenData>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_refresh: Option<DateTime<Utc>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_identity: Option<AgentIdentityAuthRecord>,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
pub struct AgentIdentityAuthRecord {
    pub agent_runtime_id: String,
    pub agent_private_key: String,
    pub account_id: String,
    pub chatgpt_user_id: String,
    pub email: String,
    pub plan_type: AccountPlanType,
    pub chatgpt_account_is_fedramp: bool,
}

pub(super) fn get_auth_file(codex_home: &Path) -> PathBuf {
    codex_home.join("auth.json")
}

pub(super) fn delete_file_if_exists(codex_home: &Path) -> std::io::Result<bool> {
    let auth_file = get_auth_file(codex_home);
    match std::fs::remove_file(&auth_file) {
        Ok(()) => Ok(true),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(err) => Err(err),
    }
}

pub(super) trait AuthStorageBackend: Debug + Send + Sync {
    fn load(&self) -> std::io::Result<Option<AuthDotJson>>;
    fn save(&self, auth: &AuthDotJson) -> std::io::Result<()>;
    fn delete(&self) -> std::io::Result<bool>;
}

#[derive(Clone, Debug)]
pub(super) struct FileAuthStorage {
    codex_home: PathBuf,
}

impl FileAuthStorage {
    pub(super) fn new(codex_home: PathBuf) -> Self {
        Self { codex_home }
    }

    /// Attempt to read and parse the `auth.json` file in the given `CODEX_HOME` directory.
    /// Returns the full AuthDotJson structure.
    pub(super) fn try_read_auth_json(&self, auth_file: &Path) -> std::io::Result<AuthDotJson> {
        let mut file = File::open(auth_file)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        let auth_dot_json: AuthDotJson = serde_json::from_str(&contents)?;

        Ok(auth_dot_json)
    }
}

impl AuthStorageBackend for FileAuthStorage {
    fn load(&self) -> std::io::Result<Option<AuthDotJson>> {
        let auth_file = get_auth_file(&self.codex_home);
        let auth_dot_json = match self.try_read_auth_json(&auth_file) {
            Ok(auth) => auth,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(err),
        };
        Ok(Some(auth_dot_json))
    }

    fn save(&self, auth_dot_json: &AuthDotJson) -> std::io::Result<()> {
        let auth_file = get_auth_file(&self.codex_home);

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

    fn delete(&self) -> std::io::Result<bool> {
        delete_file_if_exists(&self.codex_home)
    }
}

const KEYRING_SERVICE: &str = "Codex Auth";

// Const for each field of AuthDotJson, only when used on windows
// if more fields are added to struct, then update these consts
//  and windows storage functions.
const AUTH_MODE_FIELD: &str = "auth_mode";
const OPENAI_API_KEY_FIELD: &str = "openai_api_key";
const TOKENS_ID_TOKEN_FIELD: &str = "tokens.id_token";
const TOKENS_ACCESS_TOKEN_FIELD: &str = "tokens.access_token";
const TOKENS_REFRESH_TOKEN_FIELD: &str = "tokens.refresh_token";
const TOKENS_ACCOUNT_ID_FIELD: &str = "tokens.account_id";
const LAST_REFRESH_FIELD: &str = "last_refresh";

// turns codex_home path into a stable, short key string
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

#[derive(Clone, Debug)]
struct KeyringAuthStorage {
    codex_home: PathBuf,
    keyring_store: Arc<dyn KeyringStore>,
}

impl KeyringAuthStorage {
    fn new(codex_home: PathBuf, keyring_store: Arc<dyn KeyringStore>) -> Self {
        Self {
            codex_home,
            keyring_store,
        }
    }

    fn load_from_keyring(&self, key: &str) -> std::io::Result<Option<AuthDotJson>> {
        match self.keyring_store.load(KEYRING_SERVICE, key) {
            Ok(Some(serialized)) => serde_json::from_str(&serialized).map(Some).map_err(|err| {
                std::io::Error::other(format!(
                    "failed to deserialize CLI auth from keyring: {err}"
                ))
            }),
            Ok(None) => Ok(None),
            Err(error) => Err(std::io::Error::other(format!(
                "failed to load CLI auth from keyring: {}",
                error.message()
            ))),
        }
    }

    fn save_to_keyring(&self, key: &str, value: &str) -> std::io::Result<()> {
        match self.keyring_store.save(KEYRING_SERVICE, key, value) {
            Ok(()) => Ok(()),
            Err(error) => {
                let message = format!(
                    "failed to write OAuth tokens to keyring: {}",
                    error.message()
                );
                warn!("{message}");
                Err(std::io::Error::other(message))
            }
        }
    }

    fn field_key(key: &str, field_name: &str) -> String {
        format!("{key}|{field_name}")
    }

    fn save_windows_auth_to_keyring(&self, key: &str, auth: &AuthDotJson) -> std::io::Result<()> {
        let fields = [
            (
                AUTH_MODE_FIELD,
                serde_json::to_string(&auth.auth_mode).map_err(std::io::Error::other)?,
            ),
            (
                OPENAI_API_KEY_FIELD,
                serde_json::to_string(&auth.openai_api_key).map_err(std::io::Error::other)?,
            ),
            (
                TOKENS_ID_TOKEN_FIELD,
                serde_json::to_string(&auth.tokens.as_ref().map(|tokens| &tokens.id_token.raw_jwt))
                    .map_err(std::io::Error::other)?,
            ),
            (
                TOKENS_ACCESS_TOKEN_FIELD,
                serde_json::to_string(&auth.tokens.as_ref().map(|tokens| &tokens.access_token))
                    .map_err(std::io::Error::other)?,
            ),
            (
                TOKENS_REFRESH_TOKEN_FIELD,
                serde_json::to_string(&auth.tokens.as_ref().map(|tokens| &tokens.refresh_token))
                    .map_err(std::io::Error::other)?,
            ),
            (
                TOKENS_ACCOUNT_ID_FIELD,
                serde_json::to_string(
                    &auth
                        .tokens
                        .as_ref()
                        .and_then(|tokens| tokens.account_id.as_ref()),
                )
                .map_err(std::io::Error::other)?,
            ),
            (
                LAST_REFRESH_FIELD,
                serde_json::to_string(&auth.last_refresh).map_err(std::io::Error::other)?,
            ),
        ];

        for (field_name, serialized) in fields {
            self.save_to_keyring(&Self::field_key(key, field_name), &serialized)?;
        }

        if let Err(err) = self.keyring_store.delete(KEYRING_SERVICE, key) {
            warn!("failed to remove legacy CLI auth from keyring: {err}");
        }
        Ok(())
    }

    fn load_windows_auth_from_keyring(&self, key: &str) -> std::io::Result<Option<AuthDotJson>> {
        fn load_auth_field_from_keyring<T>(
            keyring_store: &dyn KeyringStore,
            key: &str,
        ) -> std::io::Result<Option<T>>
        where
            T: DeserializeOwned,
        {
            match keyring_store.load(KEYRING_SERVICE, key) {
                Ok(Some(serialized)) => {
                    serde_json::from_str(&serialized).map(Some).map_err(|err| {
                        std::io::Error::other(format!(
                            "failed to deserialize CLI auth field from keyring: {err}"
                        ))
                    })
                }
                Ok(None) => Ok(None),
                Err(error) => Err(std::io::Error::other(format!(
                    "failed to load CLI auth from keyring: {}",
                    error.message()
                ))),
            }
        }

        let auth_mode = load_auth_field_from_keyring::<Option<AuthMode>>(
            self.keyring_store.as_ref(),
            &Self::field_key(key, AUTH_MODE_FIELD),
        )?;
        let openai_api_key = load_auth_field_from_keyring::<Option<String>>(
            self.keyring_store.as_ref(),
            &Self::field_key(key, OPENAI_API_KEY_FIELD),
        )?;
        let tokens = match (
            load_auth_field_from_keyring::<Option<String>>(
                self.keyring_store.as_ref(),
                &Self::field_key(key, TOKENS_ID_TOKEN_FIELD),
            )?,
            load_auth_field_from_keyring::<Option<String>>(
                self.keyring_store.as_ref(),
                &Self::field_key(key, TOKENS_ACCESS_TOKEN_FIELD),
            )?,
            load_auth_field_from_keyring::<Option<String>>(
                self.keyring_store.as_ref(),
                &Self::field_key(key, TOKENS_REFRESH_TOKEN_FIELD),
            )?,
            load_auth_field_from_keyring::<Option<String>>(
                self.keyring_store.as_ref(),
                &Self::field_key(key, TOKENS_ACCOUNT_ID_FIELD),
            )?,
        ) {
            (None, None, None, None) => None,
            (Some(None), Some(None), Some(None), Some(None)) => Some(None),
            (
                Some(Some(id_token)),
                Some(Some(access_token)),
                Some(Some(refresh_token)),
                Some(account_id),
            ) => Some(Some(TokenData {
                id_token: parse_chatgpt_jwt_claims(&id_token).map_err(std::io::Error::other)?,
                access_token,
                refresh_token,
                account_id,
            })),
            _ => {
                return Err(std::io::Error::other(
                    "incomplete CLI auth tokens in keyring",
                ));
            }
        };
        let last_refresh = load_auth_field_from_keyring::<Option<DateTime<Utc>>>(
            self.keyring_store.as_ref(),
            &Self::field_key(key, LAST_REFRESH_FIELD),
        )?;

        if auth_mode.is_none()
            && openai_api_key.is_none()
            && tokens.is_none()
            && last_refresh.is_none()
        {
            return Ok(None);
        }

        Ok(Some(AuthDotJson {
            auth_mode: auth_mode.flatten(),
            openai_api_key: openai_api_key.flatten(),
            tokens: tokens.flatten(),
            last_refresh: last_refresh.flatten(),
        }))
    }

    fn delete_windows_auth_from_keyring(&self, key: &str) -> std::io::Result<bool> {
        let field_keys = [
            Self::field_key(key, AUTH_MODE_FIELD),
            Self::field_key(key, OPENAI_API_KEY_FIELD),
            Self::field_key(key, TOKENS_ID_TOKEN_FIELD),
            Self::field_key(key, TOKENS_ACCESS_TOKEN_FIELD),
            Self::field_key(key, TOKENS_REFRESH_TOKEN_FIELD),
            Self::field_key(key, TOKENS_ACCOUNT_ID_FIELD),
            Self::field_key(key, LAST_REFRESH_FIELD),
            key.to_string(),
        ];
        let mut removed = false;

        for field_key in field_keys {
            removed |= self
                .keyring_store
                .delete(KEYRING_SERVICE, &field_key)
                .map_err(|err| {
                    std::io::Error::other(format!("failed to delete auth from keyring: {err}"))
                })?;
        }

        Ok(removed)
    }
}

impl AuthStorageBackend for KeyringAuthStorage {
    fn load(&self) -> std::io::Result<Option<AuthDotJson>> {
        let key = compute_store_key(&self.codex_home)?;
        if cfg!(windows) {
            self.load_windows_auth_from_keyring(&key)
        } else {
            self.load_from_keyring(&key)
        }
    }

    fn save(&self, auth: &AuthDotJson) -> std::io::Result<()> {
        let key = compute_store_key(&self.codex_home)?;
        if cfg!(windows) {
            self.save_windows_auth_to_keyring(&key, auth)?;
        } else {
            let serialized = serde_json::to_string(auth).map_err(std::io::Error::other)?;
            self.save_to_keyring(&key, &serialized)?;
        }
        if let Err(err) = delete_file_if_exists(&self.codex_home) {
            warn!("failed to remove CLI auth fallback file: {err}");
        }
        Ok(())
    }

    fn delete(&self) -> std::io::Result<bool> {
        let key = compute_store_key(&self.codex_home)?;
        let keyring_removed = if cfg!(windows) {
            self.delete_windows_auth_from_keyring(&key)?
        } else {
            self.keyring_store
                .delete(KEYRING_SERVICE, &key)
                .map_err(|err| {
                    std::io::Error::other(format!("failed to delete auth from keyring: {err}"))
                })?
        };
        let file_removed = delete_file_if_exists(&self.codex_home)?;
        Ok(keyring_removed || file_removed)
    }
}

#[derive(Clone, Debug)]
struct AutoAuthStorage {
    keyring_storage: Arc<KeyringAuthStorage>,
    file_storage: Arc<FileAuthStorage>,
}

impl AutoAuthStorage {
    fn new(codex_home: PathBuf, keyring_store: Arc<dyn KeyringStore>) -> Self {
        Self {
            keyring_storage: Arc::new(KeyringAuthStorage::new(codex_home.clone(), keyring_store)),
            file_storage: Arc::new(FileAuthStorage::new(codex_home)),
        }
    }
}

impl AuthStorageBackend for AutoAuthStorage {
    fn load(&self) -> std::io::Result<Option<AuthDotJson>> {
        match self.keyring_storage.load() {
            Ok(Some(auth)) => Ok(Some(auth)),
            Ok(None) => self.file_storage.load(),
            Err(err) => {
                warn!("failed to load CLI auth from keyring, falling back to file storage: {err}");
                self.file_storage.load()
            }
        }
    }

    fn save(&self, auth: &AuthDotJson) -> std::io::Result<()> {
        match self.keyring_storage.save(auth) {
            Ok(()) => Ok(()),
            Err(err) => {
                warn!("failed to save auth to keyring, falling back to file storage: {err}");
                self.file_storage.save(auth)
            }
        }
    }

    fn delete(&self) -> std::io::Result<bool> {
        // Keyring storage will delete from disk as well
        self.keyring_storage.delete()
    }
}

// A global in-memory store for mapping codex_home -> AuthDotJson.
static EPHEMERAL_AUTH_STORE: Lazy<Mutex<HashMap<String, AuthDotJson>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

#[derive(Clone, Debug)]
struct EphemeralAuthStorage {
    codex_home: PathBuf,
}

impl EphemeralAuthStorage {
    fn new(codex_home: PathBuf) -> Self {
        Self { codex_home }
    }

    fn with_store<F, T>(&self, action: F) -> std::io::Result<T>
    where
        F: FnOnce(&mut HashMap<String, AuthDotJson>, String) -> std::io::Result<T>,
    {
        let key = compute_store_key(&self.codex_home)?;
        let mut store = EPHEMERAL_AUTH_STORE
            .lock()
            .map_err(|_| std::io::Error::other("failed to lock ephemeral auth storage"))?;
        action(&mut store, key)
    }
}

impl AuthStorageBackend for EphemeralAuthStorage {
    fn load(&self) -> std::io::Result<Option<AuthDotJson>> {
        self.with_store(|store, key| Ok(store.get(&key).cloned()))
    }

    fn save(&self, auth: &AuthDotJson) -> std::io::Result<()> {
        self.with_store(|store, key| {
            store.insert(key, auth.clone());
            Ok(())
        })
    }

    fn delete(&self) -> std::io::Result<bool> {
        self.with_store(|store, key| Ok(store.remove(&key).is_some()))
    }
}

pub(super) fn create_auth_storage(
    codex_home: PathBuf,
    mode: AuthCredentialsStoreMode,
) -> Arc<dyn AuthStorageBackend> {
    let keyring_store: Arc<dyn KeyringStore> = Arc::new(DefaultKeyringStore);
    create_auth_storage_with_keyring_store(codex_home, mode, keyring_store)
}

fn create_auth_storage_with_keyring_store(
    codex_home: PathBuf,
    mode: AuthCredentialsStoreMode,
    keyring_store: Arc<dyn KeyringStore>,
) -> Arc<dyn AuthStorageBackend> {
    match mode {
        AuthCredentialsStoreMode::File => Arc::new(FileAuthStorage::new(codex_home)),
        AuthCredentialsStoreMode::Keyring => {
            Arc::new(KeyringAuthStorage::new(codex_home, keyring_store))
        }
        AuthCredentialsStoreMode::Auto => Arc::new(AutoAuthStorage::new(codex_home, keyring_store)),
        AuthCredentialsStoreMode::Ephemeral => Arc::new(EphemeralAuthStorage::new(codex_home)),
    }
}

#[cfg(test)]
#[path = "storage_tests.rs"]
mod tests;
