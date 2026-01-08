use anyhow::Context;
use anyhow::Result;
use base64::Engine;
use chrono::Duration;
use chrono::Utc;
use codex_core::AuthManager;
use codex_core::auth::AuthCredentialsStoreMode;
use codex_core::auth::AuthDotJson;
use codex_core::auth::REFRESH_TOKEN_URL_OVERRIDE_ENV_VAR;
use codex_core::auth::RefreshTokenError;
use codex_core::auth::load_auth_dot_json;
use codex_core::auth::save_auth;
use codex_core::error::RefreshTokenFailedReason;
use codex_core::token_data::IdTokenInfo;
use codex_core::token_data::TokenData;
use core_test_support::skip_if_no_network;
use pretty_assertions::assert_eq;
use serde::Serialize;
use serde_json::json;
use std::ffi::OsString;
use tempfile::TempDir;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

const INITIAL_ACCESS_TOKEN: &str = "initial-access-token";
const INITIAL_REFRESH_TOKEN: &str = "initial-refresh-token";

#[serial_test::serial(auth_refresh)]
#[tokio::test]
async fn refresh_token_succeeds_updates_storage() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "new-access-token",
            "refresh_token": "new-refresh-token"
        })))
        .expect(1)
        .mount(&server)
        .await;

    let ctx = RefreshTokenTestContext::new(&server)?;
    let initial_last_refresh = Utc::now() - Duration::days(1);
    let initial_tokens = build_tokens(INITIAL_ACCESS_TOKEN, INITIAL_REFRESH_TOKEN);
    let initial_auth = AuthDotJson {
        openai_api_key: None,
        tokens: Some(initial_tokens.clone()),
        last_refresh: Some(initial_last_refresh),
    };
    ctx.write_auth(&initial_auth)?;

    let access = ctx
        .auth_manager
        .refresh_token()
        .await
        .context("refresh should succeed")?;
    assert_eq!(access, Some("new-access-token".to_string()));

    let refreshed_tokens = TokenData {
        access_token: "new-access-token".to_string(),
        refresh_token: "new-refresh-token".to_string(),
        ..initial_tokens.clone()
    };
    let stored = ctx.load_auth()?;
    let tokens = stored.tokens.as_ref().context("tokens should exist")?;
    assert_eq!(tokens, &refreshed_tokens);
    let refreshed_at = stored
        .last_refresh
        .as_ref()
        .context("last_refresh should be recorded")?;
    assert!(
        *refreshed_at >= initial_last_refresh,
        "last_refresh should advance"
    );

    let cached_auth = ctx
        .auth_manager
        .auth()
        .await
        .context("auth should be cached")?;
    let cached = cached_auth
        .get_token_data()
        .context("token data should be cached")?;
    assert_eq!(cached, refreshed_tokens);

    server.verify().await;
    Ok(())
}

#[serial_test::serial(auth_refresh)]
#[tokio::test]
async fn returns_fresh_tokens_as_is() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "new-access-token",
            "refresh_token": "new-refresh-token"
        })))
        .mount(&server)
        .await;

    let ctx = RefreshTokenTestContext::new(&server)?;
    let initial_last_refresh = Utc::now() - Duration::days(1);
    let initial_tokens = build_tokens(INITIAL_ACCESS_TOKEN, INITIAL_REFRESH_TOKEN);
    let initial_auth = AuthDotJson {
        openai_api_key: None,
        tokens: Some(initial_tokens.clone()),
        last_refresh: Some(initial_last_refresh),
    };
    ctx.write_auth(&initial_auth)?;

    let cached_auth = ctx
        .auth_manager
        .auth()
        .await
        .context("auth should be cached")?;
    let cached = cached_auth
        .get_token_data()
        .context("token data should remain cached")?;
    assert_eq!(cached, initial_tokens);

    let stored = ctx.load_auth()?;
    assert_eq!(stored, initial_auth);

    let requests = server.received_requests().await.unwrap_or_default();
    assert!(requests.is_empty(), "expected no refresh token requests");

    Ok(())
}

#[serial_test::serial(auth_refresh)]
#[tokio::test]
async fn refreshes_token_when_last_refresh_is_stale() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "new-access-token",
            "refresh_token": "new-refresh-token"
        })))
        .expect(1)
        .mount(&server)
        .await;

    let ctx = RefreshTokenTestContext::new(&server)?;
    let stale_refresh = Utc::now() - Duration::days(9);
    let initial_tokens = build_tokens(INITIAL_ACCESS_TOKEN, INITIAL_REFRESH_TOKEN);
    let initial_auth = AuthDotJson {
        openai_api_key: None,
        tokens: Some(initial_tokens.clone()),
        last_refresh: Some(stale_refresh),
    };
    ctx.write_auth(&initial_auth)?;

    let cached_auth = ctx
        .auth_manager
        .auth()
        .await
        .context("auth should be cached")?;
    let refreshed_tokens = TokenData {
        access_token: "new-access-token".to_string(),
        refresh_token: "new-refresh-token".to_string(),
        ..initial_tokens.clone()
    };
    let cached = cached_auth
        .get_token_data()
        .context("token data should refresh")?;
    assert_eq!(cached, refreshed_tokens);

    let stored = ctx.load_auth()?;
    let tokens = stored.tokens.as_ref().context("tokens should exist")?;
    assert_eq!(tokens, &refreshed_tokens);
    let refreshed_at = stored
        .last_refresh
        .as_ref()
        .context("last_refresh should be recorded")?;
    assert!(
        *refreshed_at >= stale_refresh,
        "last_refresh should advance"
    );

    server.verify().await;
    Ok(())
}

