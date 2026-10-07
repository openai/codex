use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use codex_config::types::AuthKeyringBackendKind;
use codex_keyring_store::CredentialStoreError;
use codex_keyring_store::KeyringStore;
use codex_keyring_store::tests::MockKeyringStore;
use futures::FutureExt;
use pretty_assertions::assert_eq;
use serde_json::json;
use tracing_test::traced_test;

use super::*;
use crate::ema_auth_policy::EmaAuthFailure;
use crate::oauth::RefreshCredentialLock;
use crate::oauth::compute_store_key;
use crate::oauth::test_support::TempCodexHome;

#[derive(Clone, Debug, Default)]
struct TracingKeyringStore(MockKeyringStore);

impl KeyringStore for TracingKeyringStore {
    fn load(
        &self,
        service: &str,
        account: &str,
    ) -> std::result::Result<Option<String>, CredentialStoreError> {
        tracing::trace!(account, "reading enterprise keyring entry");
        self.0.load(service, account)
    }

    fn save(
        &self,
        service: &str,
        account: &str,
        value: &str,
    ) -> std::result::Result<(), CredentialStoreError> {
        self.0.save(service, account, value)
    }

    fn delete(
        &self,
        service: &str,
        account: &str,
    ) -> std::result::Result<bool, CredentialStoreError> {
        tracing::trace!(account, "deleting enterprise keyring entry");
        self.0.delete(service, account)
    }
}

#[tokio::test]
async fn invalidation_rereads_under_exclusive_lock_and_preserves_newer_grants() -> Result<()> {
    let _home = TempCodexHome::new();
    for replacement in [
        None,
        Some("refresh_token"),
        Some("client_id"),
        Some("version"),
    ] {
        let tokens: StoredOAuthTokens = serde_json::from_value(json!({
            "server_name": "ema-idp:rejected-grant", "url": "https://idp.example",
            "issuer": "https://idp.example", "client_id": "client",
            "token_response": {
                "access_token": "unused", "token_type": "Bearer", "refresh_token": "old-grant"
            },
        }))?;
        let store = ResolvedOAuthCredentialStore::keyring(AuthKeyringBackendKind::Direct);
        let keyring = MockKeyringStore::default();
        let writer =
            RefreshCredentialLock::acquire_for_server(&tokens.server_name, &tokens.url).await?;
        let versions = EnterpriseOAuthGenerationFile::open(
            &tokens.server_name,
            &tokens.url,
            EnterpriseOAuthGenerationKind::Credential,
            &writer,
        )?;
        let snapshot = EmaCredentialSnapshot {
            snapshot: StoredOAuthCredentialSnapshot::new(tokens.clone(), store),
            version: versions.replace()?,
        };
        store.save(&keyring, &tokens.server_name, &tokens)?;
        let mut invalidation =
            Box::pin(snapshot.invalidate_ema_credentials_if_current_in(&keyring));
        assert!(invalidation.as_mut().now_or_never().is_none());
        assert_eq!(
            store.load(&keyring, &tokens.server_name, &tokens.url)?,
            Some(tokens.clone())
        );

        // A login that wins the lock may change any part of the atomic credential record.
        let mut latest = tokens.clone();
        match replacement {
            Some("refresh_token") => latest
                .token_response
                .0
                .set_refresh_token(Some(oauth2::RefreshToken::new("new-grant".to_string()))),
            Some("client_id") => latest.client_id = "new-client".to_string(),
            // An identical token response can still belong to a newer login.
            Some("version") => {
                invalidate_enterprise_credential_version(&tokens.server_name, &tokens.url, &writer)?
            }
            None => {}
            Some(_) => unreachable!(),
        }
        store.save(&keyring, &tokens.server_name, &latest)?;
        drop(writer);
        invalidation.await?;
        assert_eq!(
            store.load(&keyring, &tokens.server_name, &tokens.url)?,
            replacement.map(|_| latest),
        );
        let _reader =
            RefreshCredentialLock::acquire_for_server(&tokens.server_name, &tokens.url).await?;
        assert_eq!(
            versions.current()?.as_ref() != Some(&snapshot.version),
            replacement.is_none() || replacement == Some("version"),
            "invalidation advances the version only when deleting its own grant",
        );
    }
    Ok(())
}

