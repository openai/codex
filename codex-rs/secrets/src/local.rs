use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use anyhow::Context;
use anyhow::Result;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use chacha20poly1305::XChaCha20Poly1305;
use chacha20poly1305::XNonce;
use chacha20poly1305::aead::Aead;
use chacha20poly1305::aead::KeyInit;
use chacha20poly1305::aead::Payload;
use codex_keyring_store::KeyringStore;
use rand::TryRngCore;
use rand::rngs::OsRng;
use serde::Deserialize;
use serde::Serialize;
use tracing::warn;
use zeroize::Zeroizing;

use super::SecretListEntry;
use super::SecretName;
use super::SecretScope;
use super::SecretsBackend;
use super::compute_keyring_account;
use super::keyring_service;

const SECRETS_VERSION: u8 = 1;
const LOCAL_SECRETS_FILENAME: &str = "local.secrets";
const LEGACY_LOCAL_SECRETS_FILENAME: &str = "local.age";
const LOCAL_SECRETS_MAGIC: &[u8] = b"codex-local-secrets-v2\n";
const LOCAL_SECRETS_AAD: &[u8] = b"codex-local-secrets-v2";
const LOCAL_SECRETS_KEY_BYTES: usize = 32;
const LOCAL_SECRETS_NONCE_BYTES: usize = 24;

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

    fn legacy_secrets_path(&self) -> PathBuf {
        self.secrets_dir().join(LEGACY_LOCAL_SECRETS_FILENAME)
    }

    fn load_file(&self) -> Result<SecretsFile> {
        let path = self.secrets_path();
        if !path.exists() {
            let legacy_path = self.legacy_secrets_path();
            anyhow::ensure!(
                !legacy_path.exists(),
                "found legacy age-encrypted secrets file at {}; this version cannot read it",
                legacy_path.display()
            );
            return Ok(SecretsFile::new_empty());
        }

        let ciphertext = fs::read(&path)
            .with_context(|| format!("failed to read secrets file at {}", path.display()))?;
        let key = self.load_or_create_key()?;
        let plaintext = decrypt_with_key(&ciphertext, &key)?;
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

        let key = self.load_or_create_key()?;
        let plaintext =
            Zeroizing::new(serde_json::to_vec(file).context("failed to serialize secrets file")?);
        let ciphertext = encrypt_with_key(&plaintext, &key)?;
        let path = self.secrets_path();
        write_file_atomically(&path, &ciphertext)?;
        Ok(())
    }

    fn load_or_create_key(&self) -> Result<Zeroizing<[u8; LOCAL_SECRETS_KEY_BYTES]>> {
        let account = compute_keyring_account(&self.codex_home);
        let loaded = self
            .keyring_store
            .load(keyring_service(), &account)
            .map_err(|err| anyhow::anyhow!(err.message()))
            .with_context(|| format!("failed to load secrets key from keyring for {account}"))?;
        match loaded {
            Some(existing) => decode_key(&existing).with_context(|| {
                format!("failed to decode secrets key from keyring for {account}")
            }),
            None => {
                // Generate a high-entropy key and persist it in the OS keyring.
                // This keeps secrets out of plaintext config while remaining
                // fully local/offline for the MVP.
                let generated = generate_key()?;
                let encoded = Zeroizing::new(BASE64_STANDARD.encode(generated.as_slice()));
                self.keyring_store
                    .save(keyring_service(), &account, encoded.as_str())
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

fn generate_key() -> Result<Zeroizing<[u8; LOCAL_SECRETS_KEY_BYTES]>> {
    let mut bytes = Zeroizing::new([0_u8; LOCAL_SECRETS_KEY_BYTES]);
    let mut rng = OsRng;
    rng.try_fill_bytes(&mut *bytes)
        .context("failed to generate random secrets key")?;
    Ok(bytes)
}

fn decode_key(encoded: &str) -> Result<Zeroizing<[u8; LOCAL_SECRETS_KEY_BYTES]>> {
    let decoded = Zeroizing::new(
        BASE64_STANDARD
            .decode(encoded)
            .context("secrets key is not valid base64")?,
    );
    anyhow::ensure!(
        decoded.len() == LOCAL_SECRETS_KEY_BYTES,
        "secrets key must be {LOCAL_SECRETS_KEY_BYTES} bytes"
    );
    let mut key = Zeroizing::new([0_u8; LOCAL_SECRETS_KEY_BYTES]);
    key.copy_from_slice(decoded.as_slice());
    Ok(key)
}

fn encrypt_with_key(plaintext: &[u8], key: &[u8; LOCAL_SECRETS_KEY_BYTES]) -> Result<Vec<u8>> {
    let mut nonce_bytes = [0_u8; LOCAL_SECRETS_NONCE_BYTES];
    let mut rng = OsRng;
    rng.try_fill_bytes(&mut nonce_bytes)
        .context("failed to generate secrets file nonce")?;
    let nonce = XNonce::from_slice(&nonce_bytes);
    let cipher = XChaCha20Poly1305::new_from_slice(key)
        .map_err(|_| anyhow::anyhow!("invalid secrets key length"))?;
    let encrypted = cipher
        .encrypt(
            nonce,
            Payload {
                msg: plaintext,
                aad: LOCAL_SECRETS_AAD,
            },
        )
        .map_err(|_| anyhow::anyhow!("failed to encrypt secrets file"))?;

    let mut envelope =
        Vec::with_capacity(LOCAL_SECRETS_MAGIC.len() + nonce_bytes.len() + encrypted.len());
    envelope.extend_from_slice(LOCAL_SECRETS_MAGIC);
    envelope.extend_from_slice(&nonce_bytes);
    envelope.extend_from_slice(&encrypted);
    Ok(envelope)
}

fn decrypt_with_key(
    envelope: &[u8],
    key: &[u8; LOCAL_SECRETS_KEY_BYTES],
) -> Result<Zeroizing<Vec<u8>>> {
    anyhow::ensure!(
        envelope.starts_with(LOCAL_SECRETS_MAGIC),
        "unsupported local secrets file format"
    );
    let encrypted_offset = LOCAL_SECRETS_MAGIC.len() + LOCAL_SECRETS_NONCE_BYTES;
    anyhow::ensure!(
        envelope.len() >= encrypted_offset,
        "local secrets file is truncated"
    );
    let nonce = XNonce::from_slice(
        &envelope[LOCAL_SECRETS_MAGIC.len()..LOCAL_SECRETS_MAGIC.len() + LOCAL_SECRETS_NONCE_BYTES],
    );
    let encrypted = &envelope[encrypted_offset..];
    let cipher = XChaCha20Poly1305::new_from_slice(key)
        .map_err(|_| anyhow::anyhow!("invalid secrets key length"))?;
    let plaintext = cipher
        .decrypt(
            nonce,
            Payload {
                msg: encrypted,
                aad: LOCAL_SECRETS_AAD,
            },
        )
        .map_err(|_| anyhow::anyhow!("failed to decrypt secrets file"))?;
    Ok(Zeroizing::new(plaintext))
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
    fn save_file_writes_versioned_encrypted_envelope() -> Result<()> {
        let codex_home = tempfile::tempdir().expect("tempdir");
        let keyring = Arc::new(MockKeyringStore::default());
        let backend = LocalSecretsBackend::new(codex_home.path().to_path_buf(), keyring);
        let file = SecretsFile {
            version: SECRETS_VERSION,
            secrets: BTreeMap::from([(
                "global/GITHUB_TOKEN".to_string(),
                "secret-value".to_string(),
            )]),
        };

        backend.save_file(&file)?;

        let ciphertext = fs::read(backend.secrets_path())?;
        assert!(ciphertext.starts_with(LOCAL_SECRETS_MAGIC));
        assert!(
            !String::from_utf8_lossy(&ciphertext).contains("secret-value"),
            "secrets file must not store plaintext secret values"
        );
        assert_eq!(backend.load_file()?, file);
        Ok(())
    }

    #[test]
    fn load_file_rejects_unknown_envelope_format() -> Result<()> {
        let codex_home = tempfile::tempdir().expect("tempdir");
        let keyring = Arc::new(MockKeyringStore::default());
        let backend = LocalSecretsBackend::new(codex_home.path().to_path_buf(), keyring);
        fs::create_dir_all(backend.secrets_dir())?;
        fs::write(backend.secrets_path(), b"age-encryption.org/v1")?;

        let error = backend
            .load_file()
            .expect_err("unknown envelope format should be rejected");
        assert!(
            error
                .to_string()
                .contains("unsupported local secrets file format"),
            "unexpected error: {error:#}"
        );
        Ok(())
    }

    #[test]
    fn load_file_rejects_legacy_age_file() -> Result<()> {
        let codex_home = tempfile::tempdir().expect("tempdir");
        let keyring = Arc::new(MockKeyringStore::default());
        let backend = LocalSecretsBackend::new(codex_home.path().to_path_buf(), keyring);
        fs::create_dir_all(backend.secrets_dir())?;
        fs::write(backend.legacy_secrets_path(), b"age-encryption.org/v1")?;

        let error = backend
            .load_file()
            .expect_err("legacy age format should be rejected");
        assert!(
            error
                .to_string()
                .contains("found legacy age-encrypted secrets file"),
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
