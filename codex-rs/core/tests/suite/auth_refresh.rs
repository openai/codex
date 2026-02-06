use anyhow::Context;
use anyhow::Result;
use base64::Engine;
use chrono::Duration;
use chrono::Utc;
use codex_core::CodexAuth;
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
    let auth = ctx.auth.clone();

    let access = auth
        .refresh_token()
        .await
        .context("refresh should succeed")?;
    assert_eq!(access, "new-access-token");

    let stored = ctx.load_auth()?;
    let tokens = stored.tokens.as_ref().context("tokens should exist")?;
    assert_eq!(tokens.access_token, "new-access-token");
    assert_eq!(tokens.refresh_token, "new-refresh-token");
    let refreshed_at = stored
        .last_refresh
        .as_ref()
        .context("last_refresh should be recorded")?;
    assert!(
        *refreshed_at >= ctx.initial_last_refresh,
        "last_refresh should advance"
    );

    let cached = auth
        .get_token_data()
        .await
        .context("token data should be cached")?;
    assert_eq!(cached.access_token, "new-access-token");

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
    let auth = ctx.auth.clone();

    let err = auth
        .refresh_token()
        .await
        .err()
        .context("refresh should fail")?;
    assert_eq!(err.failed_reason(), Some(RefreshTokenFailedReason::Expired));

    let stored = ctx.load_auth()?;
    let tokens = stored.tokens.as_ref().context("tokens should remain")?;
    assert_eq!(tokens.access_token, INITIAL_ACCESS_TOKEN);
    assert_eq!(tokens.refresh_token, INITIAL_REFRESH_TOKEN);
    assert_eq!(
        *stored
            .last_refresh
            .as_ref()
            .context("last_refresh should remain unchanged")?,
        ctx.initial_last_refresh,
    );

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
    let auth = ctx.auth.clone();

    let err = auth
        .refresh_token()
        .await
        .err()
        .context("refresh should fail")?;
    assert!(matches!(err, RefreshTokenError::Transient(_)));
    assert_eq!(err.failed_reason(), None);

    let stored = ctx.load_auth()?;
    let tokens = stored.tokens.as_ref().context("tokens should remain")?;
    assert_eq!(tokens.access_token, INITIAL_ACCESS_TOKEN);
    assert_eq!(tokens.refresh_token, INITIAL_REFRESH_TOKEN);
    assert_eq!(
        *stored
            .last_refresh
            .as_ref()
            .context("last_refresh should remain unchanged")?,
        ctx.initial_last_refresh,
    );

    server.verify().await;
    Ok(())
}

struct RefreshTokenTestContext {
    codex_home: TempDir,
    auth: CodexAuth,
    initial_last_refresh: chrono::DateTime<Utc>,
    _env_guard: EnvGuard,
}

impl RefreshTokenTestContext {
    fn new(server: &MockServer) -> Result<Self> {
        let codex_home = TempDir::new()?;
        let initial_last_refresh = Utc::now() - Duration::days(1);
        let mut id_token = IdTokenInfo::default();
        id_token.raw_jwt = minimal_jwt();
        let tokens = TokenData {
            id_token,
            access_token: INITIAL_ACCESS_TOKEN.to_string(),
            refresh_token: INITIAL_REFRESH_TOKEN.to_string(),
            account_id: Some("account-id".to_string()),
        };
        let auth_dot_json = AuthDotJson {
            openai_api_key: None,
            tokens: Some(tokens),
            last_refresh: Some(initial_last_refresh),
        };
        save_auth(
            codex_home.path(),
            &auth_dot_json,
            AuthCredentialsStoreMode::File,
        )?;

        let endpoint = format!("{}/oauth/token", server.uri());
        let env_guard = EnvGuard::set(REFRESH_TOKEN_URL_OVERRIDE_ENV_VAR, endpoint);

        let auth = CodexAuth::from_auth_storage(codex_home.path(), AuthCredentialsStoreMode::File)?
            .context("auth should load from storage")?;

        Ok(Self {
            codex_home,
            auth,
            initial_last_refresh,
            _env_guard: env_guard,
        })
    }

    fn load_auth(&self) -> Result<AuthDotJson> {
        load_auth_dot_json(self.codex_home.path(), AuthCredentialsStoreMode::File)
            .context("load auth.json")?
            .context("auth.json should exist")
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
