//! Verifies guarded mutations, pinned storage, token mapping, and saved runtime snapshots.

use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use anyhow::Result;
use codex_config::types::AuthKeyringBackendKind;
use keyring::Error as KeyringError;
use oauth2::AccessToken;
use oauth2::RefreshToken;
use oauth2::TokenResponse;
use pretty_assertions::assert_eq;
use rmcp::transport::auth::AuthError;
use rmcp::transport::auth::CredentialStore;

use super::MockKeyringStore;
use super::TempCodexHome;
use super::assert_tokens_match_without_expiry;
use super::sample_tokens;
use crate::oauth::OAuthCredentialStore;
use crate::oauth::RefreshCredentialLock;
use crate::oauth::ResolvedOAuthCredentialStore;
use crate::oauth::compute_store_key;
use crate::oauth::load_oauth_tokens_from_file;
use crate::oauth::normalized_oauth_credentials;
use crate::oauth::save_oauth_tokens_to_file;
use crate::oauth::store_lock::OAuthStore;
use crate::oauth::store_lock::OAuthStoreLock;

#[tokio::test(flavor = "current_thread")]
async fn mutations_require_and_retain_the_transaction_guard() -> Result<()> {
    for clear in [false, true] {
        let _env = TempCodexHome::new();
        let initial = sample_tokens();
        save_oauth_tokens_to_file(&initial)?;
        let store = OAuthCredentialStore::new(
            initial.clone(),
            ResolvedOAuthCredentialStore::File,
            MockKeyringStore::default(),
            /*oauth_config*/ None,
        );
        let guard = store.acquire_transaction_guard().await?;
        let credentials = store.load().await?.unwrap();
        if !clear {
            store.clear().await?;
        }
        drop(guard);
        let result = if clear {
            store.clear().await
        } else {
            store.save(credentials.clone()).await
        };
        assert!(
            matches!(result, Err(AuthError::CredentialStoreError(error)) if error.contains("active refresh guard"))
        );
        assert_eq!(
            load_oauth_tokens_from_file(&initial.server_name, &initial.url)?.is_none(),
            !clear,
        );
        // Guard acquisition now rereads storage. Restore the deleted credential first,
        // then delete it again under the guard so the queued save must recreate it.
        save_oauth_tokens_to_file(&initial)?;
        let guard = CredentialStore::acquire_refresh_guard(&store)
            .await?
            .expect("coordinated refresh guard");
        if !clear {
            store.clear().await?;
        }
        let aggregate_lock = OAuthStoreLock::acquire_for_write(OAuthStore::File)?;
        let mut mutation = if clear {
            store.clear()
        } else {
            store.save(credentials)
        };
        assert!(futures::poll!(&mut mutation).is_pending());
        drop(mutation);
        drop(guard);

        // The aggregate lock gates I/O, but only the retained refresh guard blocks this probe.
        let mut contender = Box::pin(RefreshCredentialLock::acquire_for_server(
            &initial.server_name,
            &initial.url,
        ));
        assert!(futures::poll!(&mut contender).is_pending());
        drop(aggregate_lock);
        let _guard = tokio::time::timeout(Duration::from_secs(/*secs*/ 5), contender).await??;
        let durable = load_oauth_tokens_from_file(&initial.server_name, &initial.url)?;
        assert_eq!(durable.is_none(), clear);
    }
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn save_publishes_only_persisted_credentials() -> Result<()> {
    for fail_save in [false, true] {
        let _env = TempCodexHome::new();
        let initial = sample_tokens();
        let keyring = MockKeyringStore::default();
        let authority = ResolvedOAuthCredentialStore::Keyring(AuthKeyringBackendKind::Direct);
        authority.save(&keyring, &initial.server_name, &initial)?;
        let mut fallback = initial.clone();
        fallback
            .token_response
            .0
            .set_access_token(AccessToken::new("fallback-token".into()));
        save_oauth_tokens_to_file(&fallback)?;
        let store = OAuthCredentialStore::new(
            initial.clone(),
            authority,
            keyring.clone(),
            /*oauth_config*/ None,
        );
        let _guard = store.acquire_transaction_guard().await?;
        let mut credentials = store.load().await?.expect("stored credentials");
        let token_response = credentials
            .token_response
            .as_mut()
            .expect("stored token response");
        token_response.set_access_token(AccessToken::new("rotated-access".into()));
        token_response.set_refresh_token(Some(RefreshToken::new("rotated-refresh".into())));
        // The adapter must encode the separate authoritative grant in the stored response.
        token_response.set_scopes(/*scopes*/ None);
        if fail_save {
            let key = compute_store_key(&initial.server_name, &initial.url)?;
            keyring.set_error(&key, KeyringError::Invalid("test".into(), "save".into()));
        }
        let result = store.save(credentials).await;
        let durable = authority
            .load(&keyring, &initial.server_name, &initial.url)?
            .expect("durable keyring credentials");
        let mut expected = initial.clone();
        if fail_save {
            assert!(matches!(result, Err(AuthError::CredentialStoreError(_))));
        } else {
            result?;
            expected
                .token_response
                .0
                .set_access_token(AccessToken::new("rotated-access".into()));
            expected
                .token_response
                .0
                .set_refresh_token(Some(RefreshToken::new("rotated-refresh".into())));
            expected.expires_at = durable.expires_at;
        }
        assert_tokens_match_without_expiry(&durable, &expected);
        assert_eq!(
            store.stored_credentials().await,
            normalized_oauth_credentials(Some(&expected))
        );
        assert_tokens_match_without_expiry(
            &load_oauth_tokens_from_file(&initial.server_name, &initial.url)?.unwrap(),
            &fallback,
        );
    }
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn pinned_read_failure_does_not_adopt_fallback_credentials() -> Result<()> {
    let _env = TempCodexHome::new();
    let initial = sample_tokens();
    save_oauth_tokens_to_file(&initial)?;
    let keyring = MockKeyringStore::default();
    let key = compute_store_key(&initial.server_name, &initial.url)?;
    keyring.set_error(&key, KeyringError::Invalid("test".into(), "load".into()));
    let store = OAuthCredentialStore::new(
        initial.clone(),
        ResolvedOAuthCredentialStore::Keyring(AuthKeyringBackendKind::Direct),
        keyring,
        /*oauth_config*/ None,
    );
    let cached = store.load().await?.expect("cached credentials");
    assert_eq!(
        cached.token_response.unwrap().access_token().secret(),
        initial.token_response.0.access_token().secret()
    );
    assert!(matches!(
        store.acquire_transaction_guard().await,
        Err(AuthError::CredentialStoreError(_))
    ));
    assert_eq!(
        store.stored_credentials().await,
        normalized_oauth_credentials(Some(&initial))
    );
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn replacement_or_removal_does_not_acknowledge_a_new_runtime_snapshot() -> Result<()> {
    let _env = TempCodexHome::new();
    let initial = sample_tokens();
    save_oauth_tokens_to_file(&initial)?;
    let store = OAuthCredentialStore::new(
        initial.clone(),
        ResolvedOAuthCredentialStore::File,
        MockKeyringStore::default(),
        /*oauth_config*/ None,
    );
    let original_snapshot = store.stored_credentials().await;
    for (client_id, issuer) in [
        ("replacement-client", initial.issuer.clone()),
        (
            initial.client_id.as_str(),
            Some("https://replacement.example.test".into()),
        ),
        (initial.client_id.as_str(), None),
    ] {
        let mut replacement = initial.clone();
        replacement.client_id = client_id.into();
        replacement.issuer = issuer;
        save_oauth_tokens_to_file(&replacement)?;
        assert!(matches!(
            store.acquire_transaction_guard().await,
            Err(AuthError::AuthorizationRequired)
        ));
        assert_eq!(store.stored_credentials().await, original_snapshot);
    }
    save_oauth_tokens_to_file(&initial)?;
    let guard = store.acquire_transaction_guard().await?;
    store.clear().await?;
    assert!(load_oauth_tokens_from_file(&initial.server_name, &initial.url)?.is_none());
    drop(guard);
    assert!(matches!(
        store.acquire_transaction_guard().await,
        Err(AuthError::AuthorizationRequired)
    ));
    assert!(store.load().await?.is_some());
    assert_eq!(store.stored_credentials().await, original_snapshot);
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn storage_roundtrip_preserves_absolute_and_unknown_expiry() -> Result<()> {
    let _env = TempCodexHome::new();
    let initial = sample_tokens();
    save_oauth_tokens_to_file(&initial)?;
    let store = OAuthCredentialStore::new(
        initial.clone(),
        ResolvedOAuthCredentialStore::File,
        MockKeyringStore::default(),
        /*oauth_config*/ None,
    );
    let future_received_at = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let future_deadline = (future_received_at + 120) * 1000;
    for (received_at, expected_deadline) in [
        (Some(future_received_at), Some(future_deadline)),
        (None, None),
    ] {
        let guard = store.acquire_transaction_guard().await?;
        let mut credentials = store.load().await?.unwrap();
        credentials.token_received_at = received_at;
        credentials
            .token_response
            .as_mut()
            .unwrap()
            .set_expires_in(Some(&Duration::from_secs(/*secs*/ 120)));
        store.save(credentials).await?;
        let durable = load_oauth_tokens_from_file(&initial.server_name, &initial.url)?.unwrap();
        let mut expected = initial.clone();
        expected.expires_at = expected_deadline;
        expected
            .token_response
            .0
            .set_expires_in(expected_deadline.map(|_| Duration::ZERO).as_ref());
        assert_tokens_match_without_expiry(&durable, &expected);
        drop(guard);
        let _guard = store.acquire_transaction_guard().await?;
        let reloaded = store.load().await?.unwrap();
        let expires_in = reloaded.token_response.unwrap().expires_in();
        if let Some(expected_deadline) = expected_deadline {
            let expires_in = expires_in.expect("future token expiry");
            assert!(!expires_in.is_zero());
            let reconstructed_deadline =
                Duration::from_secs(reloaded.token_received_at.expect("token receipt time"))
                    + expires_in;
            let expected_deadline = Duration::from_millis(expected_deadline);
            // Receipt times use whole seconds, so reconstruction may lose less than a second.
            assert!(reconstructed_deadline <= expected_deadline);
            assert!(expected_deadline - reconstructed_deadline < Duration::from_secs(/*secs*/ 1));
        } else {
            assert_eq!(expires_in, None);
        }
        assert_eq!(
            store.stored_credentials().await,
            normalized_oauth_credentials(Some(&expected))
        );
    }
    Ok(())
}
