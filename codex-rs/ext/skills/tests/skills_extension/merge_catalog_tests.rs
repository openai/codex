//! Exercises V2 catalog merging, retained read routes, and unchanged legacy rendering.

use super::*;
use codex_core_plugin_common::PluginIdentity;
use codex_exec_server::DiscoverV2CapabilitiesResponse;
use codex_exec_server::ExecutorCapabilityDiscoveryV2Snapshot;
use codex_extension_api::SelectedPluginIdentity;
use codex_extension_api::SelectedPluginSnapshot;
use codex_extension_api::ToolEnvironment;
use codex_extension_api::ToolExecutor;
use codex_skills_extension::detect_implicit_skill_invocation;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;

fn cloud_skill(name: &str) -> SkillCatalogEntry {
    let package = format!("skill://Plugin_123/{name}");
    SkillCatalogEntry::new(
        SkillPackageId(package.clone()),
        SkillAuthority::new(SkillSourceKind::Cloud, "codex_apps"),
        format!("deploy:{name}"),
        format!("Cloud {name} description"),
        SkillResourceId::new(format!("{package}/SKILL.md")),
    )
    .with_remote_plugin_id(Some("Plugin_123".to_string()))
}

fn installed_skill(
    root: &str,
    name: &str,
) -> Result<SkillCatalogEntry, Box<dyn std::error::Error>> {
    let package = format!("skill://{root}/{name}");
    Ok(SkillCatalogEntry::new(
        SkillPackageId(package.clone()),
        SkillAuthority::new(SkillSourceKind::Executor, root),
        format!("deploy:{name}"),
        format!("Installed {root} description"),
        SkillResourceId::environment(
            format!("{package}/SKILL.md"),
            "env-1",
            PathUri::parse(&format!("file:///skills/{root}/SKILL.md"))?,
        ),
    )
    .with_display_path(format!("{package}/SKILL.md"))
    .with_remote_plugin_id(Some("Plugin_123".to_string())))
}

fn provider(entries: Vec<SkillCatalogEntry>) -> StaticSkillProvider {
    StaticSkillProvider {
        catalog: SkillCatalog {
            is_partial: false,
            entries,
            warnings: Vec::new(),
        },
        read_requests: Arc::default(),
        list_calls: Some(Arc::default()),
        fail_first_list: false,
    }
}

struct MergeHarness {
    registry: codex_extension_api::ExtensionRegistry<TestConfig>,
    session_store: ExtensionData,
    thread_store: ExtensionData,
    turn_store: ExtensionData,
    step_store: ExtensionData,
    roots: Vec<SelectedCapabilityRoot>,
    snapshots: Option<Vec<ExecutorCapabilityDiscoveryV2Snapshot>>,
    discovery: Option<ExecutorCapabilityDiscoverySnapshot>,
}

impl MergeHarness {
    async fn new(
        executor: StaticSkillProvider,
        cloud: StaticSkillProvider,
        config: TestConfig,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Self::with_metrics(executor, cloud, config, /*metrics*/ None).await
    }

    async fn with_metrics(
        executor: StaticSkillProvider,
        cloud: StaticSkillProvider,
        config: TestConfig,
        metrics: Option<MetricsClient>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let providers = SkillProviders::new()
            .with_executor_provider(Arc::new(executor))
            .with_cloud_provider(Arc::new(cloud));
        let mut builder = ExtensionRegistryBuilder::new();
        install_with_providers_and_metrics(
            &mut builder,
            providers,
            metrics,
            skills_extension_config,
        );
        let harness = Self {
            registry: builder.build(),
            session_store: ExtensionData::new("session"),
            thread_store: ExtensionData::new("thread"),
            turn_store: ExtensionData::new("turn-1"),
            step_store: ExtensionData::new("step-1"),
            discovery: None,
            snapshots: Some(vec![ExecutorCapabilityDiscoveryV2Snapshot {
                environment_id: "env-1".to_string(),
                result: Ok(Arc::new(DiscoverV2CapabilitiesResponse::default())),
            }]),
            roots: vec![SelectedCapabilityRoot {
                id: "skills".to_string(),
                location: CapabilityRootLocation::Environment {
                    environment_id: "env-1".to_string(),
                    path: PathUri::parse("file:///skills")?,
                },
            }],
        };
        harness.registry.thread_lifecycle_contributors()[0]
            .on_thread_start(ThreadStartInput {
                config: &config,
                session_source: &SessionSource::Cli,
                persistent_thread_state_available: true,
                environments: &[],
                mcp_resource_client: None,
                extension_metrics: None,
                session_store: &harness.session_store,
                thread_store: &harness.thread_store,
            })
            .await;
        start_registered_turn(
            &harness.registry,
            &harness.session_store,
            &harness.thread_store,
            "turn-1",
        )
        .await;
        Ok(harness)
    }

