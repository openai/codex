//! Exercise Analytics construction and reauthentication through production HTTPS routes.

#![cfg(test)]

use super::Live;
use super::tests::sign_in;
use crate::legacy_core::config::ConfigBuilder;
use anyhow::Context;
use anyhow::Result;
use base64::Engine;
use codex_backend_client::AnalyticsReport;
use codex_backend_client::AnalyticsResponse;
use codex_config::LoaderOverrides;
use codex_features::Feature;
use codex_features::RuntimeAuthMode;
use codex_protocol::account::PlanType;
use core_test_support::runtime_auth::PROBE_SERVER;
use core_test_support::runtime_auth::run_harness_through_proxy;
use core_test_support::skip_if_no_network;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::collections::BTreeSet;
use std::sync::Arc;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

async fn runtime_client(
    base_url: &str,
    mode: RuntimeAuthMode,
) -> Result<(tempfile::TempDir, Live)> {
    let home = tempfile::tempdir()?;
    let mut config = ConfigBuilder::default()
        .codex_home(home.path().to_path_buf())
        .loader_overrides(LoaderOverrides::without_managed_config_for_tests())
        .build()
        .await?;
    config.chatgpt_base_url = base_url.to_string();
    config.cli_auth_credentials_store_mode = codex_login::AuthCredentialsStoreMode::File;
    config.features.enable(Feature::RuntimeAgentInstances)?;
    config.runtime_auth_mode = mode;
    sign_in(home.path(), "account-a", "user-a", "plus");
    Ok((home, Live::new(Arc::new(config), "2026-01-15".parse()?)))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn analytics_runtime_covers_regional_account_and_report_requests() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let bases = [
        "https://chatgpt.com/backend-api",
        "https://us.chatgpt.com/backend-api",
        "https://chatgpt-staging.com/backend-api",
        "https://us.chatgpt-staging.com/backend-api",
    ];
    let modes = [RuntimeAuthMode::DualHeader, RuntimeAuthMode::AssertionOnly];
    if std::env::var(PROBE_SERVER).is_ok() {
        for base in bases {
            for mode in modes {
                let (_home, live) = runtime_client(base, mode).await?;
                let session = live
                    .session()
                    .await
                    .map_err(anyhow::Error::msg)
                    .with_context(|| format!("{base} {mode:?}"))?;
                assert_eq!(session.backend.account().plan_type, Some(PlanType::Pro));
                let response = session
                    .backend
                    .request(|client| async move {
                        client
                            .get_account_analytics(
                                AnalyticsReport::Usage,
                                "2026-01-01",
                                "2026-01-15",
                            )
                            .await
                    })
                    .await?;
                assert_eq!(
                    response,
                    AnalyticsResponse::Usage(
                        codex_backend_client::analytics_models::DailyProductSurfaceUsageResponse::default(),
                    ),
                );
            }
        }
        return Ok(());
    }

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/agent/register"))
        .respond_with(ResponseTemplate::new(/*s*/ 200).set_body_json(json!({
            "runtime_agent_instance_id": "runtime-analytics", "agent_host_id": "host-a",
            "actor": {"kind": "chatgpt", "user_id": "user-a", "account_id": "account-a"},
        })))
        .expect(/*r*/ 8)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/backend-api/wham/accounts/check"))
        .respond_with(ResponseTemplate::new(/*s*/ 200).set_body_json(json!({
            "accounts": [{"id": "account-a", "plan_type": "pro"}],
            "account_ordering": ["account-a"],
        })))
        .expect(/*r*/ 8)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/backend-api/wham/usage/daily-token-usage-breakdown"))
        .respond_with(ResponseTemplate::new(/*s*/ 200).set_body_json(json!({"data": []})))
        .expect(/*r*/ 8)
        .mount(&server)
        .await;
    run_harness_through_proxy(
        &server,
        "analytics::client::runtime_auth_tests::analytics_runtime_covers_regional_account_and_report_requests",
        /*realtime_target*/ None,
    ).await?;
    server.verify().await;
    let requests = server
        .received_requests()
        .await
        .context("captured requests")?;
    assert_eq!(requests.len(), 24);
    let mut proofs = BTreeSet::new();
    for (index, requests) in requests.as_chunks::<3>().0.iter().enumerate() {
        let base = bases[index / modes.len()];
        let mode = modes[index % modes.len()];
        let registration = &requests[0];
        assert_eq!(registration.url.path(), "/agent/register");
        assert_eq!(
            registration.headers["host"],
            if base.contains("staging") {
                "auth.api.openai.org"
            } else {
                "auth.openai.com"
            },
        );
        let body: Value = registration.body_json()?;
        let credential = body["actor_auth"]["credential"]
            .as_str()
            .context("actor credential")?;
        for request in &requests[1..] {
            assert_eq!(
                request.headers["host"],
                base.strip_prefix("https://")
                    .expect("HTTPS test URL")
                    .strip_suffix("/backend-api")
                    .expect("test API path"),
            );
            assert_eq!(request.headers["chatgpt-account-id"], "account-a");
            let authorization = request.headers["authorization"].to_str()?;
            let proof = match mode {
                RuntimeAuthMode::DualHeader => {
                    assert_eq!(authorization, format!("Bearer {credential}"));
                    request.headers["x-openai-agent-assertion"].to_str()?
                }
                RuntimeAuthMode::AssertionOnly => {
                    assert!(!request.headers.contains_key("x-openai-agent-assertion"));
                    authorization
                        .strip_prefix("AgentAssertion ")
                        .context("runtime authorization")?
                }
            };
            assert!(
                proofs.insert(proof.to_owned()),
                "every request needs a fresh assertion"
            );
            let payload = proof
                .strip_prefix("1.")
                .context("runtime assertion version")?
                .split('.')
                .nth(/*n*/ 1)
                .context("assertion payload")?;
            let claims: Value = serde_json::from_slice(
                &base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(payload)?,
            )?;
            assert_eq!(claims["runtime_agent_instance"], "runtime-analytics");
        }
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn analytics_runtime_bootstrap_preserves_sign_in_guidance() -> Result<()> {
    skip_if_no_network!(Ok(()));
    if std::env::var(PROBE_SERVER).is_ok() {
        let (_home, live) = runtime_client(
            "https://chatgpt.com/backend-api",
            RuntimeAuthMode::DualHeader,
        )
        .await?;
        assert_eq!(
            live.session().await.err().as_deref(),
            Some("Sign in again to load Analytics."),
        );
        return Ok(());
    }
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/agent/register"))
        .respond_with(ResponseTemplate::new(/*s*/ 401).set_body_json(json!({
            "error": {"code": "token_expired"},
        })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(/*s*/ 401).set_body_json(json!({
            "error": {"code": "refresh_token_expired"},
        })))
        .expect(/*r*/ 1)
        .mount(&server)
        .await;
    run_harness_through_proxy(
        &server,
        "analytics::client::runtime_auth_tests::analytics_runtime_bootstrap_preserves_sign_in_guidance",
        /*realtime_target*/ None,
    ).await?;
    server.verify().await;
    let requests = server
        .received_requests()
        .await
        .context("captured requests")?;
    assert!(
        requests
            .iter()
            .any(|request| request.url.path() == "/agent/register")
    );
    assert!(
        requests
            .iter()
            .all(|request| { matches!(request.url.path(), "/agent/register" | "/oauth/token") })
    );
    Ok(())
}
