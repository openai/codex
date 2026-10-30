//! Persists configured OAuth grants and serializes exchanges across processes.
//! Gateway storage paths and namespace are retained so existing credentials remain usable.

use std::collections::BTreeMap;
use std::fs::File;
use std::fs::OpenOptions;
use std::fs::TryLockError;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use codex_keyring_store::KeyringStore;
use codex_model_provider_info::OAuthCredentialsStore;
use codex_secrets::LocalSecretsNamespace;
use codex_secrets::SecretName;
use codex_secrets::SecretScope;
use codex_secrets::SecretsBackendKind;
use codex_secrets::SecretsManager;
use codex_utils_path::write_atomically;

use super::StoredToken;

pub(super) async fn lock_credentials(codex_home: &Path) -> io::Result<File> {
    let directory = codex_home.join("secrets");
    std::fs::create_dir_all(&directory)?;
    // Configurations have separate credential entries but rewrite the same store file.
    // Keep one stable sidecar locked from the initial read through token exchange and save.
    let path = directory.join("gateway_oauth.lock");
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)?;
    tokio::time::timeout(Duration::from_secs(/*secs*/ 60), async {
        loop {
            match file.try_lock() {
                Ok(()) => return Ok(()),
                Err(TryLockError::WouldBlock) => {
                    tokio::time::sleep(Duration::from_millis(/*millis*/ 50)).await
                }
                Err(error) => return Err(io::Error::from(error)),
            }
        }
    })
    .await
    .map_err(|_| {
        io::Error::new(
            io::ErrorKind::TimedOut,
            "timed out waiting for provider OAuth credentials",
        )
    })??;
    Ok(file)
}

pub(super) enum OAuthSessionStorage {
    Keyring(SecretsManager),
    File(PathBuf),
}

impl OAuthSessionStorage {
    pub(super) fn new(
        codex_home: PathBuf,
        mode: OAuthCredentialsStore,
        keyring: Arc<dyn KeyringStore>,
    ) -> Self {
        match mode {
            OAuthCredentialsStore::Keyring => {
                Self::Keyring(SecretsManager::new_with_keyring_store_and_namespace(
                    codex_home,
                    SecretsBackendKind::Local,
                    keyring,
                    LocalSecretsNamespace::GatewayOAuth,
                ))
            }
            OAuthCredentialsStore::File => {
                Self::File(codex_home.join("secrets").join("gateway_oauth.json"))
            }
        }
    }

    pub(super) fn load(&self, credential_id: &str) -> io::Result<Option<StoredToken>> {
        match self {
            Self::Keyring(secrets) => secrets
                .get(&SecretScope::Global, &secret_name(credential_id)?)
                .map_err(|_| io::Error::other("failed to load provider OAuth credentials"))?
                .map(|value| serde_json::from_str(&value).map_err(invalid_credentials))
                .transpose(),
            Self::File(path) => Ok(read_tokens(path)?.remove(credential_id)),
        }
    }

    pub(super) fn save(&self, credential_id: &str, token: &StoredToken) -> io::Result<()> {
        match self {
            Self::Keyring(secrets) => {
                let value = serde_json::to_string(token)?;
                secrets
                    .set(&SecretScope::Global, &secret_name(credential_id)?, &value)
                    .map_err(|_| io::Error::other("failed to save provider OAuth credentials"))
            }
            Self::File(path) => {
                let mut tokens = read_tokens(path)?;
                tokens.insert(credential_id.to_owned(), token.clone());
                write_atomically(path, &serde_json::to_string(&tokens)?)
            }
        }
    }

    pub(super) fn delete(&self, credential_id: &str) -> io::Result<bool> {
        match self {
            Self::Keyring(secrets) => secrets
                .delete(&SecretScope::Global, &secret_name(credential_id)?)
                .map_err(|_| io::Error::other("failed to remove provider OAuth credentials")),
            Self::File(path) => {
                let mut tokens = read_tokens(path)?;
                let removed = tokens.remove(credential_id).is_some();
                if removed {
                    write_atomically(path, &serde_json::to_string(&tokens)?)?;
                }
                Ok(removed)
            }
        }
    }
}

fn read_tokens(path: &Path) -> io::Result<BTreeMap<String, StoredToken>> {
    match std::fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(invalid_credentials),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(BTreeMap::new()),
        Err(err) => Err(err),
    }
}

fn invalid_credentials(_: serde_json::Error) -> io::Error {
    // Deserialization errors can contain values from the credential file.
    io::Error::other("stored provider OAuth credentials are invalid")
}

fn secret_name(credential_id: &str) -> io::Result<SecretName> {
    let digest = credential_id
        .strip_prefix("provider-oauth|")
        .ok_or_else(|| io::Error::other("invalid provider OAuth credential account"))?;
    SecretName::new(&format!("PROVIDER_OAUTH_{}", digest.to_ascii_uppercase()))
        .map_err(io::Error::other)
}