    fn tools(
        &self,
        step: &ExtensionData,
    ) -> Vec<Arc<dyn for<'call> ToolExecutor<ToolCall<'call>>>> {
        if let Some(snapshots) = &self.snapshots {
            step.insert(snapshots.clone());
        }
        if let Some(discovery) = &self.discovery {
            step.insert(discovery.clone());
        }
        self.registry.tool_contributors()[0].tools_for_step(
            &self.session_store,
            &self.thread_store,
            step,
        )
    }

    async fn render(&self) -> Result<String, Box<dyn std::error::Error>> {
        self.render_step(&self.step_store).await
    }

    async fn render_step(
        &self,
        step_store: &ExtensionData,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let turn_store = &self.turn_store;
        if let Some(plugins) = turn_store.get::<codex_extension_api::SelectedPluginSnapshot>() {
            step_store.insert(plugins.as_ref().clone());
        }
        let sections = self.registry.context_contributors()[0]
            .contribute_world_state(WorldStateContributionInput {
                previous_world_state: None,
                step_store,
                model_info: &catalog_model_info(),
                thread_id: codex_protocol::ThreadId::new(),
                turn_id: turn_store.level_id(),
                environments: &[],
                ready_selected_capability_roots: &self.roots,
                executor_capability_discovery: self.discovery.as_ref(),
                executor_capability_discovery_v2: self.snapshots.as_deref(),
                extension_metrics: None,
                session_store: &self.session_store,
                thread_store: &self.thread_store,
                turn_store,
            })
            .await;
        Ok(sections
            .iter()
            .filter_map(|section| section.render_diff(PreviousWorldStateSection::Absent).1)
            .map(|fragment| fragment.body().clone().expect_text())
            .collect::<Vec<_>>()
            .join("\n"))
    }
}

async fn call_tool(
    tools: &[Arc<dyn for<'call> ToolExecutor<ToolCall<'call>>>],
    name: &str,
    arguments: Value,
) -> Result<Value, FunctionCallError> {
    let tool = tools
        .iter()
        .find(|tool| tool.tool_name().name == name)
        .ok_or_else(|| FunctionCallError::RespondToModel("missing skill tool".to_string()))?;
    let payload = ToolPayload::Function {
        arguments: arguments.to_string(),
    };
    let access = FileSystemEnvironmentAccessor::unrestricted(&LOCAL_FS);
    let result = tool
        .handle(ToolCall {
            turn_id: "turn-1".to_string(),
            call_id: "skill-test".to_string(),
            tool_name: tool.tool_name(),
            model: "test".to_string(),
            codex_turn_metadata: None,
            truncation_policy: TruncationPolicy::Bytes(10_000),
            source: ToolCallSource::Direct,
            conversation_history: ConversationHistory::default(),
            turn_item_emitter: Arc::new(NoopTurnItemEmitter),
            attachment_store: Arc::new(InlineAttachmentStore),
            ephemeral: false,
            environments: vec![ToolEnvironment::new(
                "env-1".to_string(),
                PathUri::parse("file:///installed")
                    .map_err(|error| FunctionCallError::RespondToModel(error.to_string()))?,
                &access,
            )],
            payload: payload.clone(),
        })
        .await?;
    if name == "list" {
        assert_eq!(
            result.contains_external_context(),
            arguments["authority"]["kind"] == "cloud"
        );
    }
    result
        .post_tool_use_response("skill-test", &payload)
        .ok_or_else(|| FunctionCallError::RespondToModel("missing skill response".to_string()))
}

