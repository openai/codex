use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::sync::atomic::compiler_fence;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use age::decrypt;
use age::encrypt;
use age::scrypt::Identity as ScryptIdentity;
use age::scrypt::Recipient as ScryptRecipient;
use age::secrecy::ExposeSecret;
use age::secrecy::SecretString;
use anyhow::Context;
use anyhow::Result;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use codex_keyring_store::KeyringStore;
use rand::TryRngCore;
use rand::rngs::OsRng;
use serde::Deserialize;
use serde::Serialize;
use tracing::warn;

use super::SecretListEntry;
use super::SecretName;
use super::SecretScope;
use super::SecretsBackend;
use super::compute_keyring_account;
use super::keyring_service;

const SECRETS_VERSION: u8 = 1;
const LOCAL_SECRETS_FILENAME: &str = "local.age";

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
struct SecretsFile {
    version: u8,
    secrets: BTreeMap<String, String>,
}

impl SecretsFile {
    fn new_empty() -> Self {
        Self {
            version: SECRETS_VERSION,
            secrets: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct LocalSecretsBackend {
    codex_home: PathBuf,
    keyring_store: Arc<dyn KeyringStore>,
}

impl LocalSecretsBackend {
    pub fn new(codex_home: PathBuf, keyring_store: Arc<dyn KeyringStore>) -> Self {
        Self {
            codex_home,
            keyring_store,
        }
    }

    pub fn set(&self, scope: &SecretScope, name: &SecretName, value: &str) -> Result<()> {
        anyhow::ensure!(!value.is_empty(), "secret value must not be empty");
        let canonical_key = scope.canonical_key(name);
        let mut file = self.load_file()?;
        file.secrets.insert(canonical_key, value.to_string());
        self.save_file(&file)
    }

    pub fn get(&self, scope: &SecretScope, name: &SecretName) -> Result<Option<String>> {
        let canonical_key = scope.canonical_key(name);
        let file = self.load_file()?;
        Ok(file.secrets.get(&canonical_key).cloned())
    }

    pub fn delete(&self, scope: &SecretScope, name: &SecretName) -> Result<bool> {
        let canonical_key = scope.canonical_key(name);
        let mut file = self.load_file()?;
        let removed = file.secrets.remove(&canonical_key).is_some();
        if removed {
            self.save_file(&file)?;
        }
        Ok(removed)
    }

    pub fn list(&self, scope_filter: Option<&SecretScope>) -> Result<Vec<SecretListEntry>> {
        let file = self.load_file()?;
        let mut entries = Vec::new();
        for canonical_key in file.secrets.keys() {
            let Some(entry) = parse_canonical_key(canonical_key) else {
                warn!("skipping invalid canonical secret key: {canonical_key}");
                continue;
            };
            if let Some(scope) = scope_filter
                && entry.scope != *scope
            {
                continue;
            }
            entries.push(entry);
        }
        Ok(entries)
    }

    fn secrets_dir(&self) -> PathBuf {
        self.codex_home.join("secrets")
    }

    fn secrets_path(&self) -> PathBuf {
        self.secrets_dir().join(LOCAL_SECRETS_FILENAME)
    }

    fn load_file(&self) -> Result<SecretsFile> {
        let path = self.secrets_path();
        if !path.exists() {
            return Ok(SecretsFile::new_empty());
        }

        let ciphertext = fs::read(&path)
            .with_context(|| format!("failed to read secrets file at {}", path.display()))?;
        let passphrase = self.load_or_create_passphrase()?;
        let plaintext = decrypt_with_passphrase(&ciphertext, &passphrase)?;
        let mut parsed: SecretsFile = serde_json::from_slice(&plaintext).with_context(|| {
            format!(
                "failed to deserialize decrypted secrets file at {}",
                path.display()
            )
        })?;
        if parsed.version == 0 {
            parsed.version = SECRETS_VERSION;
        }
        anyhow::ensure!(
            parsed.version <= SECRETS_VERSION,
            "secrets file version {} is newer than supported version {}",
            parsed.version,
            SECRETS_VERSION
        );
        Ok(parsed)
    }

    fn save_file(&self, file: &SecretsFile) -> Result<()> {
        let dir = self.secrets_dir();
        fs::create_dir_all(&dir)
            .with_context(|| format!("failed to create secrets dir {}", dir.display()))?;

        let passphrase = self.load_or_create_passphrase()?;
        let plaintext = serde_json::to_vec(file).context("failed to serialize secrets file")?;
        let ciphertext = encrypt_with_passphrase(&plaintext, &passphrase)?;
        let path = self.secrets_path();
        write_file_atomically(&path, &ciphertext)?;
        Ok(())
    }

    fn load_or_create_passphrase(&self) -> Result<SecretString> {
        let account = compute_keyring_account(&self.codex_home);
        let loaded = self
            .keyring_store
            .load(keyring_service(), &account)
            .map_err(|err| anyhow::anyhow!(err.message()))
            .with_context(|| format!("failed to load secrets key from keyring for {account}"))?;
        match loaded {
            Some(existing) => Ok(SecretString::from(existing)),
            None => {
                // Generate a high-entropy key and persist it in the OS keyring.
                // This keeps secrets out of plaintext config while remaining
                // fully local/offline for the MVP.
                let generated = generate_passphrase()?;
                self.keyring_store
                    .save(keyring_service(), &account, generated.expose_secret())
                    .map_err(|err| anyhow::anyhow!(err.message()))
                    .context("failed to persist secrets key in keyring")?;
                Ok(generated)
            }
        }
    }
}

impl SecretsBackend for LocalSecretsBackend {
    fn set(&self, scope: &SecretScope, name: &SecretName, value: &str) -> Result<()> {
        LocalSecretsBackend::set(self, scope, name, value)
    }

    fn get(&self, scope: &SecretScope, name: &SecretName) -> Result<Option<String>> {
        LocalSecretsBackend::get(self, scope, name)
    }

    fn delete(&self, scope: &SecretScope, name: &SecretName) -> Result<bool> {
        LocalSecretsBackend::delete(self, scope, name)
    }

    fn list(&self, scope_filter: Option<&SecretScope>) -> Result<Vec<SecretListEntry>> {
        LocalSecretsBackend::list(self, scope_filter)
    }
}

fn write_file_atomically(path: &Path, contents: &[u8]) -> Result<()> {
    let dir = path.parent().with_context(|| {
        format!(
            "failed to compute parent directory for secrets file at {}",
            path.display()
        )
    })?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let tmp_path = dir.join(format!(
        ".{LOCAL_SECRETS_FILENAME}.tmp-{}-{nonce}",
        std::process::id()
    ));

    {
        let mut tmp_file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&tmp_path)
            .with_context(|| {
                format!(
                    "failed to create temp secrets file at {}",
                    tmp_path.display()
                )
            })?;
        tmp_file.write_all(contents).with_context(|| {
            format!(
                "failed to write temp secrets file at {}",
                tmp_path.display()
            )
        })?;
        tmp_file.sync_all().with_context(|| {
            format!("failed to sync temp secrets file at {}", tmp_path.display())
        })?;
    }

    match fs::rename(&tmp_path, path) {
        Ok(()) => Ok(()),
        Err(initial_error) => {
            #[cfg(target_os = "windows")]
            {
                if path.exists() {
                    fs::remove_file(path).with_context(|| {
                        format!(
                            "failed to remove existing secrets file at {} before replace",
                            path.display()
                        )
                    })?;
                    fs::rename(&tmp_path, path).with_context(|| {
                        format!(
                            "failed to replace secrets file at {} with {}",
                            path.display(),
                            tmp_path.display()
                        )
                    })?;
                    return Ok(());
                }
            }

            let _ = fs::remove_file(&tmp_path);
            Err(initial_error).with_context(|| {
                format!(
                    "failed to atomically replace secrets file at {} with {}",
                    path.display(),
                    tmp_path.display()
                )
            })
        }
    }
}

fn generate_passphrase() -> Result<SecretString> {
    let mut bytes = [0_u8; 32];
    let mut rng = OsRng;
    rng.try_fill_bytes(&mut bytes)
        .context("failed to generate random secrets key")?;
    // Base64 keeps the keyring payload ASCII-safe without reducing entropy.
    let encoded = BASE64_STANDARD.encode(bytes);
    wipe_bytes(&mut bytes);
    Ok(SecretString::from(encoded))
}

fn wipe_bytes(bytes: &mut [u8]) {
    for byte in bytes {
        // Volatile writes make it much harder for the compiler to elide the wipe.
        // SAFETY: `byte` is a valid mutable reference into `bytes`.
        unsafe { std::ptr::write_volatile(byte, 0) };
    }
    compiler_fence(Ordering::SeqCst);
}

fn encrypt_with_passphrase(plaintext: &[u8], passphrase: &SecretString) -> Result<Vec<u8>> {
    let recipient = ScryptRecipient::new(passphrase.clone());
    encrypt(&recipient, plaintext).context("failed to encrypt secrets file")
}

fn decrypt_with_passphrase(ciphertext: &[u8], passphrase: &SecretString) -> Result<Vec<u8>> {
    let identity = ScryptIdentity::new(passphrase.clone());
    decrypt(&identity, ciphertext).context("failed to decrypt secrets file")
}

fn parse_canonical_key(canonical_key: &str) -> Option<SecretListEntry> {
    let mut parts = canonical_key.split('/');
    let scope_kind = parts.next()?;
    match scope_kind {
        "global" => {
            let name = parts.next()?;
            if parts.next().is_some() {
                return None;
            }
            let name = SecretName::new(name).ok()?;
            Some(SecretListEntry {
                scope: SecretScope::Global,
                name,
            })
        }
        "env" => {
            let environment_id = parts.next()?;
            let name = parts.next()?;
            if parts.next().is_some() {
                return None;
            }
            let name = SecretName::new(name).ok()?;
            let scope = SecretScope::environment(environment_id.to_string()).ok()?;
            Some(SecretListEntry { scope, name })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_keyring_store::tests::MockKeyringStore;
    use keyring::Error as KeyringError;
    use pretty_assertions::assert_eq;

    #[test]
    fn load_file_rejects_newer_schema_versions() -> Result<()> {
        let codex_home = tempfile::tempdir().expect("tempdir");
        let keyring = Arc::new(MockKeyringStore::default());
        let backend = LocalSecretsBackend::new(codex_home.path().to_path_buf(), keyring);

        let file = SecretsFile {
            version: SECRETS_VERSION + 1,
            secrets: BTreeMap::new(),
        };
        backend.save_file(&file)?;

        let error = backend
            .load_file()
            .expect_err("must reject newer schema version");
        assert!(
            error.to_string().contains("newer than supported version"),
            "unexpected error: {error:#}"
        );
        Ok(())
    }

    #[test]
    fn set_fails_when_keyring_is_unavailable() -> Result<()> {
        let codex_home = tempfile::tempdir().expect("tempdir");
        let keyring = Arc::new(MockKeyringStore::default());
        let account = compute_keyring_account(codex_home.path());
        keyring.set_error(
            &account,
            KeyringError::Invalid("error".into(), "load".into()),
        );

        let backend = LocalSecretsBackend::new(codex_home.path().to_path_buf(), keyring);
        let scope = SecretScope::Global;
        let name = SecretName::new("TEST_SECRET")?;
        let error = backend
            .set(&scope, &name, "secret-value")
            .expect_err("must fail when keyring load fails");
        assert!(
            error
                .to_string()
                .contains("failed to load secrets key from keyring"),
            "unexpected error: {error:#}"
        );
        Ok(())
    }

    #[test]
    fn save_file_does_not_leave_temp_files() -> Result<()> {
        let codex_home = tempfile::tempdir().expect("tempdir");
        let keyring = Arc::new(MockKeyringStore::default());
        let backend = LocalSecretsBackend::new(codex_home.path().to_path_buf(), keyring);

        let scope = SecretScope::Global;
        let name = SecretName::new("TEST_SECRET")?;
        backend.set(&scope, &name, "one")?;
        backend.set(&scope, &name, "two")?;

        let secrets_dir = backend.secrets_dir();
        let entries = fs::read_dir(&secrets_dir)
            .with_context(|| format!("failed to read {}", secrets_dir.display()))?
            .collect::<std::io::Result<Vec<_>>>()
            .with_context(|| format!("failed to enumerate {}", secrets_dir.display()))?;

        let filenames: Vec<String> = entries
            .into_iter()
            .filter_map(|entry| entry.file_name().to_str().map(ToString::to_string))
            .collect();
        assert_eq!(filenames, vec![LOCAL_SECRETS_FILENAME.to_string()]);
        assert_eq!(backend.get(&scope, &name)?, Some("two".to_string()));
        Ok(())
    }
}
