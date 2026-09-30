use pretty_assertions::assert_eq;

use super::*;

#[test]
fn recent_invocations_refresh_recency_and_evict_old_skills() {
    let history = RecentSkillInvocations::default();
    for index in 0..=MAX_SHADOW_RESULTS {
        history.record(format!("skill-{index}"));
    }
    history.record("skill-1".to_string());

    let recent = history.snapshot();

    assert_eq!(MAX_SHADOW_RESULTS, recent.len());
    assert_eq!(Some("skill-1"), recent.first().map(String::as_str));
    assert_eq!(Some("skill-2"), recent.last().map(String::as_str));
    assert!(!recent.iter().any(|skill| skill == "skill-0"));
}

#[test]
fn rank_buckets_distinguish_results_above_twenty() {
    assert_eq!("11_20", rank_bucket(Some(20)));
    assert_eq!("21_50", rank_bucket(Some(21)));
    assert_eq!("21_50", rank_bucket(Some(50)));
    assert_eq!("miss", rank_bucket(Some(51)));
}

// These tests deliberately occupy the process-wide budget and must drain their
// workers before the next test starts.
static RANKING_TESTS: Semaphore = Semaphore::const_new(1);

#[tokio::test]
async fn turn_input_and_history_advance_while_ranking_is_blocked()
-> Result<(), Box<dyn std::error::Error>> {
    use std::collections::BTreeMap;

    use codex_extension_api::ExtensionData;
    use codex_extension_api::ExtensionRegistryBuilder;
    use codex_extension_api::SkillInvocationInput;
    use codex_extension_api::SkillInvocationKind;
    use codex_extension_api::ThreadStartInput;
    use codex_otel::MetricsConfig;
    use codex_protocol::protocol::SessionSource;
    use opentelemetry_sdk::metrics::InMemoryMetricExporter;
    use opentelemetry_sdk::metrics::data::AggregatedMetrics;
    use opentelemetry_sdk::metrics::data::MetricData;
    use opentelemetry_sdk::metrics::data::ScopeMetrics;

    use crate::SkillProviders;
    use crate::SkillsExtensionConfig;
    use crate::SkillsThreadState;
    use crate::catalog::SkillAuthority;
    use crate::catalog::SkillPackageId;
    use crate::catalog::SkillResourceId;
    use crate::install_with_providers_and_metrics;
    use crate::state::HostSkillsStepState;

    let _serial = RANKING_TESTS.acquire().await.unwrap();
    let slots = RANKING_SLOTS.acquire_many(2).await?;
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
        SkillProviders::new(),
        Some(metrics.clone()),
        SkillsExtensionConfig::clone,
    );
    let registry = builder.build();
    let config = SkillsExtensionConfig {
        include_instructions: false,
        max_context_tokens: None,
        bundled_skills_enabled: false,
        cloud_skill_enabled: false,
        shadow_selection_enabled: true,
    };
    let session_store = ExtensionData::new("session");
    let thread_store = ExtensionData::new("thread");
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

    for turn_id in ["first", "second"] {
        let turn_store = ExtensionData::new(turn_id);
        turn_store.insert(HostSkillsStepState(SkillCatalog {
            entries: ["x", "hidden", "selected"]
                .into_iter()
                .map(|name| {
                    let mut entry = SkillCatalogEntry::new(
                        SkillPackageId(name.to_string()),
                        SkillAuthority::new(SkillSourceKind::Host, "host"),
                        name,
                        "zzzz",
                        SkillResourceId::new(format!("{name}/SKILL.md")),
                    );
                    entry.prompt_visible = name != "hidden";
                    entry
                })
                .collect(),
            warnings: Vec::new(),
        }));
        let fragments = tokio::time::timeout(
            Duration::from_secs(10),
            registry.turn_input_contributors()[0].contribute(
                TurnInputContext {
                    turn_id: turn_id.to_string(),
                    user_input: vec![UserInput::Skill {
                        name: "selected".to_string(),
                        path: "selected/SKILL.md".into(),
                    }],
                    environments: Vec::new(),
                },
                /*extension_metrics*/ None,
                &session_store,
                &thread_store,
                &turn_store,
            ),
        )
        .await?;
        assert!(fragments.is_empty());
        tokio::task::yield_now().await;
        // Synchronous ranking would emit run metrics despite the occupied budget;
        // awaiting admission in contribute would instead time out above.
        assert!(
            metrics
                .snapshot()?
                .scope_metrics()
                .all(|scope| { scope.metrics().all(|metric| metric.name() != RUN_METRIC) })
        );
        for resource in [
            "x/SKILL.md",
            "x\\SKILL.md",
            "hidden/SKILL.md",
            "selected/SKILL.md",
        ] {
            registry.skill_invocation_contributors()[0]
                .on_skill_invocation(SkillInvocationInput {
                    session_store: &session_store,
                    thread_store: &thread_store,
                    turn_store: &turn_store,
                    turn_id,
                    skill_resource: resource,
                    kind: SkillInvocationKind::Implicit,
                })
                .await;
        }
        assert_eq!(
            vec!["x/SKILL.md"],
            thread_store
                .get::<SkillsThreadState>()
                .unwrap()
                .recent_skill_invocations
                .snapshot()
        );
    }
    // Both observers disappear before either ranking starts. Their queued unique
    // invocations must still be scored against their own turn-start histories.
    drop(thread_store);
    drop(slots);

    let mut runs = BTreeMap::new();
    let mut hits = BTreeMap::new();
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            // snapshot() exports deltas, so retain observations across polls.
            let snapshot = metrics.snapshot().unwrap();
            for metric in snapshot.scope_metrics().flat_map(ScopeMetrics::metrics) {
                let AggregatedMetrics::U64(MetricData::Sum(sum)) = metric.data() else {
                    continue;
                };
                for point in sum.data_points() {
                    let attribute = |key: &str| {
                        point
                            .attributes()
                            .find(|attribute| attribute.key.as_str() == key)
                            .unwrap()
                            .value
                            .as_str()
                            .to_string()
                    };
                    match metric.name() {
                        RUN_METRIC => {
                            *runs.entry(attribute("method")).or_insert(0) += point.value()
                        }
                        "codex.skills.shadow_selection.invocation" => {
                            assert_eq!("ascii_latin", attribute("query_script"));
                            *hits
                                .entry((attribute("method"), attribute("hit"), attribute("rank")))
                                .or_insert(0) += point.value();
                        }
                        _ => {}
                    }
                }
            }
            if hits.values().sum::<u64>() >= 24 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await?;
    let mut expected_runs = BTreeMap::new();
    let mut expected_hits = BTreeMap::new();
    for (method, uses_history) in [
        ("weighted_lexical_v1", false),
        ("fielded_bm25_v1", false),
        ("character_ngram_v1", false),
        ("multi_query_lexical_v1", false),
        ("rrf_lexical_char_v1", false),
        ("routing_card_exact_v1", false),
        ("character_routing_card_v1", false),
        ("lru_v1", true),
        ("lru_plus_lexical_v1", true),
        ("lru_plus_character_routing_v1", true),
        ("lru_plus_lexical_character_routing_v1", true),
        ("task_context_fusion_v1", true),
    ] {
        expected_runs.insert(method.to_string(), 2);
        expected_hits.insert(
            (method.to_string(), "false".to_string(), "miss".to_string()),
            if uses_history { 1 } else { 2 },
        );
        if uses_history {
            expected_hits.insert((method.to_string(), "true".to_string(), "1".to_string()), 1);
        }
    }
    assert_eq!(expected_runs, runs);
    assert_eq!(expected_hits, hits);
    Ok(())
}