fn selected_plugins(roots: &[Option<&str>], disabled: &[&str]) -> SelectedPluginSnapshot {
    SelectedPluginSnapshot {
        plugins: roots
            .iter()
            .map(|root| SelectedPluginIdentity {
                selected_root_id: (*root).map(str::to_string),
                plugin_id: PluginIdentity::Remote {
                    remote_plugin_id: "Plugin_123".to_string(),
                },
            })
            .collect(),
        disabled_plugin_ids: disabled.iter().map(|id| (*id).to_string()).collect(),
        ..Default::default()
    }
}

#[tokio::test]
async fn v2_merged_skills_preserve_cloud_metadata_and_accepted_plugin_routes() -> TestResult {
    let cloud = cloud_skill("work");
    let mut installed = Vec::new();
    for (id, name) in [
        ("matching", "work"),
        ("removed", "removed"),
        ("unlisted", "work"),
        ("denied", "work"),
    ] {
        installed.push(installed_skill(id, name)?.with_plugin_id(Some(id.to_string())));
    }
    let local = installed_skill("local", "local")?
        .with_remote_plugin_id(/*remote_plugin_id*/ None)
        .with_plugin_id(Some("local".to_string()));
    installed.push(local.clone());
    let standalone_cloud =
        cloud_skill("cloud").with_remote_plugin_id(/*remote_plugin_id*/ None);
    let standalone_executor = installed_skill("standalone", "executor")?
        .with_remote_plugin_id(/*remote_plugin_id*/ None);
    installed.push(standalone_executor.clone());
    let mut executor_provider = provider(installed.clone());
    executor_provider.catalog.warnings =
        vec!["Executor A warning".into(), "Executor B warning".into()];
    let mut cloud_provider = provider(vec![cloud.clone(), standalone_cloud.clone()]);
    cloud_provider.catalog.warnings = vec!["Cloud warning".into()];
    let harness = MergeHarness::new(executor_provider, cloud_provider, default_config()).await?;
    let mut selection = selected_plugins(
        &[
            None,
            Some("env-1:matching"),
            Some("env-1:removed"),
            Some("env-1:denied"),
            Some("other:unlisted"),
        ],
        &["denied"],
    );
    selection.plugins.push(SelectedPluginIdentity {
        selected_root_id: Some("env-1:local".to_string()),
        plugin_id: PluginIdentity::Local {
            plugin_id: "local".to_string(),
        },
    });
    harness.turn_store.insert(selection);
    let tools = harness.tools(&harness.step_store);
    let rendered = harness.render().await?;
    assert_eq!(rendered.matches("- deploy:work:").count(), 1);
    assert!(rendered.contains("Cloud work description"));
    assert!(!rendered.contains("Installed matching description"));
    assert!(rendered.contains("Installed local description"));
    assert!(rendered.contains("- deploy:cloud:"));
    assert!(rendered.contains("- deploy:executor:"));
    for (authority, expected_packages, warnings) in [
        (
            "cloud",
            vec![&standalone_cloud.id, &cloud.id],
            json!(["Cloud warning"]),
        ),
        (
            "executor",
            vec![&local.id, &standalone_executor.id],
            json!(["Executor A warning", "Executor B warning"]),
        ),
    ] {
        let output = call_tool(&tools, "list", json!({"authority": {"kind": authority}})).await?;
        let packages = output["skills"]
            .as_array()
            .unwrap()
            .iter()
            .map(|skill| skill["package"].clone())
            .collect::<Vec<_>>();
        assert_eq!(
            packages,
            expected_packages
                .iter()
                .map(|package| json!(package.0))
                .collect::<Vec<_>>()
        );
        assert_eq!(output["warnings"], warnings);
        assert!(
            output["skills"]
                .as_array()
                .unwrap()
                .iter()
                .all(|skill| skill["authority"]["kind"] == authority)
        );
    }
    call_tool(&tools, "read", json!({"package": cloud.id.0})).await?;
    // Unselected, disabled, and Cloud-absent installations cannot bypass the merge.
    for entry in &installed {
        let allowed = matches!(
            entry.authority.id.as_str(),
            "matching" | "local" | "standalone"
        );
        let result = call_tool(&tools, "read", json!({"package": entry.id.0})).await;
        if allowed {
            result?;
        } else {
            assert_matches!(result, Err(FunctionCallError::RespondToModel(message)) if message == "skill package is not available");
        }
    }
    Ok(())
}

