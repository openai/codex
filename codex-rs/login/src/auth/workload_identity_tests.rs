use std::path::Path;
use std::sync::atomic::AtomicUsize;
use std::time::Duration;

use base64::Engine as _;
use codex_config::types::AuthCredentialsStoreMode;
use codex_http_client::OutboundProxyPolicy;
use codex_protocol::config_types::ForcedLoginMethod;
use pretty_assertions::assert_eq;
use serial_test::serial;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;

use super::*;
use crate::AuthManager;
use crate::auth::ExternalAuthRefreshReason;

fn auth_route_config(policy: OutboundProxyPolicy) -> AuthRouteConfig {
    AuthRouteConfig::from_http_client_factory(HttpClientFactory::new(policy))
}

fn complete_environment() -> ProcessEnvironment {
    ProcessEnvironment {
        federation_rule_id: Some("rule-one".into()),
        identity_token_file: Some(std::env::temp_dir().join("identity-token").into_os_string()),
        ..ProcessEnvironment::default()
    }
}

fn api_response(access_token: &str) -> serde_json::Value {
    serde_json::json!({
        "access_token": access_token,
        "token_type": "Bearer",
        "issued_token_type": "urn:ietf:params:oauth:token-type:access_token",
        "expires_in": 600,
        "wif_product": "api",
        "api_organization_id": "org-one",
        "api_project_id": "proj-one",
        "user_id": "service-account-one"
    })
}

#[tokio::test]
async fn api_platform_auth_refreshes_with_a_rotated_assertion() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let assertion_file = temp_dir.path().join("identity-token");
    std::fs::write(&assertion_file, "assertion-one").expect("write assertion");
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(|request: &wiremock::Request| {
            let assertion = url::form_urlencoded::parse(&request.body)
                .find(|(name, _)| name == "assertion")
                .expect("assertion")
                .1;
            ResponseTemplate::new(200)
                .set_body_json(api_response(&format!("access-for-{assertion}")))
        })
        .expect(2)
        .mount(&server)
        .await;
    let config = session_config(temp_dir.path(), &server);
    let registry = WorkloadIdentitySessionRegistry::default();
    let adapter = WorkloadIdentityExternalAuth::from_config_with_registry(config, &registry)
        .expect("adapter");
    let manager = AuthManager::from_auth_for_testing_with_home(
        CodexAuth::from_api_key("unused-fallback"),
        temp_dir.path().to_path_buf(),
    );
    manager
        .set_external_auth(Arc::new(adapter))
        .await
        .expect("install WIF");
    assert_eq!(
        manager
            .auth()
            .await
            .expect("initial auth")
            .get_token()
            .expect("token"),
        "access-for-assertion-one"
    );
    let before = *manager.auth_change_state_receiver().borrow();
    std::fs::write(&assertion_file, "assertion-two").expect("rotate assertion");
    let mut recovery = manager.unauthorized_recovery();
    assert!(
        recovery.has_next(),
        "API WIF supports unauthorized recovery"
    );
    recovery.next().await.expect("refresh WIF token");
    assert_eq!(
        manager
            .auth()
            .await
            .expect("renewed auth")
            .get_token()
            .expect("token"),
        "access-for-assertion-two"
    );
    assert_eq!(
        *manager.auth_change_state_receiver().borrow(),
        crate::AuthChangeState {
            generation: before.generation + 1,
            ..before
        }
    );
    assert!(!temp_dir.path().join("auth.json").exists());

    let requests = server.received_requests().await.expect("requests");
    assert_eq!(requests.len(), 2);
    for (request, assertion) in requests.iter().zip(["assertion-one", "assertion-two"]) {
        assert_eq!(request.url.path(), "/oauth/token");
        assert_eq!(
            url::form_urlencoded::parse(&request.body)
                .into_owned()
                .collect::<std::collections::BTreeMap<_, _>>(),
            std::collections::BTreeMap::from([
                (
                    "grant_type".into(),
                    "urn:ietf:params:oauth:grant-type:jwt-bearer".into()
                ),
                ("assertion".into(), assertion.into()),
                ("federation_rule_id".into(), "rule-one".into()),
            ]),
        );
    }
}

