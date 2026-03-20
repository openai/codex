use super::*;
use crate::auth::storage::FileAuthStorage;
use crate::auth::storage::get_auth_file;
use crate::token_data::IdTokenInfo;
use crate::token_data::KnownPlan as InternalKnownPlan;
use crate::token_data::PlanType as InternalPlanType;
use crate::token_data::TokenData;
use codex_protocol::account::PlanType as AccountPlanType;

use base64::Engine;
use chrono::DateTime;
use chrono::Duration;
use codex_protocol::config_types::ForcedLoginMethod;
use pretty_assertions::assert_eq;
use serde::Serialize;
use serde_json::json;
use std::sync::Arc;
use tempfile::tempdir;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::Request;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

#[tokio::test]
async fn refresh_without_id_token() {
    let codex_home = tempdir().unwrap();
    let fake_jwt = write_auth_file(
        AuthFileParams {
            openai_api_key: None,
            chatgpt_plan_type: Some("pro".to_string()),
            chatgpt_account_id: None,
        },
        codex_home.path(),
    )
    .expect("failed to write auth file");

    let storage = create_auth_storage(
        codex_home.path().to_path_buf(),
        AuthCredentialsStoreMode::File,
    );
    let updated = super::persist_tokens(
        &storage,
        None,
        Some("new-access-token".to_string()),
        Some("new-refresh-token".to_string()),
    )
    .expect("update_tokens should succeed");

    let tokens = updated.tokens.expect("tokens should exist");
    assert_eq!(tokens.id_token.raw_jwt, fake_jwt);
    assert_eq!(tokens.access_token, "new-access-token");
    assert_eq!(tokens.refresh_token, "new-refresh-token");
}

#[test]
fn login_with_api_key_overwrites_existing_auth_json() {
    let dir = tempdir().unwrap();
    let auth_path = dir.path().join("auth.json");
    let stale_auth = json!({
        "OPENAI_API_KEY": "sk-old",
        "tokens": {
            "id_token": "stale.header.payload",
            "access_token": "stale-access",
            "refresh_token": "stale-refresh",
            "account_id": "stale-acc"
        }
    });
    std::fs::write(
        &auth_path,
        serde_json::to_string_pretty(&stale_auth).unwrap(),
    )
    .unwrap();

    super::login_with_api_key(dir.path(), "sk-new", AuthCredentialsStoreMode::File)
        .expect("login_with_api_key should succeed");

    let storage = FileAuthStorage::new(dir.path().to_path_buf());
    let auth = storage
        .try_read_auth_json(&auth_path)
        .expect("auth.json should parse");
    assert_eq!(auth.openai_api_key.as_deref(), Some("sk-new"));
    assert!(auth.tokens.is_none(), "tokens should be cleared");
}

#[test]
fn missing_auth_json_returns_none() {
    let dir = tempdir().unwrap();
    let auth = CodexAuth::from_auth_storage(dir.path(), AuthCredentialsStoreMode::File)
        .expect("call should succeed");
    assert_eq!(auth, None);
}

#[tokio::test]
#[serial(codex_api_key)]
async fn pro_account_with_no_api_key_uses_chatgpt_auth() {
    let codex_home = tempdir().unwrap();
    let fake_jwt = write_auth_file(
        AuthFileParams {
            openai_api_key: None,
            chatgpt_plan_type: Some("pro".to_string()),
            chatgpt_account_id: None,
        },
        codex_home.path(),
    )
    .expect("failed to write auth file");

    let auth = super::load_auth(codex_home.path(), false, AuthCredentialsStoreMode::File)
        .unwrap()
        .unwrap();
    assert_eq!(None, auth.api_key());
    assert_eq!(crate::AuthMode::Chatgpt, auth.auth_mode());
    assert_eq!(auth.get_chatgpt_user_id().as_deref(), Some("user-12345"));

    let auth_dot_json = auth
        .get_current_auth_json()
        .expect("AuthDotJson should exist");
    let last_refresh = auth_dot_json
        .last_refresh
        .expect("last_refresh should be recorded");

    assert_eq!(
        AuthDotJson {
            auth_mode: None,
            openai_api_key: None,
            tokens: Some(TokenData {
                id_token: IdTokenInfo {
                    email: Some("user@example.com".to_string()),
                    chatgpt_plan_type: Some(InternalPlanType::Known(InternalKnownPlan::Pro)),
                    chatgpt_user_id: Some("user-12345".to_string()),
                    chatgpt_account_id: None,
                    raw_jwt: fake_jwt,
                },
                access_token: "test-access-token".to_string(),
                refresh_token: "test-refresh-token".to_string(),
                account_id: None,
            }),
            last_refresh: Some(last_refresh),
        },
        auth_dot_json
    );
}

