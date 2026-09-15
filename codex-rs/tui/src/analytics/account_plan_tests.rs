//! Server plan discovery survives stale token claims and remains bound to the local identity.

use super::tests::live;
use super::tests::sign_in;
use super::*;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

#[tokio::test]
async fn refresh_uses_server_plan_and_routes_with_unchanged_local_credentials() {
    let server = MockServer::start().await;
    let (_home, client) = live(&server, "plus").await;
    let reads = Arc::new(AtomicUsize::new(/*v*/ 0));
    let plan_reads = Arc::clone(&reads);
    Mock::given(method("GET"))
        .and(path("/backend-api/wham/accounts/check"))
        .and(header("chatgpt-account-id", "account-a"))
        .respond_with(move |_: &wiremock::Request| {
            let plan = if plan_reads.fetch_add(/*val*/ 1, Ordering::SeqCst) == 0 {
                "team"
            } else {
                "enterprise"
            };
            ResponseTemplate::new(/*s*/ 200).set_body_json(json!({
                "accounts": [
                    {"id": "other-account", "plan_type": "free"},
                    {"id": "account-a", "plan_type": plan}
                ],
                "account_ordering": ["other-account", "account-a"]
            }))
        })
        .with_priority(/*p*/ 1)
        .expect(/*r*/ 2)
        .mount(&server)
        .await;
    for endpoint in [
        "/backend-api/wham/usage/daily-workspace-user-token-usage-breakdown",
        "/backend-api/wham/usage/daily-workspace-user-credit-usage",
    ] {
        Mock::given(method("GET"))
            .and(path(endpoint))
            .and(header("chatgpt-account-id", "account-a"))
            .respond_with(
                ResponseTemplate::new(/*s*/ 200)
                    .set_body_json(json!({"breakdown": "product", "series": [], "data": []})),
            )
            .expect(/*r*/ 1)
            .mount(&server)
            .await;
    }
    let first = client
        .history(Report::Credits, /*days*/ 7, Grouping::Surface)
        .await
        .unwrap();
    assert_eq!(client.session().await.unwrap().kind, AccountKind::Business);
    assert_eq!(
        client
            .history(Report::Credits, /*days*/ 7, Grouping::Surface)
            .await
            .unwrap(),
        first
    );
    assert_eq!(reads.load(Ordering::SeqCst), 1);
    let refreshed = Live::new(Arc::clone(&client.config), client.end_date);
    refreshed
        .history(Report::Credits, /*days*/ 7, Grouping::Surface)
        .await
        .unwrap();
    assert_eq!(
        refreshed.session().await.unwrap().kind,
        AccountKind::Enterprise
    );
    server.verify().await;
}

#[tokio::test]
async fn account_plan_lookup_recovers_from_unauthorized() {
    let server = MockServer::start().await;
    let (_home, client) = live(&server, "plus").await;
    let reads = AtomicUsize::new(/*v*/ 0);
    Mock::given(method("GET"))
        .and(path("/backend-api/wham/accounts/check"))
        .and(header("chatgpt-account-id", "account-a"))
        .respond_with(move |_: &wiremock::Request| {
            if reads.fetch_add(/*val*/ 1, Ordering::SeqCst) == 0 {
                ResponseTemplate::new(/*s*/ 401)
            } else {
                ResponseTemplate::new(/*s*/ 200).set_body_json(json!({
                    "accounts": {"account-a": {"account": {
                        "account_id": "account-a", "plan_type": "enterprise"
                    }}},
                    "account_ordering": ["account-a"]
                }))
            }
        })
        .with_priority(/*p*/ 1)
        .expect(/*r*/ 2)
        .mount(&server)
        .await;
    assert_eq!(
        client.session().await.unwrap().kind,
        AccountKind::Enterprise
    );
    server.verify().await;
}

#[tokio::test]
async fn failed_plan_discovery_can_retry_without_using_token_plan() {
    for response in [
        ResponseTemplate::new(/*s*/ 503),
        ResponseTemplate::new(/*s*/ 200).set_body_string("invalid account response"),
        ResponseTemplate::new(/*s*/ 200).set_body_json(json!({"accounts": [{"id": "account-a"}]})),
        ResponseTemplate::new(/*s*/ 200).set_body_json(json!({
            "accounts": [{"id": "other-account", "plan_type": "enterprise"}]
        })),
    ] {
        let server = MockServer::start().await;
        let (_home, client) = live(&server, "plus").await;
        Mock::given(method("GET"))
            .and(path("/backend-api/wham/accounts/check"))
            .respond_with(response)
            .with_priority(/*p*/ 1)
            .expect(/*r*/ 1)
            .up_to_n_times(/*n*/ 1)
            .mount(&server)
            .await;
        assert_eq!(
            client.session().await.err().unwrap(),
            "Couldn't load account plan. Press R to retry Analytics."
        );
        assert_eq!(client.session().await.unwrap().kind, AccountKind::Consumer);
        server.verify().await;
    }
}

#[tokio::test]
async fn account_plan_lookup_rejects_account_and_user_switches() {
    for (account, user) in [("account-b", "user-a"), ("account-a", "user-b")] {
        let server = MockServer::start().await;
        let (home, client) = live(&server, "plus").await;
        Mock::given(method("GET"))
            .and(path("/backend-api/wham/accounts/check"))
            .respond_with(move |_: &wiremock::Request| {
                sign_in(home.path(), account, user, "plus");
                ResponseTemplate::new(/*s*/ 200).set_body_json(json!({
                    "accounts": [{"id": "account-a", "plan_type": "enterprise"}]
                }))
            })
            .with_priority(/*p*/ 1)
            .expect(/*r*/ 1)
            .mount(&server)
            .await;
        assert_eq!(
            client.session().await.err().unwrap(),
            "Account changed. Press R to refresh Analytics."
        );
        server.verify().await;
    }
}
