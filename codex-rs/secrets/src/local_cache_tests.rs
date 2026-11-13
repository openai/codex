//! Cached reads must observe current filesystem and keyring authority, and writes
//! must publish only committed snapshots.

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
        keyring.delete(keyring_service(), &account)?;
        assert!(backend.get(&scope, &name).is_err());
        assert_eq!(keyring.load(keyring_service(), &account)?, None);
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

        let snapshot = backend.read_snapshot()?.expect("stored ciphertext");
        backend.set(&scope, &name, "replacement")?;
        assert_eq!(snapshot.get(&scope, &name)?, Some("original".to_string()));
        assert_eq!(backend.get(&scope, &name)?, Some("replacement".to_string()));
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

#[test]
fn saves_cache_committed_contents_and_preserve_them_after_a_failed_commit() -> Result<()> {
    let _cache_lock = SECRETS_CACHE_TEST_LOCK
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    for namespace in [
        LocalSecretsNamespace::ManagedSecrets,
        LocalSecretsNamespace::CodexAuth,
        LocalSecretsNamespace::McpOAuth,
        LocalSecretsNamespace::GatewayOAuth,
    ] {
        let home = tempfile::tempdir()?;
        let keyring = Arc::new(MockKeyringStore::default());
        let backend =
            LocalSecretsBackend::new_with_namespace(home.path().to_path_buf(), keyring, namespace);
        let scope = SecretScope::Global;
        let name = SecretName::new("TEST_SECRET")?;
        backend.set(&scope, &name, "committed")?;
        let committed = Arc::clone(
            &backend
                .decrypted_cache()
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .as_ref()
                .expect("successful save should populate the cache")
                .file,
        );

        // The first read after a save, including through a clone, needs no decryption.
        assert_eq!(
            backend.clone().get(&scope, &name)?,
            Some("committed".to_string())
        );
        assert!(Arc::ptr_eq(
            &committed,
            &backend
                .decrypted_cache()
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .as_ref()
                .expect("read should retain the committed snapshot")
                .file,
        ));

        // A directory at the destination makes the atomic replacement fail on
        // every platform without relying on permissions or timing.
        let path = backend.secrets_path();
        let backup = home.path().join("committed.age");
        fs::rename(&path, &backup)?;
        fs::create_dir(&path)?;
        let attempted = SecretsFile::new_empty();
        assert!(
            backend
                .save_file(&attempted, &backend.load_or_create_passphrase()?)
                .is_err()
        );
        assert!(Arc::ptr_eq(
            &committed,
            &backend
                .decrypted_cache()
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .as_ref()
                .expect("failed save should retain the committed snapshot")
                .file,
        ));
        fs::remove_dir(&path)?;
        fs::rename(&backup, &path)?;
        assert_eq!(backend.get(&scope, &name)?, Some("committed".to_string()));
    }
    Ok(())
}