#[tokio::test]
#[serial(codex_api_key)]
async fn loads_api_key_from_auth_json() {
    let dir = tempdir().unwrap();
    let auth_file = dir.path().join("auth.json");
    std::fs::write(
        auth_file,
        r#"{"OPENAI_API_KEY":"sk-test-key","tokens":null,"last_refresh":null}"#,
    )
    .unwrap();

    let auth = super::load_auth(dir.path(), false, AuthCredentialsStoreMode::File)
        .unwrap()
        .unwrap();
    assert_eq!(auth.auth_mode(), crate::AuthMode::ApiKey);
    assert_eq!(auth.api_key(), Some("sk-test-key"));

    assert!(auth.get_token_data().is_err());
}

#[test]
fn logout_removes_auth_file() -> Result<(), std::io::Error> {
    let dir = tempdir()?;
    let auth_dot_json = AuthDotJson {
        auth_mode: Some(ApiAuthMode::ApiKey),
        openai_api_key: Some("sk-test-key".to_string()),
        tokens: None,
        last_refresh: None,
    };
    super::save_auth(dir.path(), &auth_dot_json, AuthCredentialsStoreMode::File)?;
    let auth_file = get_auth_file(dir.path());
    assert!(auth_file.exists());
    assert!(logout(dir.path(), AuthCredentialsStoreMode::File)?);
    assert!(!auth_file.exists());
    Ok(())
}

#[test]
fn unauthorized_recovery_reports_mode_and_step_names() {
    let dir = tempdir().unwrap();
    let manager = AuthManager::shared(
        dir.path().to_path_buf(),
        false,
        AuthCredentialsStoreMode::File,
    );
    let managed = UnauthorizedRecovery {
        manager: Arc::clone(&manager),
        step: UnauthorizedRecoveryStep::Reload,
        expected_account_id: None,
        recovery_auth: None,
        mode: UnauthorizedRecoveryMode::Managed,
    };
    assert_eq!(managed.mode_name(), "managed");
    assert_eq!(managed.step_name(), "reload");

    let external = UnauthorizedRecovery {
        manager,
        step: UnauthorizedRecoveryStep::ExternalRefresh,
        expected_account_id: None,
        recovery_auth: None,
        mode: UnauthorizedRecoveryMode::External,
    };
    assert_eq!(external.mode_name(), "external");
    assert_eq!(external.step_name(), "external_refresh");
}

#[tokio::test]
#[serial(codex_api_key)]
async fn stale_proactive_refresh_uses_newer_local_auth_before_spending_cached_refresh_token() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;

    let ctx = RefreshTokenTestContext::new(&server);
    let stale_auth = managed_auth_dot_json(
        "stale-access",
        "stale-refresh",
        Some("account-id"),
        "user-123",
        "user@example.com",
        refresh_time_days_ago(31),
    );
    ctx.write_auth(&stale_auth);

    let newer_auth = managed_auth_dot_json(
        "newer-access",
        "newer-refresh",
        Some("account-id"),
        "user-123",
        "user@example.com",
        Utc::now(),
    );
    save_auth(
        ctx.codex_home.path(),
        &newer_auth,
        AuthCredentialsStoreMode::File,
    )
    .expect("persist newer auth");

    let refreshed = ctx.auth_manager.auth().await.expect("auth should exist");
    assert_eq!(
        refreshed.get_token_data().expect("token data").access_token,
        "newer-access"
    );

    server.verify().await;
}

