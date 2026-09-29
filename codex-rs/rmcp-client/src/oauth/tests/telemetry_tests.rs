//! Exercises emitted metrics through MCP storage policy and pinned-store operations.

use super::persistor_tests::authorization_manager_for;
use super::persistor_tests::test_context;
use super::*;
use codex_otel::auth_storage::AuthStorageOriginator;
use opentelemetry_sdk::metrics::data::AggregatedMetrics;
use opentelemetry_sdk::metrics::data::MetricData;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;

type MetricRecord = (String, u64, BTreeMap<String, String>);

#[test]
fn policy_and_pinned_operations_keep_distinct_outcomes_and_originator() -> Result<()> {
    if isolated_process()? {
        return Ok(());
    }
    let _env = TempCodexHome::new();
    keyring::set_default_credential_builder(keyring::mock::default_credential_builder());
    let metrics = test_metrics()?;
    let store = MockKeyringStore::default();
    let tokens = sample_tokens();
    let key = compute_store_key(&tokens.server_name, &tokens.url)?;
    store.set_error(
        &key,
        KeyringError::PlatformFailure(std::io::Error::from(std::io::ErrorKind::Unsupported).into()),
    );
    save_oauth_tokens_with_keyring_with_fallback_to_file(
        &store,
        AuthKeyringBackendKind::Direct,
        &tokens.server_name,
        &tokens,
    )?;
    store.save(KEYRING_SERVICE, &key, &serde_json::to_string(&tokens)?)?;
    let resolved = AuthStorageOriginator::from_client_name("codex_vscode")
        .sync_scope(|| {
            resolve_oauth_tokens_from_store_policy(
                &store,
                &tokens.server_name,
                &tokens.url,
                OAuthCredentialsStoreMode::Auto,
                AuthKeyringBackendKind::Direct,
            )
        })?
        .unwrap();
    let snapshot = StoredOAuthCredentialSnapshot::new(resolved.tokens, resolved.store);
    store.set_error(
        &key,
        KeyringError::PlatformFailure(std::io::Error::from(std::io::ErrorKind::TimedOut).into()),
    );
    // Pinned operations retain the original Auto policy and client on another thread.
    std::thread::scope(|scope| {
        scope
            .spawn(|| {
                // The SDK mock is empty; this probe must emit exactly one pinned observation.
                assert_eq!(
                    snapshot.reload(
                        &tokens.server_name,
                        &tokens.url,
                        OAuthCredentialsStoreMode::Auto,
                        AuthKeyringBackendKind::Direct,
                    )?,
                    None
                );
                assert!(
                    resolved
                        .store
                        .save(&store, &tokens.server_name, &tokens)
                        .is_err()
                );
                Ok::<_, anyhow::Error>(())
            })
            .join()
            .unwrap()
    })?;
    let mut expected = vec![
        expected_metric(
            /*count*/ 1,
            &[
                ("operation", "save"),
                ("actual_store", "file"),
                ("secure_outcome", "error"),
                ("fallback_reason", "secure_error"),
                ("secure_error", "unsupported"),
                ("originator", "none"),
            ],
        ),
        expected_metric(/*count*/ 1, &[]),
        expected_metric(
            /*count*/ 1,
            &[
                ("storage_phase", "pinned"),
                ("secure_outcome", "not_found"),
                ("outcome", "not_found"),
            ],
        ),
        expected_metric(
            /*count*/ 1,
            &[
                ("storage_phase", "pinned"),
                ("operation", "save"),
                ("secure_outcome", "error"),
                ("outcome", "error"),
                ("secure_error", "timeout"),
            ],
        ),
    ];
    expected.sort();
    assert_eq!(operation_metrics(&metrics)?, expected);
    metrics.shutdown()?;
    Ok(())
}

fn isolated_process() -> Result<bool> {
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
        return Ok(true);
    }
    Ok(false)
}

fn operation_metrics(metrics: &codex_otel::MetricsClient) -> Result<Vec<MetricRecord>> {
    let snapshot = metrics.snapshot()?;
    let mut actual = Vec::new();
    for metric in snapshot
        .scope_metrics()
        .flat_map(opentelemetry_sdk::metrics::data::ScopeMetrics::metrics)
    {
        if matches!(
            metric.name(),
            "codex.auth_storage.operation" | "codex.auth_storage.refresh_persist"
        ) && let AggregatedMetrics::U64(MetricData::Sum(sum)) = metric.data()
        {
            for point in sum.data_points() {
                actual.push((
                    metric.name().to_string(),
                    point.value(),
                    point
                        .attributes()
                        .map(|a| (a.key.to_string(), a.value.to_string()))
                        .collect::<BTreeMap<_, _>>(),
                ));
            }
        }
    }
    actual.sort();
    Ok(actual)
}

