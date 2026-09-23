//! Selected-workspace discovery through the public account API.

use anyhow::Result;
use app_test_support::ChatGptAuthFixture;
use app_test_support::ChatGptIdTokenClaims;
use app_test_support::TestAppServer;
use app_test_support::encode_id_token;
use app_test_support::write_chatgpt_auth;
use codex_app_server_protocol::ClientInfo;
use codex_app_server_protocol::GetAccountParams;
use codex_app_server_protocol::LoginAccountResponse;
use codex_app_server_protocol::LogoutAccountResponse;
use codex_app_server_protocol::RequestId;
use codex_config::types::AuthCredentialsStoreMode;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use test_case::test_case;
use tokio::sync::Notify;
use tokio::time::timeout;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

const READ_TIMEOUT: Duration = Duration::from_secs(/*secs*/ 60);

async fn config(home: &TempDir, backend: &MockServer) -> Result<()> {
    std::fs::write(
        home.path().join("config.toml"),
        format!(
            "chatgpt_base_url = '{}/backend-api/'\ncli_auth_credentials_store = 'file'\n",
            backend.uri()
        ),
    )?;
    // Routing uses a plain HTTP fixture; restricted policies require HTTPS/WSS.
    Mock::given(method("GET")).and(path("/backend-api/wham/config/bundle"))
        .respond_with(|request: &wiremock::Request| {
            let account_id = request.headers.get("chatgpt-account-id").expect("workspace header")
                .to_str().expect("workspace header text");
            ResponseTemplate::new(200).set_body_json(json!({"requirements_toml": {"enterprise_managed": [{
                "id": "workspace-policy", "name": "Workspace policy",
                "contents": format!("[application.network]\nenabled = false\n[application.network.domains]\n'{account_id}.example' = 'allow'"),
            }]}}))
        }).mount(backend).await;
    Ok(())
}

async fn start(home: &TempDir) -> Result<TestAppServer> {
    TestAppServer::builder()
        .with_codex_home(home.path())
        .with_env_overrides(&[("OPENAI_API_KEY", None)])
        .build_initialized_with_timeout(READ_TIMEOUT)
        .await
}

async fn read(server: &mut TestAppServer) -> Result<Value> {
    let request = server
        .send_get_account_request(GetAccountParams {
            refresh_token: false,
        })
        .await?;
    timeout(READ_TIMEOUT, server.read_response(request)).await?
}

async fn login(server: &mut TestAppServer, account: &str) -> Result<()> {
    let token = encode_id_token(
        &ChatGptIdTokenClaims::new()
            .email("user@example.com")
            .plan_type("enterprise")
            .chatgpt_account_id(account),
    )?;
    let request = server
        .send_chatgpt_auth_tokens_login_request(token, account.into(), Some("enterprise".into()))
        .await?;
    let _: LoginAccountResponse = timeout(READ_TIMEOUT, server.read_response(request)).await??;
    Ok(())
}

fn routing(account: &str, origin: &str, routing: &str) -> Value {
    json!({"chatgptAccountId": account, "backendOrigin": origin, "accountRoutingOverride": routing})
}

#[test_case(0, 0, "pro"; "immediate_responses")]
#[test_case(6, 0, "pro"; "slow_accounts_response")]
#[test_case(0, 12, "enterprise"; "slow_cloud_bundle_response")]
#[tokio::test]
async fn saved_workspace_is_discovered_once_and_not_the_default_account(
    accounts_delay_secs: u64,
    bundle_delay_secs: u64,
    plan_type: &str,
) -> Result<()> {
    let backend = MockServer::start().await;
    Mock::given(method("GET")).and(path("/backend-api/wham/accounts/check"))
        .and(header("chatgpt-account-id", "selected"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "accounts": [
                {"id": "other", "workspace_backend_origin": "https://other.example", "account_routing_override": "us"},
                {"id": "selected", "workspace_backend_origin": "https://gov.chatgpt.com", "account_routing_override": "NO_CONSTRAINT"}
            ], "default_account_id": "other"
        })).set_delay(Duration::from_secs(accounts_delay_secs)))
        .expect(if accounts_delay_secs > 5 { 2 } else { 1 })
        .mount(&backend).await;
    let home = TempDir::new()?;
    config(&home, &backend).await?;
    if bundle_delay_secs > 0 {
        Mock::given(method("GET"))
            .and(path("/backend-api/wham/config/bundle"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!({}))
                    .set_delay(Duration::from_secs(bundle_delay_secs)),
            )
            .with_priority(1)
            // Startup and account hydration each retry this delayed response via proxy fallback.
            .expect(4)
            .mount(&backend)
            .await;
    }
    write_chatgpt_auth(
        home.path(),
        ChatGptAuthFixture::new("token")
            .account_id("selected")
            .email("user@example.com")
            .plan_type(plan_type),
        AuthCredentialsStoreMode::File,
    )?;
    let mut env_overrides = vec![("OPENAI_API_KEY", None), ("CODEX_API_KEY", None)];
    env_overrides.extend(
        codex_network_proxy::PROXY_ENV_KEYS
            .iter()
            .map(|key| (*key, None)),
    );
    let mut server = TestAppServer::builder()
        .with_codex_home(home.path())
        .with_env_overrides(&env_overrides)
        .build_initialized_with_timeout(READ_TIMEOUT)
        .await?;
    let notification = timeout(
        READ_TIMEOUT,
        server.read_stream_until_notification_message("account/updated"),
    )
    .await??;
    assert_eq!(
        notification.params,
        Some(json!({"authMode": "chatgpt", "planType": plan_type}))
    );
    let expected = json!({
        "account": {"type": "chatgpt", "email": "user@example.com", "planType": plan_type},
        "requiresOpenaiAuth": true, "workspaceRouting": routing("selected", "https://gov.chatgpt.com", "NO_CONSTRAINT")
    });
    assert_eq!(read(&mut server).await?, expected);
    assert_eq!(read(&mut server).await?, expected);
    backend.verify().await;
    Ok(())
}

