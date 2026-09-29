//! Exercises configured storage policy and its emitted metadata.

use super::*;
use opentelemetry_sdk::metrics::data::AggregatedMetrics;
use opentelemetry_sdk::metrics::data::MetricData;
use pretty_assertions::assert_eq;

#[test]
fn storage_metrics_distinguish_explicit_file_from_secure_failure_fallback() -> anyhow::Result<()> {
    if std::env::var_os("CODEX_AUTH_STORAGE_TEST_CHILD").is_none() {
        let output = std::process::Command::new(std::env::current_exe()?)
            .args([
                "--exact",
                std::thread::current().name().unwrap(),
                "--nocapture",
            ])
            .env("CODEX_AUTH_STORAGE_TEST_CHILD", "1")
            .output()?;
        assert!(
            output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return Ok(());
    }
    use std::collections::BTreeMap;
    let metrics = codex_otel::install_global_metrics(codex_otel::MetricsClient::new(
        codex_otel::MetricsConfig::in_memory("test", "test", "1", Default::default())
            .with_runtime_reader(),
    )?);
    let mut expected = BTreeMap::new();
    for kind in [
        AuthKeyringBackendKind::Direct,
        AuthKeyringBackendKind::Secrets,
    ] {
        for (scenario, mode, operation, actual, secure, outcome, reason, error) in [
            (
                "success",
                AuthCredentialsStoreMode::Auto,
                "save",
                storage_telemetry::keyring_store(kind).as_ref(),
                "success",
                "success",
                "none",
                "none",
            ),
            (
                "cleanup_failed",
                AuthCredentialsStoreMode::Auto,
                "save",
                storage_telemetry::keyring_store(kind).as_ref(),
                "success",
                "success",
                "none",
                "none",
            ),
            (
                "success",
                AuthCredentialsStoreMode::Keyring,
                "save",
                storage_telemetry::keyring_store(kind).as_ref(),
                "success",
                "success",
                "none",
                "none",
            ),
            (
                "cleanup_failed",
                AuthCredentialsStoreMode::Keyring,
                "save",
                storage_telemetry::keyring_store(kind).as_ref(),
                "success",
                "success",
                "none",
                "none",
            ),
            (
                "denied",
                AuthCredentialsStoreMode::Auto,
                "save",
                "file",
                "error",
                "success",
                "secure_error",
                "access_denied",
            ),
            (
                "explicit",
                AuthCredentialsStoreMode::File,
                "save",
                "file",
                "not_attempted",
                "success",
                "none",
                "none",
            ),
            (
                "missing",
                AuthCredentialsStoreMode::Auto,
                "load",
                "file",
                "not_found",
                "success",
                "secure_entry_missing",
                "none",
            ),
            (
                "failed_file",
                AuthCredentialsStoreMode::Auto,
                "load",
                "file",
                "error",
                "error",
                "secure_error",
                "access_denied",
            ),
        ] {
            let home = tempdir()?;
            let keyring = MockKeyringStore::default();
            let key = match kind {
                AuthKeyringBackendKind::Direct => compute_store_key(home.path())?,
                AuthKeyringBackendKind::Secrets => {
                    compute_keyring_account(home.path(), LocalSecretsNamespace::CodexAuth)
                }
            };
            let auth = auth_with_prefix("private credential value");
            if operation == "load" || scenario == "success" {
                FileAuthStorage::new(home.path().to_path_buf()).save(&auth)?;
            }
            if scenario == "cleanup_failed" {
                std::fs::create_dir(get_auth_file(home.path()))?;
            }
            if scenario == "failed_file" {
                std::fs::write(get_auth_file(home.path()), "invalid private file contents")?;
            }
            if error == "access_denied" {
                keyring.set_error(
                    &key,
                    KeyringError::PlatformFailure(Box::new(std::io::Error::new(
                        std::io::ErrorKind::PermissionDenied,
                        "private account path error",
                    ))),
                );
                // Secrets loads consult the keyring only when an encrypted file exists.
                if operation == "load" && kind == AuthKeyringBackendKind::Secrets {
                    let manager = SecretsManager::new_with_keyring_store_and_namespace(
                        home.path().to_path_buf(),
                        SecretsBackendKind::Local,
                        Arc::new(MockKeyringStore::default()),
                        LocalSecretsNamespace::CodexAuth,
                    );
                    manager.set(&SecretScope::Global, &CODEX_AUTH_SECRET_NAME, "{}")?;
                }
            }
            let storage = create_auth_storage_with_store(
                home.path().to_path_buf(),
                mode,
                Arc::new(keyring),
                kind,
            );
            if operation == "load" {
                let result = storage.load();
                if outcome == "error" {
                    assert!(result.is_err());
                } else {
                    assert_eq!(result?, Some(auth.clone()));
                }
            } else {
                storage.save(&auth)?;
            }
            let store_mode = match mode {
                AuthCredentialsStoreMode::Auto => "auto",
                AuthCredentialsStoreMode::Keyring => "keyring",
                AuthCredentialsStoreMode::File => "file",
                AuthCredentialsStoreMode::Ephemeral => "ephemeral",
            };
            let tags: BTreeMap<String, String> = BTreeMap::from([
                ("credential_kind".into(), "codex".into()),
                ("store_mode".into(), store_mode.into()),
                (
                    "selected_store".into(),
                    storage_telemetry::keyring_store(kind).as_ref().into(),
                ),
                ("actual_store".into(), actual.into()),
                ("operation".into(), operation.into()),
                ("secure_outcome".into(), secure.into()),
                ("outcome".into(), outcome.into()),
                ("fallback_reason".into(), reason.into()),
                ("secure_error".into(), error.into()),
                ("storage_phase".into(), "policy".into()),
                ("originator".into(), "none".into()),
            ]);
            if secure == "success" {
                let mut cleanup_tags = tags.clone();
                cleanup_tags.extend([
                    ("actual_store".into(), "file".into()),
                    ("operation".into(), "cleanup".into()),
                    ("secure_outcome".into(), "not_attempted".into()),
                    (
                        "outcome".into(),
                        if scenario == "cleanup_failed" {
                            "error"
                        } else {
                            "success"
                        }
                        .into(),
                    ),
                ]);
                *expected.entry(cleanup_tags).or_insert(0) += 1;
            }
            *expected.entry(tags).or_insert(0) += 1;
        }
    }
    let snapshot = metrics.snapshot()?;
    let mut actual = Vec::new();
    for metric in snapshot
        .scope_metrics()
        .flat_map(opentelemetry_sdk::metrics::data::ScopeMetrics::metrics)
    {
        if metric.name() == "codex.auth_storage.operation"
            && let AggregatedMetrics::U64(MetricData::Sum(sum)) = metric.data()
        {
            for point in sum.data_points() {
                let tags: BTreeMap<String, String> = point
                    .attributes()
                    .map(|a| (a.key.to_string(), a.value.to_string()))
                    .collect();
                actual.push((point.value(), tags));
            }
        }
    }
    let mut expected: Vec<_> = expected
        .into_iter()
        .map(|(tags, count)| (count, tags))
        .collect();
    actual.sort();
    expected.sort();
    assert_eq!(actual, expected);
    metrics.shutdown()?;
    Ok(())
}

#[test]
fn auto_auth_storage_load_prefers_keyring_value() -> anyhow::Result<()> {
    let codex_home = tempdir()?;
    let mock_keyring = MockKeyringStore::default();
    let storage = AutoAuthStorage::new(
        codex_home.path().to_path_buf(),
        Arc::new(mock_keyring.clone()),
        AuthKeyringBackendKind::Secrets,
    );
    let keyring_auth = auth_with_prefix("keyring");
    seed_secrets_backend_with_auth(&mock_keyring, codex_home.path(), &keyring_auth)?;

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
    let storage = AutoAuthStorage::new(
        codex_home.path().to_path_buf(),
        Arc::new(mock_keyring),
        AuthKeyringBackendKind::Secrets,
    );

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
        AuthKeyringBackendKind::Secrets,
    );
    let key = compute_keyring_account(codex_home.path(), LocalSecretsNamespace::CodexAuth);

    let encrypted = auth_with_prefix("encrypted");
    seed_secrets_backend_with_auth(&mock_keyring, codex_home.path(), &encrypted)?;
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
        AuthKeyringBackendKind::Secrets,
    );
    let stale = auth_with_prefix("stale");
    storage.file_storage.save(&stale)?;

    let expected = auth_with_prefix("to-save");
    storage.save(&expected)?;

    assert_keyring_saved_auth_and_removed_fallback(&mock_keyring, codex_home.path(), &expected)?;
    Ok(())
}