fn expected_metric(count: u64, tags: &[(&str, &str)]) -> MetricRecord {
    let mut attributes = BTreeMap::from([
        ("credential_kind", "mcp"),
        ("store_mode", "auto"),
        ("selected_store", "direct_keyring"),
        ("actual_store", "direct_keyring"),
        ("operation", "load"),
        ("secure_outcome", "success"),
        ("outcome", "success"),
        ("fallback_reason", "none"),
        ("secure_error", "none"),
        ("storage_phase", "policy"),
        ("originator", "codex_vscode"),
    ]);
    attributes.extend(tags.iter().copied());
    (
        if attributes["operation"] == "refresh_persist" {
            "codex.auth_storage.refresh_persist"
        } else {
            "codex.auth_storage.operation"
        }
        .into(),
        count,
        attributes
            .into_iter()
            .map(|(k, v)| (k.into(), v.into()))
            .collect(),
    )
}

fn test_metrics() -> Result<codex_otel::MetricsClient> {
    Ok(codex_otel::install_global_metrics(
        codex_otel::MetricsClient::new(
            codex_otel::MetricsConfig::in_memory("test", "test", "1", Default::default())
                .with_runtime_reader(),
        )?,
    ))
}

#[test]
fn cleanup_failure_retains_policy_and_remains_best_effort() -> Result<()> {
    if isolated_process()? {
        return Ok(());
    }
    let _env = TempCodexHome::new();
    let metrics = test_metrics()?;
    let tokens = sample_tokens();
    let key = compute_store_key(&tokens.server_name, &tokens.url)?;
    std::fs::write(fallback_file_path()?, "{")?;
    let mut expected = Vec::new();
    AuthStorageOriginator::from_client_name("codex_vscode").sync_scope(|| -> Result<()> {
        for (mode, mode_tag) in [
            (OAuthCredentialsStoreMode::Auto, "auto"),
            (OAuthCredentialsStoreMode::Keyring, "keyring"),
        ] {
            let store = MockKeyringStore::default();
            save_oauth_tokens_with_keyring_and_cleanup_file(
                &store,
                mode,
                AuthKeyringBackendKind::Direct,
                &tokens.server_name,
                &tokens,
            )?;
            assert_eq!(
                store.saved_value(&key),
                Some(serde_json::to_string(&tokens)?)
            );
            expected.push(expected_metric(
                /*count*/ 1,
                &[
                    ("store_mode", mode_tag),
                    ("operation", "cleanup"),
                    ("actual_store", "file"),
                    ("secure_outcome", "not_attempted"),
                    ("outcome", "error"),
                ],
            ));
        }
        Ok(())
    })?;
    expected.sort();
    assert_eq!(operation_metrics(&metrics)?, expected);
    metrics.shutdown()?;
    Ok(())
}