#[tokio::test]
async fn v2_only_complete_cloud_catalog_excludes_unmatched_remote_skills() -> TestResult {
    let mut unavailable = provider(vec![cloud_skill("work")]);
    unavailable.fail_first_list = true;
    let mut partial = provider(Vec::new());
    partial.catalog.is_partial = true;
    for (cloud, config, keeps_remote) in [
        (
            provider(vec![cloud_skill("work")]),
            TestConfig {
                cloud_skill_enabled: false,
                ..default_config()
            },
            true,
        ),
        (unavailable, default_config(), true),
        (partial, default_config(), true),
        (provider(Vec::new()), default_config(), false),
    ] {
        let harness = MergeHarness::new(
            provider(vec![
                installed_skill("matching", "work")?.with_plugin_id(Some("matching".to_string())),
                installed_skill("standalone", "local")?
                    .with_remote_plugin_id(/*remote_plugin_id*/ None),
            ]),
            cloud,
            config,
        )
        .await?;
        harness
            .turn_store
            .insert(selected_plugins(&[None, Some("env-1:matching")], &[]));
        let rendered = harness.render().await?;
        assert_eq!(rendered.contains("- deploy:work:"), keeps_remote);
        assert!(rendered.contains("- deploy:local:"));
        assert!(!rendered.contains("Cloud work description"));
    }
    Ok(())
}

#[tokio::test]
async fn v2_disabled_remote_plugin_blocks_both_read_routes() -> TestResult {
    let cloud = cloud_skill("work");
    let installed =
        installed_skill("matching", "work")?.with_plugin_id(Some("matching".to_string()));
    let harness = MergeHarness::new(
        provider(vec![installed.clone()]),
        provider(vec![cloud.clone()]),
        default_config(),
    )
    .await?;
    harness.turn_store.insert(selected_plugins(
        &[None, Some("env-1:matching")],
        &["Plugin_123"],
    ));
    let tools = harness.tools(&harness.step_store);
    assert!(!harness.render().await?.contains("- deploy:work:"));
    for package in [&cloud.id, &installed.id] {
        let result = call_tool(&tools, "read", json!({"package": package.0})).await;
        assert_matches!(result, Err(FunctionCallError::RespondToModel(message)) if message == "skill package is not available");
    }
    Ok(())
}

