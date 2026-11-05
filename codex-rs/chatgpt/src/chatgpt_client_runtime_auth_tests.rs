//! Test captured ChatGPT credentials and runtime proofs on helper requests.

use super::*;
use base64::Engine;
use codex_features::RuntimeAuthMode;
use codex_runtime_agent_instance::AgentHostAuthorization;
use codex_runtime_agent_instance::RuntimeAuthManager;
use codex_runtime_agent_instance::RuntimeRegistrationMetadata;
use codex_runtime_agent_instance::RuntimeRole;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::collections::HashSet;
use std::sync::Arc;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::path;

#[tokio::test]
async fn helpers_use_captured_auth_and_strip_cross_origin_proofs() {
    for mode in [RuntimeAuthMode::DualHeader, RuntimeAuthMode::AssertionOnly] {
        let server = MockServer::start().await;
        let redirected = MockServer::start().await;
        Mock::given(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok": true})))
            .expect(1)
            .mount(&redirected)
            .await;
        Mock::given(path("/agent/register"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "runtime_agent_instance_id": "runtime-chatgpt", "agent_host_id": "host-a",
                "actor": {"kind": "chatgpt", "user_id": "user-a", "account_id": "workspace-a"},
            })))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(path("/data"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok": true})))
            .expect(2)
            .mount(&server)
            .await;
        Mock::given(path("/redirect"))
            .respond_with(ResponseTemplate::new(307).insert_header("location", redirected.uri()))
            .expect(1)
            .mount(&server)
            .await;
        let home = tempfile::TempDir::new().unwrap();
        let mut config = codex_core::config::ConfigBuilder::default()
            .codex_home(home.path().to_path_buf())
            .build()
            .await
            .unwrap();
        config.chatgpt_base_url = server.uri();
        let token = actor_token();
        let auth = CodexAuth::from_external_chatgpt_tokens(
            &token,
            "workspace-a",
            /*chatgpt_plan_type*/ None,
        )
        .unwrap();
        let runtime = (
            Arc::new(RuntimeAuthManager::new(
                AuthManager::from_auth_for_testing(auth.clone()),
                AgentHostAuthorization::InstallationId {
                    installation_id: uuid::Uuid::nil(),
                },
                RuntimeRegistrationMetadata {
                    harness: Some("codex".into()),
                    harness_version: None,
                    role: RuntimeRole::Orchestrator,
                },
                format!("{}/agent/register", server.uri()).parse().unwrap(),
            )),
            mode,
        );
        // No saved auth exists: these helpers must use the supplied snapshot, not reload auth.
        let get: serde_json::Value = chatgpt_get_request_with_timeout(
            &config,
            &auth,
            Some(&runtime),
            "data".into(),
            /*timeout*/ None,
        )
        .await
        .unwrap();
        let post: serde_json::Value = chatgpt_post_request_with_timeout(
            &config,
            &auth,
            "data".into(),
            &json!({}),
            Duration::from_secs(10),
            "codex",
            Some(&runtime),
        )
        .await
        .unwrap();
        assert_eq!(get, json!({"ok": true}));
        assert_eq!(post, get);
        let redirected_result = chatgpt_get_request_with_timeout::<serde_json::Value>(
            &config,
            &auth,
            Some(&runtime),
            "redirect".into(),
            /*timeout*/ None,
        )
        .await
        .unwrap();
        assert_eq!(redirected_result, get);
        let forwarded = redirected.received_requests().await.unwrap();
        assert_eq!(forwarded.len(), 1);
        assert!(!forwarded[0].headers.contains_key("authorization"));
        assert!(
            !forwarded[0]
                .headers
                .contains_key("x-openai-agent-assertion")
        );
        redirected.verify().await;
        let requests = server.received_requests().await.unwrap();
        let proofs: HashSet<_> = requests
            .iter()
            .filter(|r| r.url.path() != "/agent/register")
            .map(|r| {
                assert_eq!(r.headers["chatgpt-account-id"], "workspace-a");
                let authorization = r.headers["authorization"].to_str().unwrap();
                match mode {
                    RuntimeAuthMode::DualHeader => {
                        assert_eq!(authorization, format!("Bearer {token}"));
                        r.headers["x-openai-agent-assertion"].to_str().unwrap()
                    }
                    RuntimeAuthMode::AssertionOnly => {
                        assert!(!r.headers.contains_key("x-openai-agent-assertion"));
                        authorization.strip_prefix("AgentAssertion ").unwrap()
                    }
                }
            })
            .collect();
        assert_eq!(proofs.len(), 3);
        server.verify().await;
    }
}

fn actor_token() -> String {
    let claims = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(json!({
        "iss": "https://auth.openai.com/", "session_id": "login-a",
        "https://api.openai.com/auth": {"user_id": "user-a", "chatgpt_account_id": "workspace-a"},
    }).to_string());
    format!("header.{claims}.signature")
}

#[tokio::test]
async fn standalone_apply_keeps_custom_backend_auth_when_runtime_feature_is_enabled() {
    let server = MockServer::start().await;
    let token = actor_token();
    Mock::given(path("/task"))
        .and(wiremock::matchers::header(
            "authorization",
            format!("Bearer {token}"),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok": true})))
        .expect(1)
        .mount(&server)
        .await;
    let home = tempfile::TempDir::new().unwrap();
    codex_login::auth::login_with_chatgpt_auth_tokens(
        home.path(),
        &token,
        "workspace-a",
        /*chatgpt_plan_type*/ None,
    )
    .unwrap();
    let mut config = codex_core::config::ConfigBuilder::default()
        .codex_home(home.path().to_path_buf())
        .build()
        .await
        .unwrap();
    config.chatgpt_base_url = server.uri();
    config
        .features
        .enable(Feature::RuntimeAgentInstances)
        .unwrap();
    let response: serde_json::Value = chatgpt_get_request(&config, "task".into()).await.unwrap();
    assert_eq!(response, json!({"ok": true}));
    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    assert!(!requests[0].headers.contains_key("x-openai-agent-assertion"));
    server.verify().await;
}

#[tokio::test]
async fn standalone_apply_rejects_unsupported_openai_destination() {
    let home = tempfile::TempDir::new().unwrap();
    let mut config = codex_core::config::ConfigBuilder::default()
        .codex_home(home.path().to_path_buf())
        .build()
        .await
        .unwrap();
    config.chatgpt_base_url = "https://future.openai.com/backend-api".into();
    config
        .features
        .enable(Feature::RuntimeAgentInstances)
        .unwrap();

    let error = chatgpt_get_request::<serde_json::Value>(&config, "task".into())
        .await
        .unwrap_err();
    assert!(matches!(
        error
            .downcast_ref::<codex_protocol::error::CodexErr>()
            .map(codex_protocol::error::CodexErr::details),
        Some(codex_protocol::error::CodexErrorDetails::UnsupportedOperation(_))
    ));
}