#[test]
fn lock_errors_retain_typed_categories_through_wrappers() -> Result<()> {
    if isolated_process()? {
        return Ok(());
    }
    let metrics = test_metrics()?;
    let mut expected = Vec::new();
    AuthStorageOriginator::from_client_name("codex_vscode").sync_scope(|| {
        for (duration, category) in [
            (Duration::ZERO, "locked"),
            (Duration::from_secs(/*secs*/ 1), "timeout"),
        ] {
            for wrapped in [false, true] {
                let error = OAuthStoreLockFailure::Timeout {
                    store: OAuthStore::Secrets,
                    path: "private-lock-path".into(),
                    acquire_timeout: duration,
                };
                let error = if wrapped {
                    Error::from(OAuthKeyringLoadError::StoreLock(error)).context("private context")
                } else {
                    Error::from(error)
                };
                let result: Result<Option<()>> = Err(error);
                let mut observation = telemetry::policy(
                    OAuthCredentialsStoreMode::Auto,
                    AuthKeyringBackendKind::Secrets,
                    Operation::Load,
                );
                observation.record_load_attempt(Store::Secrets, &result);
                telemetry::record_secure_error(
                    &mut observation,
                    result.as_ref().unwrap_err().as_ref(),
                );
            }
            expected.push(expected_metric(
                /*count*/ 2,
                &[
                    ("selected_store", "secrets"),
                    ("actual_store", "secrets"),
                    ("secure_outcome", "error"),
                    ("outcome", "error"),
                    ("secure_error", category),
                ],
            ));
        }
    });
    expected.sort();
    assert_eq!(operation_metrics(&metrics)?, expected);
    metrics.shutdown()?;
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn coordinated_refresh_persistence_records_success_and_failure() -> Result<()> {
    use rmcp::transport::auth::CredentialStore;

    if isolated_process()? {
        return Ok(());
    }
    let _env = TempCodexHome::new();
    let metrics = test_metrics()?;
    for fail_save in [false, true] {
        let keyring = MockKeyringStore::default();
        let tokens = sample_tokens();
        let key = compute_store_key(&tokens.server_name, &tokens.url)?;
        keyring.save(KEYRING_SERVICE, &key, &serde_json::to_string(&tokens)?)?;
        let resolved = AuthStorageOriginator::from_client_name("codex_vscode")
            .sync_scope(|| {
                resolve_oauth_tokens_from_store_policy(
                    &keyring,
                    &tokens.server_name,
                    &tokens.url,
                    OAuthCredentialsStoreMode::Auto,
                    AuthKeyringBackendKind::Direct,
                )
            })?
            .unwrap();
        let store = OAuthCredentialStore::new(
            tokens,
            resolved.store,
            keyring.clone(),
            /*oauth_config*/ None,
        );
        let _guard = store.acquire_transaction_guard().await?;
        let mut credentials = store.load().await?.unwrap();
        credentials
            .token_response
            .as_mut()
            .unwrap()
            .set_access_token(AccessToken::new("refreshed-access".into()));
        if fail_save {
            keyring.set_error(
                &key,
                KeyringError::PlatformFailure(
                    std::io::Error::from(std::io::ErrorKind::TimedOut).into(),
                ),
            );
        }
        let _ = operation_metrics(&metrics)?;
        assert_eq!(store.save(credentials).await.is_err(), fail_save);
        let outcome = if fail_save { "error" } else { "success" };
        let expected = ["save", "refresh_persist"].map(|operation| {
            expected_metric(
                /*count*/ 1,
                &[
                    ("operation", operation),
                    ("storage_phase", "pinned"),
                    ("secure_outcome", outcome),
                    ("outcome", outcome),
                    ("secure_error", if fail_save { "timeout" } else { "none" }),
                ],
            )
        });
        assert_eq!(operation_metrics(&metrics)?, expected);
    }
    metrics.shutdown()?;
    Ok(())
}

#[expect(
    clippy::await_holding_invalid_type,
    reason = "The legacy authorization manager serializes refresh through its Tokio mutex"
)]
#[tokio::test(flavor = "current_thread")]
async fn legacy_refresh_persistence_records_each_write_once() -> Result<()> {
    use wiremock::Mock;
    use wiremock::ResponseTemplate;
    use wiremock::matchers::method;
    use wiremock::matchers::path;

    if isolated_process()? {
        return Ok(());
    }
    let metrics = test_metrics()?;
    for proactive in [false, true] {
        for fail_save in [false, true] {
            let (_env, server, initial) = test_context().await?;
            save_oauth_tokens_to_file(&initial)?;
            let file = fallback_file_path()?;
            Mock::given(method("POST"))
                .and(path("/oauth/token"))
                .respond_with(move |_: &wiremock::Request| {
                    if fail_save {
                        std::fs::write(&file, "{").unwrap();
                    }
                    ResponseTemplate::new(/*s*/ 200).set_body_json(serde_json::json!({
                        "access_token": "refreshed-access", "token_type": "Bearer",
                        "refresh_token": "rotated-refresh", "expires_in": 3600,
                    }))
                })
                .expect(1)
                .mount(&server)
                .await;
            let manager = Arc::new(Mutex::new(authorization_manager_for(&initial).await?));
            let authority = AuthStorageOriginator::from_client_name("codex_vscode")
                .sync_scope(ResolvedOAuthCredentialStore::file);
            let persistor = OAuthPersistor::new(
                initial.server_name.clone(),
                initial.url.clone(),
                Arc::clone(&manager),
                authority,
                Some(initial),
                /*oauth_config*/ None,
            );
            let result = if proactive {
                persistor.refresh_if_needed().await
            } else {
                manager.lock().await.refresh_token().await?;
                persistor.persist_if_needed().await
            };
            assert_eq!(result.is_err(), fail_save);
            let mut expected = Vec::new();
            for operation in if proactive {
                vec!["load", "save", "refresh_persist"]
            } else {
                vec!["save", "refresh_persist"]
            } {
                expected.push(expected_metric(
                    /*count*/ 1,
                    &[
                        ("operation", operation),
                        ("store_mode", "file"),
                        ("actual_store", "file"),
                        ("storage_phase", "pinned"),
                        ("secure_outcome", "not_attempted"),
                        (
                            "outcome",
                            if fail_save && operation != "load" {
                                "error"
                            } else {
                                "success"
                            },
                        ),
                    ],
                ));
            }
            expected.sort();
            assert_eq!(operation_metrics(&metrics)?, expected);
            if !fail_save {
                // The proactive path installs the persisted response; the reactive path updates
                // its snapshot. Neither should emit another milestone without another write.
                persistor.persist_if_needed().await?;
                assert_eq!(operation_metrics(&metrics)?, Vec::new());
            }
            server.verify().await;
        }
    }
    metrics.shutdown()?;
    Ok(())
}
