//! Account-scoped authentication and request identity regression coverage.

use super::*;
use crate::legacy_core::config::ConfigBuilder;
use base64::Engine;
use codex_config::LoaderOverrides;
use pretty_assertions::assert_eq;
use serde_json::json;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

fn sign_in(home: &std::path::Path, account: &str, user: &str, plan: &str) {
    let claims = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(
        json!({
            "exp": 4102444800_i64, "email": "analytics@example.test",
            "https://api.openai.com/auth": {
                "chatgpt_account_id": account, "chatgpt_user_id": user, "chatgpt_plan_type": plan,
            },
        })
        .to_string(),
    );
    let token = format!("e30.{claims}.test");
    let auth = serde_json::from_value(json!({
        "auth_mode": "chatgpt", "tokens": {"id_token": token, "access_token": token,
            "refresh_token": "test-refresh", "account_id": account},
        "last_refresh": chrono::Utc::now(),
    }))
    .unwrap();
    codex_login::save_auth(
        home,
        &auth,
        codex_login::AuthCredentialsStoreMode::File,
        codex_login::AuthKeyringBackendKind::default(),
    )
    .unwrap();
}

pub(in crate::analytics) async fn live(
    server: &MockServer,
    plan: &str,
) -> (tempfile::TempDir, Live) {
    let home = tempfile::tempdir().unwrap();
    let mut config = ConfigBuilder::default()
        .codex_home(home.path().to_path_buf())
        .loader_overrides(LoaderOverrides::without_managed_config_for_tests())
        .build()
        .await
        .unwrap();
    config.chatgpt_base_url = format!("{}/backend-api", server.uri());
    config.cli_auth_credentials_store_mode = codex_login::AuthCredentialsStoreMode::File;
    sign_in(home.path(), "account-a", "user-a", plan);
    (home, Live::new(Arc::new(config)))
}

#[tokio::test]
async fn analytics_rejects_identity_changes_during_requests() {
    let server = MockServer::start().await;
    let (home, live) = live(&server, "business").await;
    let session = live.session().await.unwrap();
    let authenticated = &session.backend;
    let result = authenticated
        .request(|_| async {
            sign_in(home.path(), "account-b", "user-a", "business");
            Ok(123)
        })
        .await;
    assert_eq!(
        result.unwrap_err().to_string(),
        "Account changed. Press R to refresh Analytics."
    );
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn analytics_rejects_account_and_user_changes_before_requests() {
    for (account, user) in [("account-b", "user-a"), ("account-a", "user-b")] {
        let server = MockServer::start().await;
        let (home, live) = live(&server, "plus").await;
        let session = live.session().await.unwrap();
        let authenticated = &session.backend;
        sign_in(home.path(), account, user, "plus");
        let requested = std::cell::Cell::new(/*value*/ false);
        let result = authenticated
            .request(|_| async {
                requested.set(/*val*/ true);
                Ok(())
            })
            .await;
        assert!(!requested.get());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Account changed. Press R to refresh Analytics."
        );
    }
}

#[tokio::test]
async fn analytics_requires_local_chatgpt_authentication() {
    for auth in [None, Some(json!({"OPENAI_API_KEY": "sk-test-only"}))] {
        let server = MockServer::start().await;
        let (home, live) = live(&server, "plus").await;
        let path = home.path().join("auth.json");
        match auth {
            Some(auth) => std::fs::write(path, serde_json::to_vec(&auth).unwrap()).unwrap(),
            None => std::fs::remove_file(path).unwrap(),
        }
        assert_eq!(
            live.session().await.err(),
            Some("Sign in locally with ChatGPT to view Analytics.".to_string())
        );
        assert!(server.received_requests().await.unwrap().is_empty());
    }
}

#[tokio::test]
async fn analytics_requests_use_reloaded_credentials_for_the_same_identity() {
    let server = MockServer::start().await;
    let (home, live) = live(&server, "plus").await;
    let session = live.session().await.unwrap();
    let authenticated = &session.backend;
    assert_eq!(
        live.account_label().as_deref(),
        Some("analytics@example.test · account-a")
    );
    let auth_path = home.path().join("auth.json");
    let mut auth: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&auth_path).unwrap()).unwrap();
    auth["tokens"]["access_token"] = json!("refreshed-access-token");
    std::fs::write(auth_path, serde_json::to_vec(&auth).unwrap()).unwrap();
    Mock::given(method("GET"))
        .and(path("/backend-api/wham/usage/daily-token-usage-breakdown"))
        .and(header("chatgpt-account-id", "account-a"))
        .and(header("authorization", "Bearer refreshed-access-token"))
        .respond_with(ResponseTemplate::new(/*s*/ 200).set_body_json(json!({"data": []})))
        .expect(/*r*/ 1)
        .mount(&server)
        .await;
    let result = authenticated
        .request(|client| async move {
            client
                .get_account_analytics(
                    codex_backend_client::AnalyticsReport::Usage,
                    "2026-09-01",
                    "2026-09-07",
                )
                .await
        })
        .await
        .unwrap();
    assert_eq!(
        result,
        codex_backend_client::AnalyticsResponse::Usage(
            codex_backend_client::analytics_models::DailyProductSurfaceUsageResponse::default(),
        ),
    );
    server.verify().await;
}

#[tokio::test]
async fn analytics_retries_unauthorized_requests_after_credentials_reload() {
    let server = MockServer::start().await;
    let (home, live) = live(&server, "plus").await;
    let session = live.session().await.unwrap();
    let authenticated = &session.backend;
    let auth_path = home.path().join("auth.json");
    let mut auth: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&auth_path).unwrap()).unwrap();
    let original_token = auth["tokens"]["access_token"].as_str().unwrap().to_owned();
    auth["tokens"]["access_token"] = json!("recovered-access-token");
    let updated_auth = serde_json::to_vec(&auth).unwrap();
    Mock::given(method("GET"))
        .and(header("authorization", format!("Bearer {original_token}")))
        .respond_with(move |_: &wiremock::Request| {
            std::fs::write(&auth_path, &updated_auth).unwrap();
            ResponseTemplate::new(/*s*/ 401)
        })
        .expect(/*r*/ 1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(header("authorization", "Bearer recovered-access-token"))
        .and(header("chatgpt-account-id", "account-a"))
        .respond_with(ResponseTemplate::new(/*s*/ 200).set_body_json(json!({"data": []})))
        .expect(/*r*/ 1)
        .mount(&server)
        .await;
    let result = authenticated
        .request(|client| async move {
            client
                .get_account_analytics(
                    codex_backend_client::AnalyticsReport::Usage,
                    "2026-09-01",
                    "2026-09-07",
                )
                .await
        })
        .await
        .unwrap();
    assert_eq!(
        result,
        codex_backend_client::AnalyticsResponse::Usage(
            codex_backend_client::analytics_models::DailyProductSurfaceUsageResponse::default(),
        ),
    );
    server.verify().await;
}

#[tokio::test]
async fn analytics_retries_session_initialization_after_sign_in() {
    let server = MockServer::start().await;
    let (home, live) = live(&server, "plus").await;
    std::fs::remove_file(home.path().join("auth.json")).unwrap();
    assert!(live.session().await.is_err());
    sign_in(home.path(), "account-a", "user-a", "plus");
    assert_eq!(
        live.session().await.unwrap().backend.account().id,
        "account-a"
    );
}