#[tokio::test]
async fn saved_chatgpt_login_without_selected_workspace_preserves_account() -> Result<()> {
    let backend = MockServer::start().await;
    Mock::given(path("/backend-api/wham/accounts/check"))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&backend)
        .await;
    let home = TempDir::new()?;
    config(&home, &backend).await?;
    write_chatgpt_auth(
        home.path(),
        ChatGptAuthFixture::new("token").plan_type("pro"),
        AuthCredentialsStoreMode::File,
    )?;
    let mut server = start(&home).await?;
    assert_eq!(
        read(&mut server).await?,
        json!({"account": {"type": "chatgpt", "email": null, "planType": "pro"},
            "requiresOpenaiAuth": true, "workspaceRouting": null})
    );
    backend.verify().await;
    Ok(())
}

#[tokio::test]
async fn stable_clients_do_not_treat_failed_workspace_discovery_as_unrestricted() -> Result<()> {
    let backend = MockServer::start().await;
    Mock::given(path("/backend-api/wham/accounts/check"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&backend)
        .await;
    let home = TempDir::new()?;
    config(&home, &backend).await?;
    write_chatgpt_auth(
        home.path(),
        ChatGptAuthFixture::new("token")
            .account_id("selected")
            .plan_type("pro"),
        AuthCredentialsStoreMode::File,
    )?;
    let mut server = TestAppServer::builder()
        .with_codex_home(home.path())
        .with_env_overrides(&[("OPENAI_API_KEY", None)])
        .build()
        .await?;
    timeout(
        READ_TIMEOUT,
        server.initialize_with_capabilities(
            ClientInfo {
                name: "routing-stable-client".into(),
                title: None,
                version: "1.0.0".into(),
            },
            /*capabilities*/ None,
        ),
    )
    .await??;
    let request = server
        .send_get_account_request(GetAccountParams {
            refresh_token: false,
        })
        .await?;
    let error = timeout(
        READ_TIMEOUT,
        server.read_stream_until_error_message(RequestId::Integer(request)),
    )
    .await??;
    assert_eq!(error.error.message, "workspace routing discovery failed");
    backend.reset().await;
    Mock::given(path("/backend-api/wham/accounts/check"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"accounts": [{
            "id": "selected", "workspace_backend_origin": "https://chatgpt.com", "account_routing_override": "NO_CONSTRAINT"
        }]})))
        .mount(&backend)
        .await;
    assert_eq!(
        read(&mut server).await?,
        json!({"account": {"type": "chatgpt", "email": null, "planType": "pro"},
            "requiresOpenaiAuth": true, "workspaceRouting": routing("selected", "https://chatgpt.com", "NO_CONSTRAINT")})
    );
    Ok(())
}

