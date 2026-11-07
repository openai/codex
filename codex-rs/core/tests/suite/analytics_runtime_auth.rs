//! Analytics plan discovery must not follow redirects with runtime authentication.

use anyhow::Result;
use base64::Engine;
use codex_backend_client::AnalyticsSession;
use codex_features::RuntimeAuthMode;
use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use codex_login::AuthCredentialsStoreMode;
use core_test_support::load_default_config_for_test;
use core_test_support::runtime_auth::PROBE_SERVER;
use core_test_support::runtime_auth::run_harness_through_proxy_with_env;
use core_test_support::skip_if_no_network;
use pretty_assertions::assert_eq;
use serde_json::json;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn analytics_does_not_follow_runtime_redirects() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let token = format!(
        "e30.{}.c2ln",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(
            json!({
                "exp": 4_102_444_800_u64,
                "iss": "https://auth.openai.com",
                "session_id": "session-analytics",
                "https://api.openai.com/auth": {
                    "chatgpt_user_id": "user-analytics",
                    "chatgpt_account_id": "account-analytics",
                },
            })
            .to_string(),
        ),
    );
    if std::env::var(PROBE_SERVER).is_ok() {
        for mode in [RuntimeAuthMode::DualHeader, RuntimeAuthMode::AssertionOnly] {
            let home = tempfile::tempdir()?;
            codex_login::auth::login_with_chatgpt_auth_tokens(
                home.path(),
                &token,
                "account-analytics",
                Some("plus"),
            )?;
            let mut config = load_default_config_for_test(&home).await;
            config.chatgpt_base_url = "https://chatgpt.com/backend-api".into();
            config.cli_auth_credentials_store_mode = AuthCredentialsStoreMode::Ephemeral;
            let result = AnalyticsSession::from_config(
                &config,
                HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
                Some(mode),
            )
            .await;
            assert!(
                result.is_err(),
                "Analytics must reject a redirected plan response"
            );
        }
        return Ok(());
    }

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/agent/register"))
        .respond_with(ResponseTemplate::new(/*s*/ 200).set_body_json(json!({
            "runtime_agent_instance_id": "runtime-analytics",
            "agent_host_id": "host-analytics",
            "actor": {
                "kind": "chatgpt",
                "user_id": "user-analytics",
                "account_id": "account-analytics",
            },
        })))
        .expect(/*r*/ 2)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/backend-api/wham/accounts/check"))
        .respond_with(ResponseTemplate::new(/*s*/ 307).insert_header(
            "Location",
            "https://chatgpt.com/backend-api/redirected-plan",
        ))
        .expect(/*r*/ 2)
        .mount(&server)
        .await;
    Mock::given(path("/backend-api/redirected-plan"))
        .respond_with(ResponseTemplate::new(/*s*/ 200).set_body_json(json!({
            "accounts": [],
            "account_ordering": [],
        })))
        .expect(/*r*/ 0)
        .mount(&server)
        .await;

    run_harness_through_proxy_with_env(
        &server,
        "suite::analytics_runtime_auth::analytics_does_not_follow_runtime_redirects",
        /*realtime_target*/ None,
        &[
            ("OPENAI_API_KEY", None),
            ("CODEX_API_KEY", None),
            ("OPENAI_FEDERATION_RULE_ID", None),
            ("OPENAI_IDENTITY_TOKEN_FILE", None),
        ],
    )
    .await?;

    let requests = server
        .received_requests()
        .await
        .expect("requests should be recorded");
    let discovery: Vec<_> = requests
        .iter()
        .filter(|request| request.url.path() == "/backend-api/wham/accounts/check")
        .collect();
    assert_eq!(discovery.len(), 2);
    assert_eq!(
        discovery[0].headers["authorization"],
        format!("Bearer {token}")
    );
    assert!(
        discovery[0]
            .headers
            .contains_key("x-openai-agent-assertion")
    );
    assert!(
        discovery[1].headers["authorization"]
            .to_str()?
            .starts_with("AgentAssertion ")
    );
    assert!(
        !discovery[1]
            .headers
            .contains_key("x-openai-agent-assertion")
    );
    server.verify().await;
    Ok(())
}