#[tokio::test]
#[serial(codex_api_key)]
async fn stale_proactive_refresh_uses_caller_snapshot_when_cache_already_changed() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;

    let ctx = RefreshTokenTestContext::new(&server);
    let stale_auth = managed_auth_dot_json(
        "stale-access",
        "stale-refresh",
        Some("account-id"),
        "user-123",
        "user@example.com",
        refresh_time_days_ago(31),
    );
    ctx.write_auth(&stale_auth);
    let stale_cached_auth = ctx.auth_manager.auth_cached().expect("cached auth");

    let newer_auth = managed_auth_dot_json(
        "newer-access",
        "newer-refresh",
        Some("account-id"),
        "user-123",
        "user@example.com",
        Utc::now(),
    );
    ctx.write_auth(&newer_auth);

    assert_eq!(
        ctx.auth_manager
            .refresh_if_stale(&stale_cached_auth)
            .await
            .expect("refresh should succeed"),
        true
    );
    assert_eq!(
        ctx.auth_manager
            .auth_cached()
            .expect("cached auth")
            .get_token_data()
            .expect("token data")
            .access_token,
        "newer-access"
    );

    server.verify().await;
}

#[tokio::test]
#[serial(codex_api_key)]
async fn stale_proactive_refresh_without_account_id_still_recovers_newer_local_auth() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;

    let ctx = RefreshTokenTestContext::new(&server);
    let stale_auth = managed_auth_dot_json(
        "stale-access",
        "stale-refresh",
        None,
        "user-123",
        "user@example.com",
        refresh_time_days_ago(31),
    );
    ctx.write_auth(&stale_auth);

    let newer_auth = managed_auth_dot_json(
        "newer-access",
        "newer-refresh",
        None,
        "user-123",
        "user@example.com",
        Utc::now(),
    );
    save_auth(
        ctx.codex_home.path(),
        &newer_auth,
        AuthCredentialsStoreMode::File,
    )
    .expect("persist newer auth");

    let refreshed = ctx.auth_manager.auth().await.expect("auth should exist");
    assert_eq!(
        refreshed.get_token_data().expect("token data").access_token,
        "newer-access"
    );

    server.verify().await;
}

#[tokio::test]
#[serial(codex_api_key)]
async fn stale_proactive_refresh_does_not_overwrite_a_different_account_on_disk() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;

    let ctx = RefreshTokenTestContext::new(&server);
    let stale_auth = managed_auth_dot_json(
        "stale-access",
        "stale-refresh",
        Some("account-a"),
        "user-a",
        "user-a@example.com",
        refresh_time_days_ago(31),
    );
    ctx.write_auth(&stale_auth);

    let other_auth = managed_auth_dot_json(
        "other-access",
        "other-refresh",
        Some("account-b"),
        "user-b",
        "user-b@example.com",
        Utc::now(),
    );
    save_auth(
        ctx.codex_home.path(),
        &other_auth,
        AuthCredentialsStoreMode::File,
    )
    .expect("persist other-account auth");

    let returned_auth = ctx.auth_manager.auth().await.expect("auth should exist");
    assert_eq!(
        returned_auth
            .get_token_data()
            .expect("token data")
            .access_token,
        "stale-access"
    );
    assert_eq!(
        ctx.auth_manager
            .auth_cached()
            .expect("cached auth")
            .get_token_data()
            .expect("token data")
            .access_token,
        "stale-access"
    );
    assert_eq!(
        CodexAuth::from_auth_storage(ctx.codex_home.path(), AuthCredentialsStoreMode::File)
            .expect("load auth from disk")
            .expect("persisted auth")
            .get_token_data()
            .expect("token data")
            .access_token,
        "other-access"
    );

    server.verify().await;
}

#[tokio::test]
#[serial(codex_api_key)]
async fn stale_proactive_refresh_does_not_adopt_a_different_user_in_same_account() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;

    let ctx = RefreshTokenTestContext::new(&server);
    let stale_auth = managed_auth_dot_json(
        "stale-access",
        "stale-refresh",
        Some("account-a"),
        "user-a",
        "user-a@example.com",
        refresh_time_days_ago(31),
    );
    ctx.write_auth(&stale_auth);

    let other_user_auth = managed_auth_dot_json(
        "other-access",
        "other-refresh",
        Some("account-a"),
        "user-b",
        "user-b@example.com",
        Utc::now(),
    );
    save_auth(
        ctx.codex_home.path(),
        &other_user_auth,
        AuthCredentialsStoreMode::File,
    )
    .expect("persist different-user auth in same account");

    let returned_auth = ctx.auth_manager.auth().await.expect("auth should exist");
    assert_eq!(
        returned_auth
            .get_token_data()
            .expect("token data")
            .access_token,
        "stale-access"
    );
    assert_eq!(
        ctx.auth_manager
            .auth_cached()
            .expect("cached auth")
            .get_token_data()
            .expect("token data")
            .access_token,
        "stale-access"
    );

    server.verify().await;
}