fn versioned_tokens() -> Result<StoredOAuthTokens> {
    let assertion = format!(
        "{}.{}.signature",
        URL_SAFE_NO_PAD.encode(br#"{"alg":"ES256"}"#),
        URL_SAFE_NO_PAD.encode(
            json!({
                "iss":"https://idp.example", "aud":"client", "sub":"user", "exp":4102444800_u64,
            })
            .to_string()
        ),
    );
    Ok(serde_json::from_value(json!({
        "server_name":"ema-idp:versioned", "url":"https://idp.example",
        "issuer":"https://idp.example", "client_id":"client",
        "token_response": {
            "access_token":"unused", "token_type":"Bearer",
            "refresh_token":"enterprise-refresh", "id_token":assertion,
        },
    }))?)
}

#[tokio::test]
async fn cached_credentials_require_the_current_version_after_waiting_for_a_writer() -> Result<()> {
    let home = TempCodexHome::new();
    for change in ["replacement", "missing", "corrupt"] {
        let tokens = versioned_tokens()?;
        let keyring = MockKeyringStore::default();
        let store = ResolvedOAuthCredentialStore::keyring(AuthKeyringBackendKind::Direct);
        store.save(&keyring, &tokens.server_name, &tokens)?;
        let snapshot = StoredOAuthCredentialSnapshot::new(tokens.clone(), store)
            .pin_ema_credentials_in(&keyring)
            .await?;
        let version_path = std::fs::read_dir(home.path().join("mcp-oauth-locks"))?
            .filter_map(std::result::Result::ok)
            .map(|entry| entry.path())
            .find(|path| {
                path.extension()
                    .is_some_and(|ext| ext == "enterprise-credential-version")
            })
            .expect("credential version");
        let writer =
            RefreshCredentialLock::acquire_for_server(&tokens.server_name, &tokens.url).await?;
        let mut acquisition = Box::pin(snapshot.acquire_current_ema_credentials());
        assert!(acquisition.as_mut().now_or_never().is_none());
        match change {
            "replacement" => {
                invalidate_enterprise_credential_version(&tokens.server_name, &tokens.url, &writer)?
            }
            "missing" => std::fs::remove_file(&version_path)?,
            "corrupt" => std::fs::write(&version_path, b"incomplete write")?,
            _ => unreachable!(),
        }
        drop(writer);
        let error = acquisition
            .await
            .expect_err("cached grant must not survive a change");
        if change != "corrupt" {
            assert_eq!(
                error.downcast_ref::<EmaAuthFailure>(),
                Some(&EmaAuthFailure::ReauthenticationRequired),
                "{change}"
            );
        }
        assert_eq!(
            store.load(&keyring, &tokens.server_name, &tokens.url)?,
            Some(tokens)
        );
        std::fs::remove_file(version_path)?;
    }
    Ok(())
}

#[tokio::test]
async fn pinning_does_not_bind_a_stale_record_to_a_new_credential_version() -> Result<()> {
    let _home = TempCodexHome::new();
    let tokens = versioned_tokens()?;
    let keyring = MockKeyringStore::default();
    let store = ResolvedOAuthCredentialStore::keyring(AuthKeyringBackendKind::Direct);
    store.save(&keyring, &tokens.server_name, &tokens)?;
    let snapshot = StoredOAuthCredentialSnapshot::new(tokens.clone(), store);
    let writer =
        RefreshCredentialLock::acquire_for_server(&tokens.server_name, &tokens.url).await?;
    let mut pinning = Box::pin(snapshot.pin_ema_credentials_in(&keyring));
    assert!(pinning.as_mut().now_or_never().is_none());
    let mut replacement = tokens;
    replacement
        .token_response
        .0
        .set_refresh_token(Some(oauth2::RefreshToken::new(
            "replacement-grant".to_string(),
        )));
    invalidate_enterprise_credential_version(&replacement.server_name, &replacement.url, &writer)?;
    store.save(&keyring, &replacement.server_name, &replacement)?;
    drop(writer);
    assert!(pinning.await.is_err());
    let current = StoredOAuthCredentialSnapshot::new(replacement, store)
        .pin_ema_credentials_in(&keyring)
        .await?;
    current.acquire_current_ema_credentials().await?;
    Ok(())
}

#[tokio::test]
async fn admitted_credentials_do_not_block_a_new_pin() -> Result<()> {
    let _home = TempCodexHome::new();
    let tokens = versioned_tokens()?;
    let keyring = MockKeyringStore::default();
    let store = ResolvedOAuthCredentialStore::keyring(AuthKeyringBackendKind::Direct);
    store.save(&keyring, &tokens.server_name, &tokens)?;
    let snapshot = StoredOAuthCredentialSnapshot::new(tokens, store);
    let pinned = snapshot.clone().pin_ema_credentials_in(&keyring).await?;
    let _active_request = pinned.acquire_current_ema_credentials().await?;

    let concurrent = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        snapshot.pin_ema_credentials_in(&keyring),
    )
    .await??;

    assert!(concurrent.version == pinned.version);
    Ok(())
}

