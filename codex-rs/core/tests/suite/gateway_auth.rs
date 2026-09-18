//! Exercises combined credentials and fail-closed gateway setup on the inference path.

use std::sync::Arc;

use anyhow::Result;
use codex_core::TurnInputRequest;
use codex_core::config::ConfigBuilder;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use codex_login::auth::AgentIdentityAuthPolicy;
use codex_model_provider::AgentIdentitySessionFallback;
use codex_model_provider::ProviderAuthScope;
use codex_model_provider::create_model_provider;
use codex_model_provider::test_support::seed_gateway_auth;
use codex_model_provider_info::GatewayOAuthConfig;
use codex_model_provider_info::GatewayOAuthDelivery;
use codex_model_provider_info::ModelProviderInfo;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::SessionSource;
use codex_protocol::user_input::UserInput;
use core_test_support::responses;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

#[tokio::test]
async fn sampling_uses_gateway_and_primary_auth() -> Result<()> {
    sampling_requires_gateway_and_primary_auth(/*expected_error*/ None).await
}

#[tokio::test]
async fn gateway_token_failure_blocks_sampling() -> Result<()> {
    sampling_requires_gateway_and_primary_auth(Some(
        "Gateway OAuth authentication failed; check the gateway configuration and credential store.",
    )).await
}

async fn sampling_requires_gateway_and_primary_auth(expected_error: Option<&str>) -> Result<()> {
    let server = MockServer::start().await;
    let home = Arc::new(TempDir::new()?);
    let response = if expected_error.is_none() {
        Some(
            responses::mount_sse_once(
                &server,
                responses::sse(vec![
                    responses::ev_response_created("gateway-response"),
                    responses::ev_completed("gateway-response"),
                ]),
            )
            .await,
        )
    } else {
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(
                ResponseTemplate::new(500).set_body_json(json!({"error": "server_error"})),
            )
            .mount(&server)
            .await;
        None
    };
    let mut provider =
        ModelProviderInfo::create_openai_provider(Some(format!("{}/v1", server.uri())));
    provider.supports_websockets = false;
    provider.gateway_oauth = Some(GatewayOAuthConfig {
        authorization_url: format!("{}/authorize", server.uri()),
        token_url: format!("{}/token", server.uri()),
        client_id: "client".into(),
        resource: None,
        scopes: vec![],
        redirect_port: None,
        delivery: GatewayOAuthDelivery::Header {
            name: "x-gateway-auth".into(),
            scheme: "Bearer".into(),
        },
    });
    let primary = AuthManager::from_auth_for_testing_with_home(
        CodexAuth::from_api_key("primary-token"),
        home.path().to_path_buf(),
    );
    let _gateway = seed_gateway_auth(
        &provider,
        &primary,
        json!({"access_token": "gateway-token", "refresh_token": "refresh-token", "expires_at": if expected_error.is_some() { 0 } else { i64::MAX }}),
    );
    let test = test_codex()
        .with_home(home)
        .with_auth(CodexAuth::from_api_key("primary-token"))
        .with_config(move |config| {
            config.model_provider = provider;
        })
        .build_with_auto_env(&server)
        .await?;
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "hello".into(),
            text_elements: vec![],
        }]))
        .await?;
    let mut error_message = None;
    wait_for_event(&test.codex, |event| {
        if let EventMsg::Error(error) = event {
            error_message = Some(error.message.clone());
        }
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    assert_eq!(error_message.as_deref(), expected_error);
    if let Some(response) = response {
        let request = response.single_request();
        assert_eq!(
            (
                request.header("authorization"),
                request.header("x-gateway-auth")
            ),
            (
                Some("Bearer primary-token".to_string()),
                Some("Bearer gateway-token".to_string())
            ),
        );
    } else {
        let requests = server
            .received_requests()
            .await
            .expect("recorded gateway requests");
        assert!(!requests.is_empty());
        assert!(
            requests
                .iter()
                .all(|request| request.url.path() == "/token")
        );
    }
    Ok(())
}

#[tokio::test]
async fn configured_gateway_http_initialization_fails_closed() -> Result<()> {
    const CHILD_ENV: &str = "CODEX_TEST_GATEWAY_INVALID_CA_CHILD";
    if std::env::var_os(CHILD_ENV).is_none() {
        let home = TempDir::new()?;
        let invalid_ca = home.path().join("invalid-ca.pem");
        std::fs::write(&invalid_ca, "not a PEM certificate")?;
        let output = std::process::Command::new(std::env::current_exe()?)
            .arg("--exact")
            .arg("suite::gateway_auth::configured_gateway_http_initialization_fails_closed")
            .arg("--nocapture")
            .env(CHILD_ENV, "1")
            .env("CODEX_CA_CERTIFICATE", invalid_ca)
            .output()?;
        assert!(
            output.status.success(),
            "gateway setup subprocess failed: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return Ok(());
    }

    let home = TempDir::new()?;
    // An invalid custom CA must not introduce a gateway dependency when none is configured.
    let config = ConfigBuilder::default()
        .codex_home(home.path().to_path_buf())
        .build()
        .await?;
    let primary =
        AuthManager::shared_from_config(&config, /*enable_codex_api_key_env*/ false).await?;
    create_model_provider(config.model_provider, Some(primary))
        .api_auth()
        .await?;
    std::fs::write(
        home.path().join("config.toml"),
        r#"
model_provider = "gateway"
[features]
respect_system_proxy = true
[model_providers.gateway]
name = "Gateway"
base_url = "http://127.0.0.1:9/v1"
[model_providers.gateway.gateway_oauth]
authorization_url = "http://127.0.0.1:9/authorize"
token_url = "http://127.0.0.1:9/token"
client_id = "client"
delivery = { kind = "header", name = "x-gateway-auth" }
"#,
    )?;
    let config = ConfigBuilder::default()
        .codex_home(home.path().to_path_buf())
        .build()
        .await?;
    let primary =
        AuthManager::shared_from_config(&config, /*enable_codex_api_key_env*/ false).await?;
    let provider = create_model_provider(config.model_provider.clone(), Some(primary));
    let error = provider.api_auth().await.err().unwrap();
    assert_eq!(
        error.to_string(),
        "failed to create provider OAuth HTTP client"
    );
    let error = provider
        .api_auth_for_scope(ProviderAuthScope {
            agent_identity_policy: AgentIdentityAuthPolicy::JwtOnly,
            session_source: SessionSource::Cli,
            agent_identity_session_fallback: AgentIdentitySessionFallback::default(),
        })
        .await
        .err()
        .unwrap();
    assert_eq!(
        error.to_string(),
        "failed to create provider OAuth HTTP client"
    );
    let contents = std::fs::read_to_string(home.path().join("config.toml"))?;
    std::fs::write(
        home.path().join("config.toml"),
        contents.replace("client_id = \"client\"", "client_id = \"\""),
    )?;
    let error = ConfigBuilder::default()
        .codex_home(home.path().to_path_buf())
        .build()
        .await
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("gateway_oauth requires a nonempty client_id")
    );
    Ok(())
}