#[tokio::test]
#[serial(codex_api_key)]
async fn stale_proactive_refresh_without_account_id_adopts_auth_with_account_id() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;

    let ctx = RefreshTokenTestContext::new(&server);
    let stale_auth = managed_auth_dot_json(
        "stale-access",
        "stale-refresh",
        None,
        "user-a",
        "user-a@example.com",
        refresh_time_days_ago(31),
    );
    ctx.write_auth(&stale_auth);

    let newer_auth = managed_auth_dot_json(
        "newer-access",
        "newer-refresh",
        Some("account-id"),
        "user-a",
        "user-a@example.com",
        Utc::now(),
    );
    save_auth(
        ctx.codex_home.path(),
        &newer_auth,
        AuthCredentialsStoreMode::File,
    )
    .expect("persist same-user auth with account id");

    let returned_auth = ctx.auth_manager.auth().await.expect("auth should exist");
    assert_eq!(
        returned_auth
            .get_token_data()
            .expect("token data")
            .access_token,
        "newer-access"
    );
    assert_eq!(
        ctx.auth_manager
            .auth_cached()
            .expect("cached auth")
            .get_token_data()
            .expect("token data")
            .access_token,
        "newer-access"
    );

    server.verify().await;
}

#[tokio::test]
#[serial(codex_api_key)]
async fn stale_proactive_refresh_without_identity_claims_still_attempts_refresh() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "refreshed-access",
            "refresh_token": "refreshed-refresh",
        })))
        .expect(1)
        .mount(&server)
        .await;

    let ctx = RefreshTokenTestContext::new(&server);
    let stale_auth = managed_auth_dot_json_without_user_id(
        "stale-access",
        "stale-refresh",
        "user-a@example.com",
        refresh_time_days_ago(31),
    );
    ctx.write_auth(&stale_auth);

    let returned_auth = ctx.auth_manager.auth().await.expect("auth should exist");
    assert_eq!(
        returned_auth
            .get_token_data()
            .expect("token data")
            .access_token,
        "refreshed-access"
    );

    server.verify().await;
}

#[tokio::test]
#[serial(codex_api_key)]
async fn refresh_token_reused_without_account_id_adopts_auth_when_disk_auth_gains_account_id() {
    let server = MockServer::start().await;
    let ctx = Arc::new(RefreshTokenTestContext::new(&server));
    let stale_auth = managed_auth_dot_json(
        "stale-access",
        "stale-refresh",
        None,
        "user-a",
        "user-a@example.com",
        refresh_time_days_ago(31),
    );
    ctx.write_auth(&stale_auth);

    let newer_auth = managed_auth_dot_json(
        "newer-access",
        "newer-refresh",
        Some("account-id"),
        "user-a",
        "user-a@example.com",
        Utc::now(),
    );
    let ctx_for_response = Arc::clone(&ctx);
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(move |_: &Request| {
            save_auth(
                ctx_for_response.codex_home.path(),
                &newer_auth,
                AuthCredentialsStoreMode::File,
            )
            .expect("persist same-user auth during reused-token response");
            ResponseTemplate::new(401)
                .set_body_json(json!({"error": {"code": "refresh_token_reused"}}))
        })
        .expect(1)
        .mount(&server)
        .await;

    ctx.auth_manager
        .refresh_token_from_authority()
        .await
        .expect("same-user auth with a new account id should be adopted");
    assert_eq!(
        ctx.auth_manager
            .auth_cached()
            .expect("cached auth")
            .get_token_data()
            .expect("token data")
            .access_token,
        "newer-access"
    );

    server.verify().await;
}

