//! Decisions outcomes cannot affect the real baseline score/publication path.
use super::*;
use crate::async_scorer::conversation::ConversationBackend;
use crate::async_scorer::decisions::DecisionsCredential;
use crate::async_scorer::decisions::DecisionsSampler;
use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use pretty_assertions::assert_eq;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn decisions_disagreement_failures_and_disabled_flag_preserve_baseline() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let trusted_skill_path = "/skills/example/SKILL.md";
    let cases = [
        ("disabled", false, 200, "high", "low", 0.0),
        ("not_initialized", true, 200, "high", "low", 0.0),
        ("cannot_escalate", true, 200, "high", "low", 0.0),
        ("cannot_lower", true, 200, "low", "high", 1.0),
        ("candidate_failure", true, 403, "low", "low", 0.0),
        ("baseline_failure", true, 200, "low", "invalid", 1.0),
    ];
    for (case, enabled, status, choice, baseline, expected_risk) in cases {
        let server = responses::start_mock_server().await;
        let fixture = GuardianFailureFixture::with_config(&format!(
            "[features]\nguardianv2_decisions_comparison = {enabled}\n[features.guardianv2]\n"
        ))
        .await?;
        let test = &fixture.test;
        let mut config = test.config.clone();
        config.features.enable(Feature::GuardianApproval)?;
        config.features.enable(Feature::GuardianV2)?;
        if enabled {
            config
                .features
                .enable(Feature::GuardianV2DecisionsComparison)?;
        }
        config.model_provider =
            ModelProviderInfo::create_openai_provider(Some(format!("{}/v1", server.uri())));
        config.model_provider.supports_websockets = false;
        let metrics = Arc::new(RecordingMetrics::default());
        let registry = &fixture.registry;
        let session = &fixture.session_store;
        let store = test.codex.thread_extension_data();
        let adaptive_model = store.get::<ModelInfo>().unwrap();
        let mut initial_model = adaptive_model.as_ref().clone();
        initial_model.guardian = Some(Default::default());
        store.insert(initial_model);
        registry.thread_lifecycle_contributors()[0]
            .on_thread_start(ThreadStartInput {
                config: &config,
                session_source: &SessionSource::Exec,
                persistent_thread_state_available: false,
                environments: &[],
                mcp_resource_client: None,
                extension_metrics: Some(metrics.clone()),
                session_store: session,
                thread_store: store,
            })
            .await;
        store.insert(adaptive_model.as_ref().clone());
        let baseline_mock = responses::mount_sse_once(
            &server,
            responses::sse(vec![
                ev_assistant_message("sample", baseline),
                ev_completed("baseline-response"),
            ]),
        )
        .await;
        let decisions_server = responses::start_mock_server().await;
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .respond_with(
                wiremock::ResponseTemplate::new(status).set_body_json(json!({
                    "answers":[{"type":"choice", "name":"guardian_risk",
                    "choice":choice}]
                })),
            )
            .expect(/*r*/ u64::from(enabled && case != "not_initialized"))
            .mount(&decisions_server)
            .await;
        // Inject only the transport, using the existing mock HTTP server instead of credentials.
        if case == "not_initialized" {
            store.remove::<DecisionsSampler>();
        } else if enabled {
            store.insert(DecisionsSampler::new(
                HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
                DecisionsCredential::ApiKey("synthetic-key".into()),
                decisions_server.uri(),
                /*auth_context*/ None,
            )?);
        } else {
            assert!(store.get::<DecisionsSampler>().is_none());
        }
        if case == "cannot_escalate" {
            store
                .get_or_init(codex_core::context::GuardianReviewEvidence::default)
                .record_trusted_skill("turn-1", trusted_skill_path.into());
        }
        metrics.0.lock().unwrap().clear();
        fixture.score_tool(ToolName::plain("read_file")).await;
        let score = tokio::time::timeout(ASYNC_TEST_TIMEOUT, async {
            loop {
                if let Some(score) = cached_score(store)
                    // The fixture's initial score has no sampling timestamp.
                    && score.sampled_at.is_some()
                {
                    break score;
                }
                tokio::task::yield_now().await;
            }
        })
        .await;
        let score = score?;
        assert_eq!(
            score.scores,
            BTreeMap::from([("action_risk".into(), expected_risk)]),
            "{case}"
        );
        let published = score.clone();
        if enabled {
            tokio::time::timeout(ASYNC_TEST_TIMEOUT, async {
                loop {
                    if metrics.0.lock().unwrap().iter().any(|sample| matches!(sample,
                        RecordedMetric::Counter(name, _, _) if name == "codex.guardian_v2.decisions_comparison.comparison")) { break; }
                    tokio::task::yield_now().await;
                }
            }).await?;
            if case == "cannot_escalate" {
                let requests = decisions_server.received_requests().await.unwrap();
                let body: serde_json::Value = serde_json::from_slice(&requests[0].body)?;
                let baseline_body = baseline_mock.single_request().body_json();
                let baseline_input = baseline_body["input"].as_array().unwrap();
                let policy_index = baseline_input
                    .iter()
                    .position(|item| item["role"] == "developer" && item["type"] == "message")
                    .unwrap();
                let policy = &baseline_input[policy_index];
                assert_eq!(
                    body["questions"][0]["instructions"],
                    policy["content"][0]["text"]
                );
                let evidence = baseline_input[policy_index + 1..]
                    .iter()
                    .map(|item| json!({"role": item["role"], "content":item["content"]}))
                    .collect::<Vec<_>>();
                assert!(evidence.iter().any(|item| {
                    item["role"] == "developer"
                        && item["content"][0]["text"]
                            .as_str()
                            .is_some_and(|text| text.contains(trusted_skill_path))
                }));
                assert_eq!(body["input"], json!(evidence));
            }
            let (outcome, reason) = match case {
                "not_initialized" => ("skipped", "not_initialized"),
                "candidate_failure" => ("failure", "http_403"),
                _ => ("success", "none"),
            };
            let outcomes = metrics.0.lock().unwrap().iter().filter(|sample| matches!(sample,
                RecordedMetric::Counter(name, _, _) if name == "codex.guardian_v2.decisions_comparison"))
                .cloned().collect::<Vec<_>>();
            assert_eq!(
                outcomes,
                vec![RecordedMetric::Counter(
                    "codex.guardian_v2.decisions_comparison".into(),
                    1,
                    vec![
                        ("outcome".into(), outcome.into()),
                        ("reason".into(), reason.into())
                    ]
                )],
                "{case}"
            );
            let (comparison, responses, decisions) = match case {
                "cannot_escalate" => ("disagree", "low", "high"),
                "cannot_lower" => ("disagree", "high", "low"),
                "baseline_failure" => ("unavailable", "unavailable", "low"),
                _ => ("unavailable", "low", "unavailable"),
            };
            let comparisons = metrics.0.lock().unwrap().iter().filter(|sample| matches!(sample,
                RecordedMetric::Counter(name, _, _) if name == "codex.guardian_v2.decisions_comparison.comparison"))
                .cloned().collect::<Vec<_>>();
            assert_eq!(
                comparisons,
                vec![RecordedMetric::Counter(
                    "codex.guardian_v2.decisions_comparison.comparison".into(),
                    1,
                    vec![
                        ("comparison".into(), comparison.into()),
                        ("responses".into(), responses.into()),
                        ("decisions".into(), decisions.into()),
                    ],
                )],
                "{case}"
            );
        }
        assert_eq!(cached_score(store), Some(published), "{case}");
        decisions_server.verify().await;
        assert_eq!(baseline_mock.requests().len(), 1);
    }
    Ok(())
}

