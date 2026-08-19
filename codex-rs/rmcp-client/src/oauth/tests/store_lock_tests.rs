use std::process::Command;
use std::sync::mpsc;
use std::time::Duration;
use std::time::Instant;

use anyhow::Context;
use anyhow::Result;
use codex_config::types::AuthKeyringBackendKind;
use codex_keyring_store::KeyringStore;
use codex_keyring_store::tests::MockKeyringStore;
use oauth2::AccessToken;
use oauth2::RefreshToken;
use oauth2::Scope;
use oauth2::TokenResponse;
use oauth2::basic::BasicTokenType;
use pretty_assertions::assert_eq;
use rmcp::transport::auth::OAuthTokenResponse;
use rmcp::transport::auth::VendorExtraTokenFields;
use tracing::Event;
use tracing::Id;
use tracing::Metadata;
use tracing::Subscriber;
use tracing::span::Attributes;
use tracing::span::Record;
use tracing::subscriber::Interest;

use super::OAuthStore;
use super::OAuthStoreLock;
use super::OAuthStoreLockFailure;
use super::OAuthStoreLockMode;
use crate::oauth::StoredOAuthCredentialSnapshot;
use crate::oauth::StoredOAuthTokens;
use crate::oauth::WrappedOAuthTokenResponse;
use crate::oauth::fallback_file_path;
use crate::oauth::load_oauth_tokens_from_file;
use crate::oauth::load_oauth_tokens_from_keyring;
use crate::oauth::resolve_oauth_tokens_from_store_policy;
use crate::oauth::save_oauth_tokens_to_file;
use crate::oauth::save_oauth_tokens_to_file_with_lock_held;
use crate::oauth::save_oauth_tokens_to_secrets_keyring_with_lock_held;
use crate::oauth::save_oauth_tokens_with_keyring;
use crate::oauth::save_oauth_tokens_with_keyring_with_fallback_to_file;
use crate::oauth::stored_oauth_credential_snapshot;
use crate::oauth::test_support::TempCodexHome;
use codex_config::types::OAuthCredentialsStoreMode;

const STORE_LOCK_CONTENTION_EVENT_TARGET: &str = "codex_rmcp_client::oauth::store_lock::contention";
// Contention is proven by the tracing event emitted after a real WouldBlock. Keep the timeout
// generous because it only bounds a failed test; it must not turn worker scheduling latency into
// a false failure on loaded CI hosts.
const STORE_LOCK_CONTENTION_EVENT_TIMEOUT: Duration = Duration::from_secs(/*secs*/ 10);

fn assert_tokens_match_without_expiry(actual: &StoredOAuthTokens, expected: &StoredOAuthTokens) {
    assert_eq!(actual.server_name, expected.server_name);
    assert_eq!(actual.url, expected.url);
    assert_eq!(actual.client_id, expected.client_id);
    assert_eq!(actual.expires_at, expected.expires_at);
    assert_token_response_match_without_expiry(&actual.token_response, &expected.token_response);
}

fn assert_token_response_match_without_expiry(
    actual: &WrappedOAuthTokenResponse,
    expected: &WrappedOAuthTokenResponse,
) {
    let actual_response = &actual.0;
    let expected_response = &expected.0;

    assert_eq!(
        actual_response.access_token().secret(),
        expected_response.access_token().secret()
    );
    assert_eq!(actual_response.token_type(), expected_response.token_type());
    assert_eq!(
        actual_response.refresh_token().map(RefreshToken::secret),
        expected_response.refresh_token().map(RefreshToken::secret),
    );
    assert_eq!(actual_response.scopes(), expected_response.scopes());
    assert_eq!(
        actual_response.extra_fields().0,
        expected_response.extra_fields().0
    );
    assert_eq!(
        actual_response.expires_in().is_some(),
        expected_response.expires_in().is_some()
    );
}