#[tokio::test]
async fn v2_rendering_and_tools_retain_their_step_catalog() -> TestResult {
    let installed =
        installed_skill("matching", "work")?.with_plugin_id(Some("matching".to_string()));
    let executor = provider(vec![installed.clone()]);
    let executor_calls = executor.list_calls.clone().unwrap();
    let cloud = provider(vec![cloud_skill("work")]);
    let cloud_calls = cloud.list_calls.clone().unwrap();
    let harness = MergeHarness::new(executor, cloud, default_config()).await?;
    harness
        .turn_store
        .insert(selected_plugins(&[None, Some("env-1:matching")], &[]));
    // Core constructs tools before World State prepares their shared step store.
    let original_tools = harness.tools(&harness.step_store);
    let early_list = call_tool(
        &original_tools,
        "list",
        json!({"authority": {"kind": "cloud"}}),
    )
    .await;
    assert_matches!(early_list, Err(FunctionCallError::RespondToModel(message)) if message == "skill catalog is not ready for this step");
    assert_eq!(executor_calls.load(Ordering::SeqCst), 0);
    let original_rendering = harness.render().await?;
    assert!(original_rendering.contains("- deploy:work:"));
    let next_step = ExtensionData::new("step-2");
    let next_tools = harness.tools(&next_step);
    harness
        .turn_store
        .insert(selected_plugins(&[], &["Plugin_123"]));
    assert!(
        !harness
            .render_step(&next_step)
            .await?
            .contains("- deploy:work:")
    );
    // Re-preparing the old step must not replace its catalog with the new policy.
    assert_eq!(harness.render().await?, original_rendering);
    for (tools, allowed) in [(&original_tools, true), (&next_tools, false)] {
        let listed = call_tool(tools, "list", json!({"authority": {"kind": "cloud"}})).await?;
        assert_eq!(
            listed["skills"].as_array().unwrap().len(),
            usize::from(allowed)
        );
        let read = call_tool(tools, "read", json!({"package": installed.id.0})).await;
        if allowed {
            read?;
        } else {
            assert_matches!(read, Err(FunctionCallError::RespondToModel(message)) if message == "skill package is not available");
        }
    }
    // Re-enabling the plugin reuses the raw projection, not the denied step's catalog.
    let enabled_step = ExtensionData::new("step-3");
    harness
        .turn_store
        .insert(selected_plugins(&[None, Some("env-1:matching")], &[]));
    let enabled_tools = harness.tools(&enabled_step);
    assert_eq!(
        harness.render_step(&enabled_step).await?,
        original_rendering
    );
    call_tool(&enabled_tools, "read", json!({"package": installed.id.0})).await?;
    assert_eq!(executor_calls.load(Ordering::SeqCst), 1);
    assert_eq!(cloud_calls.load(Ordering::SeqCst), 1);
    Ok(())
}

#[tokio::test]
async fn v2_implicit_and_explicit_invocations_retain_their_step_catalog() -> TestResult {
    let installed =
        installed_skill("matching", "work")?.with_plugin_id(Some("matching".to_string()));
    let cloud = cloud_skill("work").hidden_from_prompt();
    let cloud_provider = provider(vec![cloud.clone()]);
    let reads = cloud_provider.read_requests.clone();
    let harness = MergeHarness::new(
        provider(vec![installed.clone()]),
        cloud_provider,
        default_config(),
    )
    .await?;
    harness
        .turn_store
        .insert(selected_plugins(&[None, Some("env-1:matching")], &[]));
    assert!(!harness.render().await?.contains("- deploy:work:"));
    let workdir = PathUri::parse("file:///skills")?;
    let detect = |step_store| {
        detect_implicit_skill_invocation(
            &harness.turn_store,
            step_store,
            "env-1",
            "cat matching/SKILL.md",
            &workdir,
            /*native_workdir*/ None,
        )
    };
    let original = detect(&harness.step_store).expect("prepared step skill");
    assert_eq!(original.remote_plugin_id, cloud.remote_plugin_id);
    assert_eq!(original.plugin_id.as_deref(), Some("matching"));
    assert_matches!(&original.location, codex_analytics::SkillInvocationLocation::Resource { id, .. } if id == cloud.main_prompt.as_str());

    let next_step = ExtensionData::new("step-2");
    harness.turn_store.insert(SelectedPluginSnapshot::default());
    harness.render_step(&next_step).await?;
    assert_eq!(
        detect(&harness.step_store).map(|invocation| (
            invocation.skill_name,
            invocation.plugin_id,
            invocation.remote_plugin_id,
        )),
        Some((
            original.skill_name,
            original.plugin_id,
            original.remote_plugin_id
        )),
    );
    assert!(detect(&next_step).is_none());
    for (step_store, allowed) in [(&harness.step_store, true), (&next_step, false)] {
        let fragments = harness.registry.turn_input_contributors()[0]
            .contribute(
                TurnInputContext {
                    step_store,
                    turn_id: "turn-1".to_string(),
                    user_input: vec![UserInput::Mention {
                        name: installed.name.clone(),
                        path: installed.main_prompt.as_str().to_string(),
                    }],
                    environments: Vec::new(),
                },
                /*extension_metrics*/ None,
                &harness.session_store,
                &harness.thread_store,
                &harness.turn_store,
            )
            .await;
        assert_eq!(fragments.len(), usize::from(allowed));
        if allowed {
            assert!(
                fragments[0]
                    .render()
                    .expect_text()
                    .contains("<name>deploy:work</name>")
            );
        }
    }
    assert_eq!(
        read_request_keys(&reads),
        vec![(cloud.authority, cloud.id, cloud.main_prompt)]
    );
    Ok(())
}