#[tokio::test]
#[serial(codex_api_key)]
async fn unauthorized_recovery_reload_uses_original_auth_snapshot_for_same_account_user_guard() {
    let server = MockServer::start().await;
    let ctx = RefreshTokenTestContext::new(&server);
    let original_auth = managed_auth_dot_json(
        "original-access",
        "original-refresh",
        Some("account-id"),
        "user-a",
        "user-a@example.com",
        refresh_time_days_ago(31),
    );
    ctx.write_auth(&original_auth);

    let mut recovery = ctx.auth_manager.unauthorized_recovery();

    let different_user_auth = managed_auth_dot_json(
        "other-access",
        "other-refresh",
        Some("account-id"),
        "user-b",
        "user-b@example.com",
        Utc::now(),
    );
    save_auth(
        ctx.codex_home.path(),
        &different_user_auth,
        AuthCredentialsStoreMode::File,
    )
    .expect("persist different-user auth");
    let different_user_cached_auth =
        super::load_auth(ctx.codex_home.path(), false, AuthCredentialsStoreMode::File)
            .expect("load auth should succeed")
            .expect("auth should exist");
    ctx.auth_manager
        .set_cached_auth(Some(different_user_cached_auth));

    let err = recovery
        .next()
        .await
        .expect_err("recovery should reject a different user in the same account");
    assert_eq!(
        err.to_string(),
        REFRESH_TOKEN_ACCOUNT_MISMATCH_MESSAGE.to_string()
    );
}

#[tokio::test]
#[serial(codex_api_key)]
async fn refresh_token_reused_recovers_if_guarded_reload_finds_newer_auth() {
    let server = MockServer::start().await;
    let ctx = Arc::new(RefreshTokenTestContext::new(&server));
    let stale_auth = managed_auth_dot_json(
        "stale-access",
        "stale-refresh",
        Some("account-id"),
        "user-123",
        "user@example.com",
        refresh_time_days_ago(31),
    );
    ctx.write_auth(&stale_auth);

    let newer_auth = managed_auth_dot_json(
        "newer-access",
        "newer-refresh",
        Some("account-id"),
        "user-123",
        "user@example.com",
        Utc::now(),
    );
    let ctx_for_response = Arc::clone(&ctx);
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(move |_: &Request| {
            save_auth(
                ctx_for_response.codex_home.path(),
                &newer_auth,
                AuthCredentialsStoreMode::File,
            )
            .expect("persist newer auth during reused-token response");
            ResponseTemplate::new(401)
                .set_body_json(json!({"error": {"code": "refresh_token_reused"}}))
        })
        .expect(1)
        .mount(&server)
        .await;

    ctx.auth_manager
        .refresh_token()
        .await
        .expect("reload after reused token should recover");

    let refreshed = ctx.auth_manager.auth_cached().expect("cached auth");
    assert_eq!(
        refreshed.get_token_data().expect("token data").access_token,
        "newer-access"
    );

    server.verify().await;
}

#[tokio::test]
#[serial(codex_api_key)]
async fn refresh_token_reused_with_unchanged_local_auth_keeps_relogin_failure() {
    let server = MockServer::start().await;
    let ctx = RefreshTokenTestContext::new(&server);
    let stale_auth = managed_auth_dot_json(
        "stale-access",
        "stale-refresh",
        Some("account-id"),
        "user-123",
        "user@example.com",
        refresh_time_days_ago(31),
    );
    ctx.write_auth(&stale_auth);

    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(
            ResponseTemplate::new(401)
                .set_body_json(json!({"error": {"code": "refresh_token_reused"}})),
        )
        .expect(1)
        .mount(&server)
        .await;

    let err = ctx
        .auth_manager
        .refresh_token()
        .await
        .expect_err("unchanged local auth should still surface relogin");
    assert_eq!(
        err.failed_reason(),
        Some(RefreshTokenFailedReason::Exhausted)
    );

    server.verify().await;
}

struct AuthFileParams {
    openai_api_key: Option<String>,
    chatgpt_plan_type: Option<String>,
    chatgpt_account_id: Option<String>,
}

