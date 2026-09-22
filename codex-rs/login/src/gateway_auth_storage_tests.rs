//! Login cancellation and readiness stay responsive during blocked credential reads.

use super::*;
use codex_keyring_store::CredentialStoreError;
use codex_keyring_store::KeyringStore;
use pretty_assertions::assert_eq;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

#[derive(Debug)]
struct BlockingKeyring {
    inner: MockKeyringStore,
    async_thread: std::thread::ThreadId,
    check_thread: AtomicBool,
    gate: Mutex<
        Option<(
            tokio::sync::oneshot::Sender<()>,
            std::sync::mpsc::Receiver<()>,
        )>,
    >,
}

impl BlockingKeyring {
    fn check_thread(&self) {
        if self.check_thread.load(Ordering::Relaxed) {
            assert_ne!(std::thread::current().id(), self.async_thread);
        }
    }
}

impl KeyringStore for BlockingKeyring {
    fn load(&self, service: &str, account: &str) -> Result<Option<String>, CredentialStoreError> {
        self.check_thread();
        let gate = self.gate.lock().unwrap().take();
        if let Some((entered, release)) = gate {
            entered.send(()).unwrap();
            release
                .recv_timeout(Duration::from_secs(/*secs*/ 30))
                .unwrap();
        }
        self.inner.load(service, account)
    }

    fn save(&self, service: &str, account: &str, value: &str) -> Result<(), CredentialStoreError> {
        self.check_thread();
        self.inner.save(service, account, value)
    }

    fn delete(&self, service: &str, account: &str) -> Result<bool, CredentialStoreError> {
        self.check_thread();
        self.inner.delete(service, account)
    }
}

fn blocking_client() -> (GatewayAuthManager, tempfile::TempDir, Arc<BlockingKeyring>) {
    let home = tempfile::tempdir().unwrap();
    let keyring = Arc::new(BlockingKeyring {
        inner: MockKeyringStore::default(),
        async_thread: std::thread::current().id(),
        check_thread: AtomicBool::new(/*v*/ false),
        gate: Mutex::new(/*t*/ None),
    });
    let manager = GatewayAuthManager::new(
        loopback_config(),
        home.path().to_path_buf(),
        transport_default_auth_route_config().http_client_factory(),
        keyring.clone(),
    )
    .unwrap();
    (manager, home, keyring)
}

#[tokio::test]
async fn readiness_does_not_block_cached_requests_or_overwrite_newer_credentials() {
    let (manager, _home, keyring) = blocking_client();
    save_token(&manager, "old", /*refresh_token*/ None, i64::MAX);
    assert_eq!(manager.resolve_access_token().await.unwrap(), "old");
    keyring.check_thread.store(/*val*/ true, Ordering::Relaxed);
    let (entered_tx, entered_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    *keyring.gate.lock().unwrap() = Some((entered_tx, release_rx));
    let observer = manager.clone();
    let readiness = tokio::spawn(async move { observer.status().await });
    tokio::time::timeout(Duration::from_secs(/*secs*/ 2), entered_rx)
        .await
        .unwrap()
        .unwrap();
    // The status read has captured the old encrypted file and is blocked in keyring I/O.
    assert_eq!(
        tokio::time::timeout(
            Duration::from_secs(/*secs*/ 2),
            manager.resolve_access_token()
        )
        .await
        .unwrap()
        .unwrap(),
        "old"
    );
    let writer = manager.clone();
    tokio::task::spawn_blocking(move || {
        save_token(
            &writer,
            "replacement",
            /*refresh_token*/ None,
            i64::MAX,
        );
    })
    .await
    .unwrap();
    assert_eq!(
        tokio::time::timeout(
            // Refresh decrypts the persisted replacement; only cached requests must be immediate.
            Duration::from_secs(/*secs*/ 10),
            manager.refresh_access_token("old"),
        )
        .await
        .unwrap()
        .unwrap(),
        "replacement"
    );
    release_tx.send(()).unwrap();
    assert_eq!(
        readiness.await.unwrap().unwrap(),
        crate::GatewayAuthStatus::Succeeded
    );
    assert_eq!(manager.resolve_access_token().await.unwrap(), "replacement");
}

#[tokio::test]
async fn cancel_during_keyring_read_releases_login_and_cached_credentials() {
    let (manager, _home, keyring) = blocking_client();
    save_token(&manager, "existing", /*refresh_token*/ None, i64::MAX);
    assert_eq!(manager.resolve_access_token().await.unwrap(), "existing");
    let mut events = manager.state.status_tx.subscribe();
    keyring.check_thread.store(/*val*/ true, Ordering::Relaxed);
    let (entered_tx, entered_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    *keyring.gate.lock().unwrap() = Some((entered_tx, release_rx));
    let cancel = Arc::new(tokio::sync::Notify::new());
    let login_manager = manager.clone();
    let login_cancel = Arc::clone(&cancel);
    let login = tokio::spawn(async move {
        login_manager
            .login_with_browser(login_cancel.notified(), |_| panic!("unexpected browser"))
            .await
    });
    tokio::time::timeout(Duration::from_secs(/*secs*/ 2), entered_rx)
        .await
        .unwrap()
        .unwrap();
    let error = tokio::time::timeout(
        Duration::from_secs(/*secs*/ 2),
        manager.resolve_access_token(),
    )
    .await
    .expect("requests must not wait for login credential I/O")
    .unwrap_err();
    assert_eq!(
        error
            .get_ref()
            .and_then(|error| error.downcast_ref::<crate::GatewayAuthError>()),
        Some(&crate::GatewayAuthError::LoginInProgress),
    );
    assert_eq!(
        manager.status().await.unwrap(),
        crate::GatewayAuthStatus::Started
    );
    assert_eq!(
        events.try_recv().unwrap().status,
        crate::GatewayAuthStatus::Started
    );
    cancel.notify_one();
    let error = tokio::time::timeout(Duration::from_secs(/*secs*/ 2), login)
        .await
        .unwrap()
        .unwrap()
        .unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::Interrupted);
    assert_eq!(
        events.try_recv().unwrap().status,
        crate::GatewayAuthStatus::Failed {
            message: "Gateway sign-in was canceled".into(),
        }
    );
    // Both request consumers and a new login must proceed before the keyring read returns.
    assert_eq!(
        tokio::time::timeout(
            // Failed status makes the request reread and decrypt the stored credential.
            Duration::from_secs(/*secs*/ 10),
            manager.resolve_access_token()
        )
        .await
        .unwrap()
        .unwrap(),
        "existing"
    );
    let error = manager
        .login_with_browser(std::future::ready(()), |_| panic!("unexpected browser"))
        .await
        .unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::Interrupted);
    release_tx.send(()).unwrap();
}
