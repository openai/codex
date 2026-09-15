//! Account readiness is verified after an in-flight backend read, including concurrent login.

use anyhow::Result;
use app_test_support::ChatGptAuthFixture;
use app_test_support::ChatGptIdTokenClaims;
use app_test_support::TestAppServer;
use app_test_support::encode_id_token;
use app_test_support::write_chatgpt_auth;
use axum::Json;
use axum::Router;
use axum::http::HeaderMap;
use axum::routing::get;
use codex_app_server_protocol::GetAccountRateLimitsResponse;
use codex_app_server_protocol::LoginAccountResponse;
use codex_config::types::AuthCredentialsStoreMode;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use test_case::test_case;
use tokio::net::TcpListener;
use tokio::sync::Notify;
use tokio::time::timeout;

const READ_TIMEOUT: Duration = Duration::from_secs(/*secs*/ 10);

#[test_case("workspace-a", "user-b", /*verified*/ false; "user_switch")]
#[test_case("workspace-b", "user-a", /*verified*/ false; "workspace_switch")]
#[test_case("workspace-a", "user-a", /*verified*/ true; "same_identity_token_refresh")]
#[tokio::test]
async fn identity_is_rechecked_after_backend_response(
    account: &str,
    user: &str,
    verified: bool,
) -> Result<()> {
    let home = TempDir::new()?;
    write_chatgpt_auth(
        home.path(),
        ChatGptAuthFixture::new("old-token")
            .account_id("workspace-a")
            .chatgpt_user_id("user-a")
            .plan_type("team"),
        AuthCredentialsStoreMode::File,
    )?;
    let entered = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let request_entered = Arc::clone(&entered);
    let response_release = Arc::clone(&release);
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    std::fs::write(
        home.path().join("config.toml"),
        format!("chatgpt_base_url = \"http://{}\"\n", listener.local_addr()?),
    )?;
    let router = Router::new().route(
        "/api/codex/usage",
        get(move |headers: HeaderMap| {
            let entered = Arc::clone(&request_entered);
            let release = Arc::clone(&response_release);
            async move {
                assert_eq!(headers["authorization"], "Bearer old-token");
                assert_eq!(headers["chatgpt-account-id"], "workspace-a");
                entered.notify_one();
                release.notified().await;
                Json(json!({
                    "account_id": "workspace-a", "user_id": "user-a", "plan_type": "team",
                    "rate_limit": {"allowed": true, "limit_reached": false,
                        "primary_window": {"used_percent": 42, "limit_window_seconds": 3600,
                            "reset_after_seconds": 120, "reset_at": 2000000000}}
                }))
            }
        }),
    );
    let server = tokio::spawn(async move { axum::serve(listener, router).await });
    let mut app = TestAppServer::builder()
        .with_codex_home(home.path())
        .with_env_overrides(&[("OPENAI_API_KEY", None)])
        .build_initialized_with_timeout(READ_TIMEOUT)
        .await?;
    let request = app
        .send_request(
            "account/rateLimits/read",
            Some(json!({"excludeResetCreditDetails": true})),
        )
        .await?;
    timeout(READ_TIMEOUT, entered.notified()).await?;
    let token = encode_id_token(
        &ChatGptIdTokenClaims::new()
            .chatgpt_account_id(account)
            .chatgpt_user_id(user)
            .plan_type("team"),
    )?;
    let login = app
        .send_chatgpt_auth_tokens_login_request(token, account.into(), Some("team".into()))
        .await?;
    let response: LoginAccountResponse = timeout(READ_TIMEOUT, app.read_response(login)).await??;
    assert_eq!(response, LoginAccountResponse::ChatgptAuthTokens {});
    release.notify_one();
    let response: GetAccountRateLimitsResponse =
        timeout(READ_TIMEOUT, app.read_response(request)).await??;
    server.abort();
    let snapshot = json!({
        "limitId": "codex", "planType": "team",
        "primary": {"usedPercent": 42, "windowDurationMins": 60, "resetsAt": 2000000000}
    });
    let expected: GetAccountRateLimitsResponse = serde_json::from_value(json!({
        "ordinaryUsageAllowed": if verified { Some(true) } else { None },
        "accountId": "workspace-a",
        "rateLimits": snapshot, "rateLimitsByLimitId": {"codex": snapshot}
    }))?;
    assert_eq!(response, expected);
    Ok(())
}