fn sample_tokens() -> StoredOAuthTokens {
    let mut response = OAuthTokenResponse::new(
        AccessToken::new("access-token".to_string()),
        BasicTokenType::Bearer,
        VendorExtraTokenFields::default(),
    );
    response.set_refresh_token(Some(RefreshToken::new("refresh-token".to_string())));
    response.set_scopes(Some(vec![
        Scope::new("scope-a".to_string()),
        Scope::new("scope-b".to_string()),
    ]));
    let expires_in = Duration::from_secs(3600);
    response.set_expires_in(Some(&expires_in));
    let expires_at = crate::oauth::compute_expires_at_millis(&response);

    StoredOAuthTokens {
        server_name: "test-server".to_string(),
        url: "https://example.test".to_string(),
        issuer: Some("https://issuer.example.test".to_string()),
        client_id: "client-id".to_string(),
        token_response: WrappedOAuthTokenResponse(response),
        expires_at,
    }
}

#[test]
fn legacy_rmcp_oauth_keyring_credentials_remain_readable() -> Result<()> {
    let _env = TempCodexHome::new();
    let keyring_store = MockKeyringStore::default();
    let mut expected = sample_tokens();
    expected.expires_at = None;
    expected.token_response.0.set_expires_in(None);

    let serialized = serde_json::json!({
        "server_name": "test-server",
        "url": "https://example.test",
        "client_id": "client-id",
        "token_response": {
            "access_token": "access-token",
            "token_type": "Bearer",
            "refresh_token": "refresh-token",
            "scope": "scope-a scope-b",
        },
    })
    .to_string();
    let key = crate::oauth::compute_store_key(&expected.server_name, &expected.url)?;
    keyring_store.save(crate::oauth::KEYRING_SERVICE, &key, &serialized)?;

    let resolved = resolve_oauth_tokens_from_store_policy(
        &keyring_store,
        &expected.server_name,
        &expected.url,
        OAuthCredentialsStoreMode::Auto,
        AuthKeyringBackendKind::Direct,
    )?
    .expect("OAuth credentials written before the rmcp upgrade should remain readable");

    assert_eq!(
        resolved.store,
        crate::oauth::ResolvedOAuthCredentialStore::Keyring(AuthKeyringBackendKind::Direct)
    );
    assert_tokens_match_without_expiry(&resolved.tokens, &expected);
    assert!(crate::oauth::oauth_tokens_are_usable(&resolved.tokens));
    Ok(())
}

const LOCK_HOLDER_CHILD_TEST: &str =
    "oauth::store_lock::tests::store_lock_is_released_when_holder_process_exits_child";
const LOCK_HOLDER_READY_PATH_ENV: &str = "CODEX_OAUTH_STORE_LOCK_CHILD_READY_PATH";

#[test]
fn store_lock_is_released_when_holder_process_exits() -> Result<()> {
    let env = TempCodexHome::new();
    let ready_file = env.path().join("lock-holder-ready");
    let mut child = Command::new(std::env::current_exe()?)
        .arg("--exact")
        .arg(LOCK_HOLDER_CHILD_TEST)
        .arg("--ignored")
        .env("CODEX_HOME", env.path())
        .env(LOCK_HOLDER_READY_PATH_ENV, &ready_file)
        .spawn()
        .context("spawn OAuth store lock holder test process")?;

    let test_result = (|| -> Result<()> {
        let started = Instant::now();
        while !ready_file.exists() {
            if started.elapsed() > Duration::from_secs(/*secs*/ 5) {
                anyhow::bail!("timed out waiting for child process to acquire OAuth store lock");
            }
            std::thread::sleep(Duration::from_millis(/*millis*/ 20));
        }

        let error = match OAuthStoreLock::acquire_in_with_mode(
            env.path(),
            OAuthStore::File,
            Duration::from_millis(/*millis*/ 100),
            OAuthStoreLockMode::Exclusive,
        ) {
            Ok(_) => {
                anyhow::bail!("live holder process should keep the OAuth store lock unavailable")
            }
            Err(error) => error,
        };
        assert!(matches!(error, OAuthStoreLockFailure::Timeout { .. }));

        child
            .kill()
            .context("kill OAuth store lock holder process")?;
        let status = child
            .wait()
            .context("wait for killed OAuth store lock holder process")?;
        assert!(!status.success());
        let _lock = OAuthStoreLock::acquire_in_with_mode(
            env.path(),
            OAuthStore::File,
            Duration::from_secs(/*secs*/ 1),
            OAuthStoreLockMode::Exclusive,
        )?;
        Ok(())
    })();

    if let Ok(None) = child.try_wait() {
        let _ = child.kill();
        let _ = child.wait();
    }

    test_result
}