#[tokio::test]
async fn keyring_failure_does_not_reuse_the_pinned_refresh_token() -> Result<()> {
    let _home = TempCodexHome::new();
    let tokens: StoredOAuthTokens = serde_json::from_value(json!({
        "server_name": "ema-idp:keyring-failure",
        "url": "https://idp.example",
        "issuer": "https://idp.example",
        "client_id": "client",
        "token_response": {
            "access_token": "unused",
            "token_type": "Bearer",
            "refresh_token": "stale-refresh",
        },
    }))?;
    let key = compute_store_key(&tokens.server_name, &tokens.url)?;
    let _lock = RefreshCredentialLock::acquire_for_server(&tokens.server_name, &tokens.url).await?;
    let snapshot = StoredOAuthCredentialSnapshot::new(
        tokens,
        ResolvedOAuthCredentialStore::keyring(AuthKeyringBackendKind::Direct),
    );
    let keyring = MockKeyringStore::default();
    keyring.set_error(
        &key,
        keyring::Error::Invalid("backend".into(), "unavailable".into()),
    );
    let error = snapshot
        .load_ema_credentials(&keyring)
        .expect_err("keyring failure must be terminal");
    assert!(
        error
            .to_string()
            .contains("failed to read enterprise IdP credentials from keyring")
    );
    Ok(())
}

