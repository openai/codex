//! Exercises config reloads while preparation is waiting on executor discovery.

use super::*;
use pretty_assertions::assert_eq;
use tokio::sync::Notify;

struct PausedExecutor {
    entered: Notify,
    resume: Notify,
    queries: Mutex<Vec<(bool, bool)>>,
}

impl SkillProvider for PausedExecutor {
    fn list(&self, query: SkillListQuery) -> SkillProviderFuture<'_, SkillCatalog> {
        Box::pin(async move {
            self.queries
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push((query.include_bundled_skills, query.include_cloud_skills));
            self.entered.notify_one();
            self.resume.notified().await;
            Ok(SkillCatalog::default())
        })
    }

    fn read<'a>(
        &'a self,
        _request: SkillReadRequest<'a>,
    ) -> SkillProviderFuture<'a, SkillReadResult> {
        Box::pin(async { panic!("executor reads are not expected") })
    }

    fn search(&self, _request: SkillSearchRequest) -> SkillProviderFuture<'_, SkillSearchResult> {
        Box::pin(async { panic!("executor searches are not expected") })
    }
}

#[tokio::test]
async fn config_reload_during_preparation_applies_to_next_step_but_reads_still_check_live_policy()
-> TestResult {
    let executor = Arc::new(PausedExecutor {
        entered: Notify::new(),
        resume: Notify::new(),
        queries: Mutex::default(),
    });
    let cloud = test_entry(
        SkillSourceKind::Cloud,
        "codex_apps",
        "cloud/demo",
        "skill://cloud/demo/SKILL.md",
    );
    let cloud_reads = Arc::default();
    let providers = SkillProviders::new()
        .with_executor_provider(executor.clone())
        .with_cloud_provider(Arc::new(StaticSkillProvider {
            catalog: SkillCatalog {
                entries: vec![cloud],
                ..Default::default()
            },
            read_requests: Arc::clone(&cloud_reads),
            list_calls: None,
            fail_first_list: false,
        }));
    let mut builder = ExtensionRegistryBuilder::new();
    install_with_providers(&mut builder, providers, skills_extension_config);
    let registry = builder.build();
    let session = ExtensionData::new("session");
    let thread = ExtensionData::new("thread");
    let turn = ExtensionData::new("turn");
    let config = default_config();
    let updated = TestConfig {
        cloud_skill_enabled: false,
        bundled_skills_enabled: false,
        ..config.clone()
    };
    registry.thread_lifecycle_contributors()[0]
        .on_thread_start(ThreadStartInput {
            config: &config,
            session_source: &SessionSource::Cli,
            persistent_thread_state_available: true,
            environments: &[],
            mcp_resource_client: None,
            extension_metrics: None,
            session_store: &session,
            thread_store: &thread,
        })
        .await;
    start_registered_turn(&registry, &session, &thread, "turn").await;
    let roots = vec![SelectedCapabilityRoot {
        id: "skills".into(),
        location: CapabilityRootLocation::Environment {
            environment_id: "env-1".into(),
            path: PathUri::parse("file:///skills")?,
        },
    }];
    let step = ExtensionData::new("step");
    let next_step = ExtensionData::new("next-step");
    let model_info = catalog_model_info();
    for (store, expected_enabled) in [(&step, true), (&next_step, false)] {
        let (sections, ()) = tokio::join!(
            prepared_world_state(
                &registry,
                &roots,
                WorldStateContributionInput {
                    previous_world_state: None,
                    step_store: store,
                    model_info: &model_info,
                    thread_id: codex_protocol::ThreadId::new(),
                    turn_id: "turn",
                    environments: &[],
                    extension_metrics: None,
                    session_store: &session,
                    thread_store: &thread,
                    turn_store: &turn,
                }
            ),
            async {
                executor.entered.notified().await;
                registry.config_contributors()[0]
                    .on_config_changed(&session, &thread, &config, &updated);
                executor.resume.notify_one();
            }
        );
        let rendered = sections
            .iter()
            .filter_map(|section| section.render_diff(PreviousWorldStateSection::Absent).1)
            .map(|fragment| fragment.body().clone().expect_text())
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(
            rendered.contains("cloud/demo"),
            expected_enabled,
            "{rendered}"
        );
    }
    assert_eq!(
        *executor.queries.lock().unwrap(),
        vec![(true, true), (false, false)]
    );
    let tools = registry.tool_contributors()[0].tools_for_step(&session, &thread, &step);
    let result = merge_catalog_tests::call_tool(
        &tools,
        "read",
        serde_json::json!({"package": "cloud/demo"}),
    )
    .await;
    assert_matches!(result, Err(FunctionCallError::RespondToModel(message)) if message == "failed to read skill resource");
    assert!(read_request_keys(&cloud_reads).is_empty());
    Ok(())
}