#[test]
fn auto_auth_storage_save_falls_back_when_keyring_errors() -> anyhow::Result<()> {
    let codex_home = tempdir()?;
    let mock_keyring = MockKeyringStore::default();
    let storage = AutoAuthStorage::new(
        codex_home.path().to_path_buf(),
        Arc::new(mock_keyring.clone()),
        AuthKeyringBackendKind::Secrets,
    );
    let key = compute_keyring_account(codex_home.path(), LocalSecretsNamespace::CodexAuth);
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
        AuthKeyringBackendKind::Secrets,
    );
    let auth = auth_with_prefix("to-delete");
    let auth_file = seed_secrets_backend_and_fallback_auth_file_for_delete(
        &mock_keyring,
        codex_home.path(),
        &auth,
    )?;

    let removed = storage.delete()?;

    assert!(removed, "delete should report removal");
    assert_eq!(storage.load()?, None, "encrypted auth should be removed");
    assert!(
        !auth_file.exists(),
        "fallback auth.json should be removed after delete"
    );
    Ok(())
}

#[test]
fn direct_keyring_auth_storage_saves_legacy_keyring_entry() -> anyhow::Result<()> {
    let codex_home = tempdir()?;
    let mock_keyring = MockKeyringStore::default();
    let storage = DirectKeyringAuthStorage::new(
        codex_home.path().to_path_buf(),
        Arc::new(mock_keyring.clone()),
        AuthCredentialsStoreMode::Keyring,
    );
    let auth_file = get_auth_file(codex_home.path());
    std::fs::write(&auth_file, "stale")?;
    let auth = auth_with_prefix("direct");

    storage.save(&auth)?;

    let legacy_key = compute_store_key(codex_home.path())?;
    let saved_value = mock_keyring
        .saved_value(&legacy_key)
        .context("direct keyring auth entry should exist")?;
    assert_eq!(saved_value, serde_json::to_string(&auth)?);
    assert!(!encrypted_auth_file(codex_home.path()).exists());
    assert!(
        !auth_file.exists(),
        "fallback auth.json should be removed after keyring save"
    );
    assert_eq!(storage.load()?, Some(auth));
    Ok(())
}

#[test]
fn secrets_keyring_auth_storage_save_persists_and_removes_fallback_file() -> anyhow::Result<()> {
    let codex_home = tempdir()?;
    let mock_keyring = MockKeyringStore::default();
    let storage = SecretsKeyringAuthStorage::new(
        codex_home.path().to_path_buf(),
        Arc::new(mock_keyring.clone()),
        AuthCredentialsStoreMode::Keyring,
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
        agent_identity: None,
        personal_access_token: None,
        bedrock_api_key: None,
        bedrock_access_keys: None,
    };

    storage.save(&auth)?;

    assert_keyring_saved_auth_and_removed_fallback(&mock_keyring, codex_home.path(), &auth)?;
    Ok(())
}
