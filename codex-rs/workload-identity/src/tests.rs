use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;

use pretty_assertions::assert_eq;
use reqwest::Client;
use tempfile::TempDir;
use url::Url;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

use super::*;
use crate::exchange::JWT_BEARER_GRANT_TYPE;
use crate::exchange::parse_loopback_token_url;

fn target() -> WorkloadIdentityTarget {
    WorkloadIdentityTarget {
        federation_rule_id: "rule-one".to_string(),
        principal_id: "user-one".to_string(),
        tenant_id: "tenant-one".to_string(),
        workspace_id: "workspace-one".to_string(),
    }
}

fn config(source: WorkloadIdentityAssertionSource) -> WorkloadIdentityConfig {
    WorkloadIdentityConfig::new(target(), source).expect("valid workload identity config")
}

fn make_exchange(
    server: &MockServer,
    source: WorkloadIdentityAssertionSource,
) -> WorkloadIdentityExchange {
    WorkloadIdentityExchange::with_client_builder(
        config(source),
        Url::parse(&format!("{}/oauth/token", server.uri())).expect("valid token URL"),
        Client::builder().no_proxy(),
    )
    .expect("valid exchange")
}

fn success(access_token: &str) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(serde_json::json!({
        "access_token": access_token,
        "token_type": "Bearer",
        "expires_in": 600,
        "chatgpt_account_id": "workspace-one",
        "chatgpt_plan_type": "enterprise",
        "user_id": "user-one"
    }))
}

#[test]
fn configuration_rejects_empty_selectors() {
    let mut target = target();
    target.workspace_id = "  ".to_string();
    assert!(matches!(
        WorkloadIdentityConfig::new(
            target,
            WorkloadIdentityAssertionSource::Environment("assertion".to_string())
        ),
        Err(WorkloadIdentityError::InvalidConfigurationField(
            "workspace_id"
        ))
    ));
}

#[test]
fn loopback_override_rejects_non_loopback_and_url_metadata() {
    for valid in [
        "https://localhost:3000/oauth/token",
        "http://127.0.0.1:3000/oauth/token",
        "http://[::1]:3000/oauth/token",
    ] {
        assert!(parse_loopback_token_url(valid).is_ok());
    }
    for invalid in [
        "https://auth.example.com/oauth/token",
        "http://auth.localhost:3000/oauth/token",
        "https://user:password@localhost/oauth/token",
        "https://localhost/oauth/token?assertion=secret",
        "file:///tmp/token",
    ] {
        assert!(matches!(
            parse_loopback_token_url(invalid),
            Err(WorkloadIdentityError::InvalidTokenUrl)
        ));
    }
}

#[tokio::test]
async fn exchange_sends_the_rfc_7523_contract_and_caches_the_result() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(success("access-one"))
        .mount(&server)
        .await;
    let exchange = make_exchange(
        &server,
        WorkloadIdentityAssertionSource::Environment("assertion-one".to_string()),
    );

    let expected = WorkloadIdentityToken {
        access_token: "access-one".to_string(),
        chatgpt_account_id: "workspace-one".to_string(),
        chatgpt_plan_type: Some("enterprise".to_string()),
        expires_in: 600,
    };
    assert_eq!(exchange.resolve().await.expect("first exchange"), expected);
    assert_eq!(exchange.resolve().await.expect("cached exchange"), expected);

    let requests = server.received_requests().await.expect("received requests");
    assert_eq!(requests.len(), 1);
    assert_eq!(
        url::form_urlencoded::parse(&requests[0].body)
            .into_owned()
            .collect::<HashMap<_, _>>(),
        HashMap::from([
            ("assertion".to_string(), "assertion-one".to_string()),
            ("federation_rule_id".to_string(), "rule-one".to_string()),
            ("grant_type".to_string(), JWT_BEARER_GRANT_TYPE.to_string()),
            ("principal_id".to_string(), "user-one".to_string()),
            ("tenant_id".to_string(), "tenant-one".to_string()),
            ("workspace_id".to_string(), "workspace-one".to_string()),
        ])
    );
}