#[tokio::test]
async fn guardian_budget_rejects_both_backends_before_sending() -> Result<()> {
    skip_if_no_network!(Ok(()));
    for expected_reason in [Some("preparation_failed"), None] {
        let enabled = expected_reason.is_some();
        let fixture = GuardianFailureFixture::with_config(&format!(
            "[features]\nguardianv2_decisions_comparison = {enabled}\n[features.guardianv2]\nenabled = true\n",
        ))
        .await?;
        let server = responses::start_mock_server().await;
        let store = fixture.test.codex.thread_extension_data();
        let metrics = Arc::new(RecordingMetrics::default());
        store.insert(GuardianV2ScoreProgress::new(Some(metrics.clone())));
        // Replace the conversation backend to force the budget failure.
        let mut sampler_config = crate::async_scorer::sampler::tests::sampler_config(server.uri());
        sampler_config.max_input_tokens = 256;
        store.insert(LunaSampler::new(sampler_config));
        store.insert(ConversationBackend::new(
            store.get::<LunaSampler>().unwrap(),
        ));
        if enabled {
            store.insert(DecisionsSampler::new(
                HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
                DecisionsCredential::ApiKey("synthetic-key".into()),
                server.uri(),
                /*auth_context*/ None,
            )?);
        }
        fixture.score_tool(ToolName::plain("read_file")).await;
        fixture.assert_fails_closed("elevated_risk").await?;
        tokio::time::timeout(ASYNC_TEST_TIMEOUT, async {
            loop {
                let completed = {
                    let samples = metrics.0.lock().unwrap();
                    samples.iter().any(|sample| matches!(sample,
                        RecordedMetric::Counter(name, _, _) if name == CLASSIFICATION_METRIC))
                        && (!enabled || samples.iter().any(|sample| matches!(sample,
                            RecordedMetric::Counter(name, _, _) if name == "codex.guardian_v2.decisions_comparison")))
                };
                if completed {
                    break;
                }
                tokio::task::yield_now().await;
            }
        }).await?;
        assert!(server.received_requests().await.unwrap().is_empty());
        let samples = metrics.0.lock().unwrap();
        let outcomes = samples
            .iter()
            .filter(|sample| {
                matches!(sample,
            RecordedMetric::Counter(name, _, _) if name == "codex.guardian_v2.decisions_comparison")
            })
            .cloned()
            .collect::<Vec<_>>();
        let expected = expected_reason
            .into_iter()
            .map(|reason| {
                RecordedMetric::Counter(
                    "codex.guardian_v2.decisions_comparison".into(),
                    1,
                    vec![
                        ("outcome".into(), "skipped".into()),
                        ("reason".into(), reason.into()),
                    ],
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(outcomes, expected, "enabled={enabled}");
        assert!(!samples.iter().any(|sample| matches!(sample,
            RecordedMetric::Histogram(name, _, _)
                if name == "codex.guardian_v2.decisions_comparison.duration_ms")));
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn decisions_startup_uses_thread_runtime_and_staging_destination() -> anyhow::Result<()> {
    use base64::Engine;
    use codex_features::RuntimeAuthMode;
    use codex_history::ResponseItemEnvelope;
    use codex_login::AuthManager;
    use codex_login::CodexAuth;
    use codex_model_provider::ProviderAuthContext;
    use codex_runtime_agent_instance::AgentHostAuthorization;
    use codex_runtime_agent_instance::RuntimeAuthManager;
    use codex_runtime_agent_instance::RuntimeRegistrationMetadata;
    use codex_runtime_agent_instance::RuntimeRole;
    use core_test_support::runtime_auth::PROBE_SERVER;
    use core_test_support::runtime_auth::run_harness_through_proxy;
    use std::collections::BTreeSet;
    use wiremock::Mock;
    use wiremock::MockServer;
    use wiremock::ResponseTemplate;
    use wiremock::matchers::method;
    use wiremock::matchers::path;

    core_test_support::skip_if_no_network!(Ok(()));
    if let Ok(server) = std::env::var(PROBE_SERVER) {
        let fixture = GuardianFailureFixture::new().await?;
        let credentials =
            AuthManager::from_auth_for_testing(CodexAuth::from_api_key("thread-api-key"));
        let runtime = Arc::new(RuntimeAuthManager::new(
            Arc::clone(&credentials),
            AgentHostAuthorization::InstallationId {
                installation_id: uuid::Uuid::nil(),
            },
            RuntimeRegistrationMetadata {
                harness: Some("codex".into()),
                harness_version: None,
                role: RuntimeRole::Orchestrator,
            },
            "https://auth.api.openai.org/agent/register".parse()?,
        ));
        let request = crate::async_scorer::sampler::tests::sample_request("runtime-auth");
        let (input, _) = LunaSampler::finish_input(
            &request,
            LunaSampler::prepare_setup(&request),
            request
                .input
                .iter()
                .cloned()
                .map(ResponseItemEnvelope::new)
                .collect(),
        );
        for mode in [RuntimeAuthMode::DualHeader, RuntimeAuthMode::AssertionOnly] {
            let context = ProviderAuthContext {
                credentials: Arc::clone(&credentials),
                runtime_auth: Some((Arc::clone(&runtime), mode)),
            };
            let mut config = fixture.test.config.clone();
            config.model_provider_id = "openai".into();
            config.model_provider =
                ModelProviderInfo::create_openai_provider(/*base_url*/ None);
            config.chatgpt_base_url = "https://chatgpt-staging.com/backend-api".into();
            config.approvals_reviewer = ApprovalsReviewer::User;
            config.features.enable(Feature::GuardianApproval)?;
            config.features.enable(Feature::GuardianV2)?;
            config
                .features
                .enable(Feature::GuardianV2DecisionsComparison)?;
            let store = fixture.test.codex.thread_extension_data();
            store.insert(context.clone());
            fixture.registry.thread_lifecycle_contributors()[0]
                .on_thread_start(ThreadStartInput {
                    config: &config,
                    session_source: &SessionSource::Exec,
                    persistent_thread_state_available: false,
                    environments: &[],
                    mcp_resource_client: None,
                    extension_metrics: None,
                    session_store: &fixture.session_store,
                    thread_store: store,
                })
                .await;
            let sampler = store
                .get::<DecisionsSampler>()
                .expect("thread startup should initialize Decisions");
            for _ in 0..2 {
                let outcome = sampler
                    .start_conversation_comparison(&input)
                    .finish()
                    .await?
                    .0?;
                assert_eq!(outcome, "low");
            }
            let custom = Arc::new(DecisionsSampler::new(
                HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
                DecisionsCredential::ApiKey("comparison-key".into()),
                format!("{server}/custom-decisions"),
                Some(&context),
            )?);
            assert_eq!(
                custom
                    .start_conversation_comparison(&input)
                    .finish()
                    .await?
                    .0?,
                "low"
            );
        }
        return Ok(());
    }
    let server = MockServer::start().await;
    Mock::given(method("POST")).and(path("/agent/register"))
        .and(wiremock::matchers::body_partial_json(json!({
            "actor_auth": {"type": "platform_api_key", "credential": "thread-api-key"}
        })))
        .respond_with(ResponseTemplate::new(/*s*/ 200).set_body_json(json!({
            "runtime_agent_instance_id": "runtime-decisions", "agent_host_id": "host",
            "actor": {"kind": "platform", "user_id": "actor", "organization_id": "org", "project_id": null}
        }))).expect(/*r*/ 1).mount(&server).await;
    let response =
        json!({"answers": [{"type": "choice", "name": "guardian_risk", "choice": "low"}]});
    for (url, destination) in [
        (
            "/v1/decisions",
            "https://api.openai.org/v1/decisions-same-origin",
        ),
        (
            "/v1/decisions-same-origin",
            "https://api.openai.com/v1/decisions-final",
        ),
    ] {
        Mock::given(method("POST"))
            .and(path(url))
            .respond_with(ResponseTemplate::new(/*s*/ 307).insert_header("Location", destination))
            .expect(/*r*/ 4)
            .mount(&server)
            .await;
    }
    for (url, count) in [("/v1/decisions-final", 4), ("/custom-decisions", 2)] {
        Mock::given(method("POST"))
            .and(path(url))
            .respond_with(ResponseTemplate::new(/*s*/ 200).set_body_json(response.clone()))
            .expect(count)
            .mount(&server)
            .await;
    }
    run_harness_through_proxy(
        &server,
        "async_scorer::extension::tests::decisions_lifecycle_tests::decisions_startup_uses_thread_runtime_and_staging_destination",
        /*realtime_target*/ None,
    )
    .await?;
    let requests = server.received_requests().await.unwrap();
    let mut proofs = BTreeSet::new();
    let mut dual = 0;
    let mut assertion_only = 0;
    for request in requests.iter().filter(|r| r.url.path() == "/v1/decisions") {
        assert_eq!(request.headers["host"], "api.openai.org");
        let authorization = request.headers["authorization"].to_str()?;
        let proof = if let Some(proof) = request.headers.get("x-openai-agent-assertion") {
            assert_eq!(authorization, "Bearer thread-api-key");
            dual += 1;
            proof.to_str()?.to_owned()
        } else {
            assertion_only += 1;
            authorization
                .strip_prefix("AgentAssertion ")
                .unwrap()
                .to_owned()
        };
        let payload: serde_json::Value = serde_json::from_slice(
            &base64::engine::general_purpose::URL_SAFE_NO_PAD
                .decode(proof.split('.').nth(/*n*/ 2).unwrap())?,
        )?;
        assert_eq!(payload["runtime_agent_instance"], "runtime-decisions");
        assert!(
            proofs.insert(proof),
            "each classification needs a fresh assertion"
        );
    }
    assert_eq!((dual, assertion_only), (2, 2));
    for request in requests
        .iter()
        .filter(|r| r.url.path() == "/v1/decisions-same-origin")
    {
        assert_eq!(request.headers["host"], "api.openai.org");
        assert!(requests.iter().any(|initial| {
            initial.url.path() == "/v1/decisions"
                && request.headers.get("authorization") == initial.headers.get("authorization")
                && request.headers.get("x-openai-agent-assertion")
                    == initial.headers.get("x-openai-agent-assertion")
        }));
    }
    for request in requests
        .iter()
        .filter(|r| r.url.path() == "/v1/decisions-final")
    {
        assert_eq!(request.headers["host"], "api.openai.com");
        assert!(!request.headers.contains_key("authorization"));
        assert!(!request.headers.contains_key("x-openai-agent-assertion"));
    }
    for request in requests
        .iter()
        .filter(|r| r.url.path() == "/custom-decisions")
    {
        assert_eq!(request.headers["authorization"], "Bearer comparison-key");
        assert!(!request.headers.contains_key("x-openai-agent-assertion"));
    }
    server.verify().await;
    Ok(())
}
