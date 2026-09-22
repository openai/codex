//! Explicit browser login shares credentials with request consumers and remains cancellable.

use super::*;
use crate::GatewayAuthError;
use crate::GatewayAuthStatus;
use pretty_assertions::assert_eq;
use tokio::sync::Notify;

#[tokio::test]
async fn status_subscriptions_survive_gateway_changes_and_are_isolated_by_home() {
    let server = MockServer::start().await;
    let home = tempfile::tempdir().unwrap();
    let runtime = crate::AuthRuntimeConfig {
        codex_home: home.path().to_path_buf(),
        auth_route_config: transport_default_auth_route_config(),
    };
    // Subscribe before any manager exists, as app-server does without gateway configuration.
    let mut events = crate::subscribe_gateway_auth_status(&runtime);
    crate::GatewayLoginControl::for_runtime(&runtime).require_explicit_login();
    let keyring = Arc::new(MockKeyringStore::default());
    let (other_manager, _other_home) = client(config(&server), keyring.clone());
    other_manager.resolve_access_token().await.unwrap_err();
    assert!(matches!(
        events.try_recv(),
        Err(tokio::sync::broadcast::error::TryRecvError::Empty)
    ));

    for client_id in ["first-gateway", "replacement-gateway"] {
        let mut gateway = config(&server);
        gateway.client_id = client_id.to_string();
        let manager = GatewayAuthManager::new(
            gateway.clone(),
            home.path().to_path_buf(),
            runtime.auth_route_config.http_client_factory(),
            keyring.clone(),
        )
        .unwrap();
        manager.resolve_access_token().await.unwrap_err();
        let event = tokio::time::timeout(Duration::from_secs(/*secs*/ 2), events.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            (event.config, event.status),
            (gateway, GatewayAuthStatus::NotReady)
        );
        // The next gateway must reach the same subscriber after the last manager is dropped.
        drop(manager);
    }
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn cancel_releases_callback_while_reads_and_requests_stay_responsive() {
    let server = MockServer::start().await;
    let (manager, _home) = client(config(&server), Arc::new(MockKeyringStore::default()));
    let observer = manager.clone();
    let mut events = manager.state.status_tx.subscribe();
    let cancel = Notify::new();
    let (url_tx, url_rx) = tokio::sync::oneshot::channel();
    let login = manager.login_with_browser(cancel.notified(), |url| {
        let redirect = url
            .query_pairs()
            .find(|(key, _)| key == "redirect_uri")
            .unwrap()
            .1
            .into_owned();
        url_tx.send(url::Url::parse(&redirect).unwrap()).unwrap();
    });
    let check = async {
        let callback = url_rx.await.unwrap();
        assert_eq!(observer.status().await.unwrap(), GatewayAuthStatus::Started);
        assert_eq!(
            observer.resolve_access_token().await.unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
        cancel.notify_one();
        callback
    };
    let (result, callback) = tokio::join!(login, check);
    assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::Interrupted);
    let failed = GatewayAuthStatus::Failed {
        message: "Gateway sign-in was canceled".to_string(),
    };
    assert_eq!(observer.status().await.unwrap(), failed);
    assert_eq!(
        events.recv().await.unwrap().status,
        GatewayAuthStatus::Started
    );
    assert_eq!(events.recv().await.unwrap().status, failed);
    tokio::time::timeout(Duration::from_secs(/*secs*/ 2), async {
        loop {
            if let Ok(listener) = TcpListener::bind(("127.0.0.1", callback.port().unwrap())) {
                break listener;
            }
            tokio::time::sleep(Duration::from_millis(/*millis*/ 10)).await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn credential_read_failure_finishes_login_status() {
    let server = MockServer::start().await;
    let (manager, home) = client(config(&server), Arc::new(MockKeyringStore::default()));
    let path = home.path().join("secrets/gateway_oauth.age");
    std::fs::create_dir_all(&path).unwrap();
    let mut events = manager.state.status_tx.subscribe();
    let error = manager
        .login_with_browser(std::future::pending(), |_| panic!("unexpected browser"))
        .await
        .unwrap_err();
    let message = "Gateway sign-in failed; check the gateway configuration and credential store.";
    assert_eq!(error.to_string(), message);
    assert_eq!(
        events.try_recv().unwrap().status,
        GatewayAuthStatus::Started
    );
    let failed = GatewayAuthStatus::Failed {
        message: message.into(),
    };
    assert_eq!(events.try_recv().unwrap().status, failed);
    std::fs::remove_dir(&path).unwrap();
    assert_eq!(manager.status().await.unwrap(), failed);
    let cancel = Notify::new();
    let error = manager
        .login_with_browser(cancel.notified(), |_| cancel.notify_one())
        .await
        .unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::Interrupted);
}

#[tokio::test]
async fn rejected_credentials_stay_not_ready_until_replaced() {
    let server = MockServer::start().await;
    let keyring = Arc::new(MockKeyringStore::default());
    let (manager, _home) = client(config(&server), keyring.clone());
    let mut events = manager.state.status_tx.subscribe();
    save_token(
        &manager,
        "rejected",
        Some("revoked-refresh"),
        Utc::now().timestamp() + 3600,
    );
    Mock::given(method("POST"))
        .and(path("/token"))
        .and(body_string_contains("grant_type=refresh_token"))
        .respond_with(
            ResponseTemplate::new(/*s*/ 400).set_body_json(json!({"error": "invalid_grant"})),
        )
        .expect(/*r*/ 1)
        .mount(&server)
        .await;
    assert_eq!(
        manager
            .refresh_access_token("rejected")
            .await
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::PermissionDenied
    );
    assert_eq!(manager.status().await.unwrap(), GatewayAuthStatus::NotReady);
    assert_eq!(
        events.try_recv().unwrap().status,
        GatewayAuthStatus::NotReady
    );
    for _ in 0..2 {
        assert_eq!(
            manager.resolve_access_token().await.unwrap_err().kind(),
            std::io::ErrorKind::PermissionDenied
        );
        assert_eq!(
            events.try_recv().unwrap().status,
            GatewayAuthStatus::NotReady
        );
        assert_eq!(manager.status().await.unwrap(), GatewayAuthStatus::NotReady);
        assert!(matches!(
            events.try_recv(),
            Err(tokio::sync::broadcast::error::TryRecvError::Empty)
        ));
    }
    // Canceling a new login must not make the previously rejected credential reusable.
    manager
        .login_with_browser(std::future::ready(()), |_| panic!("unexpected browser"))
        .await
        .unwrap_err();
    assert_eq!(
        manager.resolve_access_token().await.unwrap_err().kind(),
        std::io::ErrorKind::PermissionDenied
    );
    save_token(
        &manager,
        "replacement",
        Some("new-refresh"),
        Utc::now().timestamp() + 3600,
    );
    assert_eq!(
        manager.status().await.unwrap(),
        GatewayAuthStatus::Succeeded
    );
    assert_eq!(manager.resolve_access_token().await.unwrap(), "replacement");

    // A rejected token with no refresh token also requires replacement credentials.
    save_token(
        &manager,
        "no-refresh",
        /*refresh_token*/ None,
        Utc::now().timestamp() + 3600,
    );
    assert_eq!(
        manager
            .refresh_access_token("no-refresh")
            .await
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::PermissionDenied
    );
    assert_eq!(
        manager.resolve_access_token().await.unwrap_err().kind(),
        std::io::ErrorKind::PermissionDenied
    );
    Mock::given(method("POST"))
        .and(path("/token"))
        .and(body_string_contains("grant_type=authorization_code"))
        .respond_with(ResponseTemplate::new(/*s*/ 200).set_body_json(json!({
            "access_token": "signed-in", "expires_in": 3600,
        })))
        .expect(/*r*/ 1)
        .mount(&server)
        .await;
    manager
        .login_with_browser(std::future::pending(), complete_browser_authorization)
        .await
        .unwrap();
    let cache_lock = Arc::clone(&manager.state.cached_token).lock_owned().await;
    assert_eq!(
        manager.status().await.unwrap(),
        GatewayAuthStatus::Succeeded
    );
    drop(cache_lock);
    assert_eq!(manager.resolve_access_token().await.unwrap(), "signed-in");
}

#[tokio::test]
async fn dropping_login_clears_in_progress_status() {
    let server = MockServer::start().await;
    let (manager, _home) = client(config(&server), Arc::new(MockKeyringStore::default()));
    let observer = manager.clone();
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let login = tokio::spawn(async move {
        manager
            .login_with_browser(std::future::pending(), |_| {
                let _ = started_tx.send(());
            })
            .await
    });
    started_rx.await.unwrap();
    assert_eq!(observer.status().await.unwrap(), GatewayAuthStatus::Started);
    login.abort();
    assert!(login.await.unwrap_err().is_cancelled());
    assert_eq!(
        observer.status().await.unwrap(),
        GatewayAuthStatus::Failed {
            message: "Gateway sign-in was canceled".to_string()
        }
    );
}

#[tokio::test]
async fn expired_readiness_stays_not_ready_while_refresh_is_blocked() {
    for read_before_refresh in [false, true] {
        let server = MockServer::start().await;
        let (manager, home) = client(config(&server), Arc::new(MockKeyringStore::default()));
        let expired = StoredToken {
            access_token: "expired".into(),
            refresh_token: Some("refresh".into()),
            expires_at: Some(Utc::now().timestamp() - 1),
        };
        manager.save_token(&expired).unwrap();
        // Seed the last successful observation with a credential that has since expired.
        manager.publish_status(GatewayAuthStatus::Succeeded, Some(&expired));
        let mut events = manager.state.status_tx.subscribe();
        Mock::given(method("POST"))
            .and(path("/token"))
            .and(body_string_contains("grant_type=refresh_token"))
            .respond_with(ResponseTemplate::new(/*s*/ 200).set_body_json(json!({
                "access_token": "renewed", "expires_in": 3600,
            })))
            .expect(/*r*/ 1)
            .mount(&server)
            .await;
        if read_before_refresh {
            assert_eq!(manager.status().await.unwrap(), GatewayAuthStatus::NotReady);
        }
        // Block the real refresh on the cross-process store lock after it takes the cache lock.
        let store_lock = super::super::storage::lock_credentials(home.path())
            .await
            .unwrap();
        let refresh = manager.resolve_access_token();
        tokio::pin!(refresh);
        tokio::select! {
            result = &mut refresh => panic!("refresh unexpectedly completed: {result:?}"),
            ready = tokio::time::timeout(Duration::from_secs(/*secs*/ 5), async {
                while manager.state.cached_token.try_lock().is_ok() {
                    tokio::task::yield_now().await;
                }
            }) => ready.expect("refresh holds cache lock"),
        }
        assert_eq!(manager.status().await.unwrap(), GatewayAuthStatus::NotReady);
        assert_eq!(manager.status().await.unwrap(), GatewayAuthStatus::NotReady);
        assert_eq!(
            events.try_recv().unwrap().status,
            GatewayAuthStatus::NotReady
        );
        assert!(matches!(
            events.try_recv(),
            Err(tokio::sync::broadcast::error::TryRecvError::Empty)
        ));
        assert!(server.received_requests().await.unwrap().is_empty());
        drop(store_lock);
        assert_eq!(refresh.await.unwrap(), "renewed");
        // A busy cache must still report a newly refreshed, unexpired credential as usable.
        let cache_lock = Arc::clone(&manager.state.cached_token).lock_owned().await;
        assert_eq!(
            manager.status().await.unwrap(),
            GatewayAuthStatus::Succeeded
        );
        drop(cache_lock);
        assert_eq!(
            events.try_recv().unwrap().status,
            GatewayAuthStatus::Succeeded
        );
        assert_eq!(
            manager.status().await.unwrap(),
            GatewayAuthStatus::Succeeded
        );
    }
}

#[tokio::test]
async fn external_credentials_recover_missing_and_failed_readiness() {
    let server = MockServer::start().await;
    for failed_login in [false, true] {
        let keyring = Arc::new(MockKeyringStore::default());
        let (manager, _home) = client(config(&server), keyring.clone());
        let expected = if failed_login {
            // The failed attempt starts with an existing usable token. It must not immediately
            // look successful again just because that same token is still on disk.
            save_token(
                &manager,
                "old",
                /*refresh_token*/ None,
                Utc::now().timestamp() + 3600,
            );
            assert_eq!(manager.resolve_access_token().await.unwrap(), "old");
            save_token(
                &manager,
                "baseline",
                /*refresh_token*/ None,
                Utc::now().timestamp() + 3600,
            );
            let cancel = Notify::new();
            manager
                .login_with_browser(cancel.notified(), |_| cancel.notify_one())
                .await
                .unwrap_err();
            GatewayAuthStatus::Failed {
                message: "Gateway sign-in was canceled".into(),
            }
        } else {
            manager.resolve_access_token().await.unwrap_err();
            GatewayAuthStatus::NotReady
        };
        assert_eq!(manager.status().await.unwrap(), expected);
        if failed_login {
            // An older cached token must not be mistaken for a new external login.
            assert_eq!(manager.resolve_access_token().await.unwrap(), "baseline");
            assert_eq!(manager.status().await.unwrap(), expected);
        }
        save_token(
            &manager,
            "expired",
            /*refresh_token*/ None,
            Utc::now().timestamp() - 1,
        );
        assert_eq!(manager.status().await.unwrap(), expected);
        save_token(
            &manager,
            "replacement",
            /*refresh_token*/ None,
            Utc::now().timestamp() + 3600,
        );
        if !failed_login {
            // A request may load the replacement before the next readiness read.
            assert_eq!(manager.resolve_access_token().await.unwrap(), "replacement");
        }
        assert_eq!(
            manager.status().await.unwrap(),
            GatewayAuthStatus::Succeeded
        );
        assert_eq!(manager.resolve_access_token().await.unwrap(), "replacement");
    }
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn login_waits_for_background_credentials_and_can_cancel_while_queued() {
    let server = MockServer::start().await;
    let (manager, _home) = client(config(&server), Arc::new(MockKeyringStore::default()));
    for cancel_while_queued in [true, false] {
        // The credential mutex is also held by background resolution and refresh.
        let cache_lock = Arc::clone(&manager.state.cached_token).lock_owned().await;
        let cancel = tokio::sync::Notify::new();
        let (url_tx, mut url_rx) = tokio::sync::oneshot::channel();
        let login = manager.login_with_browser(cancel.notified(), |_| {
            let _ = url_tx.send(());
        });
        tokio::pin!(login);
        tokio::select! {
            biased;
            result = &mut login => panic!("login must wait for credentials: {result:?}"),
            _ = std::future::ready(()) => {}
        }
        assert!(matches!(
            url_rx.try_recv(),
            Err(tokio::sync::oneshot::error::TryRecvError::Empty)
        ));
        let duplicate = manager
            .login_with_browser(std::future::pending(), |_| panic!("duplicate browser"))
            .await
            .unwrap_err();
        assert_eq!(
            duplicate
                .get_ref()
                .and_then(|error| error.downcast_ref::<GatewayAuthError>())
                .copied(),
            Some(GatewayAuthError::LoginInProgress),
        );
        if cancel_while_queued {
            cancel.notify_one();
            let error = tokio::time::timeout(Duration::from_secs(/*secs*/ 2), &mut login)
                .await
                .unwrap()
                .unwrap_err();
            assert_eq!(error.kind(), std::io::ErrorKind::Interrupted);
            assert_eq!(manager.status().await.unwrap(), GatewayAuthStatus::NotReady);
            drop(cache_lock);
        } else {
            drop(cache_lock);
            let check = async {
                url_rx.await.unwrap();
                assert_eq!(manager.status().await.unwrap(), GatewayAuthStatus::Started);
                cancel.notify_one();
            };
            let (result, ()) = tokio::join!(login, check);
            assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::Interrupted);
        }
    }
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn legacy_policy_authorizes_missing_and_rejected_credentials() {
    for reauthorize in [false, true] {
        let server = MockServer::start().await;
        let (manager, _home) = client(config(&server), Arc::new(MockKeyringStore::default()));
        if reauthorize {
            save_token(
                &manager,
                "rejected",
                Some("revoked"),
                Utc::now().timestamp() - 1,
            );
            Mock::given(method("POST"))
                .and(body_string_contains("grant_type=refresh_token"))
                .respond_with(
                    ResponseTemplate::new(/*s*/ 400)
                        .set_body_json(json!({"error": "invalid_grant"})),
                )
                .expect(/*r*/ 1)
                .mount(&server)
                .await;
        }
        Mock::given(method("POST"))
            .and(body_string_contains("grant_type=authorization_code"))
            .respond_with(
                ResponseTemplate::new(/*s*/ 200)
                    .set_body_json(json!({"access_token": "signed-in"})),
            )
            .expect(/*r*/ 1)
            .mount(&server)
            .await;
        assert_eq!(
            manager
                .resolve_with_browser(super::super::RefreshPolicy::WhenExpired, |_| panic!(
                    "explicit mode opened browser"
                ))
                .await
                .unwrap_err()
                .kind(),
            std::io::ErrorKind::PermissionDenied
        );
        manager.control.allow_automatic_login();
        assert_eq!(
            manager
                .resolve_with_browser(
                    super::super::RefreshPolicy::WhenExpired,
                    complete_browser_authorization
                )
                .await
                .unwrap(),
            "signed-in"
        );
        assert_eq!(
            manager.status().await.unwrap(),
            GatewayAuthStatus::Succeeded
        );
    }
}
