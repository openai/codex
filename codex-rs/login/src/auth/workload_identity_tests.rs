use std::collections::HashMap;
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use codex_config::config_toml::WorkloadIdentityToml;
use codex_config::types::AuthCredentialsStoreMode;
use codex_protocol::auth::AuthMode;
use pretty_assertions::assert_eq;
use serial_test::serial;
use tempfile::TempDir;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

use super::*;
use crate::auth::AuthKeyringBackendKind;
use crate::auth::AuthManager;
use crate::auth::AuthManagerConfig;
use crate::auth::CODEX_ACCESS_TOKEN_ENV_VAR;
use crate::auth::CODEX_API_KEY_ENV_VAR;
use crate::auth::login_with_api_key;
use crate::outbound_proxy::AuthRouteConfig;

const TOKEN_URL_OVERRIDE_ENV_VAR: &str = "CODEX_WIF_TOKEN_URL_OVERRIDE";
const WORKSPACE_ID: &str = "workspace-one";

struct TestConfig {
    codex_home: PathBuf,
    workload_identity: Option<WorkloadIdentityToml>,
}

impl AuthManagerConfig for TestConfig {
    fn codex_home(&self) -> PathBuf {
        self.codex_home.clone()
    }

    fn cli_auth_credentials_store_mode(&self) -> AuthCredentialsStoreMode {
        AuthCredentialsStoreMode::File
    }

    fn auth_keyring_backend_kind(&self) -> AuthKeyringBackendKind {
        AuthKeyringBackendKind::default()
    }

    fn forced_chatgpt_workspace_id(&self) -> Option<Vec<String>> {
        Some(vec![WORKSPACE_ID.to_string()])
    }

    fn chatgpt_base_url(&self) -> String {
        "https://chatgpt.com/backend-api/".to_string()
    }

    fn workload_identity_config(&self) -> Option<WorkloadIdentityToml> {
        self.workload_identity.clone()
    }

    fn auth_route_config(&self) -> Option<AuthRouteConfig> {
        None
    }
}

struct EnvVarGuard {
    name: &'static str,
    previous: Option<OsString>,
}

impl EnvVarGuard {
    fn set(name: &'static str, value: &str) -> Self {
        let guard = Self::remove(name);
        // SAFETY: these tests are serialized with every auth test that mutates
        // the process environment.
        unsafe { std::env::set_var(name, value) };
        guard
    }

    fn remove(name: &'static str) -> Self {
        let previous = std::env::var_os(name);
        // SAFETY: these tests are serialized with every auth test that mutates
        // the process environment.
        unsafe { std::env::remove_var(name) };
        Self { name, previous }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        // SAFETY: these tests are serialized with every auth test that mutates
        // the process environment.
        unsafe {
            match &self.previous {
                Some(value) => std::env::set_var(self.name, value),
                None => std::env::remove_var(self.name),
            }
        }
    }
}

fn test_config(codex_home: &TempDir, token_file: Option<PathBuf>) -> TestConfig {
    TestConfig {
        codex_home: codex_home.path().to_path_buf(),
        workload_identity: Some(WorkloadIdentityToml {
            federation_rule_id: Some("rule-one".to_string()),
            principal_id: Some("user-one".to_string()),
            tenant_id: Some("tenant-one".to_string()),
            workspace_id: Some(WORKSPACE_ID.to_string()),
            identity_token_file: token_file.map(|path| {
                codex_config::AbsolutePathBuf::from_absolute_path(path)
                    .expect("absolute token file")
            }),
        }),
    }
}

fn fake_access_token(marker: &str) -> String {
    let payload = serde_json::json!({
        "marker": marker,
        "https://api.openai.com/auth": {
            "chatgpt_user_id": "user-one"
        }
    });
    let payload = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&payload).expect("serialize claims"));
    format!("e30.{payload}.c2ln")
}

fn success(access_token: &str) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(serde_json::json!({
        "access_token": access_token,
        "token_type": "Bearer",
        "expires_in": 600,
        "chatgpt_account_id": WORKSPACE_ID,
        "chatgpt_plan_type": "enterprise",
        "user_id": "user-one"
    }))
}

