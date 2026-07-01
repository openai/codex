use std::sync::mpsc;
use std::time::Duration;

use anyhow::Result;
use tracing::Event;
use tracing::Id;
use tracing::Metadata;
use tracing::Subscriber;
use tracing::span::Attributes;
use tracing::span::Record;
use tracing::subscriber::Interest;

use super::MockKeyringStore;
use super::TempCodexHome;
use super::assert_tokens_match_without_expiry;
use super::sample_tokens;
use crate::oauth::OAuthStore;
use crate::oauth::OAuthStoreLock;
use crate::oauth::OAuthStoreLockFailure;
use crate::oauth::fallback_file_path;
use crate::oauth::load_oauth_tokens_from_file;
use crate::oauth::load_oauth_tokens_from_keyring;
use crate::oauth::load_oauth_tokens_from_keyring_with_fallback_to_file;
use crate::oauth::save_oauth_tokens_to_file;
use crate::oauth::save_oauth_tokens_to_file_with_lock_held;
use crate::oauth::save_oauth_tokens_to_secrets_keyring_with_lock_held;
use crate::oauth::save_oauth_tokens_with_keyring;
use crate::oauth::save_oauth_tokens_with_keyring_with_fallback_to_file;
use codex_config::types::AuthKeyringBackendKind;

const STORE_LOCK_CONTENTION_EVENT_TARGET: &str = "codex_rmcp_client::oauth::store_lock::contention";

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
    let error = load_oauth_tokens_from_keyring_with_fallback_to_file(
        &keyring_store,
        AuthKeyringBackendKind::Secrets,
        &tokens.server_name,
        &tokens.url,
    )
    .expect_err("aggregate-store lock failure must abort Auto resolution");

    assert!(error.downcast_ref::<OAuthStoreLockFailure>().is_some());
    let loaded = load_oauth_tokens_from_file(&tokens.server_name, &tokens.url)?
        .expect("fallback File should remain independently readable");
    assert_tokens_match_without_expiry(&loaded, &tokens);
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
    operation: impl FnOnce() -> Result<T> + Send + 'static,
) -> Result<T>
where
    T: Send + 'static,
{
    let held_lock =
        OAuthStoreLock::acquire_in(codex_home, store, Duration::from_millis(/*millis*/ 100))?;
    let (contended_tx, contended_rx) = mpsc::channel();
    let (result_tx, result_rx) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        tracing::subscriber::with_default(LockContentionSubscriber { contended_tx }, || {
            result_tx
                .send(operation())
                .expect("send contending OAuth store operation result");
        });
    });

    // This event is emitted only after `try_lock()` returns WouldBlock, so the test fails if the
    // operation stops acquiring the aggregate-store lock.
    contended_rx.recv_timeout(Duration::from_secs(/*secs*/ 1))?;
    drop(held_lock);
    let result = result_rx.recv_timeout(Duration::from_secs(/*secs*/ 10))??;
    worker
        .join()
        .expect("contending OAuth store worker should finish");
    Ok(result)
}

#[test]
fn file_store_lock_preserves_updates_for_different_servers() -> Result<()> {
    let env = TempCodexHome::new();
    let first = sample_tokens();
    let mut second = sample_tokens();
    second.server_name = "second-server".to_string();
    second.url = "https://second.example.test".to_string();

    let held_lock = OAuthStoreLock::acquire_in(
        env.path(),
        OAuthStore::File,
        Duration::from_millis(/*millis*/ 100),
    )?;
    let (contended_tx, contended_rx) = mpsc::channel();
    let (result_tx, result_rx) = mpsc::channel();
    let second_for_writer = second.clone();
    let writer = std::thread::spawn(move || {
        tracing::subscriber::with_default(LockContentionSubscriber { contended_tx }, || {
            result_tx
                .send(save_oauth_tokens_to_file(&second_for_writer))
                .expect("send writer result");
        });
    });

    contended_rx.recv_timeout(Duration::from_secs(/*secs*/ 1))?;
    save_oauth_tokens_to_file_with_lock_held(&first)?;
    drop(held_lock);
    result_rx.recv_timeout(Duration::from_secs(/*secs*/ 10))??;
    writer.join().expect("file store writer should finish");

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
    let loaded = complete_after_store_lock_contention(env.path(), OAuthStore::File, move || {
        load_oauth_tokens_from_file(&server_name, &url)
    })?
    .expect("file credentials should remain readable after contention");
    assert_tokens_match_without_expiry(&loaded, &tokens);

    let key = crate::oauth::compute_store_key(&tokens.server_name, &tokens.url)?;
    let removed = complete_after_store_lock_contention(env.path(), OAuthStore::File, move || {
        crate::oauth::delete_oauth_tokens_from_file(&key)
    })?;
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

    let held_lock = OAuthStoreLock::acquire_in(
        env.path(),
        OAuthStore::Secrets,
        Duration::from_millis(/*millis*/ 100),
    )?;
    let (contended_tx, contended_rx) = mpsc::channel();
    let (result_tx, result_rx) = mpsc::channel();
    let store_for_writer = keyring_store.clone();
    let second_for_writer = second.clone();
    let writer = std::thread::spawn(move || {
        tracing::subscriber::with_default(LockContentionSubscriber { contended_tx }, || {
            result_tx
                .send(save_oauth_tokens_with_keyring(
                    &store_for_writer,
                    AuthKeyringBackendKind::Secrets,
                    &second_for_writer.server_name,
                    &second_for_writer,
                ))
                .expect("send writer result");
        });
    });

    contended_rx.recv_timeout(Duration::from_secs(/*secs*/ 1))?;
    let first_serialized = serde_json::to_string(&first)?;
    save_oauth_tokens_to_secrets_keyring_with_lock_held(
        &keyring_store,
        &first.server_name,
        &first,
        &first_serialized,
    )?;
    drop(held_lock);
    result_rx.recv_timeout(Duration::from_secs(/*secs*/ 10))??;
    writer.join().expect("secrets store writer should finish");

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
    let loaded =
        complete_after_store_lock_contention(env.path(), OAuthStore::Secrets, move || {
            load_oauth_tokens_from_keyring(
                &store_for_load,
                AuthKeyringBackendKind::Secrets,
                &server_name,
                &url,
            )
        })?
        .expect("encrypted credentials should remain readable after contention");
    assert_tokens_match_without_expiry(&loaded, &tokens);

    let store_for_delete = keyring_store.clone();
    let server_name = tokens.server_name.clone();
    let url = tokens.url.clone();
    let removed =
        complete_after_store_lock_contention(env.path(), OAuthStore::Secrets, move || {
            crate::oauth::delete_oauth_tokens_from_secrets_keyring(
                &store_for_delete,
                &server_name,
                &url,
            )
        })?;
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
