use chrono::DateTime;
use chrono::Utc;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
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
use codex_app_server_protocol::AuthMode;
use codex_keyring_store::DefaultKeyringStore;
use codex_keyring_store::KeyringStore;
use once_cell::sync::Lazy;

/// Determine where Codex should store CLI auth credentials.
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum AuthCredentialsStoreMode {
    #[default]
    /// Persist credentials in CODEX_HOME/auth.json.
    File,
    /// Persist credentials in the keyring. Fail if unavailable.
    Keyring,
    /// Use keyring when available; otherwise, fall back to a file in CODEX_HOME.
    Auto,
    /// Store credentials in memory only for the current process.
    Ephemeral,
}

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
const KEYRING_LAYOUT_VERSION: &str = "v2";
const KEYRING_ACTIVE_REVISION_ENTRY: &str = "active";
const KEYRING_MANIFEST_ENTRY: &str = "manifest";
const KEYRING_OPENAI_API_KEY_ENTRY: &str = "OPENAI_API_KEY";
const KEYRING_ID_TOKEN_ENTRY: &str = "tokens.id_token";
const KEYRING_ACCESS_TOKEN_ENTRY: &str = "tokens.access_token";
const KEYRING_REFRESH_TOKEN_ENTRY: &str = "tokens.refresh_token";
const KEYRING_ACCOUNT_ID_ENTRY: &str = "tokens.account_id";
const KEYRING_RECORD_ENTRIES: [&str; 5] = [
    KEYRING_MANIFEST_ENTRY,
    KEYRING_OPENAI_API_KEY_ENTRY,
    KEYRING_ID_TOKEN_ENTRY,
    KEYRING_ACCESS_TOKEN_ENTRY,
    KEYRING_REFRESH_TOKEN_ENTRY,
];

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

fn keyring_layout_key(base_key: &str, suffix: &str) -> String {
    format!("{base_key}|{KEYRING_LAYOUT_VERSION}|{suffix}")
}

fn keyring_revision_key(base_key: &str, revision: &str, suffix: &str) -> String {
    format!("{base_key}|{KEYRING_LAYOUT_VERSION}|{revision}|{suffix}")
}