#[tokio::test]
async fn login_and_workspace_switch_notify_after_routing_is_ready_then_logout_clears_it()
-> Result<()> {
    let backend = MockServer::start().await;
    Mock::given(method("GET")).and(path("/backend-api/wham/accounts/check"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"accounts": [
            {"id": "first", "workspace_backend_origin": "https://chatgpt.com", "account_routing_override": "us_cr"},
            {"id": "second", "workspace_backend_origin": "https://gov.chatgpt.com", "account_routing_override": "us"}
        ]}))).expect(2).mount(&backend).await;
    let home = TempDir::new()?;
    config(&home, &backend).await?;
    let mut server = start(&home).await?;
    for (account, origin, value) in [
        ("first", "https://chatgpt.com", "us_cr"),
        ("second", "https://gov.chatgpt.com", "us"),
    ] {
        login(&mut server, account).await?;
        let notification = timeout(
            READ_TIMEOUT,
            server.read_stream_until_notification_message("account/updated"),
        )
        .await??;
        assert_eq!(
            notification.params,
            Some(json!({"authMode": "chatgptAuthTokens", "planType": "enterprise"}))
        );
        assert_eq!(
            read(&mut server).await?["workspaceRouting"],
            routing(account, origin, value)
        );
        let request = server.send_config_requirements_read_request().await?;
        let requirements: Value = timeout(READ_TIMEOUT, server.read_response(request)).await??;
        assert_eq!(
            requirements["requirements"]["application"],
            json!({"network": {
                "enabled": false, "domains": {format!("{account}.example"): "allow"}
            }})
        );
    }
    let request = server.send_logout_account_request().await?;
    let _: LogoutAccountResponse = timeout(READ_TIMEOUT, server.read_response(request)).await??;
    assert_eq!(
        read(&mut server).await?,
        json!({"account": null, "requiresOpenaiAuth": true, "workspaceRouting": null})
    );
    backend.verify().await;
    Ok(())
}

#[tokio::test]
async fn unavailable_or_malformed_discovery_never_returns_unrestricted_success() -> Result<()> {
    for response in [
        ResponseTemplate::new(503),
        ResponseTemplate::new(200).set_body_json(json!({"accounts": [{"id": "selected"}]})),
        ResponseTemplate::new(200).set_body_json(json!({"accounts": [{"id": "selected", "account_routing_override": "us_cr"}]})),
        ResponseTemplate::new(200).set_body_json(json!({"accounts": [{"id": "selected", "workspace_backend_origin": "NO_CONSTRAINT"}]})),
        ResponseTemplate::new(200).set_body_json(json!({"accounts": [{"id": "selected", "workspace_backend_origin": null, "account_routing_override": null}]})),
        ResponseTemplate::new(200).set_body_json(json!({"accounts": [{"id": "selected", "workspace_backend_origin": 7, "account_routing_override": "us_cr"}]})),
        ResponseTemplate::new(200).set_body_json(json!({"accounts": [{"id": "selected", "workspace_backend_origin": "NO_CONSTRAINT", "account_routing_override": "unknown"}]})),
        ResponseTemplate::new(200).set_body_json(json!({"accounts": []})),
    ] {
        let backend = MockServer::start().await;
        Mock::given(method("GET")).and(path("/backend-api/wham/accounts/check"))
            .respond_with(response).mount(&backend).await;
        let home = TempDir::new()?;
        config(&home, &backend).await?;
        write_chatgpt_auth(home.path(), ChatGptAuthFixture::new("token").account_id("selected"), AuthCredentialsStoreMode::File)?;
        let mut server = start(&home).await?;
        let request = server.send_get_account_request(GetAccountParams { refresh_token: false }).await?;
        let error = timeout(READ_TIMEOUT, server.read_stream_until_error_message(RequestId::Integer(request))).await??;
        assert!(error.error.message.contains("routing"), "{error:?}");
        backend.reset().await;
        Mock::given(method("GET")).and(path("/backend-api/wham/accounts/check"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"accounts": [{
                "id": "selected", "workspace_backend_origin": "https://chatgpt.com", "account_routing_override": "NO_CONSTRAINT"
            }]}))).mount(&backend).await;
        assert_eq!(read(&mut server).await?["workspaceRouting"], routing("selected", "https://chatgpt.com", "NO_CONSTRAINT"));
    }
    Ok(())
}

#[tokio::test]
async fn late_startup_discovery_is_discarded_on_workspace_switch_and_logout() -> Result<()> {
    for next_account in [Some("second"), Some("first"), None] {
        let backend = MockServer::start().await;
        let started = Arc::new(Notify::new());
        let request_started = Arc::clone(&started);
        Mock::given(method("GET")).and(path("/backend-api/wham/accounts/check"))
            .and(header("chatgpt-account-id", "first"))
            .and(header("authorization", "Bearer token"))
            .respond_with(move |_: &wiremock::Request| {
                request_started.notify_one();
                ResponseTemplate::new(200).set_delay(Duration::from_secs(/*secs*/ 2))
                    .set_body_json(json!({"accounts": [{"id": "first", "workspace_backend_origin": "https://old.example", "account_routing_override": "us_cr"}]}))
            }).with_priority(1).mount(&backend).await;
        Mock::given(method("GET")).and(path("/backend-api/wham/accounts/check"))
            .and(header("chatgpt-account-id", next_account.unwrap_or("second")))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"accounts": [
                {"id": next_account.unwrap_or("second"), "workspace_backend_origin": "https://new.example", "account_routing_override": "us"}
            ]}))).mount(&backend).await;
        let home = TempDir::new()?;
        config(&home, &backend).await?;
        write_chatgpt_auth(
            home.path(),
            ChatGptAuthFixture::new("token").account_id("first"),
            AuthCredentialsStoreMode::File,
        )?;
        let mut server = start(&home).await?;
        timeout(READ_TIMEOUT, started.notified()).await?;
        if let Some(account) = next_account {
            login(&mut server, account).await?;
        } else {
            let request = server.send_logout_account_request().await?;
            let _: LogoutAccountResponse =
                timeout(READ_TIMEOUT, server.read_response(request)).await??;
        }
        let expected = next_account
            .map(|account| routing(account, "https://new.example", "us"))
            .unwrap_or(Value::Null);
        assert_eq!(read(&mut server).await?["workspaceRouting"], expected);
        tokio::time::sleep(Duration::from_secs(/*secs*/ 2)).await;
        assert_eq!(read(&mut server).await?["workspaceRouting"], expected);
    }
    Ok(())
}

