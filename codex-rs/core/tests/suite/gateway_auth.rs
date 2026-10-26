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
use codex_model_provider_info::ModelProviderInfo;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::SessionSource;
use codex_protocol::user_input::UserInput;
use core_test_support::responses;
use core_test_support::test_codex::TestCodex;
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
async fn inference_sends_chatgpt_and_gateway_credentials_to_the_configured_headers() -> Result<()> {
    sample_with_gateway(
        "requires_openai_auth = true",
        "delivery = { kind = 'header', name = 'x-gateway-auth' }",
        CodexAuth::from_external_chatgpt_tokens(
            "header.e30.sig",
            "test-account",
            /*chatgpt_plan_type*/ None,
        )?,
        &[
            ("authorization", "Bearer header.e30.sig"),
            ("x-gateway-auth", "Bearer gateway-token"),
            ("chatgpt-account-id", "test-account"),
        ],
        /*expected_error*/ None,
    )
    .await
}

#[tokio::test]
async fn gateway_token_failure_blocks_sampling() -> Result<()> {
    sample_with_gateway(
        "requires_openai_auth = true",
        "delivery = { kind = 'header', name = 'x-gateway-auth' }",
        CodexAuth::from_api_key("provider-token"),
        &[],
        Some("Gateway OAuth authentication failed; check the gateway configuration and credential store."),
    ).await
}

#[tokio::test]
async fn omitted_delivery_sends_only_gateway_auth_even_when_signed_in() -> Result<()> {
    sample_with_gateway(
        "requires_openai_auth = false",
        "",
        CodexAuth::create_dummy_chatgpt_auth_for_testing(),
        &[("authorization", "Bearer gateway-token")],
        /*expected_error*/ None,
    )
    .await
}

async fn sample_with_gateway(
    provider_settings: &str,
    gateway_settings: &str,
    primary_auth: CodexAuth,
    expected_headers: &[(&str, &str)],
    expected_error: Option<&str>,
) -> Result<()> {
    let server = MockServer::start().await;
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
    let expires_at = if expected_error.is_some() {
        0
    } else {
        i64::MAX
    };
    let test = gateway_codex(
        &server,
        provider_settings,
        gateway_settings,
        primary_auth,
        expires_at,
    )
    .await?;
    assert_eq!(run_gateway_turn(&test).await?.as_deref(), expected_error);
    if let Some(response) = response {
        let request = response.single_request();
        for name in [
            "authorization",
            "x-gateway-auth",
            "cookie",
            "chatgpt-account-id",
        ] {
            let expected = expected_headers
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| *value);
            assert_eq!(
                request.header(name).as_deref(),
                expected,
                "{gateway_settings}: {name}"
            );
        }
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

async fn gateway_codex(
    server: &MockServer,
    provider_settings: &str,
    gateway_settings: &str,
    primary_auth: CodexAuth,
    expires_at: i64,
) -> Result<TestCodex> {
    let home = Arc::new(TempDir::new()?);
    let provider: ModelProviderInfo = toml::from_str(&format!(
        r#"
name = "gateway"
base_url = "{base}/v1"
{provider_settings}
[gateway_oauth]
authorization_url = "{base}/authorize"
token_url = "{base}/token"
client_id = "client"
{gateway_settings}
"#,
        base = server.uri()
    ))?;
    let primary = AuthManager::from_auth_for_testing_with_home(
        primary_auth.clone(),
        home.path().to_path_buf(),
    );
    let _gateway = seed_gateway_auth(
        &provider,
        &primary,
        json!({"access_token": "gateway-token", "refresh_token": "refresh-token", "expires_at": expires_at}),
    );
    test_codex()
        .with_home(home)
        .with_auth(primary_auth)
        .with_config(move |config| {
            config.model_provider = provider;
        })
        .build_with_auto_env(server)
        .await
}

async fn run_gateway_turn(test: &TestCodex) -> Result<Option<String>> {
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
    Ok(error_message)
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