fn write_auth_file(params: AuthFileParams, codex_home: &Path) -> std::io::Result<String> {
    let auth_file = get_auth_file(codex_home);
    // Create a minimal valid JWT for the id_token field.
    #[derive(Serialize)]
    struct Header {
        alg: &'static str,
        typ: &'static str,
    }
    let header = Header {
        alg: "none",
        typ: "JWT",
    };
    let mut auth_payload = serde_json::json!({
        "chatgpt_user_id": "user-12345",
        "user_id": "user-12345",
    });

    if let Some(chatgpt_plan_type) = params.chatgpt_plan_type {
        auth_payload["chatgpt_plan_type"] = serde_json::Value::String(chatgpt_plan_type);
    }

    if let Some(chatgpt_account_id) = params.chatgpt_account_id {
        let org_value = serde_json::Value::String(chatgpt_account_id);
        auth_payload["chatgpt_account_id"] = org_value;
    }

    let payload = serde_json::json!({
        "email": "user@example.com",
        "email_verified": true,
        "https://api.openai.com/auth": auth_payload,
    });
    let b64 = |b: &[u8]| base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b);
    let header_b64 = b64(&serde_json::to_vec(&header)?);
    let payload_b64 = b64(&serde_json::to_vec(&payload)?);
    let signature_b64 = b64(b"sig");
    let fake_jwt = format!("{header_b64}.{payload_b64}.{signature_b64}");

    let auth_json_data = json!({
        "OPENAI_API_KEY": params.openai_api_key,
        "tokens": {
            "id_token": fake_jwt,
            "access_token": "test-access-token",
            "refresh_token": "test-refresh-token"
        },
        "last_refresh": Utc::now(),
    });
    let auth_json = serde_json::to_string_pretty(&auth_json_data)?;
    std::fs::write(auth_file, auth_json)?;
    Ok(fake_jwt)
}

struct RefreshTokenTestContext {
    codex_home: tempfile::TempDir,
    auth_manager: Arc<AuthManager>,
    _env_guard: EnvVarGuard,
}

impl RefreshTokenTestContext {
    fn new(server: &MockServer) -> Self {
        let codex_home = tempdir().expect("tempdir");
        let endpoint = format!("{}/oauth/token", server.uri());
        let env_guard = EnvVarGuard::set(REFRESH_TOKEN_URL_OVERRIDE_ENV_VAR, &endpoint);
        let auth_manager = AuthManager::shared(
            codex_home.path().to_path_buf(),
            false,
            AuthCredentialsStoreMode::File,
        );
        Self {
            codex_home,
            auth_manager,
            _env_guard: env_guard,
        }
    }

    fn write_auth(&self, auth_dot_json: &AuthDotJson) {
        save_auth(
            self.codex_home.path(),
            auth_dot_json,
            AuthCredentialsStoreMode::File,
        )
        .expect("write auth");
        self.auth_manager.reload();
    }
}

fn managed_auth_dot_json(
    access_token: &str,
    refresh_token: &str,
    account_id: Option<&str>,
    chatgpt_user_id: &str,
    email: &str,
    last_refresh: DateTime<Utc>,
) -> AuthDotJson {
    AuthDotJson {
        auth_mode: Some(ApiAuthMode::Chatgpt),
        openai_api_key: None,
        tokens: Some(TokenData {
            id_token: IdTokenInfo {
                email: Some(email.to_string()),
                chatgpt_user_id: Some(chatgpt_user_id.to_string()),
                raw_jwt: minimal_jwt(chatgpt_user_id, email),
                ..IdTokenInfo::default()
            },
            access_token: access_token.to_string(),
            refresh_token: refresh_token.to_string(),
            account_id: account_id.map(str::to_owned),
        }),
        last_refresh: Some(last_refresh),
    }
}

fn managed_auth_dot_json_without_user_id(
    access_token: &str,
    refresh_token: &str,
    email: &str,
    last_refresh: DateTime<Utc>,
) -> AuthDotJson {
    AuthDotJson {
        auth_mode: Some(ApiAuthMode::Chatgpt),
        openai_api_key: None,
        tokens: Some(TokenData {
            id_token: IdTokenInfo {
                email: Some(email.to_string()),
                raw_jwt: minimal_jwt_without_user_id(email),
                ..IdTokenInfo::default()
            },
            access_token: access_token.to_string(),
            refresh_token: refresh_token.to_string(),
            account_id: None,
        }),
        last_refresh: Some(last_refresh),
    }
}

