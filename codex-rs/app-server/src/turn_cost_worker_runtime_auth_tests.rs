//! Exercise runtime-authenticated turn-cost requests through OpenAI and provider aliases.

use super::*;
use codex_features::RuntimeAuthMode;
use codex_runtime_agent_instance::AgentHostAuthorization;
use codex_runtime_agent_instance::RuntimeAuthManager;
use codex_runtime_agent_instance::RuntimeRegistrationMetadata;
use codex_runtime_agent_instance::RuntimeRole;
use pretty_assertions::assert_eq;
use test_case::test_case;
use wiremock::matchers::body_partial_json;

#[test_case(false; "default OpenAI provider")]
#[test_case(true; "OpenAI alias")]
#[tokio::test]
async fn turn_costs_register_with_requested_organization(openai_alias: bool) {
    for mode in [RuntimeAuthMode::DualHeader, RuntimeAuthMode::AssertionOnly] {
        let server = MockServer::start().await;
        let registration = if openai_alias {
            ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "runtime_agent_instance_id": "runtime-telemetry", "agent_host_id": "host-a",
                "actor": {"kind": "platform", "user_id": "user-a", "organization_id": "org-a", "project_id": null},
            }))
        } else {
            // Inspect registration without sending a request to the real OpenAI destination.
            ResponseTemplate::new(403)
        };
        Mock::given(path("/agent/register"))
            .and(body_partial_json(serde_json::json!({
                "actor_auth": {"organization_id": "org-a"},
            })))
            .respond_with(registration)
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(path(TURN_COST_PATH))
            .and(header("openai-organization", "org-a"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"turns": []})),
            )
            .expect(if openai_alias { 2 } else { 0 })
            .mount(&server)
            .await;
        let credentials = AuthManager::from_auth_for_testing(CodexAuth::from_api_key("key-a"));
        let mut worker = test_runtime(&server, credentials.clone()).await;
        let owner = Arc::new(RuntimeAuthManager::new(
            credentials.clone(),
            AgentHostAuthorization::InstallationId {
                installation_id: uuid::Uuid::nil(),
            },
            RuntimeRegistrationMetadata {
                harness: Some("codex".into()),
                harness_version: None,
                role: RuntimeRole::Orchestrator,
            },
            format!("{}/agent/register", server.uri()).parse().unwrap(),
        ));
        let mut info = codex_model_provider_info::ModelProviderInfo::create_openai_provider(Some(
            format!("{}/v1", server.uri()),
        ));
        info.name = "OpenAI alias".to_string();
        info.http_headers = Some(HashMap::from([(
            "OpenAI-Organization".to_string(),
            "org-a".into(),
        )]));
        if openai_alias {
            worker.backend = TurnCostBackend::ModelProvider(
                codex_model_provider::test_support::runtime_model_provider(
                    info,
                    credentials,
                    (owner, mode),
                ),
            );
        } else {
            let config = Arc::make_mut(&mut worker.config);
            config.model_provider = info;
            config.chatgpt_base_url = "https://chatgpt.com".to_string();
            worker.runtime_auth = Some((owner, mode));
            worker
                .query_turn_costs(&[])
                .await
                .expect_err("registration denial must stop the cost request");
            server.verify().await;
            continue;
        }
        for _ in 0..2 {
            assert!(
                worker
                    .query_turn_costs(&[])
                    .await
                    .unwrap()
                    .unwrap()
                    .is_empty()
            );
        }
        let requests = server.received_requests().await.unwrap();
        let proofs: Vec<_> = requests
            .iter()
            .filter(|r| r.url.path() != "/agent/register")
            .map(|r| {
                let authorization = r.headers["authorization"].to_str().unwrap();
                match mode {
                    RuntimeAuthMode::DualHeader => {
                        assert_eq!(authorization, "Bearer key-a");
                        r.headers["x-openai-agent-assertion"]
                            .to_str()
                            .unwrap()
                            .to_owned()
                    }
                    RuntimeAuthMode::AssertionOnly => {
                        assert!(!r.headers.contains_key("x-openai-agent-assertion"));
                        authorization
                            .strip_prefix("AgentAssertion ")
                            .unwrap()
                            .to_owned()
                    }
                }
            })
            .collect();
        assert_eq!(proofs.len(), 2);
        assert_eq!(proofs.iter().collect::<HashSet<_>>().len(), 2);
        server.verify().await;
    }
}
