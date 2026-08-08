use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;

use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use url::Url;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

use super::ACCESS_TOKEN_TYPE;
use super::JWT_BEARER_GRANT_TYPE;
use super::WorkloadIdentityExchange;
use super::WorkloadIdentityToken;
use crate::WorkloadIdentityConfig;
use crate::WorkloadIdentityError;

fn assertion_file(assertion: &str) -> (TempDir, PathBuf) {
    let temp_dir = TempDir::new().expect("tempdir");
    let path = temp_dir.path().join("identity-token");
    std::fs::write(&path, assertion).expect("write assertion");
    (temp_dir, path)
}

fn make_exchange(path: PathBuf, server: &MockServer) -> WorkloadIdentityExchange {
    WorkloadIdentityExchange::new(
        WorkloadIdentityConfig::new("idpm_rule_one".to_string(), path).expect("valid config"),
        Url::parse(&format!("{}/oauth/token", server.uri())).expect("valid token URL"),
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
    )
    .expect("valid exchange")
}

fn success(access_token: &str, expires_in: u64) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(serde_json::json!({
        "access_token": access_token,
        "issued_token_type": ACCESS_TOKEN_TYPE,
        "token_type": "Bearer",
        "expires_in": expires_in,
        "scope": "openid profile email chatgpt.workspace.feature.allow-codex-local-access.access",
        "chatgpt_account_id": "workspace-one",
        "chatgpt_account_user_id": "membership-one",
        "user_id": "user-one",
        "chatgpt_plan_type": "enterprise"
    }))
}

#[tokio::test]
async fn exchange_sends_three_field_contract_and_caches_valid_response() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .and(header("content-type", "application/x-www-form-urlencoded"))
        .respond_with(success("sensitive-access-token", /*expires_in*/ 600))
        .mount(&server)
        .await;
    let (_temp_dir, assertion_path) = assertion_file("assertion-one\n");
    let exchange = make_exchange(assertion_path, &server);

    let expected = WorkloadIdentityToken {
        access_token: "sensitive-access-token".to_string(),
        chatgpt_account_id: "workspace-one".to_string(),
        chatgpt_account_user_id: "membership-one".to_string(),
        chatgpt_plan_type: Some("enterprise".to_string()),
        expires_in: 600,
        scope: "openid profile email chatgpt.workspace.feature.allow-codex-local-access.access"
            .to_string(),
        user_id: "user-one".to_string(),
        version: 1,
    };
    assert_eq!(exchange.resolve().await.expect("exchange"), expected);
    assert_eq!(exchange.resolve().await.expect("cached token"), expected);
    assert!(!format!("{expected:?}").contains("sensitive-access-token"));

    let requests = server.received_requests().await.expect("requests");
    assert_eq!(requests.len(), 1);
    assert_eq!(
        url::form_urlencoded::parse(&requests[0].body)
            .into_owned()
            .collect::<Vec<_>>(),
        vec![
            ("grant_type".to_string(), JWT_BEARER_GRANT_TYPE.to_string()),
            ("assertion".to_string(), "assertion-one".to_string()),
            (
                "federation_rule_id".to_string(),
                "idpm_rule_one".to_string()
            ),
        ]
    );
}

#[tokio::test]
async fn concurrent_resolve_and_rejected_token_refresh_are_single_flight() {
    let server = MockServer::start().await;
    let calls = Arc::new(AtomicUsize::new(0));
    Mock::given(method("POST"))
        .respond_with({
            let calls = Arc::clone(&calls);
            move |_request: &wiremock::Request| {
                let call = calls.fetch_add(1, Ordering::SeqCst) + 1;
                success(&format!("access-{call}"), /*expires_in*/ 600)
                    .set_delay(Duration::from_millis(50))
            }
        })
        .mount(&server)
        .await;
    let (_temp_dir, assertion_path) = assertion_file("assertion-one");
    let exchange = Arc::new(make_exchange(assertion_path.clone(), &server));

    let resolves = (0..8)
        .map(|_| {
            let exchange = Arc::clone(&exchange);
            tokio::spawn(async move { exchange.resolve().await })
        })
        .collect::<Vec<_>>();
    let mut initial = None;
    for resolve in resolves {
        let token = resolve.await.expect("join resolve").expect("resolve");
        assert_eq!(token.access_token, "access-1");
        initial = Some(token);
    }
    tokio::fs::write(&assertion_path, "assertion-two\n")
        .await
        .expect("rotate assertion");
    let version = initial.expect("initial token").version();
    let refreshes = (0..8)
        .map(|_| {
            let exchange = Arc::clone(&exchange);
            tokio::spawn(async move { exchange.refresh(version).await })
        })
        .collect::<Vec<_>>();
    for refresh in refreshes {
        assert_eq!(
            refresh
                .await
                .expect("join refresh")
                .expect("refresh")
                .access_token,
            "access-2"
        );
    }

    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert_eq!(
        server
            .received_requests()
            .await
            .expect("requests")
            .iter()
            .map(|request| {
                url::form_urlencoded::parse(&request.body)
                    .find(|(name, _)| name == "assertion")
                    .map(|(_, value)| value.into_owned())
                    .expect("assertion field")
            })
            .collect::<Vec<_>>(),
        vec!["assertion-one", "assertion-two"]
    );
}