fn refresh_time_days_ago(days: i64) -> DateTime<Utc> {
    Utc::now() - Duration::days(days)
}

fn minimal_jwt(chatgpt_user_id: &str, email: &str) -> String {
    #[derive(Serialize)]
    struct Header {
        alg: &'static str,
        typ: &'static str,
    }

    let header = Header {
        alg: "none",
        typ: "JWT",
    };
    let payload = json!({
        "sub": chatgpt_user_id,
        "email": email,
        "https://api.openai.com/auth": {
            "chatgpt_user_id": chatgpt_user_id,
        }
    });

    let b64 = |bytes: &[u8]| base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes);
    let header_b64 = b64(&serde_json::to_vec(&header).expect("serialize header"));
    let payload_b64 = b64(&serde_json::to_vec(&payload).expect("serialize payload"));
    let signature_b64 = b64(b"sig");
    format!("{header_b64}.{payload_b64}.{signature_b64}")
}

fn minimal_jwt_without_user_id(email: &str) -> String {
    #[derive(Serialize)]
    struct Header {
        alg: &'static str,
        typ: &'static str,
    }

    let header = Header {
        alg: "none",
        typ: "JWT",
    };
    let payload = json!({
        "email": email,
        "https://api.openai.com/auth": {}
    });

    let b64 = |bytes: &[u8]| base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes);
    let header_b64 = b64(&serde_json::to_vec(&header).expect("serialize header"));
    let payload_b64 = b64(&serde_json::to_vec(&payload).expect("serialize payload"));
    let signature_b64 = b64(b"sig");
    format!("{header_b64}.{payload_b64}.{signature_b64}")
}

async fn build_config(
    codex_home: &Path,
    forced_login_method: Option<ForcedLoginMethod>,
    forced_chatgpt_workspace_id: Option<String>,
) -> AuthConfig {
    AuthConfig {
        codex_home: codex_home.to_path_buf(),
        auth_credentials_store_mode: AuthCredentialsStoreMode::File,
        forced_login_method,
        forced_chatgpt_workspace_id,
    }
}

/// Use sparingly.
/// TODO (gpeal): replace this with an injectable env var provider.
#[cfg(test)]
struct EnvVarGuard {
    key: &'static str,
    original: Option<std::ffi::OsString>,
}

#[cfg(test)]
impl EnvVarGuard {
    fn set(key: &'static str, value: &str) -> Self {
        let original = env::var_os(key);
        unsafe {
            env::set_var(key, value);
        }
        Self { key, original }
    }
}

#[cfg(test)]
impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        unsafe {
            match &self.original {
                Some(value) => env::set_var(self.key, value),
                None => env::remove_var(self.key),
            }
        }
    }
}

#[tokio::test]
async fn enforce_login_restrictions_logs_out_for_method_mismatch() {
    let codex_home = tempdir().unwrap();
    login_with_api_key(codex_home.path(), "sk-test", AuthCredentialsStoreMode::File)
        .expect("seed api key");

    let config = build_config(codex_home.path(), Some(ForcedLoginMethod::Chatgpt), None).await;

    let err =
        super::enforce_login_restrictions(&config).expect_err("expected method mismatch to error");
    assert!(err.to_string().contains("ChatGPT login is required"));
    assert!(
        !codex_home.path().join("auth.json").exists(),
        "auth.json should be removed on mismatch"
    );
}

#[tokio::test]
#[serial(codex_api_key)]
async fn enforce_login_restrictions_logs_out_for_workspace_mismatch() {
    let codex_home = tempdir().unwrap();
    let _jwt = write_auth_file(
        AuthFileParams {
            openai_api_key: None,
            chatgpt_plan_type: Some("pro".to_string()),
            chatgpt_account_id: Some("org_another_org".to_string()),
        },
        codex_home.path(),
    )
    .expect("failed to write auth file");

    let config = build_config(codex_home.path(), None, Some("org_mine".to_string())).await;

    let err = super::enforce_login_restrictions(&config)
        .expect_err("expected workspace mismatch to error");
    assert!(err.to_string().contains("workspace org_mine"));
    assert!(
        !codex_home.path().join("auth.json").exists(),
        "auth.json should be removed on mismatch"
    );
}