#[tokio::test]
async fn api_only_login_does_not_discover_chatgpt_routing() -> Result<()> {
    let backend = MockServer::start().await;
    Mock::given(path("/backend-api/wham/accounts/check"))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&backend)
        .await;
    let home = TempDir::new()?;
    config(&home, &backend).await?;
    std::fs::write(
        home.path().join("requirements.toml"),
        "allowed_login_methods = ['api']",
    )?;
    let mut server = start(&home).await?;
    let request = server.send_login_account_api_key_request("sk-test").await?;
    let _: LoginAccountResponse = timeout(READ_TIMEOUT, server.read_response(request)).await??;
    assert_eq!(
        read(&mut server).await?,
        json!({"account": {"type": "apiKey"}, "requiresOpenaiAuth": true, "workspaceRouting": null})
    );
    backend.verify().await;
    Ok(())
}

#[tokio::test]
async fn failed_workspace_requirements_do_not_fall_back_to_startup_config() -> Result<()> {
    let backend = MockServer::start().await;
    let home = TempDir::new()?;
    config(&home, &backend).await?;
    let mut server = start(&home).await?;
    backend.reset().await;
    Mock::given(path("/backend-api/wham/config/bundle"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            json!({"requirements_toml": {"enterprise_managed": [{
                "id": "invalid", "name": "Invalid requirements", "contents": "application = true",
            }]}}),
        ))
        .mount(&backend)
        .await;
    Mock::given(path("/backend-api/wham/accounts/check"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&backend)
        .await;
    login(&mut server, "selected").await?;
    let notification = timeout(
        READ_TIMEOUT,
        server.read_stream_until_notification_message("account/login/completed"),
    )
    .await??;
    assert_eq!(notification.params.unwrap()["success"], false);
    let request = server
        .send_get_account_request(GetAccountParams {
            refresh_token: false,
        })
        .await?;
    let error = timeout(
        READ_TIMEOUT,
        server.read_stream_until_error_message(RequestId::Integer(request)),
    )
    .await??;
    assert_eq!(error.error.message, "failed to load workspace requirements");
    backend.verify().await;
    Ok(())
}

#[tokio::test]
async fn backend_config_changed_during_discovery_is_retried_with_fresh_config() -> Result<()> {
    let old_backend = MockServer::start().await;
    let new_backend = MockServer::start().await;
    let started = Arc::new(Notify::new());
    let request_started = Arc::clone(&started);
    Mock::given(path("/backend-api/wham/accounts/check"))
        .respond_with(move |_: &wiremock::Request| {
            request_started.notify_one();
            ResponseTemplate::new(200)
                .set_delay(Duration::from_secs(/*secs*/ 2))
                .set_body_json(json!({"accounts": [{
                    "id": "selected", "workspace_backend_origin": "https://old.example",
                    "account_routing_override": "us_cr"
                }]}))
        })
        .mount(&old_backend)
        .await;
    Mock::given(path("/backend-api/wham/accounts/check"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"accounts": [{
                "id": "selected", "workspace_backend_origin": "https://new.example",
                "account_routing_override": "us"
            }]})),
        )
        .expect(1)
        .mount(&new_backend)
        .await;
    let home = TempDir::new()?;
    config(&home, &old_backend).await?;
    let mut server = start(&home).await?;
    login(&mut server, "selected").await?;
    timeout(READ_TIMEOUT, started.notified()).await?;
    config(&home, &new_backend).await?;
    let completed = timeout(
        READ_TIMEOUT,
        server.read_stream_until_notification_message("account/login/completed"),
    )
    .await??;
    assert_eq!(
        completed.params,
        Some(json!({
            "loginId": null, "success": false,
            "error": "configuration changed during workspace routing discovery; retry account/read",
            "onboardingEntrypoint": null,
        }))
    );
    assert_eq!(
        read(&mut server).await?["workspaceRouting"],
        routing("selected", "https://new.example", "us")
    );
    new_backend.verify().await;
    Ok(())
}