fn next_keyring_revision() -> String {
    Utc::now()
        .timestamp_nanos_opt()
        .unwrap_or_else(|| Utc::now().timestamp_micros() * 1_000)
        .to_string()
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
struct KeyringAuthManifest {
    auth_mode: Option<AuthMode>,
    has_openai_api_key: bool,
    has_tokens: bool,
    has_account_id: bool,
    last_refresh: Option<DateTime<Utc>>,
}

impl From<&AuthDotJson> for KeyringAuthManifest {
    fn from(auth: &AuthDotJson) -> Self {
        let has_account_id = auth
            .tokens
            .as_ref()
            .and_then(|tokens| tokens.account_id.as_ref())
            .is_some();
        Self {
            auth_mode: auth.auth_mode,
            has_openai_api_key: auth.openai_api_key.is_some(),
            has_tokens: auth.tokens.is_some(),
            has_account_id,
            last_refresh: auth.last_refresh,
        }
    }
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

    fn load_legacy_from_keyring(&self, key: &str) -> std::io::Result<Option<AuthDotJson>> {
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

    fn load_secret_from_keyring(&self, key: &str, field: &str) -> std::io::Result<Option<Vec<u8>>> {
        match self.keyring_store.load_secret(KEYRING_SERVICE, key) {
            Ok(secret) => Ok(secret),
            Err(error) => Err(std::io::Error::other(format!(
                "failed to load {field} from keyring: {}",
                error.message()
            ))),
        }
    }

    fn load_utf8_secret_from_keyring(
        &self,
        key: &str,
        field: &str,
    ) -> std::io::Result<Option<String>> {
        let Some(secret) = self.load_secret_from_keyring(key, field)? else {
            return Ok(None);
        };
        String::from_utf8(secret).map(Some).map_err(|err| {
            std::io::Error::other(format!(
                "failed to decode {field} from keyring as UTF-8: {err}"
            ))
        })
    }

    fn save_secret_to_keyring(&self, key: &str, value: &[u8], field: &str) -> std::io::Result<()> {
        match self.keyring_store.save_secret(KEYRING_SERVICE, key, value) {
            Ok(()) => Ok(()),
            Err(error) => {
                let message = format!("failed to write {field} to keyring: {}", error.message());
                warn!("{message}");
                Err(std::io::Error::other(message))
            }
        }
    }

    fn load_active_revision(&self, base_key: &str) -> std::io::Result<Option<String>> {
        let active_key = keyring_layout_key(base_key, KEYRING_ACTIVE_REVISION_ENTRY);
        self.load_utf8_secret_from_keyring(&active_key, "active auth revision")
    }

    fn load_required_utf8_secret(&self, key: &str, field: &str) -> std::io::Result<String> {
        self.load_utf8_secret_from_keyring(key, field)?
            .ok_or_else(|| std::io::Error::other(format!("missing {field} in keyring")))
    }

    fn load_manifest(
        &self,
        base_key: &str,
        revision: &str,
    ) -> std::io::Result<KeyringAuthManifest> {
        let manifest_key = keyring_revision_key(base_key, revision, KEYRING_MANIFEST_ENTRY);
        let manifest = self
            .load_secret_from_keyring(&manifest_key, "auth manifest")?
            .ok_or_else(|| std::io::Error::other("missing auth manifest in keyring"))?;
        serde_json::from_slice(&manifest).map_err(|err| {
            std::io::Error::other(format!(
                "failed to deserialize auth manifest from keyring: {err}"
            ))
        })
    }

    fn load_v2_from_keyring(&self, base_key: &str, revision: &str) -> std::io::Result<AuthDotJson> {
        let manifest = self.load_manifest(base_key, revision)?;
        let openai_api_key = if manifest.has_openai_api_key {
            let key = keyring_revision_key(base_key, revision, KEYRING_OPENAI_API_KEY_ENTRY);
            Some(self.load_required_utf8_secret(&key, "OPENAI_API_KEY")?)
        } else {
            None
        };
        let tokens = if manifest.has_tokens {
            let id_token_key = keyring_revision_key(base_key, revision, KEYRING_ID_TOKEN_ENTRY);
            let id_token = self.load_required_utf8_secret(&id_token_key, "ID token")?;
            let access_token_key =
                keyring_revision_key(base_key, revision, KEYRING_ACCESS_TOKEN_ENTRY);
            let access_token = self.load_required_utf8_secret(&access_token_key, "access token")?;
            let refresh_token_key =
                keyring_revision_key(base_key, revision, KEYRING_REFRESH_TOKEN_ENTRY);
            let refresh_token =
                self.load_required_utf8_secret(&refresh_token_key, "refresh token")?;
            let account_id = if manifest.has_account_id {
                let account_id_key =
                    keyring_revision_key(base_key, revision, KEYRING_ACCOUNT_ID_ENTRY);
                Some(self.load_required_utf8_secret(&account_id_key, "account ID")?)
            } else {
                None
            };
            Some(TokenData {
                id_token: crate::token_data::parse_chatgpt_jwt_claims(&id_token)
                    .map_err(std::io::Error::other)?,
                access_token,
                refresh_token,
                account_id,
            })
        } else {
            None
        };
        Ok(AuthDotJson {
            auth_mode: manifest.auth_mode,
            openai_api_key,
            tokens,
            last_refresh: manifest.last_refresh,
        })
    }

    fn load_from_keyring(&self, base_key: &str) -> std::io::Result<Option<AuthDotJson>> {
        if let Some(revision) = self.load_active_revision(base_key)? {
            return self.load_v2_from_keyring(base_key, &revision).map(Some);
        }
        self.load_legacy_from_keyring(base_key)
    }

    fn write_optional_secret(
        &self,
        base_key: &str,
        revision: &str,
        entry: &str,
        value: Option<&str>,
        field: &str,
    ) -> std::io::Result<()> {
        if let Some(value) = value {
            let key = keyring_revision_key(base_key, revision, entry);
            self.save_secret_to_keyring(&key, value.as_bytes(), field)?;
        }
        Ok(())
    }

    fn delete_keyring_entry(&self, key: &str) -> std::io::Result<bool> {
        self.keyring_store
            .delete(KEYRING_SERVICE, key)
            .map_err(|err| {
                std::io::Error::other(format!("failed to delete auth from keyring: {err}"))
            })
    }

    fn delete_v2_revision(&self, base_key: &str, revision: &str) -> std::io::Result<bool> {
        let mut removed = false;
        for entry in KEYRING_RECORD_ENTRIES {
            let key = keyring_revision_key(base_key, revision, entry);
            removed |= self.delete_keyring_entry(&key)?;
        }
        let account_id_key = keyring_revision_key(base_key, revision, KEYRING_ACCOUNT_ID_ENTRY);
        removed |= self.delete_keyring_entry(&account_id_key)?;
        Ok(removed)
    }

    fn delete_from_keyring_only(&self) -> std::io::Result<bool> {
        let base_key = compute_store_key(&self.codex_home)?;
        let mut removed = false;
        if let Some(revision) = self.load_active_revision(&base_key)? {
            removed |= self.delete_v2_revision(&base_key, &revision)?;
            let active_key = keyring_layout_key(&base_key, KEYRING_ACTIVE_REVISION_ENTRY);
            removed |= self.delete_keyring_entry(&active_key)?;
        }
        removed |= self.delete_keyring_entry(&base_key)?;
        Ok(removed)
    }

    fn save_v2_to_keyring(&self, base_key: &str, auth: &AuthDotJson) -> std::io::Result<()> {
        let previous_revision = match self.load_active_revision(base_key) {
            Ok(revision) => revision,
            Err(err) => {
                warn!("failed to read previous auth revision from keyring: {err}");
                None
            }
        };
        let revision = next_keyring_revision();
        let manifest = KeyringAuthManifest::from(auth);

        self.write_optional_secret(
            base_key,
            &revision,
            KEYRING_OPENAI_API_KEY_ENTRY,
            auth.openai_api_key.as_deref(),
            "OPENAI_API_KEY",
        )?;
        if let Some(tokens) = auth.tokens.as_ref() {
            self.write_optional_secret(
                base_key,
                &revision,
                KEYRING_ID_TOKEN_ENTRY,
                Some(&tokens.id_token.raw_jwt),
                "ID token",
            )?;
            self.write_optional_secret(
                base_key,
                &revision,
                KEYRING_ACCESS_TOKEN_ENTRY,
                Some(&tokens.access_token),
                "access token",
            )?;
            self.write_optional_secret(
                base_key,
                &revision,
                KEYRING_REFRESH_TOKEN_ENTRY,
                Some(&tokens.refresh_token),
                "refresh token",
            )?;
            self.write_optional_secret(
                base_key,
                &revision,
                KEYRING_ACCOUNT_ID_ENTRY,
                tokens.account_id.as_deref(),
                "account ID",
            )?;
        }

        let manifest_key = keyring_revision_key(base_key, &revision, KEYRING_MANIFEST_ENTRY);
        let manifest_bytes = serde_json::to_vec(&manifest).map_err(std::io::Error::other)?;
        self.save_secret_to_keyring(&manifest_key, &manifest_bytes, "auth manifest")?;

        let active_key = keyring_layout_key(base_key, KEYRING_ACTIVE_REVISION_ENTRY);
        self.save_secret_to_keyring(&active_key, revision.as_bytes(), "active auth revision")?;

        if let Some(previous_revision) = previous_revision
            && previous_revision != revision
            && let Err(err) = self.delete_v2_revision(base_key, &previous_revision)
        {
            warn!("failed to remove stale auth revision from keyring: {err}");
        }
        if let Err(err) = self.delete_keyring_entry(base_key) {
            warn!("failed to remove legacy auth entry from keyring: {err}");
        }
        Ok(())
    }
}

impl AuthStorageBackend for KeyringAuthStorage {
    fn load(&self) -> std::io::Result<Option<AuthDotJson>> {
        let key = compute_store_key(&self.codex_home)?;
        self.load_from_keyring(&key)
    }

    fn save(&self, auth: &AuthDotJson) -> std::io::Result<()> {
        let base_key = compute_store_key(&self.codex_home)?;
        self.save_v2_to_keyring(&base_key, auth)?;
        if let Err(err) = delete_file_if_exists(&self.codex_home) {
            warn!("failed to remove CLI auth fallback file: {err}");
        }
        Ok(())
    }

    fn delete(&self) -> std::io::Result<bool> {
        let keyring_removed = self.delete_from_keyring_only()?;
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
mod tests {
    use super::*;
    use crate::token_data::IdTokenInfo;
    use anyhow::Context;
    use base64::Engine;
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use tempfile::tempdir;

    use codex_keyring_store::CredentialStoreError;
    use codex_keyring_store::tests::MockKeyringStore;
    use keyring::Error as KeyringError;

    #[derive(Clone, Debug)]
    struct SaveSecretErrorKeyringStore {
        inner: MockKeyringStore,
    }

    impl KeyringStore for SaveSecretErrorKeyringStore {
        fn load(
            &self,
            service: &str,
            account: &str,
        ) -> Result<Option<String>, CredentialStoreError> {
            self.inner.load(service, account)
        }

        fn load_secret(
            &self,
            service: &str,
            account: &str,
        ) -> Result<Option<Vec<u8>>, CredentialStoreError> {
            self.inner.load_secret(service, account)
        }

        fn save(
            &self,
            service: &str,
            account: &str,
            value: &str,
        ) -> Result<(), CredentialStoreError> {
            self.inner.save(service, account, value)
        }

        fn save_secret(
            &self,
            _service: &str,
            _account: &str,
            _value: &[u8],
        ) -> Result<(), CredentialStoreError> {
            Err(CredentialStoreError::new(KeyringError::Invalid(
                "error".into(),
                "save".into(),
            )))
        }

        fn delete(&self, service: &str, account: &str) -> Result<bool, CredentialStoreError> {
            self.inner.delete(service, account)
        }
    }

    #[tokio::test]
    async fn file_storage_load_returns_auth_dot_json() -> anyhow::Result<()> {
        let codex_home = tempdir()?;
        let storage = FileAuthStorage::new(codex_home.path().to_path_buf());
        let auth_dot_json = AuthDotJson {
            auth_mode: Some(AuthMode::ApiKey),
            openai_api_key: Some("test-key".to_string()),
            tokens: None,
            last_refresh: Some(Utc::now()),
        };

        storage
            .save(&auth_dot_json)
            .context("failed to save auth file")?;

        let loaded = storage.load().context("failed to load auth file")?;
        assert_eq!(Some(auth_dot_json), loaded);
        Ok(())
    }

    #[tokio::test]
    async fn file_storage_save_persists_auth_dot_json() -> anyhow::Result<()> {
        let codex_home = tempdir()?;
        let storage = FileAuthStorage::new(codex_home.path().to_path_buf());
        let auth_dot_json = AuthDotJson {
            auth_mode: Some(AuthMode::ApiKey),
            openai_api_key: Some("test-key".to_string()),
            tokens: None,
            last_refresh: Some(Utc::now()),
        };

        let file = get_auth_file(codex_home.path());
        storage
            .save(&auth_dot_json)
            .context("failed to save auth file")?;

        let same_auth_dot_json = storage
            .try_read_auth_json(&file)
            .context("failed to read auth file after save")?;
        assert_eq!(auth_dot_json, same_auth_dot_json);
        Ok(())
    }

    #[test]
    fn file_storage_delete_removes_auth_file() -> anyhow::Result<()> {
        let dir = tempdir()?;
        let auth_dot_json = AuthDotJson {
            auth_mode: Some(AuthMode::ApiKey),
            openai_api_key: Some("sk-test-key".to_string()),
            tokens: None,
            last_refresh: None,
        };
        let storage = create_auth_storage(dir.path().to_path_buf(), AuthCredentialsStoreMode::File);
        storage.save(&auth_dot_json)?;
        assert!(dir.path().join("auth.json").exists());
        let storage = FileAuthStorage::new(dir.path().to_path_buf());
        let removed = storage.delete()?;
        assert!(removed);
        assert!(!dir.path().join("auth.json").exists());
        Ok(())
    }

    #[test]
    fn ephemeral_storage_save_load_delete_is_in_memory_only() -> anyhow::Result<()> {
        let dir = tempdir()?;
        let storage = create_auth_storage(
            dir.path().to_path_buf(),
            AuthCredentialsStoreMode::Ephemeral,
        );
        let auth_dot_json = AuthDotJson {
            auth_mode: Some(AuthMode::ApiKey),
            openai_api_key: Some("sk-ephemeral".to_string()),
            tokens: None,
            last_refresh: Some(Utc::now()),
        };

        storage.save(&auth_dot_json)?;
        let loaded = storage.load()?;
        assert_eq!(Some(auth_dot_json), loaded);

        let removed = storage.delete()?;
        assert!(removed);
        let loaded = storage.load()?;
        assert_eq!(None, loaded);
        assert!(!get_auth_file(dir.path()).exists());
        Ok(())
    }

    fn seed_keyring_and_fallback_auth_file_for_delete(
        storage: &KeyringAuthStorage,
        codex_home: &Path,
        auth: &AuthDotJson,
    ) -> anyhow::Result<(String, String, PathBuf)> {
        storage.save(auth)?;
        let base_key = compute_store_key(codex_home)?;
        let revision = storage
            .load_active_revision(&base_key)?
            .context("active auth revision should exist")?;
        let auth_file = get_auth_file(codex_home);
        std::fs::write(&auth_file, "stale")?;
        Ok((base_key, revision, auth_file))
    }

    fn seed_keyring_with_auth<F>(
        mock_keyring: &MockKeyringStore,
        compute_key: F,
        auth: &AuthDotJson,
    ) -> anyhow::Result<()>
    where
        F: FnOnce() -> std::io::Result<String>,
    {
        let key = compute_key()?;
        let serialized = serde_json::to_string(auth)?;
        mock_keyring.save(KEYRING_SERVICE, &key, &serialized)?;
        Ok(())
    }

    fn assert_keyring_saved_auth_and_removed_fallback(
        mock_keyring: &MockKeyringStore,
        base_key: &str,
        codex_home: &Path,
        expected: &AuthDotJson,
    ) {
        let active_key = keyring_layout_key(base_key, KEYRING_ACTIVE_REVISION_ENTRY);
        let revision = mock_keyring
            .saved_secret_utf8(&active_key)
            .expect("active auth revision should exist");
        assert!(
            mock_keyring.saved_value(base_key).is_none(),
            "legacy keyring entry should not be used for split auth storage"
        );
        let manifest_key = keyring_revision_key(base_key, &revision, KEYRING_MANIFEST_ENTRY);
        let manifest_bytes = mock_keyring
            .saved_secret(&manifest_key)
            .expect("auth manifest should exist");
        let manifest: KeyringAuthManifest =
            serde_json::from_slice(&manifest_bytes).expect("manifest should deserialize");
        assert_eq!(manifest, KeyringAuthManifest::from(expected));

        let openai_api_key_key =
            keyring_revision_key(base_key, &revision, KEYRING_OPENAI_API_KEY_ENTRY);
        assert_eq!(
            mock_keyring.saved_secret_utf8(&openai_api_key_key),
            expected.openai_api_key
        );

        if let Some(tokens) = expected.tokens.as_ref() {
            let id_token_key = keyring_revision_key(base_key, &revision, KEYRING_ID_TOKEN_ENTRY);
            assert_eq!(
                mock_keyring.saved_secret_utf8(&id_token_key),
                Some(tokens.id_token.raw_jwt.clone())
            );
            let access_token_key =
                keyring_revision_key(base_key, &revision, KEYRING_ACCESS_TOKEN_ENTRY);
            assert_eq!(
                mock_keyring.saved_secret_utf8(&access_token_key),
                Some(tokens.access_token.clone())
            );
            let refresh_token_key =
                keyring_revision_key(base_key, &revision, KEYRING_REFRESH_TOKEN_ENTRY);
            assert_eq!(
                mock_keyring.saved_secret_utf8(&refresh_token_key),
                Some(tokens.refresh_token.clone())
            );
            let account_id_key =
                keyring_revision_key(base_key, &revision, KEYRING_ACCOUNT_ID_ENTRY);
            assert_eq!(
                mock_keyring.saved_secret_utf8(&account_id_key),
                tokens.account_id.clone()
            );
        }
        let auth_file = get_auth_file(codex_home);
        assert!(
            !auth_file.exists(),
            "fallback auth.json should be removed after keyring save"
        );
    }

    fn id_token_with_prefix(prefix: &str) -> IdTokenInfo {
        #[derive(Serialize)]
        struct Header {
            alg: &'static str,
            typ: &'static str,
        }

        let header = Header {
            alg: "none",
            typ: "JWT",
        };
        let payload = json!({
            "email": format!("{prefix}@example.com"),
            "https://api.openai.com/auth": {
                "chatgpt_account_id": format!("{prefix}-account"),
            },
        });
        let encode = |bytes: &[u8]| base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes);
        let header_b64 = encode(&serde_json::to_vec(&header).expect("serialize header"));
        let payload_b64 = encode(&serde_json::to_vec(&payload).expect("serialize payload"));
        let signature_b64 = encode(b"sig");
        let fake_jwt = format!("{header_b64}.{payload_b64}.{signature_b64}");

        crate::token_data::parse_chatgpt_jwt_claims(&fake_jwt).expect("fake JWT should parse")
    }

    fn auth_with_prefix(prefix: &str) -> AuthDotJson {
        AuthDotJson {
            auth_mode: Some(AuthMode::ApiKey),
            openai_api_key: Some(format!("{prefix}-api-key")),
            tokens: Some(TokenData {
                id_token: id_token_with_prefix(prefix),
                access_token: format!("{prefix}-access"),
                refresh_token: format!("{prefix}-refresh"),
                account_id: Some(format!("{prefix}-account-id")),
            }),
            last_refresh: None,
        }
    }

    #[test]
    fn keyring_auth_storage_load_supports_legacy_single_entry() -> anyhow::Result<()> {
        let codex_home = tempdir()?;
        let mock_keyring = MockKeyringStore::default();
        let storage = KeyringAuthStorage::new(
            codex_home.path().to_path_buf(),
            Arc::new(mock_keyring.clone()),
        );
        let expected = AuthDotJson {
            auth_mode: Some(AuthMode::ApiKey),
            openai_api_key: Some("sk-test".to_string()),
            tokens: None,
            last_refresh: None,
        };
        seed_keyring_with_auth(
            &mock_keyring,
            || compute_store_key(codex_home.path()),
            &expected,
        )?;

        let loaded = storage.load()?;
        assert_eq!(Some(expected), loaded);
        Ok(())
    }

    #[test]
    fn keyring_auth_storage_load_returns_deserialized_v2_auth() -> anyhow::Result<()> {
        let codex_home = tempdir()?;
        let mock_keyring = MockKeyringStore::default();
        let storage =
            KeyringAuthStorage::new(codex_home.path().to_path_buf(), Arc::new(mock_keyring));
        let expected = auth_with_prefix("split");

        storage.save(&expected)?;

        let loaded = storage.load()?;
        assert_eq!(Some(expected), loaded);
        Ok(())
    }

    #[test]
    fn keyring_auth_storage_compute_store_key_for_home_directory() -> anyhow::Result<()> {
        let codex_home = PathBuf::from("~/.codex");

        let key = compute_store_key(codex_home.as_path())?;

        assert_eq!(key, "cli|940db7b1d0e4eb40");
        Ok(())
    }

    #[test]
    fn keyring_auth_storage_save_persists_and_removes_fallback_file() -> anyhow::Result<()> {
        let codex_home = tempdir()?;
        let mock_keyring = MockKeyringStore::default();
        let storage = KeyringAuthStorage::new(
            codex_home.path().to_path_buf(),
            Arc::new(mock_keyring.clone()),
        );
        let auth_file = get_auth_file(codex_home.path());
        std::fs::write(&auth_file, "stale")?;
        let auth = AuthDotJson {
            auth_mode: Some(AuthMode::Chatgpt),
            openai_api_key: None,
            tokens: Some(TokenData {
                id_token: Default::default(),
                access_token: "access".to_string(),
                refresh_token: "refresh".to_string(),
                account_id: Some("account".to_string()),
            }),
            last_refresh: Some(Utc::now()),
        };

        storage.save(&auth)?;

        let key = compute_store_key(codex_home.path())?;
        assert_keyring_saved_auth_and_removed_fallback(
            &mock_keyring,
            &key,
            codex_home.path(),
            &auth,
        );
        Ok(())
    }

    #[test]
    fn keyring_auth_storage_delete_removes_keyring_and_file() -> anyhow::Result<()> {
        let codex_home = tempdir()?;
        let mock_keyring = MockKeyringStore::default();
        let storage = KeyringAuthStorage::new(
            codex_home.path().to_path_buf(),
            Arc::new(mock_keyring.clone()),
        );
        let auth = auth_with_prefix("delete");
        let (base_key, revision, auth_file) =
            seed_keyring_and_fallback_auth_file_for_delete(&storage, codex_home.path(), &auth)?;

        let removed = storage.delete()?;

        assert!(removed, "delete should report removal");
        let active_key = keyring_layout_key(&base_key, KEYRING_ACTIVE_REVISION_ENTRY);
        assert!(
            !mock_keyring.contains(&active_key),
            "active revision should be removed"
        );
        for entry in KEYRING_RECORD_ENTRIES {
            let key = keyring_revision_key(&base_key, &revision, entry);
            assert!(
                !mock_keyring.contains(&key),
                "keyring entry should be removed"
            );
        }
        let account_id_key = keyring_revision_key(&base_key, &revision, KEYRING_ACCOUNT_ID_ENTRY);
        assert!(
            !mock_keyring.contains(&account_id_key),
            "account id entry should be removed"
        );
        assert!(
            !auth_file.exists(),
            "fallback auth.json should be removed after keyring delete"
        );
        Ok(())
    }

    #[test]
    fn auto_auth_storage_load_prefers_keyring_value() -> anyhow::Result<()> {
        let codex_home = tempdir()?;
        let mock_keyring = MockKeyringStore::default();
        let storage = AutoAuthStorage::new(
            codex_home.path().to_path_buf(),
            Arc::new(mock_keyring.clone()),
        );
        let keyring_auth = auth_with_prefix("keyring");
        seed_keyring_with_auth(
            &mock_keyring,
            || compute_store_key(codex_home.path()),
            &keyring_auth,
        )?;

        let file_auth = auth_with_prefix("file");
        storage.file_storage.save(&file_auth)?;

        let loaded = storage.load()?;
        assert_eq!(loaded, Some(keyring_auth));
        Ok(())
    }

    #[test]
    fn auto_auth_storage_load_uses_file_when_keyring_empty() -> anyhow::Result<()> {
        let codex_home = tempdir()?;
        let mock_keyring = MockKeyringStore::default();
        let storage = AutoAuthStorage::new(codex_home.path().to_path_buf(), Arc::new(mock_keyring));

        let expected = auth_with_prefix("file-only");
        storage.file_storage.save(&expected)?;

        let loaded = storage.load()?;
        assert_eq!(loaded, Some(expected));
        Ok(())
    }

    #[test]
    fn auto_auth_storage_load_falls_back_when_keyring_errors() -> anyhow::Result<()> {
        let codex_home = tempdir()?;
        let mock_keyring = MockKeyringStore::default();
        let storage = AutoAuthStorage::new(
            codex_home.path().to_path_buf(),
            Arc::new(mock_keyring.clone()),
        );
        let key = compute_store_key(codex_home.path())?;
        let active_key = keyring_layout_key(&key, KEYRING_ACTIVE_REVISION_ENTRY);
        mock_keyring.set_error(
            &active_key,
            KeyringError::Invalid("error".into(), "load".into()),
        );

        let expected = auth_with_prefix("fallback");
        storage.file_storage.save(&expected)?;

        let loaded = storage.load()?;
        assert_eq!(loaded, Some(expected));
        Ok(())
    }

    #[test]
    fn auto_auth_storage_save_prefers_keyring() -> anyhow::Result<()> {
        let codex_home = tempdir()?;
        let mock_keyring = MockKeyringStore::default();
        let storage = AutoAuthStorage::new(
            codex_home.path().to_path_buf(),
            Arc::new(mock_keyring.clone()),
        );
        let key = compute_store_key(codex_home.path())?;

        let stale = auth_with_prefix("stale");
        storage.file_storage.save(&stale)?;

        let expected = auth_with_prefix("to-save");
        storage.save(&expected)?;

        assert_keyring_saved_auth_and_removed_fallback(
            &mock_keyring,
            &key,
            codex_home.path(),
            &expected,
        );
        Ok(())
    }

    #[test]
    fn auto_auth_storage_save_falls_back_when_keyring_errors() -> anyhow::Result<()> {
        let codex_home = tempdir()?;
        let mock_keyring = MockKeyringStore::default();
        let failing_keyring = SaveSecretErrorKeyringStore {
            inner: mock_keyring.clone(),
        };
        let storage =
            AutoAuthStorage::new(codex_home.path().to_path_buf(), Arc::new(failing_keyring));
        let key = compute_store_key(codex_home.path())?;
        let active_key = keyring_layout_key(&key, KEYRING_ACTIVE_REVISION_ENTRY);

        let auth = auth_with_prefix("fallback");
        storage.save(&auth)?;

        let auth_file = get_auth_file(codex_home.path());
        assert!(
            auth_file.exists(),
            "fallback auth.json should be created when keyring save fails"
        );
        let saved = storage
            .file_storage
            .load()?
            .context("fallback auth should exist")?;
        assert_eq!(saved, auth);
        assert!(
            mock_keyring.saved_secret_utf8(&active_key).is_none(),
            "keyring should not point to a saved auth revision when save fails"
        );
        Ok(())
    }

    #[test]
    fn auto_auth_storage_delete_removes_keyring_and_file() -> anyhow::Result<()> {
        let codex_home = tempdir()?;
        let mock_keyring = MockKeyringStore::default();
        let storage = AutoAuthStorage::new(
            codex_home.path().to_path_buf(),
            Arc::new(mock_keyring.clone()),
        );
        let auth = auth_with_prefix("auto-delete");
        let (base_key, revision, auth_file) = seed_keyring_and_fallback_auth_file_for_delete(
            storage.keyring_storage.as_ref(),
            codex_home.path(),
            &auth,
        )?;

        let removed = storage.delete()?;

        assert!(removed, "delete should report removal");
        assert!(
            !mock_keyring.contains(&keyring_layout_key(
                &base_key,
                KEYRING_ACTIVE_REVISION_ENTRY
            )),
            "active revision should be removed"
        );
        for entry in KEYRING_RECORD_ENTRIES {
            let key = keyring_revision_key(&base_key, &revision, entry);
            assert!(
                !mock_keyring.contains(&key),
                "keyring entry should be removed"
            );
        }
        assert!(
            !mock_keyring.contains(&keyring_revision_key(
                &base_key,
                &revision,
                KEYRING_ACCOUNT_ID_ENTRY
            )),
            "account id entry should be removed"
        );
        assert!(
            !auth_file.exists(),
            "fallback auth.json should be removed after delete"
        );
        Ok(())
    }
}