#[test]
#[ignore = "child process for store_lock_is_released_when_holder_process_exits"]
fn store_lock_is_released_when_holder_process_exits_child() -> Result<()> {
    let ready_file = match std::env::var_os(LOCK_HOLDER_READY_PATH_ENV) {
        Some(path) => std::path::PathBuf::from(path),
        None => return Ok(()),
    };
    let _lock = OAuthStoreLock::acquire_for_write(OAuthStore::File)?;
    std::fs::write(ready_file, b"ready")?;
    loop {
        std::thread::sleep(Duration::from_secs(/*secs*/ 60));
    }
}

#[test]
fn auto_save_secrets_lock_failure_does_not_fall_back_to_file() -> Result<()> {
    let env = TempCodexHome::new();
    let lock_dir = env.path().join("mcp-oauth-locks");
    std::fs::create_dir_all(&lock_dir)?;
    // Break only the Secrets lock path. The distinct File lock remains usable, so Auto would
    // successfully write fallback credentials if it mistook coordination failure for backend
    // unavailability.
    std::fs::create_dir(lock_dir.join("secrets-store.lock"))?;
    let keyring_store = MockKeyringStore::default();
    let tokens = sample_tokens();

    let error = save_oauth_tokens_with_keyring_with_fallback_to_file(
        &keyring_store,
        AuthKeyringBackendKind::Secrets,
        &tokens.server_name,
        &tokens,
    )
    .expect_err("aggregate-store lock failure must abort Auto persistence");

    assert!(error.downcast_ref::<OAuthStoreLockFailure>().is_some());
    assert!(!fallback_file_path()?.exists());
    save_oauth_tokens_to_file(&tokens)?;
    let loaded = load_oauth_tokens_from_file(&tokens.server_name, &tokens.url)?
        .expect("fallback File should remain independently writable");
    assert_tokens_match_without_expiry(&loaded, &tokens);
    Ok(())
}

#[test]
fn auto_load_secrets_lock_failure_does_not_fall_back_to_file() -> Result<()> {
    let env = TempCodexHome::new();
    let keyring_store = MockKeyringStore::default();
    let tokens = sample_tokens();
    save_oauth_tokens_to_file(&tokens)?;

    let lock_dir = env.path().join("mcp-oauth-locks");
    std::fs::create_dir(lock_dir.join("secrets-store.lock"))?;
    let error = resolve_oauth_tokens_from_store_policy(
        &keyring_store,
        &tokens.server_name,
        &tokens.url,
        OAuthCredentialsStoreMode::Auto,
        AuthKeyringBackendKind::Secrets,
    )
    .expect_err("aggregate-store lock failure must abort Auto resolution");

    assert!(error.downcast_ref::<OAuthStoreLockFailure>().is_some());
    let loaded = load_oauth_tokens_from_file(&tokens.server_name, &tokens.url)?
        .expect("fallback File should remain independently readable");
    assert_tokens_match_without_expiry(&loaded, &tokens);
    Ok(())
}