#[tokio::test]
#[serial(codex_api_key)]
async fn enforce_login_restrictions_allows_matching_workspace() {
    let codex_home = tempdir().unwrap();
    let _jwt = write_auth_file(
        AuthFileParams {
            openai_api_key: None,
            chatgpt_plan_type: Some("pro".to_string()),
            chatgpt_account_id: Some("org_mine".to_string()),
        },
        codex_home.path(),
    )
    .expect("failed to write auth file");

    let config = build_config(codex_home.path(), None, Some("org_mine".to_string())).await;

    super::enforce_login_restrictions(&config).expect("matching workspace should succeed");
    assert!(
        codex_home.path().join("auth.json").exists(),
        "auth.json should remain when restrictions pass"
    );
}

#[tokio::test]
async fn enforce_login_restrictions_allows_api_key_if_login_method_not_set_but_forced_chatgpt_workspace_id_is_set()
 {
    let codex_home = tempdir().unwrap();
    login_with_api_key(codex_home.path(), "sk-test", AuthCredentialsStoreMode::File)
        .expect("seed api key");

    let config = build_config(codex_home.path(), None, Some("org_mine".to_string())).await;

    super::enforce_login_restrictions(&config).expect("matching workspace should succeed");
    assert!(
        codex_home.path().join("auth.json").exists(),
        "auth.json should remain when restrictions pass"
    );
}

#[tokio::test]
#[serial(codex_api_key)]
async fn enforce_login_restrictions_blocks_env_api_key_when_chatgpt_required() {
    let _guard = EnvVarGuard::set(CODEX_API_KEY_ENV_VAR, "sk-env");
    let codex_home = tempdir().unwrap();

    let config = build_config(codex_home.path(), Some(ForcedLoginMethod::Chatgpt), None).await;

    let err = super::enforce_login_restrictions(&config)
        .expect_err("environment API key should not satisfy forced ChatGPT login");
    assert!(
        err.to_string()
            .contains("ChatGPT login is required, but an API key is currently being used.")
    );
}

#[test]
fn plan_type_maps_known_plan() {
    let codex_home = tempdir().unwrap();
    let _jwt = write_auth_file(
        AuthFileParams {
            openai_api_key: None,
            chatgpt_plan_type: Some("pro".to_string()),
            chatgpt_account_id: None,
        },
        codex_home.path(),
    )
    .expect("failed to write auth file");

    let auth = super::load_auth(codex_home.path(), false, AuthCredentialsStoreMode::File)
        .expect("load auth")
        .expect("auth available");

    pretty_assertions::assert_eq!(auth.account_plan_type(), Some(AccountPlanType::Pro));
}

#[test]
fn plan_type_maps_unknown_to_unknown() {
    let codex_home = tempdir().unwrap();
    let _jwt = write_auth_file(
        AuthFileParams {
            openai_api_key: None,
            chatgpt_plan_type: Some("mystery-tier".to_string()),
            chatgpt_account_id: None,
        },
        codex_home.path(),
    )
    .expect("failed to write auth file");

    let auth = super::load_auth(codex_home.path(), false, AuthCredentialsStoreMode::File)
        .expect("load auth")
        .expect("auth available");

    pretty_assertions::assert_eq!(auth.account_plan_type(), Some(AccountPlanType::Unknown));
}

#[test]
fn missing_plan_type_maps_to_unknown() {
    let codex_home = tempdir().unwrap();
    let _jwt = write_auth_file(
        AuthFileParams {
            openai_api_key: None,
            chatgpt_plan_type: None,
            chatgpt_account_id: None,
        },
        codex_home.path(),
    )
    .expect("failed to write auth file");

    let auth = super::load_auth(codex_home.path(), false, AuthCredentialsStoreMode::File)
        .expect("load auth")
        .expect("auth available");

    pretty_assertions::assert_eq!(auth.account_plan_type(), Some(AccountPlanType::Unknown));
}
