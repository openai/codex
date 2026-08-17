use std::collections::BTreeMap;

use pretty_assertions::assert_eq;

use super::*;

struct FailedReads(StaticSkillProvider);

impl SkillProvider for FailedReads {
    fn list(&self, query: SkillListQuery) -> SkillProviderFuture<'_, SkillCatalog> {
        self.0.list(query)
    }

    fn read(&self, _request: SkillReadRequest) -> SkillProviderFuture<'_, SkillReadResult> {
        Box::pin(async { Err(SkillProviderError::new("read unavailable")) })
    }

    fn search(&self, request: SkillSearchRequest) -> SkillProviderFuture<'_, SkillSearchResult> {
        self.0.search(request)
    }
}

fn text_input(text: &str) -> Vec<UserInput> {
    vec![UserInput::Text {
        text: text.to_string(),
        text_elements: Vec::new(),
    }]
}

fn explicit_input() -> Vec<UserInput> {
    vec![UserInput::Skill {
        name: "x".to_string(),
        path: PathBuf::from("x/SKILL.md"),
    }]
}

#[tokio::test]
async fn task_context_recovers_prior_requests_and_explicit_intent_without_changing_controls()
-> TestResult {
    let mut opaque = test_entry(SkillSourceKind::Host, "host", "host/x", "x/SKILL.md");
    opaque.description = "zzzz".to_string();
    let provider = Arc::new(FailedReads(StaticSkillProvider {
        catalog: SkillCatalog {
            entries: vec![
                test_entry(
                    SkillSourceKind::Host,
                    "host",
                    "host/lint-fix",
                    "lint-fix/SKILL.md",
                ),
                opaque,
            ],
            warnings: Vec::new(),
        },
        read_requests: Arc::new(Mutex::new(Vec::new())),
        list_calls: None,
        fail_first_list: false,
    }));
    let metrics = MetricsClient::new(
        MetricsConfig::in_memory(
            "test",
            "codex-skills-extension",
            env!("CARGO_PKG_VERSION"),
            InMemoryMetricExporter::default(),
        )
        .with_runtime_reader(),
    )?;
    let mut builder = ExtensionRegistryBuilder::new();
    install_with_providers_and_metrics(
        &mut builder,
        SkillProviders::new().with_host_provider(provider),
        Some(metrics.clone()),
        skills_extension_config,
    );
    let registry = builder.build();
    let session_store = ExtensionData::new("session");
    let mut config = default_config();
    config.include_instructions = false;
    config.shadow_selection_enabled = true;
    for (thread_id, turns, resource) in [
        (
            "prior-request",
            vec![
                ("a1", text_input("Fix lint errors.")),
                ("a2", text_input("continue")),
            ],
            "lint-fix/SKILL.md",
        ),
        (
            "explicit-intent",
            vec![("b1", explicit_input()), ("b2", text_input("continue"))],
            "x/SKILL.md",
        ),
        (
            "same-turn",
            vec![("c1", explicit_input()), ("c1", text_input("continue"))],
            "x/SKILL.md",
        ),
        (
            "cold-thread",
            vec![("d1", text_input("continue"))],
            "x/SKILL.md",
        ),
    ] {
        let thread_store = ExtensionData::new(thread_id);
        registry.thread_lifecycle_contributors()[0]
            .on_thread_start(ThreadStartInput {
                config: &config,
                session_source: &SessionSource::Cli,
                persistent_thread_state_available: true,
                environments: &[],
                mcp_resource_client: None,
                extension_metrics: None,
                session_store: &session_store,
                thread_store: &thread_store,
            })
            .await;
        let observed_turn = turns.last().ok_or("test needs a turn")?.0;
        for (turn_id, user_input) in turns {
            let fragments = registry.turn_input_contributors()[0]
                .contribute(
                    TurnInputContext {
                        turn_id: turn_id.to_string(),
                        user_input,
                        environments: Vec::new(),
                    },
                    /*extension_metrics*/ None,
                    &session_store,
                    &thread_store,
                    &ExtensionData::new(turn_id),
                )
                .await;
            assert!(fragments.is_empty());
        }
        registry.skill_invocation_contributors()[0]
            .on_skill_invocation(SkillInvocationInput {
                session_store: &session_store,
                thread_store: &thread_store,
                turn_store: &ExtensionData::new(observed_turn),
                turn_id: observed_turn,
                skill_resource: resource,
                kind: SkillInvocationKind::Implicit,
            })
            .await;
    }

    let controls = [
        "lru_v1",
        "weighted_lexical_v1",
        "lru_plus_lexical_v1",
        "lru_plus_character_routing_v1",
        "lru_plus_lexical_character_routing_v1",
    ];
    let snapshot = metrics.snapshot()?;
    let metric = snapshot
        .scope_metrics()
        .flat_map(opentelemetry_sdk::metrics::data::ScopeMetrics::metrics)
        .find(|metric| metric.name() == "codex.skills.shadow_selection.invocation")
        .ok_or("shadow invocation metric should exist")?;
    let AggregatedMetrics::U64(MetricData::Sum(sum)) = metric.data() else {
        panic!("unexpected shadow metric: {:?}", metric.data());
    };
    let actual = sum
        .data_points()
        .filter_map(|point| {
            let attribute = |key| {
                point
                    .attributes()
                    .find(|value| value.key.as_str() == key)
                    .map(|value| value.value.as_str().to_string())
            };
            let method = attribute("method")?;
            (controls.contains(&method.as_str()) || method == "task_context_fusion_v1")
                .then(|| ((method, attribute("hit").expect("hit tag")), point.value()))
        })
        .fold(BTreeMap::new(), |mut totals, (key, count)| {
            *totals.entry(key).or_insert(0) += count;
            totals
        });
    let mut expected = controls
        .into_iter()
        .map(|method| ((method.to_string(), "false".to_string()), 4))
        .collect::<BTreeMap<_, _>>();
    expected.insert(
        ("task_context_fusion_v1".to_string(), "false".to_string()),
        /*value*/ 2,
    );
    expected.insert(
        ("task_context_fusion_v1".to_string(), "true".to_string()),
        /*value*/ 2,
    );
    assert_eq!(expected, actual);
    Ok(())
}
