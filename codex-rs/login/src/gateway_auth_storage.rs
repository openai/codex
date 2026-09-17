//! Persists gateway credentials and serializes exchanges across all configurations in one store.

use std::fs::File;
use std::fs::OpenOptions;
use std::fs::TryLockError;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use codex_keyring_store::KeyringStore;
use codex_secrets::LocalSecretsNamespace;
use codex_secrets::SecretName;
use codex_secrets::SecretScope;
use codex_secrets::SecretsBackendKind;
use codex_secrets::SecretsManager;

pub(super) async fn lock_credentials(codex_home: &Path) -> io::Result<File> {
    let directory = codex_home.join("secrets");
    std::fs::create_dir_all(&directory)?;
    // Configurations have separate credential entries but rewrite the same encrypted file.
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

pub(super) struct GatewayAuthStorage(SecretsManager);

impl GatewayAuthStorage {
    pub(super) fn new(codex_home: PathBuf, keyring: Arc<dyn KeyringStore>) -> Self {
        Self(SecretsManager::new_with_keyring_store_and_namespace(
            codex_home,
            SecretsBackendKind::Local,
            keyring,
            LocalSecretsNamespace::GatewayOAuth,
        ))
    }

    pub(super) fn load(&self, credential_id: &str) -> io::Result<Option<String>> {
        self.0
            .get(&SecretScope::Global, &secret_name(credential_id)?)
            .map_err(|_| io::Error::other("failed to load provider OAuth credentials"))
    }

    pub(super) fn save(&self, credential_id: &str, value: &str) -> io::Result<()> {
        self.0
            .set(&SecretScope::Global, &secret_name(credential_id)?, value)
            .map_err(|_| io::Error::other("failed to save provider OAuth credentials"))
    }
}

fn secret_name(credential_id: &str) -> io::Result<SecretName> {
    let digest = credential_id
        .strip_prefix("provider-oauth|")
        .ok_or_else(|| io::Error::other("invalid provider OAuth credential account"))?;
    SecretName::new(&format!("PROVIDER_OAUTH_{}", digest.to_ascii_uppercase()))
        .map_err(io::Error::other)
}