fn resolve_for_test(
    environment: ProcessEnvironment,
    chatgpt_base_url: &str,
) -> Result<Option<WorkloadIdentitySessionConfig>, WorkloadIdentitySessionError> {
    resolve_config(
        chatgpt_base_url,
        environment,
        auth_route_config(OutboundProxyPolicy::ReqwestDefault),
    )
}

#[test]
fn markers_select_wif_and_partial_configuration_fails_closed() {
    assert!(
        resolve_for_test(
            ProcessEnvironment::default(),
            "https://chatgpt.com/backend-api",
        )
        .expect("no markers")
        .is_none()
    );
    assert!(
        resolve_for_test(
            ProcessEnvironment {
                workload_identity_context: Some(r#"{"instance_id":"box-one"}"#.into()),
                ..ProcessEnvironment::default()
            },
            "https://chatgpt.com/backend-api",
        )
        .expect("context alone is not a WIF marker")
        .is_none()
    );
    for (environment, missing) in [
        (
            ProcessEnvironment {
                federation_rule_id: None,
                ..complete_environment()
            },
            OPENAI_FEDERATION_RULE_ID_ENV_VAR,
        ),
        (
            ProcessEnvironment {
                identity_token_file: None,
                ..complete_environment()
            },
            OPENAI_IDENTITY_TOKEN_FILE_ENV_VAR,
        ),
    ] {
        let error = resolve_for_test(environment, "https://chatgpt.com/backend-api")
            .expect_err("partial WIF must not fall back");
        assert!(error.to_string().contains(missing), "{error}");
    }

    let relative = ProcessEnvironment {
        identity_token_file: Some("relative.jwt".into()),
        ..complete_environment()
    };
    assert!(
        resolve_for_test(relative, "https://chatgpt.com/backend-api",)
            .expect_err("relative assertion path")
            .to_string()
            .contains("absolute path")
    );
}

#[test]
fn app_environment_is_enforced() {
    for (chatgpt_base_url, expected_environment, expected_token_url) in [
        (
            "https://chatgpt.com/backend-api/",
            WorkloadIdentityEnvironment::Production,
            PROD_TOKEN_URL,
        ),
        (
            "https://chatgpt-staging.com/backend-api",
            WorkloadIdentityEnvironment::Staging,
            STAGING_TOKEN_URL,
        ),
    ] {
        let config = resolve_for_test(complete_environment(), chatgpt_base_url)
            .expect("trusted app routing")
            .expect("WIF selected");
        assert_eq!(config.environment, expected_environment);
        assert_eq!(config.token_url.as_str(), expected_token_url);
    }

    let error = resolve_for_test(
        complete_environment(),
        "https://example.invalid/backend-api",
    )
    .expect_err("untrusted auth environment");
    assert!(error.to_string().contains("app routing"));
}

#[test]
fn workload_context_is_preserved_without_logging_its_value() {
    let context = r#"{"instance_id":"box-one"}"#;
    let config = resolve_for_test(
        ProcessEnvironment {
            workload_identity_context: Some(context.into()),
            ..complete_environment()
        },
        "https://chatgpt.com/backend-api",
    )
    .expect("valid configuration")
    .expect("WIF selected");

    assert!(!format!("{config:?}").contains(context));
}

fn session_config(directory: &Path, server: &MockServer) -> WorkloadIdentitySessionConfig {
    let assertion_file = directory.join("identity-token");
    std::fs::write(&assertion_file, "assertion-one").expect("write assertion");
    WorkloadIdentitySessionConfig {
        assertion_file,
        environment: WorkloadIdentityEnvironment::Staging,
        federation_rule_id: "rule-one".to_string(),
        http_client_factory: HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
        token_url: Url::parse(&format!("{}/oauth/token", server.uri())).expect("token URL"),
        workload_identity_context: None,
    }
}

fn jwt(label: &str, user_id: &str) -> String {
    let encode = |value: &serde_json::Value| {
        base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(serde_json::to_vec(value).expect("serialize JWT part"))
    };
    let header = encode(&serde_json::json!({"alg": "none", "typ": "JWT"}));
    let payload = encode(&serde_json::json!({
        "jti": label,
        "https://api.openai.com/auth": {
            "chatgpt_account_id": "account-one",
            "chatgpt_plan_type": "enterprise",
            "chatgpt_user_id": user_id,
            "user_id": user_id
        }
    }));
    format!("{header}.{payload}.sig")
}

fn success_response(label: &str, account_user_id: &str, user_id: &str) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(serde_json::json!({
        "access_token": jwt(label, user_id),
        "token_type": "Bearer",
        "issued_token_type": "urn:ietf:params:oauth:token-type:access_token",
        "expires_in": 600,
        "scope": "model.request",
        "chatgpt_account_id": "account-one",
        "chatgpt_account_user_id": account_user_id,
        "chatgpt_plan_type": "enterprise",
        "user_id": user_id
    }))
}

