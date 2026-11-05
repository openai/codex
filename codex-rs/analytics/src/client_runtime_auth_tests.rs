//! Runtime signing and destination isolation at the analytics delivery boundary.

use super::*;
use crate::facts::PluginMeasurementRow;
use crate::facts::PluginMeasurementsInput;
use codex_features::RuntimeAuthMode;
use codex_runtime_agent_instance::AgentHostAuthorization;
use codex_runtime_agent_instance::RuntimeAuthManager;
use codex_runtime_agent_instance::RuntimeAuthSlot;
use codex_runtime_agent_instance::RuntimeRegistrationMetadata;
use codex_runtime_agent_instance::RuntimeRole;
use pretty_assertions::assert_eq;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::path;

async fn emit(client: &AnalyticsEventsClient, execution_id: &str) {
    client.track_plugin_measurements(PluginMeasurementsInput {
        thread_id: "thread-a".into(),
        turn_id: "turn-a".into(),
        item_id: "item-a".into(),
        originator: "codex_cli_rs".into(),
        model_slug: None,
        reasoning_effort: None,
        plugin_id: "plugin-a".into(),
        execution_id: execution_id.into(),
        operation: "security_scan".into(),
        rows: vec![PluginMeasurementRow {
            measurement_name: "findings".into(),
            number_value: 1.0,
            dimensions: Default::default(),
        }],
    });
    client.flush().await;
}

#[tokio::test]
async fn runtime_delivery_signs_each_batch_and_strips_cross_origin_auth() {
    for mode in [RuntimeAuthMode::DualHeader, RuntimeAuthMode::AssertionOnly] {
        let server = MockServer::start().await;
        let redirected = MockServer::start().await;
        Mock::given(path("/"))
            .respond_with(ResponseTemplate::new(200))
            .expect(2)
            .mount(&redirected)
            .await;
        Mock::given(path("/agent/register"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "runtime_agent_instance_id": "runtime-analytics", "agent_host_id": "host-a",
                "actor": {"kind": "platform", "user_id": "user-a", "organization_id": "org-a", "project_id": null},
            })))
            .expect(1).mount(&server).await;
        Mock::given(path("/codex/analytics-events/events"))
            .respond_with(ResponseTemplate::new(307).insert_header("location", redirected.uri()))
            .expect(2)
            .mount(&server)
            .await;
        let credentials = AuthManager::from_auth_for_testing(CodexAuth::from_api_key("key-a"));
        let runtime = (
            Arc::new(RuntimeAuthManager::new(
                Arc::clone(&credentials),
                AgentHostAuthorization::InstallationId {
                    installation_id: "00000000-0000-0000-0000-000000000000".parse().unwrap(),
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
        // Inject the selected context at the delivery boundary for the loopback fixture.
        let client = AnalyticsEventsClient {
            queue: Some(AnalyticsEventsQueue::new(
                credentials,
                AnalyticsEventsDestination::Http {
                    base_url: server.uri().into(),
                    api_key_headers: Default::default(),
                },
                Some((Arc::new(std::sync::OnceLock::from(runtime.0)), runtime.1)),
            )),
        };
        emit(&client, "first").await;
        emit(&client, "second").await;
        let forwarded = redirected.received_requests().await.unwrap();
        assert_eq!(forwarded.len(), 2);
        for request in &forwarded {
            assert!(!request.headers.contains_key("authorization"));
            assert!(!request.headers.contains_key("x-openai-agent-assertion"));
        }
        redirected.verify().await;
        let requests = server.received_requests().await.unwrap();
        let proofs: Vec<_> = requests
            .iter()
            .filter(|r| r.url.path() == "/codex/analytics-events/events")
            .map(|request| {
                let authorization = request.headers["authorization"].to_str().unwrap();
                match mode {
                    RuntimeAuthMode::DualHeader => {
                        assert_eq!(authorization, "Bearer key-a");
                        request.headers["x-openai-agent-assertion"]
                            .to_str()
                            .unwrap()
                            .to_string()
                    }
                    RuntimeAuthMode::AssertionOnly => {
                        assert!(!request.headers.contains_key("x-openai-agent-assertion"));
                        authorization
                            .strip_prefix("AgentAssertion ")
                            .unwrap()
                            .to_string()
                    }
                }
            })
            .collect();
        assert_eq!(proofs.len(), 2);
        assert_ne!(proofs[0], proofs[1]);
        server.verify().await;
    }
}

#[tokio::test]
async fn custom_analytics_destination_does_not_initialize_runtime_auth() {
    for mode in [RuntimeAuthMode::DualHeader, RuntimeAuthMode::AssertionOnly] {
        let server = MockServer::start().await;
        Mock::given(path("/codex/analytics-events/events"))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&server)
            .await;
        let client = AnalyticsEventsClient::new_with_runtime(
            AuthManager::from_auth_for_testing(CodexAuth::from_api_key("key-a")),
            server.uri(),
            Some(true),
            Some((RuntimeAuthSlot::default(), mode)),
            /*api_key_headers*/ Default::default(),
        );
        emit(&client, "custom").await;
        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].headers["authorization"], "Bearer key-a");
        assert!(!requests[0].headers.contains_key("x-openai-agent-assertion"));
        server.verify().await;
    }
}