#[tokio::test]
async fn file_source_is_reread_on_refresh() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(success("access-one"))
        .mount(&server)
        .await;
    let temp_dir = TempDir::new().expect("tempdir");
    let token_file = temp_dir.path().join("identity-token");
    tokio::fs::write(&token_file, "assertion-one\n")
        .await
        .expect("write assertion");
    let exchange = make_exchange(
        &server,
        WorkloadIdentityAssertionSource::File(token_file.clone()),
    );

    exchange.resolve().await.expect("initial exchange");
    tokio::fs::write(&token_file, "assertion-two\n")
        .await
        .expect("rotate assertion");
    exchange.refresh().await.expect("refresh exchange");

    let requests = server.received_requests().await.expect("received requests");
    let assertions = requests
        .iter()
        .map(|request| {
            url::form_urlencoded::parse(&request.body)
                .find(|(name, _)| name == "assertion")
                .map(|(_, value)| value.into_owned())
                .expect("assertion field")
        })
        .collect::<Vec<_>>();
    assert_eq!(assertions, vec!["assertion-one", "assertion-two"]);
}

#[tokio::test]
async fn concurrent_resolution_performs_one_exchange() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(success("access-one").set_delay(Duration::from_millis(50)))
        .mount(&server)
        .await;
    let exchange = Arc::new(make_exchange(
        &server,
        WorkloadIdentityAssertionSource::Environment("assertion-one".to_string()),
    ));

    let tasks = (0..8)
        .map(|_| {
            let exchange = Arc::clone(&exchange);
            tokio::spawn(async move { exchange.resolve().await })
        })
        .collect::<Vec<_>>();
    for task in tasks {
        task.await.expect("join exchange").expect("exchange");
    }
    assert_eq!(server.received_requests().await.expect("requests").len(), 1);

    let refreshes = (0..8)
        .map(|_| {
            let exchange = Arc::clone(&exchange);
            tokio::spawn(async move { exchange.refresh().await })
        })
        .collect::<Vec<_>>();
    for refresh in refreshes {
        refresh.await.expect("join refresh").expect("refresh");
    }
    assert_eq!(server.received_requests().await.expect("requests").len(), 2);
}

#[tokio::test]
async fn exchange_retries_transient_statuses_without_exposing_secrets() {
    let server = MockServer::start().await;
    let calls = Arc::new(AtomicUsize::new(0));
    Mock::given(method("POST"))
        .respond_with({
            let calls = Arc::clone(&calls);
            move |_request: &wiremock::Request| match calls.fetch_add(1, Ordering::SeqCst) {
                0 => ResponseTemplate::new(503).set_body_string("sensitive detail"),
                1 => ResponseTemplate::new(429),
                _ => success("access-one"),
            }
        })
        .mount(&server)
        .await;
    let exchange = make_exchange(
        &server,
        WorkloadIdentityAssertionSource::Environment("sensitive-assertion".to_string()),
    );

    assert_eq!(
        exchange
            .resolve()
            .await
            .expect("retried exchange")
            .access_token,
        "access-one"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 3);

    let rejected_server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(400).set_body_string("sensitive detail"))
        .mount(&rejected_server)
        .await;
    let rejected = make_exchange(
        &rejected_server,
        WorkloadIdentityAssertionSource::Environment("sensitive-assertion".to_string()),
    )
    .resolve()
    .await
    .expect_err("exchange should be rejected")
    .to_string();
    assert_eq!(
        rejected,
        "the workload identity token exchange was rejected with HTTP 400"
    );
}

#[tokio::test]
async fn exchange_rejects_mismatched_or_overlong_tokens() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": "access-one",
            "token_type": "Bearer",
            "expires_in": 3601,
            "chatgpt_account_id": "workspace-two",
            "user_id": "user-two"
        })))
        .mount(&server)
        .await;
    let exchange = make_exchange(
        &server,
        WorkloadIdentityAssertionSource::Environment("assertion-one".to_string()),
    );

    assert!(matches!(
        exchange.resolve().await,
        Err(WorkloadIdentityError::InvalidExchangeResponse)
    ));
}
