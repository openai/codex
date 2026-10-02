//! Decisions outcomes cannot affect the real baseline score/publication path.
use super::*;
use crate::async_scorer::decisions::DecisionsSampler;
use codex_http_client::HttpClientBuilder;
use pretty_assertions::assert_eq;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn decisions_disagreement_failures_and_disabled_flag_preserve_baseline() -> Result<()> {
    skip_if_no_network!(Ok(()));
    for (case, enabled, status, choice, baseline, expected_risk) in [
        ("disabled", false, 200, "high", "low", 0.0),
        ("not_initialized", true, 200, "high", "low", 0.0),
        ("cannot_escalate", true, 200, "high", "low", 0.0),
        ("cannot_lower", true, 200, "low", "high", 1.0),
        ("candidate_failure", true, 403, "low", "low", 0.0),
        ("baseline_failure", true, 200, "low", "invalid", 1.0),
    ] {
        let server = responses::start_mock_server().await;
        let fixture = GuardianFailureFixture::with_config(&format!(
            "[features]\nguardianv2_decisions_comparison = {enabled}\n"
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
                HttpClientBuilder::new().build_direct()?,
                "synthetic-key".into(),
                decisions_server.uri(),
            )?);
        } else {
            assert!(store.get::<DecisionsSampler>().is_none());
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
                let policy = baseline_input
                    .iter()
                    .find(|item| item["role"] == "developer" && item["type"] == "message")
                    .unwrap();
                assert_eq!(
                    body["questions"][0]["instructions"],
                    policy["content"][0]["text"]
                );
                let evidence = baseline_input
                    .iter()
                    .filter(|item| item["role"] == "user")
                    .map(|item| json!({"role": item["role"], "content":item["content"]}))
                    .collect::<Vec<_>>();
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
            if case == "not_initialized" {
                let comparisons = metrics.0.lock().unwrap().iter().filter(|sample| matches!(sample,
                    RecordedMetric::Counter(name, _, _) if name == "codex.guardian_v2.decisions_comparison.comparison"))
                    .cloned().collect::<Vec<_>>();
                assert_eq!(
                    comparisons,
                    vec![RecordedMetric::Counter(
                        "codex.guardian_v2.decisions_comparison.comparison".into(),
                        1,
                        vec![
                            ("comparison".into(), "unavailable".into()),
                            ("responses".into(), "low".into()),
                            ("decisions".into(), "unavailable".into()),
                        ],
                    )]
                );
            }
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
    let fixture = GuardianFailureFixture::new().await?;
    let server = responses::start_mock_server().await;
    let store = fixture.test.codex.thread_extension_data();
    let mut config = fixture.test.config.clone();
    config.features.enable(Feature::GuardianApproval)?;
    config.features.enable(Feature::GuardianV2)?;
    config
        .features
        .enable(Feature::GuardianV2DecisionsComparison)?;
    let metrics = Arc::new(RecordingMetrics::default());
    fixture.registry.thread_lifecycle_contributors()[0]
        .on_thread_start(ThreadStartInput {
            config: &config,
            session_source: &SessionSource::Exec,
            persistent_thread_state_available: false,
            environments: &[],
            mcp_resource_client: None,
            extension_metrics: Some(metrics.clone()),
            session_store: &fixture.session_store,
            thread_store: store,
        })
        .await;
    let mut sampler_config = crate::async_scorer::sampler::tests::sampler_config(server.uri());
    sampler_config.max_input_tokens = 256;
    store.insert(LunaSampler::new(sampler_config));
    store.insert(DecisionsSampler::new(
        HttpClientBuilder::new().build_direct()?,
        "synthetic-key".into(),
        server.uri(),
    )?);
    metrics.0.lock().unwrap().clear();
    fixture.score_tool(ToolName::plain("read_file")).await;
    fixture.assert_fails_closed("elevated_risk").await?;
    tokio::time::timeout(ASYNC_TEST_TIMEOUT, async {
        loop {
            if metrics.0.lock().unwrap().iter().any(|sample| matches!(sample,
                RecordedMetric::Counter(name, _, _) if name == "codex.guardian_v2.decisions_comparison")) {
                break;
            }
            tokio::task::yield_now().await;
        }
    }).await?;
    assert!(server.received_requests().await.unwrap().is_empty());
    assert!(metrics.0.lock().unwrap().contains(&RecordedMetric::Counter(
        "codex.guardian_v2.decisions_comparison".into(),
        1,
        vec![
            ("outcome".into(), "skipped".into()),
            ("reason".into(), "input_too_large".into())
        ],
    )));
    Ok(())
}