#[test]
fn oauth_credential_probes_skip_contended_file_and_secrets_stores() -> Result<()> {
    let _env = TempCodexHome::new();
    let tokens = sample_tokens();
    let file = OAuthCredentialsStoreMode::File;
    let auto = OAuthCredentialsStoreMode::Auto;
    let direct = AuthKeyringBackendKind::Direct;
    let secrets = AuthKeyringBackendKind::Secrets;
    save_oauth_tokens_to_file(&tokens)?;
    let snapshot =
        stored_oauth_credential_snapshot(&tokens.server_name, &tokens.url, file, direct)?
            .expect("fallback credentials should exist before contention");
    let reload = |store_mode, keyring_backend_kind| {
        snapshot.reload(
            &tokens.server_name,
            &tokens.url,
            store_mode,
            keyring_backend_kind,
        )
    };
    let probe = |store_mode, keyring_backend_kind| {
        StoredOAuthCredentialSnapshot::for_runtime_refresh(
            /*previous*/ None,
            &tokens.server_name,
            &tokens.url,
            store_mode,
            keyring_backend_kind,
        )
    };
    let refresh = |url| {
        StoredOAuthCredentialSnapshot::for_runtime_refresh(
            Some(&snapshot),
            &tokens.server_name,
            url,
            file,
            direct,
        )
    };

    {
        let _lock = OAuthStoreLock::acquire_for_write(OAuthStore::File)?;
        assert_eq!(reload(file, direct)?, None);
        assert_eq!(reload(auto, secrets)?, None);
        assert_eq!(probe(file, direct)?, None);
        let contended = refresh(&tokens.url)?.expect("the previous snapshot should be retained");
        assert!(contended.store_was_contended());
        assert_eq!(contended, snapshot);
        assert_eq!(
            refresh("https://another.example.test")?,
            None,
            "store contention must not replay credentials for another endpoint",
        );
    }

    assert_eq!(reload(file, direct)?, Some(snapshot.credentials().clone()));
    let refreshed = refresh(&tokens.url)?.expect("the unlocked store should still contain tokens");
    assert!(!refreshed.store_was_contended());

    let _lock = OAuthStoreLock::acquire_for_write(OAuthStore::Secrets)?;
    assert_eq!(
        probe(auto, secrets)?,
        None,
        "a locked Secrets authority must not fall back to the stale File entry",
    );
    Ok(())
}

struct LockContentionSubscriber {
    contended_tx: mpsc::Sender<()>,
}

impl Subscriber for LockContentionSubscriber {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.target() == STORE_LOCK_CONTENTION_EVENT_TARGET
    }

    fn register_callsite(&self, metadata: &'static Metadata<'static>) -> Interest {
        if self.enabled(metadata) {
            Interest::always()
        } else {
            Interest::never()
        }
    }

    fn max_level_hint(&self) -> Option<tracing::level_filters::LevelFilter> {
        Some(tracing::level_filters::LevelFilter::DEBUG)
    }

    fn new_span(&self, _span: &Attributes<'_>) -> Id {
        Id::from_u64(/*u*/ 1)
    }

    fn record(&self, _span: &Id, _values: &Record<'_>) {}

    fn record_follows_from(&self, _span: &Id, _follows_from: &Id) {}

    fn event(&self, event: &Event<'_>) {
        if self.enabled(event.metadata()) {
            self.contended_tx
                .send(())
                .expect("signal actual OAuth store lock contention");
        }
    }

    fn enter(&self, _span: &Id) {}

    fn exit(&self, _span: &Id) {}
}

fn complete_after_store_lock_contention<T>(
    codex_home: &std::path::Path,
    store: OAuthStore,
    while_locked: impl FnOnce() -> Result<()>,
    operation: impl FnOnce() -> Result<T> + Send + 'static,
) -> Result<T>
where
    T: Send + 'static,
{
    std::thread::scope(|scope| {
        let held_lock = OAuthStoreLock::acquire_in_with_mode(
            codex_home,
            store,
            Duration::from_millis(/*millis*/ 100),
            OAuthStoreLockMode::Exclusive,
        )?;
        let (contended_tx, contended_rx) = mpsc::channel();
        let worker = scope.spawn(move || {
            tracing::subscriber::with_default(LockContentionSubscriber { contended_tx }, operation)
        });

        // This event is emitted only after `try_lock()` returns WouldBlock, so the test fails if the
        // operation stops acquiring the aggregate-store lock.
        contended_rx
            .recv_timeout(STORE_LOCK_CONTENTION_EVENT_TIMEOUT)
            .context("timed out waiting for actual OAuth store lock contention")?;
        while_locked()?;
        drop(held_lock);
        worker
            .join()
            .expect("contending OAuth store worker should finish")
    })
}