#[tokio::test]
async fn v1_retains_legacy_plugin_disablement() -> TestResult {
    let installed =
        installed_skill("legacy", "work")?.with_remote_plugin_id(/*remote_plugin_id*/ None);
    for disabled in [false, true] {
        let mut executor = provider(vec![installed.clone()]);
        executor.catalog.warnings = vec!["Executor warning".into()];
        let reads = executor.read_requests.clone();
        let mut cloud = provider(vec![cloud_skill("work")]);
        cloud.catalog.warnings = vec!["Cloud warning".into()];
        let mut harness = MergeHarness::new(executor, cloud, default_config()).await?;
        harness.snapshots = None;
        harness.discovery = Some(ExecutorCapabilityDiscoverySnapshot::new(
            &[],
            Vec::new(),
            Default::default(),
        ));
        harness.turn_store.insert(SelectedPluginSnapshot {
            plugins: vec![SelectedPluginIdentity {
                selected_root_id: Some(installed.authority.id.clone()),
                plugin_id: PluginIdentity::Local {
                    plugin_id: "legacy@marketplace".to_string(),
                },
            }],
            disabled_plugin_roots: if disabled {
                vec![installed.authority.id.clone()]
            } else {
                Vec::new()
            },
            ..Default::default()
        });
        let tools = harness.tools(&harness.step_store);
        let rendered = harness.render().await?;
        assert!(rendered.contains("Cloud work description"));
        assert!(!rendered.contains("Installed legacy description"));
        let listed = call_tool(&tools, "list", json!({"authority": {"kind": "executor"}})).await?;
        assert_eq!(
            listed["skills"].as_array().unwrap().len(),
            usize::from(!disabled)
        );
        assert_eq!(listed["warnings"], json!(["Executor warning"]));
        let cloud = call_tool(&tools, "list", json!({"authority": {"kind": "cloud"}})).await?;
        assert_eq!(cloud["skills"].as_array().unwrap().len(), 1);
        assert_eq!(cloud["warnings"], json!(["Cloud warning"]));
        let read = call_tool(&tools, "read", json!({"package": installed.id.0})).await;
        if disabled {
            assert_matches!(read, Err(FunctionCallError::RespondToModel(message)) if message == "skill package is not available");
        } else {
            read?;
        }
        assert_eq!(read_request_keys(&reads).len(), usize::from(!disabled));
    }
    Ok(())
}

