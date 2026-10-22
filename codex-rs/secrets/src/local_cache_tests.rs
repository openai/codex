//! Cached reads must observe current filesystem and keyring authority.

use super::*;
use codex_keyring_store::tests::MockKeyringStore;
use keyring::Error as KeyringError;
use pretty_assertions::assert_eq;

#[test]
fn cached_reads_check_file_presence_key_rotation_and_keyring_errors() -> Result<()> {
    let _cache_lock = SECRETS_CACHE_TEST_LOCK
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    for namespace in [
        LocalSecretsNamespace::ManagedSecrets,
        LocalSecretsNamespace::CodexAuth,
    ] {
        let home = tempfile::tempdir()?;
        let keyring = Arc::new(MockKeyringStore::default());
        let backend = LocalSecretsBackend::new_with_namespace(
            home.path().to_path_buf(),
            keyring.clone(),
            namespace,
        );
        let scope = SecretScope::Global;
        let name = SecretName::new("TEST_SECRET")?;
        backend.set(&scope, &name, "original")?;
        assert_eq!(backend.get(&scope, &name)?, Some("original".to_string()));
        let ciphertext = fs::read(backend.secrets_path())?;
        let account = compute_keyring_account(home.path(), namespace);
        let original_key = keyring
            .load(keyring_service(), &account)?
            .expect("persisted encryption key");

        // A cached snapshot cannot make a removed file appear to exist.
        fs::remove_file(backend.secrets_path())?;
        assert_eq!(backend.get(&scope, &name)?, None);
        assert_eq!(backend.list(/*scope_filter*/ None)?, Vec::new());
        fs::write(backend.secrets_path(), &ciphertext)?;
        assert_eq!(backend.get(&scope, &name)?, Some("original".to_string()));

        // Neither a changed encryption key nor a keyring error may reuse plaintext.
        let replacement_key = generate_passphrase()?;
        keyring.save(keyring_service(), &account, replacement_key.expose_secret())?;
        assert!(backend.get(&scope, &name).is_err());
        keyring.save(keyring_service(), &account, &original_key)?;
        assert_eq!(backend.get(&scope, &name)?, Some("original".to_string()));
        keyring.set_error(
            &account,
            KeyringError::PlatformFailure(Box::new(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "keyring is locked",
            ))),
        );
        assert!(backend.get(&scope, &name).is_err());

        fs::write(backend.secrets_path(), b"invalid ciphertext")?;
        assert!(backend.get(&scope, &name).is_err());
        fs::write(backend.secrets_path(), &ciphertext)?;
        assert_eq!(backend.get(&scope, &name)?, Some("original".to_string()));
    }
    Ok(())
}

#[test]
fn shared_caches_keep_homes_and_namespaces_isolated() -> Result<()> {
    let _cache_lock = SECRETS_CACHE_TEST_LOCK
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    let homes = [tempfile::tempdir()?, tempfile::tempdir()?];
    let keyring = Arc::new(MockKeyringStore::default());
    let scope = SecretScope::Global;
    let name = SecretName::new("TEST_SECRET")?;
    let mut backends = Vec::new();
    for (index, home) in homes.iter().enumerate() {
        for namespace in [
            LocalSecretsNamespace::ManagedSecrets,
            LocalSecretsNamespace::CodexAuth,
            LocalSecretsNamespace::McpOAuth,
        ] {
            let backend = LocalSecretsBackend::new_with_namespace(
                home.path().to_path_buf(),
                keyring.clone(),
                namespace,
            );
            let value = format!("{index}-{namespace:?}");
            backend.set(&scope, &name, &value)?;
            backends.push((backend, value));
        }
    }
    for (backend, value) in backends.iter().rev().chain(backends.iter()) {
        assert_eq!(backend.get(&scope, &name)?, Some(value.clone()));
        assert_eq!(
            backend.list(Some(&scope))?,
            vec![SecretListEntry {
                scope: scope.clone(),
                name: name.clone(),
            }]
        );
    }
    Ok(())
}