#[traced_test]
#[tokio::test]
async fn enterprise_credential_errors_and_keyring_traces_hide_account_key() -> Result<()> {
    let home = TempCodexHome::new();
    let sentinel = "private-workspace-sentinel";
    let mut tokens = versioned_tokens()?;
    tokens.server_name = format!("ema-idp:{sentinel}");
    let key = compute_store_key(&tokens.server_name, &tokens.url)?;
    let store = ResolvedOAuthCredentialStore::keyring(AuthKeyringBackendKind::Direct);
    let keyring = TracingKeyringStore::default();
    let snapshot = StoredOAuthCredentialSnapshot::new(tokens.clone(), store);
    tracing::trace!("enterprise keyring privacy test capture enabled");
    assert!(logs_contain(
        "enterprise keyring privacy test capture enabled"
    ));

    keyring
        .0
        .set_error(&key, keyring::Error::Invalid("account".into(), key.clone()));
    let error = snapshot
        .load_ema_credentials(&keyring)
        .expect_err("backend read must fail");
    assert!(!format!("{error} {error:?} {error:#}").contains(sentinel));
    keyring
        .0
        .set_error(&key, keyring::Error::Invalid("account".into(), key.clone()));
    let error = snapshot
        .clone()
        .pin_ema_credentials_in(&keyring)
        .await
        .err()
        .expect("backend pin must fail");
    assert!(!format!("{error} {error:?} {error:#}").contains(sentinel));
    assert_eq!(error.downcast_ref::<EmaAuthFailure>(), None);

    let keyring = TracingKeyringStore::default();
    let removed = snapshot
        .clone()
        .pin_ema_credentials_in(&keyring)
        .await
        .err()
        .expect("removed grant must fail");
    assert_eq!(
        removed.downcast_ref::<EmaAuthFailure>(),
        Some(&EmaAuthFailure::ReauthenticationRequired),
    );
    assert!(!format!("{removed} {removed:?} {removed:#}").contains(sentinel));
    store.save(&keyring, &tokens.server_name, &tokens)?;
    let pinned = snapshot.clone().pin_ema_credentials_in(&keyring).await?;
    keyring
        .0
        .set_error(&key, keyring::Error::Invalid("account".into(), key.clone()));
    let error = pinned
        .invalidate_ema_credentials_if_current_in(&keyring)
        .await
        .unwrap_err();
    assert!(!format!("{error} {error:?} {error:#}").contains(sentinel));

    let successful_keyring = TracingKeyringStore::default();
    store.save(&successful_keyring, &tokens.server_name, &tokens)?;
    snapshot
        .clone()
        .pin_ema_credentials_in(&successful_keyring)
        .await?
        .invalidate_ema_credentials_if_current_in(&successful_keyring)
        .await?;
    assert_eq!(successful_keyring.0.saved_value(&key), None);

    let lock_dir = home.path().join("mcp-oauth-locks");
    std::fs::rename(&lock_dir, home.path().join("held-locks"))?;
    std::fs::write(&lock_dir, b"not a directory")?;
    for error in [
        snapshot
            .pin_ema_credentials_in(&keyring)
            .await
            .err()
            .expect("lock must fail"),
        pinned.acquire_current_ema_credentials().await.unwrap_err(),
        pinned
            .invalidate_ema_credentials_if_current_in(&keyring)
            .await
            .unwrap_err(),
    ] {
        assert!(!format!("{error} {error:?} {error:#}").contains(sentinel));
    }
    assert!(!logs_contain(sentinel));
    Ok(())
}

#[test]
fn ordinary_oauth_names_cannot_alias_enterprise_credential_keys() -> Result<()> {
    let _home = TempCodexHome::new();
    let issuer = "https://idp.example";
    let enterprise_name = "ema-idp:synthetic-identity";
    let ordinary: codex_config::McpServerConfig = serde_json::from_value(json!({
        "url": issuer,
        "oauth": {"client_id": "idp-client"},
    }))?;
    let ordinary_name = ordinary.oauth_credential_name(enterprise_name);

    pretty_assertions::assert_ne!(
        compute_store_key(&ordinary_name, issuer)?,
        compute_store_key(enterprise_name, issuer)?,
        "an ordinary server name must not select the enterprise credential namespace"
    );
    let legacy_key = compute_store_key("ordinary-server", issuer)?;
    let legacy_hash = legacy_key.split_once('|').expect("legacy key separator").1;
    pretty_assertions::assert_eq!(
        compute_store_key(&ordinary_name, issuer)?,
        format!("{enterprise_name}|{legacy_hash}"),
        "escaping the reserved prefix preserves the pre-EMA ordinary credential key"
    );

    let keyring = MockKeyringStore::default();
    let store = ResolvedOAuthCredentialStore::keyring(AuthKeyringBackendKind::Direct);
    let enterprise_tokens: StoredOAuthTokens = serde_json::from_value(json!({
        "server_name": enterprise_name, "url": issuer, "issuer": issuer,
        "client_id": "idp-client", "token_response": {
            "access_token": "unused", "token_type": "Bearer", "refresh_token": "enterprise-refresh"
        }
    }))?;
    store.save(&keyring, enterprise_name, &enterprise_tokens)?;
    let mut ordinary_tokens = enterprise_tokens.clone();
    ordinary_tokens.server_name = ordinary_name.to_string();
    store.save(&keyring, &ordinary_name, &ordinary_tokens)?;
    assert!(store.delete(&keyring, &ordinary_name, issuer)?);
    pretty_assertions::assert_eq!(
        store.load(&keyring, enterprise_name, issuer)?,
        Some(enterprise_tokens),
        "ordinary OAuth save/logout must not overwrite or remove the enterprise entry"
    );
    Ok(())
}