fn remove_explicit_auth() -> [EnvVarGuard; 2] {
    [
        EnvVarGuard::remove(CODEX_API_KEY_ENV_VAR),
        EnvVarGuard::remove(CODEX_ACCESS_TOKEN_ENV_VAR),
    ]
}

#[tokio::test]
#[serial(codex_auth_env)]
async fn manager_uses_wif_over_persisted_auth_and_rereads_the_token_file() {
    let codex_home = TempDir::new().expect("tempdir");
    login_with_api_key(
        codex_home.path(),
        "sk-persisted",
        AuthCredentialsStoreMode::File,
        AuthKeyringBackendKind::default(),
    )
    .expect("seed persisted auth");
    let auth_path = codex_home.path().join("auth.json");
    let persisted_before = std::fs::read(&auth_path).expect("read persisted auth");
    let token_file = codex_home.path().join("identity-token");
    tokio::fs::write(&token_file, "assertion-one\n")
        .await
        .expect("write assertion");

    let server = MockServer::start().await;
    let calls = Arc::new(AtomicUsize::new(0));
    let access_tokens = [fake_access_token("one"), fake_access_token("two")];
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with({
            let calls = Arc::clone(&calls);
            move |_request: &wiremock::Request| {
                let index = calls.fetch_add(1, Ordering::SeqCst).min(1);
                success(&access_tokens[index])
            }
        })
        .mount(&server)
        .await;
    let _explicit_auth = remove_explicit_auth();
    let _endpoint = EnvVarGuard::set(
        TOKEN_URL_OVERRIDE_ENV_VAR,
        &format!("{}/oauth/token", server.uri()),
    );
    let _conflicting_environment = [
        EnvVarGuard::set(FEDERATION_RULE_ID_ENV_VAR, "wrong-rule"),
        EnvVarGuard::set(PRINCIPAL_ID_ENV_VAR, "wrong-principal"),
        EnvVarGuard::set(TENANT_ID_ENV_VAR, "wrong-tenant"),
        EnvVarGuard::set(WORKSPACE_ID_ENV_VAR, "wrong-workspace"),
        EnvVarGuard::set(IDENTITY_TOKEN_ENV_VAR, "wrong-assertion"),
    ];
    let config = test_config(&codex_home, Some(token_file.clone()));

    let manager = AuthManager::shared_from_config(&config, /*enable_codex_api_key_env*/ true).await;
    let initial = manager.auth_cached().expect("workload auth");
    assert_eq!(initial.api_auth_mode(), AuthMode::ChatgptAuthTokens);
    assert_eq!(
        initial.get_token().expect("access token"),
        fake_access_token("one")
    );
    assert_eq!(initial.get_account_id().as_deref(), Some(WORKSPACE_ID));
    assert_eq!(
        std::fs::read(&auth_path).expect("read persisted auth"),
        persisted_before
    );

    tokio::fs::write(&token_file, "assertion-two\n")
        .await
        .expect("rotate assertion");
    manager
        .refresh_token_from_authority()
        .await
        .expect("refresh workload auth");
    assert_eq!(
        manager
            .auth_cached()
            .expect("refreshed auth")
            .get_token()
            .expect("access token"),
        fake_access_token("two")
    );

    let requests = server.received_requests().await.expect("received requests");
    let forms = requests
        .iter()
        .map(|request| {
            url::form_urlencoded::parse(&request.body)
                .into_owned()
                .collect::<HashMap<_, _>>()
        })
        .collect::<Vec<_>>();
    assert_eq!(forms[0]["assertion"], "assertion-one");
    assert_eq!(forms[1]["assertion"], "assertion-two");
    assert_eq!(forms[0]["federation_rule_id"], "rule-one");
    assert_eq!(forms[0]["principal_id"], "user-one");
    assert_eq!(forms[0]["tenant_id"], "tenant-one");
    assert_eq!(forms[0]["workspace_id"], WORKSPACE_ID);

    let second_manager =
        AuthManager::shared_from_config(&config, /*enable_codex_api_key_env*/ true).await;
    assert!(second_manager.has_external_auth());
    assert_eq!(calls.load(Ordering::SeqCst), 3);
}