#[test]
fn aggregate_store_readers_share_access_while_writers_remain_exclusive() -> Result<()> {
    let env = TempCodexHome::new();

    for store in [OAuthStore::File, OAuthStore::Secrets] {
        let readers = (0..2)
            .map(|_| {
                OAuthStoreLock::acquire_in_with_mode(
                    env.path(),
                    store,
                    Duration::from_millis(/*millis*/ 100),
                    OAuthStoreLockMode::Shared,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;

        let writer_error = match OAuthStoreLock::acquire_in_with_mode(
            env.path(),
            store,
            Duration::from_millis(/*millis*/ 100),
            OAuthStoreLockMode::Exclusive,
        ) {
            Ok(_) => anyhow::bail!("an active OAuth store reader must exclude writers"),
            Err(error) => error,
        };
        assert!(matches!(
            writer_error,
            OAuthStoreLockFailure::Timeout { .. }
        ));
        drop(readers);
        let _writer = OAuthStoreLock::acquire_in_with_mode(
            env.path(),
            store,
            Duration::from_millis(/*millis*/ 100),
            OAuthStoreLockMode::Exclusive,
        )?;
    }

    Ok(())
}

#[test]
fn aggregate_store_credential_loads_can_share_an_existing_reader() -> Result<()> {
    let env = TempCodexHome::new();
    let keyring_store = MockKeyringStore::default();
    let tokens = sample_tokens();
    save_oauth_tokens_to_file(&tokens)?;
    save_oauth_tokens_with_keyring(
        &keyring_store,
        AuthKeyringBackendKind::Secrets,
        &tokens.server_name,
        &tokens,
    )?;

    let file_reader = OAuthStoreLock::acquire_in_with_mode(
        env.path(),
        OAuthStore::File,
        Duration::from_millis(/*millis*/ 100),
        OAuthStoreLockMode::Shared,
    )?;
    let file_tokens = load_oauth_tokens_from_file(&tokens.server_name, &tokens.url)?
        .expect("a File credential read should coexist with another reader");
    assert_tokens_match_without_expiry(&file_tokens, &tokens);
    drop(file_reader);

    let secrets_reader = OAuthStoreLock::acquire_in_with_mode(
        env.path(),
        OAuthStore::Secrets,
        Duration::from_millis(/*millis*/ 100),
        OAuthStoreLockMode::Shared,
    )?;
    let secrets_tokens = load_oauth_tokens_from_keyring(
        &keyring_store,
        AuthKeyringBackendKind::Secrets,
        &tokens.server_name,
        &tokens.url,
    )?
    .expect("a Secrets credential read should coexist with another reader");
    assert_tokens_match_without_expiry(&secrets_tokens, &tokens);
    drop(secrets_reader);

    Ok(())
}

#[test]
fn file_store_lock_preserves_updates_for_different_servers() -> Result<()> {
    let env = TempCodexHome::new();
    let first = sample_tokens();
    let mut second = sample_tokens();
    second.server_name = "second-server".to_string();
    second.url = "https://second.example.test".to_string();

    let second_for_writer = second.clone();
    complete_after_store_lock_contention(
        env.path(),
        OAuthStore::File,
        || save_oauth_tokens_to_file_with_lock_held(&first),
        move || save_oauth_tokens_to_file(&second_for_writer),
    )?;

    let loaded_first = load_oauth_tokens_from_file(&first.server_name, &first.url)?
        .expect("first server tokens should remain stored");
    let loaded_second = load_oauth_tokens_from_file(&second.server_name, &second.url)?
        .expect("second server tokens should be stored");
    assert_tokens_match_without_expiry(&loaded_first, &first);
    assert_tokens_match_without_expiry(&loaded_second, &second);
    Ok(())
}

#[test]
fn file_store_load_and_delete_observe_aggregate_lock() -> Result<()> {
    let env = TempCodexHome::new();
    let tokens = sample_tokens();
    save_oauth_tokens_to_file(&tokens)?;

    let server_name = tokens.server_name.clone();
    let url = tokens.url.clone();
    let loaded = complete_after_store_lock_contention(
        env.path(),
        OAuthStore::File,
        || Ok(()),
        move || load_oauth_tokens_from_file(&server_name, &url),
    )?
    .expect("file credentials should remain readable after contention");
    assert_tokens_match_without_expiry(&loaded, &tokens);

    let key = crate::oauth::compute_store_key(&tokens.server_name, &tokens.url)?;
    let removed = complete_after_store_lock_contention(
        env.path(),
        OAuthStore::File,
        || Ok(()),
        move || crate::oauth::delete_oauth_tokens_from_file(&key),
    )?;
    assert!(removed);
    assert!(load_oauth_tokens_from_file(&tokens.server_name, &tokens.url)?.is_none());
    Ok(())
}

#[test]
fn secrets_store_lock_preserves_updates_for_different_servers() -> Result<()> {
    let env = TempCodexHome::new();
    let keyring_store = MockKeyringStore::default();
    let first = sample_tokens();
    let mut second = sample_tokens();
    second.server_name = "second-server".to_string();
    second.url = "https://second.example.test".to_string();

    let store_for_writer = keyring_store.clone();
    let second_for_writer = second.clone();
    complete_after_store_lock_contention(
        env.path(),
        OAuthStore::Secrets,
        || {
            let first_serialized = serde_json::to_string(&first)?;
            save_oauth_tokens_to_secrets_keyring_with_lock_held(
                &keyring_store,
                &first.server_name,
                &first,
                &first_serialized,
            )
        },
        move || {
            save_oauth_tokens_with_keyring(
                &store_for_writer,
                AuthKeyringBackendKind::Secrets,
                &second_for_writer.server_name,
                &second_for_writer,
            )
        },
    )?;

    let loaded_first = load_oauth_tokens_from_keyring(
        &keyring_store,
        AuthKeyringBackendKind::Secrets,
        &first.server_name,
        &first.url,
    )?
    .expect("first server tokens should remain stored");
    let loaded_second = load_oauth_tokens_from_keyring(
        &keyring_store,
        AuthKeyringBackendKind::Secrets,
        &second.server_name,
        &second.url,
    )?
    .expect("second server tokens should be stored");
    assert_tokens_match_without_expiry(&loaded_first, &first);
    assert_tokens_match_without_expiry(&loaded_second, &second);
    Ok(())
}

#[test]
fn secrets_store_load_and_delete_observe_aggregate_lock() -> Result<()> {
    let env = TempCodexHome::new();
    let keyring_store = MockKeyringStore::default();
    let tokens = sample_tokens();
    save_oauth_tokens_with_keyring(
        &keyring_store,
        AuthKeyringBackendKind::Secrets,
        &tokens.server_name,
        &tokens,
    )?;

    let store_for_load = keyring_store.clone();
    let server_name = tokens.server_name.clone();
    let url = tokens.url.clone();
    let loaded = complete_after_store_lock_contention(
        env.path(),
        OAuthStore::Secrets,
        || Ok(()),
        move || {
            Ok(load_oauth_tokens_from_keyring(
                &store_for_load,
                AuthKeyringBackendKind::Secrets,
                &server_name,
                &url,
            )?)
        },
    )?
    .expect("encrypted credentials should remain readable after contention");
    assert_tokens_match_without_expiry(&loaded, &tokens);

    let store_for_delete = keyring_store.clone();
    let server_name = tokens.server_name.clone();
    let url = tokens.url.clone();
    let removed = complete_after_store_lock_contention(
        env.path(),
        OAuthStore::Secrets,
        || Ok(()),
        move || {
            crate::oauth::delete_oauth_tokens_from_secrets_keyring(
                &store_for_delete,
                &server_name,
                &url,
            )
        },
    )?;
    assert!(removed);
    assert!(
        load_oauth_tokens_from_keyring(
            &keyring_store,
            AuthKeyringBackendKind::Secrets,
            &tokens.server_name,
            &tokens.url,
        )?
        .is_none()
    );
    Ok(())
}