#[tokio::test]
async fn v2_shadow_observation_tracks_merged_skills_independently_of_read_route() -> TestResult {
    for executor_route in [false, true] {
        let metrics = MetricsClient::new(
            MetricsConfig::in_memory(
                "test",
                "codex-skills-extension",
                env!("CARGO_PKG_VERSION"),
                InMemoryMetricExporter::default(),
            )
            .with_runtime_reader(),
        )?;
        let installed =
            installed_skill("matching", "work")?.with_plugin_id(Some("matching".to_string()));
        let cloud = cloud_skill("work");
        let executor = provider(vec![installed.clone()]);
        let cloud_provider = provider(vec![cloud.clone()]);
        let executor_reads = executor.read_requests.clone();
        let cloud_reads = cloud_provider.read_requests.clone();
        let mut harness = MergeHarness::with_metrics(
            executor,
            cloud_provider,
            TestConfig {
                include_instructions: false,
                shadow_selection_enabled: true,
                ..default_config()
            },
            Some(metrics.clone()),
        )
        .await?;
        for turn_id in ["turn-1", "turn-2"] {
            harness.turn_store = ExtensionData::new(turn_id);
            harness.step_store = ExtensionData::new(format!("{turn_id}-step"));
            start_registered_turn(
                &harness.registry,
                &harness.session_store,
                &harness.thread_store,
                turn_id,
            )
            .await;
            harness
                .turn_store
                .insert(selected_plugins(&[None, Some("env-1:matching")], &[]));
            let tools = harness.tools(&harness.step_store);
            harness.render().await?;
            let fragments = harness.registry.turn_input_contributors()[0]
                .contribute(
                    TurnInputContext {
                        step_store: &harness.step_store,
                        turn_id: turn_id.to_string(),
                        user_input: vec![UserInput::Text {
                            text: "continue".to_string(),
                            text_elements: Vec::new(),
                        }],
                        environments: Vec::new(),
                    },
                    /*extension_metrics*/ None,
                    &harness.session_store,
                    &harness.thread_store,
                    &harness.turn_store,
                )
                .await;
            assert!(fragments.is_empty());
            if turn_id == "turn-1" {
                let entry = if executor_route { &installed } else { &cloud };
                // Repeated reads should yield only one shadow observation for this turn.
                for _ in 0..2 {
                    let output = call_tool(&tools, "read", json!({"package": entry.id.0})).await?;
                    assert_eq!(output["resource"], json!(entry.main_prompt.as_str()));
                }
            } else {
                // The next turn must recover the observed Cloud candidate in both history selectors.
                harness.registry.skill_invocation_contributors()[0]
                    .on_skill_invocation(SkillInvocationInput {
                        session_store: &harness.session_store,
                        thread_store: &harness.thread_store,
                        turn_store: &harness.turn_store,
                        turn_id,
                        skill_resource: cloud.main_prompt.as_str(),
                        kind: SkillInvocationKind::Implicit,
                    })
                    .await;
            }
        }
        let requested = if executor_route { &installed } else { &cloud };
        let reads = if executor_route {
            &executor_reads
        } else {
            &cloud_reads
        };
        let other_reads = if executor_route {
            &cloud_reads
        } else {
            &executor_reads
        };
        assert_eq!(
            read_request_keys(reads),
            vec![
                (
                    requested.authority.clone(),
                    requested.id.clone(),
                    requested.main_prompt.clone()
                );
                if executor_route { 2 } else { 1 }
            ]
        );
        assert!(read_request_keys(other_reads).is_empty());

        let snapshots = super::shadow_task_context_tests::collect_shadow_observations(
            &metrics,
            "codex.skills.shadow_selection.invocation",
            /*expected*/ 24,
        )
        .await?;
        let actual = snapshots
            .iter()
            .flat_map(opentelemetry_sdk::metrics::data::ResourceMetrics::scope_metrics)
            .flat_map(opentelemetry_sdk::metrics::data::ScopeMetrics::metrics)
            .filter(|metric| metric.name() == "codex.skills.shadow_selection.invocation")
            .flat_map(|metric| match metric.data() {
                AggregatedMetrics::U64(MetricData::Sum(sum)) => sum.data_points(),
                data => panic!("unexpected shadow metric: {data:?}"),
            })
            .filter_map(|point| {
                let attribute = |key| {
                    point
                        .attributes()
                        .find(|attribute| attribute.key.as_str() == key)
                        .map(|attribute| attribute.value.as_str().to_string())
                };
                let method = attribute("method")?;
                matches!(method.as_str(), "lru_v1" | "task_context_fusion_v1")
                    .then(|| ((method, attribute("hit").expect("hit tag")), point.value()))
            })
            .fold(
                std::collections::BTreeMap::new(),
                |mut totals, (key, count)| {
                    *totals.entry(key).or_insert(0) += count;
                    totals
                },
            );
        let expected = ["lru_v1", "task_context_fusion_v1"]
            .into_iter()
            .flat_map(|method| {
                ["false", "true"]
                    .into_iter()
                    .map(move |hit| ((method.to_string(), hit.to_string()), 1))
            })
            .collect::<std::collections::BTreeMap<_, _>>();
        assert_eq!(actual, expected, "executor_route={executor_route}");
    }
    Ok(())
}