#[tokio::test]
async fn analytics_between_model_calls_keeps_the_selected_api_resources() {
    use codex_http_client::ClientRouteClass;
    use codex_login::default_client::ClientRedirectPolicy;
    use codex_model_provider_info::ModelProviderInfo;
    use std::collections::HashMap;

    for mode in [RuntimeAuthMode::DualHeader, RuntimeAuthMode::AssertionOnly] {
        let server = MockServer::start().await;
        Mock::given(path("/agent/register"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "runtime_agent_instance_id": "runtime-selected", "agent_host_id": "host-a",
                "actor": {"kind": "platform", "user_id": "user-a", "organization_id": "org-selected", "project_id": null},
            })))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(path("/v1/responses"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"output": []})),
            )
            .expect(2)
            .mount(&server)
            .await;
        Mock::given(path("/codex/analytics-events/events"))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&server)
            .await;
        let credentials = AuthManager::from_auth_for_testing(CodexAuth::from_api_key("key-a"));
        let owner = Arc::new(RuntimeAuthManager::new(
            Arc::clone(&credentials),
            AgentHostAuthorization::InstallationId {
                installation_id: "00000000-0000-0000-0000-000000000000".parse().unwrap(),
            },
            RuntimeRegistrationMetadata {
                harness: Some("codex".into()),
                harness_version: None,
                role: RuntimeRole::Orchestrator,
            },
            format!("{}/agent/register", server.uri()).parse().unwrap(),
        ));
        let mut info =
            ModelProviderInfo::create_openai_provider(Some(format!("{}/v1", server.uri())));
        info.env_http_headers = None;
        info.http_headers = Some(HashMap::from([
            ("OpenAI-Organization".into(), "org-selected".into()),
            ("OpenAI-Project".into(), "proj-selected".into()),
            ("X-Model-Secret".into(), "model-only".into()),
        ]));
        let destination = AnalyticsEventsDestination::from_base_url(
            server.uri().into(),
            info.build_header_map().unwrap(),
        );
        let analytics = AnalyticsEventsClient {
            queue: Some(AnalyticsEventsQueue::new(
                Arc::clone(&credentials),
                destination,
                Some((
                    Arc::new(std::sync::OnceLock::from(Arc::clone(&owner))),
                    mode,
                )),
            )),
        };
        let provider = codex_model_provider::test_support::runtime_model_provider(
            info,
            Arc::clone(&credentials),
            (owner, mode),
        );
        for attempt in 0..2 {
            let request = codex_model_provider::resolve_request(
                provider.as_ref(),
                /*scope*/ None,
                /*routing*/ None,
                credentials.http_client_factory(),
            )
            .await
            .unwrap();
            let url = request.api_provider.url_for_path("responses");
            let headers = request
                .api_auth
                .apply_auth_headers(request.api_provider.headers)
                .await
                .unwrap();
            let client = codex_login::default_client::create_client_for_route_async(
                request.http_client_factory,
                url.clone(),
                ClientRouteClass::Api,
                ClientRedirectPolicy::Default,
            )
            .await
            .unwrap();
            assert!(
                client
                    .post(&url)
                    .headers(headers)
                    .json(&serde_json::json!({"input": "hello"}))
                    .send()
                    .await
                    .unwrap()
                    .status()
                    .is_success()
            );
            if attempt == 0 {
                emit(&analytics, "between-model-calls").await;
            }
        }
        let requests = server.received_requests().await.unwrap();
        let registrations: Vec<_> = requests
            .iter()
            .filter(|request| request.url.path() == "/agent/register")
            .collect();
        assert_eq!(
            registrations.len(),
            1,
            "analytics must reuse the model registration"
        );
        let registration: serde_json::Value = registrations[0].body_json().unwrap();
        assert_eq!(
            registration["actor_auth"]["organization_id"],
            "org-selected"
        );
        assert_eq!(registration["actor_auth"]["project_id"], "proj-selected");
        for request in requests
            .iter()
            .filter(|request| request.url.path() != "/agent/register")
        {
            assert_eq!(request.headers["openai-organization"], "org-selected");
            assert_eq!(request.headers["openai-project"], "proj-selected");
            match mode {
                RuntimeAuthMode::DualHeader => {
                    assert_eq!(request.headers["authorization"], "Bearer key-a");
                    assert!(request.headers.contains_key("x-openai-agent-assertion"));
                }
                RuntimeAuthMode::AssertionOnly => {
                    assert!(
                        request.headers["authorization"]
                            .to_str()
                            .unwrap()
                            .starts_with("AgentAssertion ")
                    );
                    assert!(!request.headers.contains_key("x-openai-agent-assertion"));
                }
            }
            if request.url.path() == "/codex/analytics-events/events" {
                assert!(!request.headers.contains_key("x-model-secret"));
            }
        }
        server.verify().await;
    }
}
