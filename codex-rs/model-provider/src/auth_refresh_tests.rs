//! Verifies that concurrent proactive and unauthorized requests share one OAuth refresh.

use super::auth_provider_from_auth_manager;
use codex_login::AuthCredentialsStoreMode;
use codex_login::AuthHeaders;
use codex_login::AuthKeyringBackendKind;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use codex_login::ExternalAuth;
use codex_login::RefreshTokenError;
use codex_login::auth::RefreshTokenFailedError;
use codex_login::auth::RefreshTokenFailedReason;
use http::HeaderMap;
use http::HeaderValue;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::task::Poll;
use tempfile::TempDir;
use tokio::sync::Barrier;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::body_partial_json;
use wiremock::matchers::method;
use wiremock::matchers::path;

#[tokio::test]
async fn concurrent_and_late_rejections_share_one_refresh() {
    check_concurrent_refresh(RefreshTrigger::Unauthorized).await;
}

#[tokio::test]
async fn concurrent_proactive_requests_share_one_refresh() {
    check_concurrent_refresh(RefreshTrigger::Proactive).await;
}

struct RotatingExternalAuth {
    refreshes: AtomicUsize,
    barrier: Barrier,
}

impl ExternalAuth for RotatingExternalAuth {
    fn resolve(&self) -> codex_login::ExternalAuthFuture<'_, CodexAuth> {
        Box::pin(async {
            let token = if self.refreshes.load(Ordering::SeqCst) == 0 {
                "Bearer initial"
            } else {
                "Bearer replacement"
            };
            Ok(CodexAuth::Headers(AuthHeaders::new(HeaderMap::from_iter(
                [(http::header::AUTHORIZATION, HeaderValue::from_static(token))],
            ))))
        })
    }

    fn refresh(
        &self,
        _context: codex_login::ExternalAuthRefreshContext,
    ) -> codex_login::ExternalAuthFuture<'_, CodexAuth> {
        Box::pin(async {
            if self.refreshes.fetch_add(1, Ordering::SeqCst) != 0 {
                return Err(std::io::Error::other("redundant refresh rejected"));
            }
            self.barrier.wait().await;
            self.resolve().await
        })
    }

    fn classify_error(&self, error: std::io::Error) -> RefreshTokenError {
        RefreshTokenError::Permanent(RefreshTokenFailedError::new(
            RefreshTokenFailedReason::Other,
            error.to_string(),
        ))
    }
}

#[tokio::test]
async fn concurrent_external_rejections_share_one_refresh() {
    let external = Arc::new(RotatingExternalAuth {
        refreshes: AtomicUsize::new(0),
        barrier: Barrier::new(2),
    });
    let initial = external.resolve().await.unwrap();
    let manager = AuthManager::from_auth_for_testing(initial.clone());
    manager.set_external_auth(external.clone()).await.unwrap();
    let provider = auth_provider_from_auth_manager(manager, &initial);
    let rejected = provider.to_auth_headers().unwrap();
    let mut first = provider.refresh_after_unauthorized(rejected.clone());
    let mut second = provider.refresh_after_unauthorized(rejected.clone());
    std::future::poll_fn(|cx| {
        assert!(first.as_mut().poll(cx).is_pending());
        assert!(second.as_mut().poll(cx).is_pending());
        Poll::Ready(())
    })
    .await;
    external.barrier.wait().await;
    assert_eq!(tokio::join!(first, second), (true, true));
    assert!(provider.refresh_after_unauthorized(rejected).await);
    assert_eq!(external.refreshes.load(Ordering::SeqCst), 1);
    assert_eq!(
        provider.apply_auth_headers(HeaderMap::new()).await.unwrap()[http::header::AUTHORIZATION],
        "Bearer replacement"
    );
}

#[derive(Clone, Copy)]
enum RefreshTrigger {
    Unauthorized,
    Proactive,
}