#[tokio::test]
#[serial(codex_auth_env)]
async fn manager_accepts_environment_only_configuration() {
    let codex_home = TempDir::new().expect("tempdir");
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(success(&fake_access_token("environment")))
        .expect(1)
        .mount(&server)
        .await;

    let _explicit_auth = remove_explicit_auth();
    let _endpoint = EnvVarGuard::set(
        TOKEN_URL_OVERRIDE_ENV_VAR,
        &format!("{}/oauth/token", server.uri()),
    );
    let _workload_environment = [
        EnvVarGuard::set(FEDERATION_RULE_ID_ENV_VAR, "rule-environment"),
        EnvVarGuard::set(PRINCIPAL_ID_ENV_VAR, "user-one"),
        EnvVarGuard::set(TENANT_ID_ENV_VAR, "tenant-environment"),
        EnvVarGuard::set(WORKSPACE_ID_ENV_VAR, WORKSPACE_ID),
        EnvVarGuard::set(IDENTITY_TOKEN_ENV_VAR, "assertion-environment"),
        EnvVarGuard::remove(IDENTITY_TOKEN_FILE_ENV_VAR),
    ];
    let config = TestConfig {
        codex_home: codex_home.path().to_path_buf(),
        workload_identity: None,
    };

    let manager = AuthManager::shared_from_config(&config, /*enable_codex_api_key_env*/ true).await;

    assert_eq!(
        manager
            .auth_cached()
            .expect("workload auth")
            .get_token()
            .expect("access token"),
        fake_access_token("environment")
    );
    let request = server
        .received_requests()
        .await
        .expect("received requests")
        .pop()
        .expect("token exchange");
    let form = url::form_urlencoded::parse(&request.body)
        .into_owned()
        .collect::<HashMap<_, _>>();
    assert_eq!(form["assertion"], "assertion-environment");
    assert_eq!(form["federation_rule_id"], "rule-environment");
    assert_eq!(form["principal_id"], "user-one");
    assert_eq!(form["tenant_id"], "tenant-environment");
    assert_eq!(form["workspace_id"], WORKSPACE_ID);
}

#[tokio::test]
#[serial(codex_auth_env)]
async fn explicit_api_key_precedes_workload_identity() {
    let codex_home = TempDir::new().expect("tempdir");
    let token_file = codex_home.path().join("identity-token");
    std::fs::write(&token_file, "assertion-one").expect("write assertion");
    let _access_token = EnvVarGuard::remove(CODEX_ACCESS_TOKEN_ENV_VAR);
    let _api_key = EnvVarGuard::set(CODEX_API_KEY_ENV_VAR, "sk-explicit");
    let config = test_config(&codex_home, Some(token_file));

    let manager = AuthManager::shared_from_config(&config, /*enable_codex_api_key_env*/ true).await;

    assert_eq!(
        manager
            .auth_cached()
            .and_then(|auth| auth.api_key().map(str::to_string)),
        Some("sk-explicit".to_string())
    );
    assert!(!manager.has_external_auth());
}

#[tokio::test]
#[serial(codex_auth_env)]
async fn invalid_workload_identity_fails_closed_instead_of_using_persisted_auth() {
    let codex_home = TempDir::new().expect("tempdir");
    login_with_api_key(
        codex_home.path(),
        "sk-persisted",
        AuthCredentialsStoreMode::File,
        AuthKeyringBackendKind::default(),
    )
    .expect("seed persisted auth");
    let _explicit_auth = remove_explicit_auth();
    let _identity_token = EnvVarGuard::remove(IDENTITY_TOKEN_ENV_VAR);
    let _identity_token_file = EnvVarGuard::remove(IDENTITY_TOKEN_FILE_ENV_VAR);
    let config = test_config(&codex_home, /*token_file*/ None);

    let manager = AuthManager::shared_from_config(&config, /*enable_codex_api_key_env*/ true).await;

    assert!(manager.has_external_auth());
    assert!(manager.auth_cached().is_none());
    assert!(manager.auth().await.is_none());
}