/// Hold an actual blocking worker until the test releases it. The timeout keeps a
/// failing assertion from leaving runtime shutdown stuck on the test worker.
struct BlockedSelector {
    started: tokio::sync::mpsc::UnboundedSender<()>,
    release: Mutex<std::sync::mpsc::Receiver<()>>,
}

impl CheapSkillSelector for BlockedSelector {
    fn method(&self) -> &'static str {
        "blocked_test"
    }

    fn select(
        &self,
        _query: &str,
        _documents: &[SkillSelectionDocument<'_>],
        _limit: usize,
    ) -> CheapSkillSelection {
        self.started.send(()).unwrap();
        self.release
            .lock()
            .unwrap()
            .recv_timeout(Duration::from_secs(10))
            .unwrap();
        CheapSkillSelection::default()
    }
}

#[tokio::test]
async fn ranking_budget_is_shared_and_held_until_blocking_work_finishes() {
    let _serial = RANKING_TESTS.acquire().await.unwrap();
    let mut workers = Vec::new();
    for index in 0..3 {
        let (started, mut starts) = tokio::sync::mpsc::unbounded_channel();
        let (release, releases) = std::sync::mpsc::channel();
        // Separate experiment instances must share the same process-wide budget.
        let experiment = Arc::new(ShadowSelectionExperiment {
            selectors: vec![Box::new(BlockedSelector {
                started,
                release: Mutex::new(releases),
            })],
            metrics_client: None,
        });
        let query = ShadowQuery {
            text: "test".to_string(),
            truncated: false,
        };
        let input = ShadowSelectionInput {
            catalog: SkillCatalog::default(),
            host_snapshot: None,
            task_snapshot: TaskContextSnapshot {
                query: query.clone(),
                recent_skills: Vec::new(),
            },
            query,
            eligible_ids: HashSet::new(),
            recent_skill_resources: Vec::new(),
        };
        let (invocations, pending) = tokio::sync::mpsc::unbounded_channel();
        drop(invocations);
        let mut worker = Box::pin(experiment.evaluate(input, pending, tracing::Span::none()));
        // Polling to the first await deterministically attempts admission.
        assert!(futures::poll!(worker.as_mut()).is_pending());
        if index < 2 {
            assert_eq!(
                Some(()),
                tokio::time::timeout(Duration::from_secs(10), starts.recv())
                    .await
                    .unwrap()
            );
            assert_eq!(1 - index, RANKING_SLOTS.available_permits());
        } else {
            assert!(starts.try_recv().is_err());
        }
        workers.push((tokio::spawn(worker), release, starts));
    }
    let (first, release_first, _) = workers.remove(0);
    release_first.send(()).unwrap();
    first.await.unwrap();
    let (second, release_second, _) = workers.remove(0);
    let (third, release_third, mut third_starts) = workers.remove(0);
    assert_eq!(
        Some(()),
        tokio::time::timeout(Duration::from_secs(10), third_starts.recv())
            .await
            .unwrap()
    );
    assert_eq!(0, RANKING_SLOTS.available_permits());

    // Cancelling evaluation cannot free a slot still occupied by blocking work.
    third.abort();
    assert!(third.await.unwrap_err().is_cancelled());
    assert_eq!(0, RANKING_SLOTS.available_permits());
    release_second.send(()).unwrap();
    release_third.send(()).unwrap();
    second.await.unwrap();
    let _all_returned =
        tokio::time::timeout(Duration::from_secs(10), RANKING_SLOTS.acquire_many(2))
            .await
            .unwrap()
            .unwrap();
}