#[serial_test::serial(auth_refresh)]
#[tokio::test]
async fn refresh_token_returns_permanent_error_for_expired_refresh_token() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({
            "error": {
                "code": "refresh_token_expired"
            }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let ctx = RefreshTokenTestContext::new(&server)?;
    let initial_last_refresh = Utc::now() - Duration::days(1);
    let initial_tokens = build_tokens(INITIAL_ACCESS_TOKEN, INITIAL_REFRESH_TOKEN);
    let initial_auth = AuthDotJson {
        openai_api_key: None,
        tokens: Some(initial_tokens.clone()),
        last_refresh: Some(initial_last_refresh),
    };
    ctx.write_auth(&initial_auth)?;

    let err = ctx
        .auth_manager
        .refresh_token()
        .await
        .err()
        .context("refresh should fail")?;
    assert_eq!(err.failed_reason(), Some(RefreshTokenFailedReason::Expired));

    let stored = ctx.load_auth()?;
    assert_eq!(stored, initial_auth);
    let cached_auth = ctx
        .auth_manager
        .auth()
        .await
        .context("auth should remain cached")?;
    let cached = cached_auth
        .get_token_data()
        .context("token data should remain cached")?;
    assert_eq!(cached, initial_tokens);

    server.verify().await;
    Ok(())
}

#[serial_test::serial(auth_refresh)]
#[tokio::test]
async fn refresh_token_returns_transient_error_on_server_failure() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(500).set_body_json(json!({
            "error": "temporary-failure"
        })))
        .expect(1)
        .mount(&server)
        .await;

    let ctx = RefreshTokenTestContext::new(&server)?;
    let initial_last_refresh = Utc::now() - Duration::days(1);
    let initial_tokens = build_tokens(INITIAL_ACCESS_TOKEN, INITIAL_REFRESH_TOKEN);
    let initial_auth = AuthDotJson {
        openai_api_key: None,
        tokens: Some(initial_tokens.clone()),
        last_refresh: Some(initial_last_refresh),
    };
    ctx.write_auth(&initial_auth)?;

    let err = ctx
        .auth_manager
        .refresh_token()
        .await
        .err()
        .context("refresh should fail")?;
    assert!(matches!(err, RefreshTokenError::Transient(_)));
    assert_eq!(err.failed_reason(), None);

    let stored = ctx.load_auth()?;
    assert_eq!(stored, initial_auth);
    let cached_auth = ctx
        .auth_manager
        .auth()
        .await
        .context("auth should remain cached")?;
    let cached = cached_auth
        .get_token_data()
        .context("token data should remain cached")?;
    assert_eq!(cached, initial_tokens);

    server.verify().await;
    Ok(())
}

struct RefreshTokenTestContext {
    codex_home: TempDir,
    auth_manager: AuthManager,
    _env_guard: EnvGuard,
}

impl RefreshTokenTestContext {
    fn new(server: &MockServer) -> Result<Self> {
        let codex_home = TempDir::new()?;

        let endpoint = format!("{}/oauth/token", server.uri());
        let env_guard = EnvGuard::set(REFRESH_TOKEN_URL_OVERRIDE_ENV_VAR, endpoint);

        let auth_manager = AuthManager::new(
            codex_home.path().to_path_buf(),
            false,
            AuthCredentialsStoreMode::File,
        );

        Ok(Self {
            codex_home,
            auth_manager,
            _env_guard: env_guard,
        })
    }

    fn load_auth(&self) -> Result<AuthDotJson> {
        load_auth_dot_json(self.codex_home.path(), AuthCredentialsStoreMode::File)
            .context("load auth.json")?
            .context("auth.json should exist")
    }

    fn write_auth(&self, auth_dot_json: &AuthDotJson) -> Result<()> {
        save_auth(
            self.codex_home.path(),
            auth_dot_json,
            AuthCredentialsStoreMode::File,
        )?;
        self.auth_manager.reload();
        Ok(())
    }
}

struct EnvGuard {
    key: &'static str,
    original: Option<OsString>,
}

impl EnvGuard {
    fn set(key: &'static str, value: String) -> Self {
        let original = std::env::var_os(key);
        // SAFETY: these tests execute serially, so updating the process environment is safe.
        unsafe {
            std::env::set_var(key, &value);
        }
        Self { key, original }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        // SAFETY: the guard restores the original environment value before other tests run.
        unsafe {
            match &self.original {
                Some(value) => std::env::set_var(self.key, value),
                None => std::env::remove_var(self.key),
            }
        }
    }
}

fn minimal_jwt() -> String {
    #[derive(Serialize)]
    struct Header {
        alg: &'static str,
        typ: &'static str,
    }

    let header = Header {
        alg: "none",
        typ: "JWT",
    };
    let payload = json!({ "sub": "user-123" });

    fn b64(data: &[u8]) -> String {
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(data)
    }

    let header_bytes = match serde_json::to_vec(&header) {
        Ok(bytes) => bytes,
        Err(err) => panic!("serialize header: {err}"),
    };
    let payload_bytes = match serde_json::to_vec(&payload) {
        Ok(bytes) => bytes,
        Err(err) => panic!("serialize payload: {err}"),
    };
    let header_b64 = b64(&header_bytes);
    let payload_b64 = b64(&payload_bytes);
    let signature_b64 = b64(b"sig");
    format!("{header_b64}.{payload_b64}.{signature_b64}")
}

fn build_tokens(access_token: &str, refresh_token: &str) -> TokenData {
    let mut id_token = IdTokenInfo::default();
    id_token.raw_jwt = minimal_jwt();
    TokenData {
        id_token,
        access_token: access_token.to_string(),
        refresh_token: refresh_token.to_string(),
        account_id: Some("account-id".to_string()),
    }
}