#[tokio::test]
async fn rejected_token_waiting_on_proactive_fallback_still_forces_refresh() {
    let server = MockServer::start().await;
    let calls = Arc::new(AtomicUsize::new(0));
    Mock::given(method("POST"))
        .respond_with({
            let calls = Arc::clone(&calls);
            move |_request: &wiremock::Request| match calls.fetch_add(1, Ordering::SeqCst) {
                0 => success("access-one", /*expires_in*/ 600),
                1 => ResponseTemplate::new(503).set_delay(Duration::from_millis(200)),
                2.. => success("access-three", /*expires_in*/ 600),
            }
        })
        .mount(&server)
        .await;
    let (_temp_dir, assertion_path) = assertion_file("assertion-one");
    let exchange = Arc::new(make_exchange(assertion_path, &server));
    let initial = exchange.resolve().await.expect("initial exchange");
    exchange
        .state
        .lock()
        .await
        .cached
        .as_mut()
        .expect("cached token")
        .refresh_at = std::time::Instant::now();

    let proactive = tokio::spawn({
        let exchange = Arc::clone(&exchange);
        async move { exchange.resolve().await }
    });
    tokio::time::timeout(Duration::from_secs(1), async {
        while calls.load(Ordering::SeqCst) < 2 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("proactive exchange started");

    let forced = exchange.refresh(initial.version());
    tokio::pin!(forced);
    assert!(
        tokio::time::timeout(Duration::from_millis(20), forced.as_mut())
            .await
            .is_err(),
        "forced refresh should wait for the proactive exchange"
    );
    let fallback = proactive
        .await
        .expect("join proactive refresh")
        .expect("cached fallback");
    assert_eq!(fallback.access_token, initial.access_token);

    let refreshed = forced.await.expect("forced refresh");
    assert_eq!(refreshed.access_token, "access-three");
    assert_ne!(refreshed.version(), initial.version());
    assert_eq!(calls.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn transient_proactive_refresh_failure_uses_still_valid_token() {
    let server = MockServer::start().await;
    let calls = Arc::new(AtomicUsize::new(0));
    Mock::given(method("POST"))
        .respond_with({
            let calls = Arc::clone(&calls);
            move |_request: &wiremock::Request| {
                if calls.fetch_add(1, Ordering::SeqCst) == 0 {
                    success("access-one", /*expires_in*/ 4)
                } else {
                    ResponseTemplate::new(503).set_body_string("sensitive server detail")
                }
            }
        })
        .mount(&server)
        .await;
    let (_temp_dir, assertion_path) = assertion_file("assertion-one");
    let exchange = make_exchange(assertion_path, &server);
    let initial = exchange.resolve().await.expect("initial exchange");
    exchange
        .state
        .lock()
        .await
        .cached
        .as_mut()
        .expect("cached token")
        .refresh_at = std::time::Instant::now();

    let fallback = exchange.resolve().await.expect("cached fallback");
    assert_eq!(fallback.access_token, initial.access_token);
    assert_eq!(fallback.version(), initial.version());
    assert_eq!(exchange.resolve().await.expect("delayed retry"), fallback);
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[test]
fn configuration_requires_an_absolute_file_and_secure_token_url() {
    assert!(matches!(
        WorkloadIdentityConfig::new("idpm_rule_one".to_string(), PathBuf::from("relative.jwt")),
        Err(WorkloadIdentityError::AssertionFileMustBeAbsolute)
    ));

    let (_temp_dir, assertion_path) = assertion_file("assertion-one");
    let config = WorkloadIdentityConfig::new("idpm_rule_one".to_string(), assertion_path)
        .expect("valid config");
    assert!(matches!(
        WorkloadIdentityExchange::new(
            config,
            Url::parse("http://auth.example.com/oauth/token").expect("parse URL"),
            HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
        ),
        Err(WorkloadIdentityError::InvalidTokenUrl)
    ));
}

#[tokio::test]
async fn exchange_rejects_oversized_assertions_and_incomplete_responses() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": "access-one",
            "issued_token_type": ACCESS_TOKEN_TYPE,
            "token_type": "Bearer",
            "expires_in": 600,
            "scope": "openid",
            "chatgpt_account_id": "workspace-one",
            "user_id": "user-one"
        })))
        .mount(&server)
        .await;
    let (_temp_dir, assertion_path) = assertion_file(&"x".repeat(16 * 1024 + 1));
    let exchange = make_exchange(assertion_path.clone(), &server);
    assert!(matches!(
        exchange.resolve().await,
        Err(WorkloadIdentityError::AssertionTooLarge)
    ));

    tokio::fs::write(&assertion_path, "valid-assertion")
        .await
        .expect("replace assertion");
    assert!(matches!(
        exchange.resolve().await,
        Err(WorkloadIdentityError::InvalidExchangeResponse)
    ));
}
