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

fn seed_windows_keyring_with_auth(
    mock_keyring: &MockKeyringStore,
    key: &str,
    auth: &AuthDotJson,
) -> anyhow::Result<()> {
    mock_keyring.save(
        KEYRING_SERVICE,
        &KeyringAuthStorage::field_key(key, AUTH_MODE_FIELD),
        &serde_json::to_string(&auth.auth_mode)?,
    )?;
    mock_keyring.save(
        KEYRING_SERVICE,
        &KeyringAuthStorage::field_key(key, OPENAI_API_KEY_FIELD),
        &serde_json::to_string(&auth.openai_api_key)?,
    )?;
    mock_keyring.save(
        KEYRING_SERVICE,
        &KeyringAuthStorage::field_key(key, TOKENS_FIELD),
        &serde_json::to_string(&auth.tokens)?,
    )?;
    mock_keyring.save(
        KEYRING_SERVICE,
        &KeyringAuthStorage::field_key(key, LAST_REFRESH_FIELD),
        &serde_json::to_string(&auth.last_refresh)?,
    )?;
    Ok(())
}

fn assert_keyring_saved_auth_and_removed_fallback(
    mock_keyring: &MockKeyringStore,
    key: &str,
    codex_home: &Path,
    expected: &AuthDotJson,
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

fn assert_windows_keyring_saved_auth_and_removed_fallback(
    mock_keyring: &MockKeyringStore,
    key: &str,
    codex_home: &Path,
    expected: &AuthDotJson,
) {
    let auth_mode_key = KeyringAuthStorage::field_key(key, AUTH_MODE_FIELD);
    let openai_api_key_key = KeyringAuthStorage::field_key(key, OPENAI_API_KEY_FIELD);
    let tokens_key = KeyringAuthStorage::field_key(key, TOKENS_FIELD);
    let last_refresh_key = KeyringAuthStorage::field_key(key, LAST_REFRESH_FIELD);

    assert_eq!(
        mock_keyring.saved_value(&auth_mode_key),
        Some(serde_json::to_string(&expected.auth_mode).expect("serialize auth_mode")),
    );
    assert_eq!(
        mock_keyring.saved_value(&openai_api_key_key),
        Some(serde_json::to_string(&expected.openai_api_key).expect("serialize openai_api_key")),
    );
    assert_eq!(
        mock_keyring.saved_value(&tokens_key),
        Some(serde_json::to_string(&expected.tokens).expect("serialize tokens")),
    );
    assert_eq!(
        mock_keyring.saved_value(&last_refresh_key),
        Some(serde_json::to_string(&expected.last_refresh).expect("serialize last_refresh")),
    );
    assert!(
        mock_keyring.saved_value(key).is_none(),
        "legacy whole-auth key should be removed after windows keyring save"
    );
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
fn keyring_auth_storage_load_returns_deserialized_auth() -> anyhow::Result<()> {
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
    assert_keyring_saved_auth_and_removed_fallback(&mock_keyring, &key, codex_home.path(), &auth);
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
    let (key, auth_file) =
        seed_keyring_and_fallback_auth_file_for_delete(&mock_keyring, codex_home.path(), || {
            compute_store_key(codex_home.path())
        })?;

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
fn keyring_auth_storage_windows_load_combines_split_entries() -> anyhow::Result<()> {
    let codex_home = tempdir()?;
    let mock_keyring = MockKeyringStore::default();
    let storage = KeyringAuthStorage::new(
        codex_home.path().to_path_buf(),
        Arc::new(mock_keyring.clone()),
    );
    let expected = auth_with_prefix("windows-load");
    let key = compute_store_key(codex_home.path())?;
    seed_windows_keyring_with_auth(&mock_keyring, &key, &expected)?;

    let loaded = storage.load_windows_auth_from_keyring(&key)?;

    assert_eq!(Some(expected), loaded);
    Ok(())
}

#[test]
fn keyring_auth_storage_windows_load_ignores_legacy_entry() -> anyhow::Result<()> {
    let codex_home = tempdir()?;
    let mock_keyring = MockKeyringStore::default();
    let storage = KeyringAuthStorage::new(
        codex_home.path().to_path_buf(),
        Arc::new(mock_keyring.clone()),
    );
    let expected = auth_with_prefix("windows-legacy");
    let key = compute_store_key(codex_home.path())?;
    seed_keyring_with_auth(&mock_keyring, || Ok(key.clone()), &expected)?;

    let loaded = storage.load_windows_auth_from_keyring(&key)?;

    assert_eq!(None, loaded);
    Ok(())
}

#[test]
fn keyring_auth_storage_windows_save_persists_split_entries() -> anyhow::Result<()> {
    let codex_home = tempdir()?;
    let mock_keyring = MockKeyringStore::default();
    let storage = KeyringAuthStorage::new(
        codex_home.path().to_path_buf(),
        Arc::new(mock_keyring.clone()),
    );
    let auth_file = get_auth_file(codex_home.path());
    std::fs::write(&auth_file, "stale")?;
    let auth = auth_with_prefix("windows-save");
    let key = compute_store_key(codex_home.path())?;
    mock_keyring.save(KEYRING_SERVICE, &key, "legacy-auth")?;

    storage.save_windows_auth_to_keyring(&key, &auth)?;
    if let Err(err) = delete_file_if_exists(codex_home.path()) {
        panic!("failed to remove fallback auth file in test: {err}");
    }

    assert_windows_keyring_saved_auth_and_removed_fallback(
        &mock_keyring,
        &key,
        codex_home.path(),
        &auth,
    );
    Ok(())
}

#[test]
fn keyring_auth_storage_windows_delete_removes_split_entries_and_file() -> anyhow::Result<()> {
    let codex_home = tempdir()?;
    let mock_keyring = MockKeyringStore::default();
    let storage = KeyringAuthStorage::new(
        codex_home.path().to_path_buf(),
        Arc::new(mock_keyring.clone()),
    );
    let key = compute_store_key(codex_home.path())?;
    let auth = auth_with_prefix("windows-delete");
    seed_windows_keyring_with_auth(&mock_keyring, &key, &auth)?;
    mock_keyring.save(KEYRING_SERVICE, &key, "legacy-auth")?;
    let auth_file = get_auth_file(codex_home.path());
    std::fs::write(&auth_file, "stale")?;

    let removed = storage.delete_windows_auth_from_keyring(&key)?;
    let file_removed = delete_file_if_exists(codex_home.path())?;

    assert!(removed, "delete should report removal");
    assert!(file_removed, "fallback auth.json should be removed");
    assert!(!mock_keyring.contains(&KeyringAuthStorage::field_key(
        key.as_str(),
        AUTH_MODE_FIELD
    )));
    assert!(!mock_keyring.contains(&KeyringAuthStorage::field_key(
        key.as_str(),
        OPENAI_API_KEY_FIELD,
    )));
    assert!(!mock_keyring.contains(&KeyringAuthStorage::field_key(key.as_str(), TOKENS_FIELD)));
    assert!(!mock_keyring.contains(&KeyringAuthStorage::field_key(
        key.as_str(),
        LAST_REFRESH_FIELD,
    )));
    assert!(!mock_keyring.contains(&key));
    assert!(!auth_file.exists());
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
    mock_keyring.set_error(&key, KeyringError::Invalid("error".into(), "load".into()));

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
    let storage = AutoAuthStorage::new(
        codex_home.path().to_path_buf(),
        Arc::new(mock_keyring.clone()),
    );
    let key = compute_store_key(codex_home.path())?;
    mock_keyring.set_error(&key, KeyringError::Invalid("error".into(), "save".into()));

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
    let (key, auth_file) =
        seed_keyring_and_fallback_auth_file_for_delete(&mock_keyring, codex_home.path(), || {
            compute_store_key(codex_home.path())
        })?;

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
