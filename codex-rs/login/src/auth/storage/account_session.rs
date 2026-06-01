use super::AuthDotJson;
use super::AuthStorageBackend;
use super::EPHEMERAL_AUTH_STORE;
use super::KEYRING_SERVICE;
use super::compute_store_key;
use codex_config::types::AuthCredentialsStoreMode;
use codex_keyring_store::DefaultKeyringStore;
use codex_keyring_store::KeyringStore;
use std::collections::HashMap;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::Read;
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::warn;

pub(in crate::auth) fn create_account_session_auth_storage(
    codex_home: PathBuf,
    session_id: &str,
    mode: AuthCredentialsStoreMode,
) -> std::io::Result<Arc<dyn AuthStorageBackend>> {
    validate_session_id(session_id)?;
    let keyring_store: Arc<dyn KeyringStore> = Arc::new(DefaultKeyringStore);
    Ok(match mode {
        AuthCredentialsStoreMode::File => Arc::new(FileAuthStorage::new(&codex_home, session_id)),
        AuthCredentialsStoreMode::Keyring => Arc::new(KeyringAuthStorage::new(
            &codex_home,
            session_id,
            keyring_store,
        )?),
        AuthCredentialsStoreMode::Auto => Arc::new(AutoAuthStorage::new(
            &codex_home,
            session_id,
            keyring_store,
        )?),
        AuthCredentialsStoreMode::Ephemeral => {
            Arc::new(EphemeralAuthStorage::new(&codex_home, session_id)?)
        }
    })
}

fn validate_session_id(session_id: &str) -> std::io::Result<()> {
    let mut components = Path::new(session_id).components();
    if matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none() {
        return Ok(());
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        "account session ID must be a single path component",
    ))
}

fn auth_file(codex_home: &Path, session_id: &str) -> PathBuf {
    codex_home
        .join("account-sessions")
        .join(format!("{session_id}.json"))
}

fn store_key(codex_home: &Path, session_id: &str) -> std::io::Result<String> {
    Ok(format!(
        "{}|account-session|{session_id}",
        compute_store_key(codex_home)?
    ))
}

fn delete_file_if_exists(path: &Path) -> std::io::Result<bool> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(true),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(err) => Err(err),
    }
}

#[derive(Clone, Debug)]
struct FileAuthStorage {
    auth_file: PathBuf,
}

impl FileAuthStorage {
    fn new(codex_home: &Path, session_id: &str) -> Self {
        Self {
            auth_file: auth_file(codex_home, session_id),
        }
    }
}

impl AuthStorageBackend for FileAuthStorage {
    fn load(&self) -> std::io::Result<Option<AuthDotJson>> {
        let mut file = match File::open(&self.auth_file) {
            Ok(file) => file,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(err),
        };
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        serde_json::from_str(&contents)
            .map(Some)
            .map_err(Into::into)
    }

    fn save(&self, auth: &AuthDotJson) -> std::io::Result<()> {
        if let Some(parent) = self.auth_file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut options = OpenOptions::new();
        options.truncate(true).write(true).create(true);
        #[cfg(unix)]
        {
            options.mode(0o600);
        }
        let mut file = options.open(&self.auth_file)?;
        file.write_all(serde_json::to_string_pretty(auth)?.as_bytes())?;
        file.flush()
    }

    fn delete(&self) -> std::io::Result<bool> {
        delete_file_if_exists(&self.auth_file)
    }
}

#[derive(Clone, Debug)]
struct KeyringAuthStorage {
    auth_file: PathBuf,
    store_key: String,
    keyring_store: Arc<dyn KeyringStore>,
}

impl KeyringAuthStorage {
    fn new(
        codex_home: &Path,
        session_id: &str,
        keyring_store: Arc<dyn KeyringStore>,
    ) -> std::io::Result<Self> {
        Ok(Self {
            auth_file: auth_file(codex_home, session_id),
            store_key: store_key(codex_home, session_id)?,
            keyring_store,
        })
    }
}

impl AuthStorageBackend for KeyringAuthStorage {
    fn load(&self) -> std::io::Result<Option<AuthDotJson>> {
        match self.keyring_store.load(KEYRING_SERVICE, &self.store_key) {
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

    fn save(&self, auth: &AuthDotJson) -> std::io::Result<()> {
        let serialized = serde_json::to_string(auth).map_err(std::io::Error::other)?;
        match self
            .keyring_store
            .save(KEYRING_SERVICE, &self.store_key, &serialized)
        {
            Ok(()) => {}
            Err(error) => {
                let message = format!(
                    "failed to write OAuth tokens to keyring: {}",
                    error.message()
                );
                warn!("{message}");
                return Err(std::io::Error::other(message));
            }
        }
        if let Err(err) = delete_file_if_exists(&self.auth_file) {
            warn!("failed to remove CLI auth fallback file: {err}");
        }
        Ok(())
    }

    fn delete(&self) -> std::io::Result<bool> {
        let keyring_removed = self
            .keyring_store
            .delete(KEYRING_SERVICE, &self.store_key)
            .map_err(|err| {
                std::io::Error::other(format!("failed to delete auth from keyring: {err}"))
            });
        let file_removed = delete_file_if_exists(&self.auth_file);
        Ok(keyring_removed? || file_removed?)
    }
}

#[derive(Clone, Debug)]
struct AutoAuthStorage {
    keyring_storage: Arc<KeyringAuthStorage>,
    file_storage: Arc<FileAuthStorage>,
}

impl AutoAuthStorage {
    fn new(
        codex_home: &Path,
        session_id: &str,
        keyring_store: Arc<dyn KeyringStore>,
    ) -> std::io::Result<Self> {
        Ok(Self {
            keyring_storage: Arc::new(KeyringAuthStorage::new(
                codex_home,
                session_id,
                keyring_store,
            )?),
            file_storage: Arc::new(FileAuthStorage::new(codex_home, session_id)),
        })
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
        self.keyring_storage.delete()
    }
}

#[derive(Clone, Debug)]
struct EphemeralAuthStorage {
    store_key: String,
}

impl EphemeralAuthStorage {
    fn new(codex_home: &Path, session_id: &str) -> std::io::Result<Self> {
        Ok(Self {
            store_key: store_key(codex_home, session_id)?,
        })
    }

    fn with_store<F, T>(&self, action: F) -> std::io::Result<T>
    where
        F: FnOnce(&mut HashMap<String, AuthDotJson>, String) -> std::io::Result<T>,
    {
        let mut store = EPHEMERAL_AUTH_STORE
            .lock()
            .map_err(|_| std::io::Error::other("failed to lock ephemeral auth storage"))?;
        action(&mut store, self.store_key.clone())
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
