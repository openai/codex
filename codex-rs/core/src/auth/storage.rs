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
use tracing::warn;

use crate::token_data::TokenData;
use codex_keyring_store::DefaultKeyringStore;
use codex_keyring_store::KeyringStore;
use uuid::Uuid;

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

pub const AUTH_STORE_VERSION: u8 = 2;
pub const DEFAULT_OAUTH_PROVIDER_ID: &str = "openai";
pub const DEFAULT_OAUTH_NAMESPACE: &str = "default";

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub(crate) struct AuthStore {
    pub version: u8,
    #[serde(
        rename = "OPENAI_API_KEY",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub openai_api_key: Option<String>,
    #[serde(default)]
    pub providers: HashMap<String, AuthProviderEntry>,
}

impl Default for AuthStore {
    fn default() -> Self {
        Self {
            version: AUTH_STORE_VERSION,
            openai_api_key: None,
            providers: HashMap::new(),
        }
    }
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub(crate) enum AuthProviderEntry {
    Oauth(OAuthProvider),
    Api { key: String },
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Default)]
pub(crate) struct OAuthProvider {
    #[serde(default)]
    pub active: HashMap<String, String>,
    #[serde(default)]
    pub order: HashMap<String, Vec<String>>,
    #[serde(default)]
    pub records: Vec<OAuthRecord>,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub(crate) struct OAuthRecord {
    pub id: String,
    #[serde(default = "default_oauth_namespace")]
    pub namespace: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub tokens: TokenData,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_refresh: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(default)]
    pub health: OAuthHealth,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Default)]
pub(crate) struct OAuthHealth {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cooldown_until: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exhausted_until: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub requires_relogin: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_status_code: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub success_count: u64,
    #[serde(default)]
    pub failure_count: u64,
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_false(value: &bool) -> bool {
    !*value
}

fn default_oauth_namespace() -> String {
    DEFAULT_OAUTH_NAMESPACE.to_string()
}

pub(crate) fn normalize_oauth_namespace(namespace: &str) -> String {
    let trimmed = namespace.trim();
    if trimmed.is_empty() {
        DEFAULT_OAUTH_NAMESPACE.to_string()
    } else {
        trimmed.to_string()
    }
}

pub(crate) fn parse_auth_store(contents: &str) -> std::io::Result<AuthStore> {
    let value: serde_json::Value = serde_json::from_str(contents)?;
    let is_store = match value.get("version") {
        Some(version) => version.as_u64().is_some(),
        None => value.get("providers").is_some(),
    };

    if is_store {
        let store: AuthStore = serde_json::from_value(value)
            .map_err(|err| std::io::Error::other(format!("failed to parse auth store: {err}")))?;
        if store.version != AUTH_STORE_VERSION {
            return Err(std::io::Error::other(format!(
                "unsupported auth store version {}",
                store.version
            )));
        }
        return Ok(store);
    }

    let legacy: AuthDotJson = serde_json::from_value(value).map_err(std::io::Error::other)?;
    Ok(auth_store_from_legacy(legacy))
}

pub(crate) fn auth_store_from_legacy(auth: AuthDotJson) -> AuthStore {
    let mut store = AuthStore {
        openai_api_key: auth.openai_api_key,
        ..Default::default()
    };

    if let Some(tokens) = auth.tokens {
        let record_id = Uuid::new_v4().to_string();
        let now = auth.last_refresh.unwrap_or_else(Utc::now);
        let record = OAuthRecord {
            id: record_id.clone(),
            namespace: DEFAULT_OAUTH_NAMESPACE.to_string(),
            label: tokens.id_token.email.clone(),
            tokens,
            last_refresh: auth.last_refresh,
            created_at: now,
            updated_at: now,
            health: OAuthHealth::default(),
        };
        let mut provider = OAuthProvider {
            active: HashMap::new(),
            order: HashMap::new(),
            records: vec![record],
        };
        provider
            .active
            .insert(DEFAULT_OAUTH_NAMESPACE.to_string(), record_id.clone());
        provider
            .order
            .insert(DEFAULT_OAUTH_NAMESPACE.to_string(), vec![record_id]);
        store.providers.insert(
            DEFAULT_OAUTH_PROVIDER_ID.to_string(),
            AuthProviderEntry::Oauth(provider),
        );
    }

    store
}

pub(crate) fn auth_dot_json_from_store(
    store: &AuthStore,
    provider_id: &str,
    namespace: &str,
) -> Option<AuthDotJson> {
    let provider = store.providers.get(provider_id)?;
    let provider = match provider {
        AuthProviderEntry::Oauth(provider) => provider,
        _ => return None,
    };
    let namespace = normalize_oauth_namespace(namespace);
    let record_id = provider.active.get(&namespace).cloned().or_else(|| {
        provider
            .order
            .get(&namespace)
            .and_then(|order| order.first().cloned())
    })?;
    let record = provider
        .records
        .iter()
        .find(|record| record.id == record_id && record.namespace == namespace)?;

    Some(AuthDotJson {
        openai_api_key: store.openai_api_key.clone(),
        tokens: Some(record.tokens.clone()),
        last_refresh: record.last_refresh,
    })
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
    fn load(&self) -> std::io::Result<Option<AuthStore>>;
    fn save(&self, auth: &AuthStore) -> std::io::Result<()>;
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

    /// Attempt to read and refresh the `auth.json` file in the given `CODEX_HOME` directory.
    /// Returns the full AuthStore structure after migrating legacy data if necessary.
    pub(super) fn try_read_auth_store(&self, auth_file: &Path) -> std::io::Result<AuthStore> {
        let mut file = File::open(auth_file)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        parse_auth_store(&contents)
    }
}

impl AuthStorageBackend for FileAuthStorage {
    fn load(&self) -> std::io::Result<Option<AuthStore>> {
        let auth_file = get_auth_file(&self.codex_home);
        let store = match self.try_read_auth_store(&auth_file) {
            Ok(store) => store,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(err),
        };
        Ok(Some(store))
    }