async fn check_concurrent_refresh(trigger: RefreshTrigger) {
    let server = MockServer::start().await;
    let home = TempDir::new().unwrap();
    let auth = json!({
        "tokens": {
            "id_token": "e30.eyJodHRwczovL2FwaS5vcGVuYWkuY29tL2F1dGgiOnsiY2hhdGdwdF91c2VyX2lkIjoib3JpZ2luYWwtdXNlciIsImNoYXRncHRfYWNjb3VudF9pZCI6Im9yaWdpbmFsLWFjY291bnQifX0.sig",
            "access_token": "initial-token",
            "refresh_token": "initial-refresh-token",
            "account_id": "original-account",
            "oauth_provenance": {
                "issuer": server.uri(),
                "client_id": "concurrent-refresh-test"
            }
        },
        "last_refresh": match trigger {
            RefreshTrigger::Unauthorized => "2099-01-01T00:00:00Z",
            RefreshTrigger::Proactive => "2000-01-01T00:00:00Z",
        }
    });
    std::fs::write(
        home.path().join("auth.json"),
        serde_json::to_vec(&auth).unwrap(),
    )
    .unwrap();
    let manager = Arc::new(
        AuthManager::new(
            home.path().to_path_buf(),
            /*enable_codex_api_key_env*/ false,
            AuthCredentialsStoreMode::File,
            /*forced_chatgpt_workspace_id*/ None,
            /*chatgpt_base_url*/ None,
            AuthKeyringBackendKind::default(),
            codex_login::test_support::transport_default_auth_route_config(),
        )
        .await,
    );
    let provider =
        auth_provider_from_auth_manager(manager.clone(), &manager.auth_cached().unwrap());
    let rejected_headers = provider.to_auth_headers().unwrap();
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .and(body_partial_json(
            json!({"refresh_token": "initial-refresh-token"}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "replacement-token",
            "refresh_token": "replacement-refresh-token"
        })))
        .expect(1)
        .mount(&server)
        .await;

    // A redundant refresh could permanently fail even though the replacement works.
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .and(body_partial_json(
            json!({"refresh_token": "replacement-refresh-token"}),
        ))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({
            "error": "invalid_grant",
        })))
        .with_priority(10)
        .expect(0)
        .mount(&server)
        .await;

    let refresh = || async {
        match trigger {
            RefreshTrigger::Unauthorized => {
                provider
                    .refresh_after_unauthorized(rejected_headers.clone())
                    .await
            }
            RefreshTrigger::Proactive => {
                let headers = provider.apply_auth_headers(HeaderMap::new()).await.unwrap();
                assert_eq!(
                    headers[http::header::AUTHORIZATION],
                    "Bearer replacement-token"
                );
                true
            }
        }
    };
    let mut first = Box::pin(refresh());
    let mut second = Box::pin(refresh());
    let mut third = Box::pin(refresh());
    // Queue both waiters before the runtime can service the first OAuth request.
    std::future::poll_fn(|cx| {
        assert!(first.as_mut().poll(cx).is_pending());
        assert!(second.as_mut().poll(cx).is_pending());
        assert!(third.as_mut().poll(cx).is_pending());
        Poll::Ready(())
    })
    .await;
    assert_eq!(tokio::join!(first, second, third), (true, true, true));
    // A delayed 401 for the old token also reuses the completed refresh.
    assert!(
        provider
            .refresh_after_unauthorized(rejected_headers.clone())
            .await
    );
    let updated_headers = provider.apply_auth_headers(HeaderMap::new()).await.unwrap();
    assert_eq!(
        updated_headers[http::header::AUTHORIZATION],
        "Bearer replacement-token"
    );

    // A rejection of the new credential must still be able to refresh again.
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .and(body_partial_json(
            json!({"refresh_token": "replacement-refresh-token"}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "next-token",
            "refresh_token": "next-refresh-token"
        })))
        .expect(1)
        .mount(&server)
        .await;
    assert!(provider.refresh_after_unauthorized(updated_headers).await);
    assert_eq!(
        provider.apply_auth_headers(HeaderMap::new()).await.unwrap()[http::header::AUTHORIZATION],
        "Bearer next-token"
    );
    // A permanent failure must not prevent a later guarded reload of fresh credentials.
    let rejected = provider.to_auth_headers().unwrap();
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .and(body_partial_json(
            json!({"refresh_token": "next-refresh-token"}),
        ))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({"error": "invalid_grant"})))
        .expect(1)
        .mount(&server)
        .await;
    assert!(!provider.refresh_after_unauthorized(rejected.clone()).await);
    let path = home.path().join("auth.json");
    let mut stored: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    stored["tokens"]["access_token"] = json!("reloaded-token");
    std::fs::write(path, serde_json::to_vec(&stored).unwrap()).unwrap();
    assert!(provider.refresh_after_unauthorized(rejected).await);
    assert_eq!(
        provider.to_auth_headers().unwrap()[http::header::AUTHORIZATION],
        "Bearer reloaded-token"
    );
    server.verify().await;
}