#[tokio::test]
async fn compatible_adapters_share_exchange() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(success_response(
            "access-one",
            "account-user-one",
            "user-one",
        ))
        .expect(1)
        .mount(&server)
        .await;
    let registry = WorkloadIdentitySessionRegistry::default();
    let context = r#"{"instance_id":"box-one"}"#;
    let mut first_config = session_config(temp_dir.path(), &server);
    first_config.workload_identity_context = Some(context.into());
    let second_config = first_config.clone();
    let first = WorkloadIdentityExternalAuth::from_config_with_registry(first_config, &registry)
        .expect("first adapter");
    let second = WorkloadIdentityExternalAuth::from_config_with_registry(second_config, &registry)
        .expect("second adapter");

    assert!(Arc::ptr_eq(&first.session, &second.session));
    assert_eq!(
        first
            .resolve()
            .await
            .expect("first auth")
            .get_token()
            .expect("first token"),
        second
            .resolve()
            .await
            .expect("second auth")
            .get_token()
            .expect("second token")
    );

    let requests = server.received_requests().await.expect("requests");
    assert_eq!(requests.len(), 1);
    assert_eq!(
        url::form_urlencoded::parse(&requests[0].body)
            .find(|(name, _)| name == "workload_identity_context")
            .map(|(_, value)| value.into_owned()),
        Some(context.to_string())
    );
}

#[tokio::test]
async fn incompatible_process_session_settings_are_rejected() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let server = MockServer::start().await;
    let registry = WorkloadIdentitySessionRegistry::default();
    let base = session_config(temp_dir.path(), &server);
    let _active = WorkloadIdentityExternalAuth::from_config_with_registry(base.clone(), &registry)
        .expect("active adapter");

    let mut different_rule = base.clone();
    different_rule.federation_rule_id = "rule-two".to_string();
    let mut different_file = base.clone();
    different_file.assertion_file = temp_dir.path().join("identity-token-two");
    std::fs::write(&different_file.assertion_file, "assertion-two").expect("write assertion");
    let mut different_environment = base.clone();
    different_environment.environment = WorkloadIdentityEnvironment::Production;
    let mut different_context = base.clone();
    different_context.workload_identity_context = Some(r#"{"instance_id":"box-two"}"#.into());
    let mut different_route = base;
    different_route.http_client_factory =
        HttpClientFactory::new(OutboundProxyPolicy::RespectSystemProxy);

    let different_route_adapter =
        WorkloadIdentityExternalAuth::from_config_with_registry(different_route, &registry)
            .expect("route changes reuse the process-owned session");
    assert!(Arc::ptr_eq(
        &_active.session,
        &different_route_adapter.session
    ));

    for config in [
        different_rule,
        different_file,
        different_environment,
        different_context,
    ] {
        assert!(matches!(
            WorkloadIdentityExternalAuth::from_config_with_registry(config, &registry),
            Err(WorkloadIdentitySessionError::ConflictingConfiguration)
        ));
    }
}