    fn save(&self, auth_store: &AuthStore) -> std::io::Result<()> {
        let auth_file = get_auth_file(&self.codex_home);

        if let Some(parent) = auth_file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json_data = serde_json::to_string_pretty(auth_store)?;
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

    fn load_from_keyring(&self, key: &str) -> std::io::Result<Option<AuthStore>> {
        match self.keyring_store.load(KEYRING_SERVICE, key) {
            Ok(Some(serialized)) => parse_auth_store(&serialized).map(Some).map_err(|err| {
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
}

impl AuthStorageBackend for KeyringAuthStorage {
    fn load(&self) -> std::io::Result<Option<AuthStore>> {
        let key = compute_store_key(&self.codex_home)?;
        self.load_from_keyring(&key)
    }

    fn save(&self, auth: &AuthStore) -> std::io::Result<()> {
        let key = compute_store_key(&self.codex_home)?;
        // Simpler error mapping per style: prefer method reference over closure
        let serialized = serde_json::to_string(auth).map_err(std::io::Error::other)?;
        self.save_to_keyring(&key, &serialized)?;
        if let Err(err) = delete_file_if_exists(&self.codex_home) {
            warn!("failed to remove CLI auth fallback file: {err}");
        }
        Ok(())
    }

    fn delete(&self) -> std::io::Result<bool> {
        let key = compute_store_key(&self.codex_home)?;
        let keyring_removed = self
            .keyring_store
            .delete(KEYRING_SERVICE, &key)
            .map_err(|err| {
                std::io::Error::other(format!("failed to delete auth from keyring: {err}"))
            })?;
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
    fn load(&self) -> std::io::Result<Option<AuthStore>> {
        match self.keyring_storage.load() {
            Ok(Some(auth)) => Ok(Some(auth)),
            Ok(None) => self.file_storage.load(),
            Err(err) => {
                warn!("failed to load CLI auth from keyring, falling back to file storage: {err}");
                self.file_storage.load()
            }
        }
    }

    fn save(&self, auth: &AuthStore) -> std::io::Result<()> {
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

    use codex_keyring_store::tests::MockKeyringStore;
    use keyring::Error as KeyringError;

    #[tokio::test]
    async fn file_storage_load_returns_auth_store() -> anyhow::Result<()> {
        let codex_home = tempdir()?;
        let storage = FileAuthStorage::new(codex_home.path().to_path_buf());
        let auth_dot_json = AuthDotJson {
            openai_api_key: Some("test-key".to_string()),
            tokens: None,
            last_refresh: Some(Utc::now()),
        };
        let auth_store = auth_store_from_legacy(auth_dot_json);

        storage
            .save(&auth_store)
            .context("failed to save auth file")?;

        let loaded = storage.load().context("failed to load auth file")?;
        assert_eq!(Some(auth_store), loaded);
        Ok(())
    }

    #[tokio::test]
    async fn file_storage_save_persists_auth_store() -> anyhow::Result<()> {
        let codex_home = tempdir()?;
        let storage = FileAuthStorage::new(codex_home.path().to_path_buf());
        let auth_dot_json = AuthDotJson {
            openai_api_key: Some("test-key".to_string()),
            tokens: None,
            last_refresh: Some(Utc::now()),
        };
        let auth_store = auth_store_from_legacy(auth_dot_json);

        let file = get_auth_file(codex_home.path());
        storage
            .save(&auth_store)
            .context("failed to save auth file")?;

        let same_auth_store = storage
            .try_read_auth_store(&file)
            .context("failed to read auth file after save")?;
        assert_eq!(auth_store, same_auth_store);
        Ok(())
    }

    #[test]
    fn file_storage_delete_removes_auth_file() -> anyhow::Result<()> {
        let dir = tempdir()?;
        let auth_dot_json = AuthDotJson {
            openai_api_key: Some("sk-test-key".to_string()),
            tokens: None,
            last_refresh: None,
        };
        let auth_store = auth_store_from_legacy(auth_dot_json);
        let storage = create_auth_storage(dir.path().to_path_buf(), AuthCredentialsStoreMode::File);
        storage.save(&auth_store)?;
        assert!(dir.path().join("auth.json").exists());
        let storage = FileAuthStorage::new(dir.path().to_path_buf());
        let removed = storage.delete()?;
        assert!(removed);
        assert!(!dir.path().join("auth.json").exists());
        Ok(())
    }

    fn seed_keyring_and_fallback_auth_file_for_delete<F>(
        mock_keyring: &MockKeyringStore,
        codex_home: &Path,
        compute_key: F,
    ) -> anyhow::Result<(String, PathBuf)>
    where
        F: FnOnce() -> std::io::Result<String>,
    {
        let key = compute_key()?;
        mock_keyring.save(KEYRING_SERVICE, &key, "{}")?;
        let auth_file = get_auth_file(codex_home);
        std::fs::write(&auth_file, "stale")?;
        Ok((key, auth_file))
    }

    fn seed_keyring_with_store<F>(
        mock_keyring: &MockKeyringStore,
        compute_key: F,
        auth: &AuthStore,
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
        key: &str,
        codex_home: &Path,
        expected: &AuthStore,
    ) {
        let saved_value = mock_keyring
            .saved_value(key)
            .expect("keyring entry should exist");
        let expected_serialized = serde_json::to_string(expected).expect("serialize expected auth");
        assert_eq!(saved_value, expected_serialized);
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

        crate::token_data::parse_id_token(&fake_jwt).expect("fake JWT should parse")
    }

    fn auth_with_prefix(prefix: &str) -> AuthDotJson {
        AuthDotJson {
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

    fn store_with_prefix(prefix: &str) -> AuthStore {
        auth_store_from_legacy(auth_with_prefix(prefix))
    }

    #[test]
    fn keyring_auth_storage_load_returns_deserialized_auth() -> anyhow::Result<()> {
        let codex_home = tempdir()?;
        let mock_keyring = MockKeyringStore::default();
        let storage = KeyringAuthStorage::new(
            codex_home.path().to_path_buf(),
            Arc::new(mock_keyring.clone()),
        );
        let expected = auth_store_from_legacy(AuthDotJson {
            openai_api_key: Some("sk-test".to_string()),
            tokens: None,
            last_refresh: None,
        });
        seed_keyring_with_store(
            &mock_keyring,
            || compute_store_key(codex_home.path()),
            &expected,
        )?;

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
        let auth = auth_store_from_legacy(AuthDotJson {
            openai_api_key: None,
            tokens: Some(TokenData {
                id_token: Default::default(),
                access_token: "access".to_string(),
                refresh_token: "refresh".to_string(),
                account_id: Some("account".to_string()),
            }),
            last_refresh: Some(Utc::now()),
        });

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
        let (key, auth_file) = seed_keyring_and_fallback_auth_file_for_delete(
            &mock_keyring,
            codex_home.path(),
            || compute_store_key(codex_home.path()),
        )?;

        let removed = storage.delete()?;

        assert!(removed, "delete should report removal");
        assert!(
            !mock_keyring.contains(&key),
            "keyring entry should be removed"
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
        let keyring_auth = store_with_prefix("keyring");
        seed_keyring_with_store(
            &mock_keyring,
            || compute_store_key(codex_home.path()),
            &keyring_auth,
        )?;

        let file_auth = store_with_prefix("file");
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

        let expected = store_with_prefix("file-only");
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
        mock_keyring.set_error(&key, KeyringError::Invalid("error".into(), "load".into()));

        let expected = store_with_prefix("fallback");
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

        let stale = store_with_prefix("stale");
        storage.file_storage.save(&stale)?;

        let expected = store_with_prefix("to-save");
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
        let storage = AutoAuthStorage::new(
            codex_home.path().to_path_buf(),
            Arc::new(mock_keyring.clone()),
        );
        let key = compute_store_key(codex_home.path())?;
        mock_keyring.set_error(&key, KeyringError::Invalid("error".into(), "save".into()));

        let auth = store_with_prefix("fallback");
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
            mock_keyring.saved_value(&key).is_none(),
            "keyring should not contain value when save fails"
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
        let (key, auth_file) = seed_keyring_and_fallback_auth_file_for_delete(
            &mock_keyring,
            codex_home.path(),
            || compute_store_key(codex_home.path()),
        )?;

        let removed = storage.delete()?;

        assert!(removed, "delete should report removal");
        assert!(
            !mock_keyring.contains(&key),
            "keyring entry should be removed"
        );
        assert!(
            !auth_file.exists(),
            "fallback auth.json should be removed after delete"
        );
        Ok(())
    }
}