#[tokio::test]
async fn refresh_preserves_identity_and_invalid_tokens_are_reexchanged() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let server = MockServer::start().await;
    let request_count = Arc::new(AtomicUsize::new(0));
    let response_count = Arc::clone(&request_count);
    Mock::given(method("POST"))
        .respond_with(move |_request: &wiremock::Request| {
            match response_count.fetch_add(1, Ordering::SeqCst) {
                0 => success_response("access-one", "account-user-one", "user-one"),
                1 => success_response("access-two", "account-user-two", "user-one"),
                _ => success_response("access-three", "account-user-one", "user-one"),
            }
        })
        .mount(&server)
        .await;
    let registry = WorkloadIdentitySessionRegistry::default();
    let adapter = WorkloadIdentityExternalAuth::from_config_with_registry(
        session_config(temp_dir.path(), &server),
        &registry,
    )
    .expect("adapter");
    adapter.resolve().await.expect("initial auth");

    let error = adapter
        .refresh(ExternalAuthRefreshContext {
            reason: ExternalAuthRefreshReason::Unauthorized,
            previous_account_id: Some("account-one".to_string()),
        })
        .await
        .expect_err("identity change must be rejected");
    assert!(matches!(
        adapter.classify_error(error),
        RefreshTokenError::Permanent(_)
    ));
    assert_eq!(
        adapter
            .resolve()
            .await
            .expect("corrected token is re-exchanged")
            .get_token()
            .expect("token"),
        jwt("access-three", "user-one")
    );
    assert_eq!(request_count.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn api_refresh_preserves_product_and_identity() {
    for changed_field in [
        Some("api_organization_id"),
        Some("api_project_id"),
        Some("user_id"),
        None,
    ] {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let server = MockServer::start().await;
        let request_count = Arc::new(AtomicUsize::new(0));
        let response_count = Arc::clone(&request_count);
        Mock::given(method("POST"))
            .respond_with(move |_request: &wiremock::Request| {
                match response_count.fetch_add(1, Ordering::SeqCst) {
                    0 => ResponseTemplate::new(200).set_body_json(api_response("access-one")),
                    1 => match changed_field {
                        Some(field) => {
                            let mut body = api_response("access-two");
                            body[field] = "another-identity".into();
                            ResponseTemplate::new(200).set_body_json(body)
                        }
                        None => success_response("access-two", "account-user-one", "user-one"),
                    },
                    _ => ResponseTemplate::new(200).set_body_json(api_response("access-three")),
                }
            })
            .mount(&server)
            .await;
        let registry = WorkloadIdentitySessionRegistry::default();
        let adapter = WorkloadIdentityExternalAuth::from_config_with_registry(
            session_config(temp_dir.path(), &server),
            &registry,
        )
        .expect("adapter");
        assert_eq!(
            adapter.resolve().await.expect("initial API auth"),
            CodexAuth::from_api_key("access-one")
        );
        let error = adapter
            .refresh(ExternalAuthRefreshContext {
                reason: ExternalAuthRefreshReason::Unauthorized,
                previous_account_id: None,
            })
            .await
            .expect_err("product or identity change must be rejected");
        assert!(matches!(
            adapter.classify_error(error),
            RefreshTokenError::Permanent(_)
        ));
        assert_eq!(
            adapter
                .resolve()
                .await
                .expect("corrected token is re-exchanged"),
            CodexAuth::from_api_key("access-three")
        );
        assert_eq!(request_count.load(Ordering::SeqCst), 3);
    }
}

#[tokio::test]
#[serial(codex_auth_env)]
async fn resolved_product_obeys_login_and_workspace_restrictions() {
    for (product, login_method, workspace, allowed) in [
        ("api", ForcedLoginMethod::Api, None, true),
        ("api", ForcedLoginMethod::Chatgpt, None, false),
        ("chatgpt", ForcedLoginMethod::Chatgpt, None, true),
        ("chatgpt", ForcedLoginMethod::Api, None, false),
        (
            "chatgpt",
            ForcedLoginMethod::Chatgpt,
            Some("account-one"),
            true,
        ),
        (
            "chatgpt",
            ForcedLoginMethod::Chatgpt,
            Some("another-account"),
            false,
        ),
        (
            "api",
            ForcedLoginMethod::Chatgpt,
            Some("account-one"),
            false,
        ),
    ] {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let server = MockServer::start().await;
        let response = if product == "api" {
            ResponseTemplate::new(200).set_body_json(api_response("api-access"))
        } else {
            success_response("chatgpt-access", "account-user-one", "user-one")
        };
        Mock::given(method("POST"))
            .respond_with(response)
            .expect(1)
            .mount(&server)
            .await;
        let registry = WorkloadIdentitySessionRegistry::default();
        let adapter = WorkloadIdentityExternalAuth::from_config_with_registry(
            session_config(temp_dir.path(), &server),
            &registry,
        )
        .expect("adapter");
        let manager = AuthManager::shared_from_auth_config(
            AuthConfig {
                codex_home: temp_dir.path().to_path_buf(),
                auth_credentials_store_mode: AuthCredentialsStoreMode::Ephemeral,
                keyring_backend_kind: Default::default(),
                forced_login_method: Some(login_method),
                chatgpt_base_url: None,
                forced_chatgpt_workspace_id: workspace.map(|id| vec![id.to_string()]),
                managed_auth_policy: Default::default(),
                auth_route_config: auth_route_config(OutboundProxyPolicy::ReqwestDefault),
            },
            /*enable_codex_api_key_env*/ false,
        )
        .await
        .expect("valid auth policy");
        let result = manager.set_external_auth(Arc::new(adapter)).await;
        assert_eq!(
            result.is_ok(),
            allowed,
            "{product} with {login_method:?} and {workspace:?}"
        );
        if allowed {
            let auth = manager.auth().await.expect("accepted WIF auth");
            assert_eq!(auth.is_api_key_auth(), product == "api");
        } else {
            assert!(
                manager.auth().await.is_none(),
                "rejected WIF must not install credentials"
            );
        }
    }
}

#[tokio::test]
async fn concurrent_refreshes_share_one_exchange() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let server = MockServer::start().await;
    let request_count = Arc::new(AtomicUsize::new(0));
    let response_count = Arc::clone(&request_count);
    Mock::given(method("POST"))
        .respond_with(move |_request: &wiremock::Request| {
            if response_count.fetch_add(1, Ordering::SeqCst) == 0 {
                success_response("access-one", "account-user-one", "user-one")
            } else {
                success_response("access-two", "account-user-one", "user-one")
                    .set_delay(Duration::from_millis(30))
            }
        })
        .mount(&server)
        .await;
    let registry = WorkloadIdentitySessionRegistry::default();
    let first = Arc::new(
        WorkloadIdentityExternalAuth::from_config_with_registry(
            session_config(temp_dir.path(), &server),
            &registry,
        )
        .expect("first adapter"),
    );
    let second = Arc::new(
        WorkloadIdentityExternalAuth::from_config_with_registry(
            session_config(temp_dir.path(), &server),
            &registry,
        )
        .expect("second adapter"),
    );
    first.resolve().await.expect("first resolve");
    second.resolve().await.expect("second resolve");
    let context = ExternalAuthRefreshContext {
        reason: ExternalAuthRefreshReason::Unauthorized,
        previous_account_id: Some("account-one".to_string()),
    };

    let (first_refresh, second_refresh) =
        tokio::join!(first.refresh(context.clone()), second.refresh(context));
    assert_eq!(
        first_refresh
            .expect("first refresh")
            .get_token()
            .expect("first token"),
        second_refresh
            .expect("second refresh")
            .get_token()
            .expect("second token")
    );
    assert_eq!(request_count.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn exchange_errors_map_to_retry_policy() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let server = MockServer::start().await;
    let registry = WorkloadIdentitySessionRegistry::default();
    let adapter = WorkloadIdentityExternalAuth::from_config_with_registry(
        session_config(temp_dir.path(), &server),
        &registry,
    )
    .expect("adapter");

    let cases = [
        (WorkloadIdentityError::ExchangeRejected(400), false),
        (WorkloadIdentityError::ExchangeRejected(408), true),
        (
            WorkloadIdentityError::AssertionFile {
                path: temp_dir.path().join("missing"),
                source: Arc::new(std::io::Error::from(std::io::ErrorKind::NotFound)),
            },
            true,
        ),
    ];
    for (error, transient) in cases {
        let classified = adapter.classify_error(std::io::Error::other(error));
        assert_eq!(
            matches!(classified, RefreshTokenError::Transient(_)),
            transient
        );
    }
}
