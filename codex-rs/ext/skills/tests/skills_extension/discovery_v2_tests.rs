//! Verifies metadata-only V2 skill projection and refresh without legacy discovery.

use super::*;
use codex_exec_server::DiscoverV2CapabilitiesResponse;
use codex_exec_server::DiscoveredPluginFiles;
use codex_exec_server::EnvironmentManager;
use codex_exec_server::ExecutorCapabilityDiscoveryV2Snapshot;
use codex_exec_server::ExecutorPlugin;
use codex_exec_server::ExecutorSkill;
use codex_skills::SkillDependencies;
use codex_skills::SkillPolicy;
use codex_skills::SkillToolDependency;
use codex_skills_extension::ExecutorSkillProvider;
use codex_skills_extension::merge_catalogs;
use pretty_assertions::assert_eq;

fn installation(
    environment_id: &str,
) -> Result<ExecutorCapabilityDiscoveryV2Snapshot, Box<dyn std::error::Error>> {
    Ok(ExecutorCapabilityDiscoveryV2Snapshot {
        environment_id: environment_id.to_string(),
        result: Ok(Arc::new(DiscoverV2CapabilitiesResponse {
            plugins: vec![ExecutorPlugin {
                id: "local-name@marketplace".to_string(),
                display_name: "Local plugin".to_string(),
                remote_plugin_id: Some("Plugin_123".to_string()),
                version: "1".to_string(),
                root: PathUri::parse("file:///plugins/local-name/1")?,
                files: DiscoveredPluginFiles {
                    manifest: codex_exec_server::CapabilityTextFile {
                        path: PathUri::parse(
                            "file:///plugins/local-name/1/.codex-plugin/plugin.json",
                        )?,
                        contents: r#"{"name":"local-name"}"#.to_string(),
                    },
                    mcp_config: None,
                    apps_config: None,
                },
                skills: vec![ExecutorSkill {
                    name: "local-name:work".to_string(),
                    description: "Environment description".to_string(),
                    short_description: None,
                    path: PathUri::parse(
                        "file:///plugins/local-name/1/skills/other-directory/SKILL.md",
                    )?,
                    discovery_root: None,
                    dependencies: None,
                    policy: None,
                }],
            }],
            ..Default::default()
        })),
    })
}

#[tokio::test]
async fn v2_steps_refresh_metadata_and_preserve_separate_environments() -> TestResult {
    let provider = Arc::new(ExecutorSkillProvider::new_with_restriction_product(
        Arc::new(EnvironmentManager::default_for_tests()),
        /*restriction_product*/ None,
    ));
    let dependencies = Some(SkillDependencies {
        tools: vec![SkillToolDependency {
            r#type: "mcp".into(),
            value: "deploy".into(),
            description: None,
            transport: Some("stdio".into()),
            command: Some("deploy-mcp".into()),
            url: None,
            oauth_callback_port: None,
        }],
    });
    let skill_path = PathUri::parse("file:///executor/skills/dashboard/SKILL.md")?;
    let mut snapshots = vec![installation("remote-a")?, installation("remote-b")?];
    for snapshot in &mut snapshots {
        Arc::make_mut(snapshot.result.as_mut().unwrap())
            .skills
            .push(ExecutorSkill {
                name: "dashboard".to_string(),
                description: "Build a dashboard".to_string(),
                short_description: None,
                path: skill_path.clone(),
                discovery_root: Some(SkillDiscoveryRoot {
                    id: "codex-home-skills".to_string(),
                    path: PathUri::parse("file:///executor/skills")?,
                }),
                dependencies: dependencies.clone(),
                policy: Some(SkillPolicy {
                    allow_implicit_invocation: Some(false),
                    products: vec![],
                }),
            });
    }
    let query = SkillListQuery {
        turn_id: "turn-1".to_string(),
        executor_roots: Vec::new(),
        resolved_executor_roots: Vec::new(),
        host_snapshot: None,
        include_host_skills: false,
        include_bundled_skills: false,
        include_cloud_skills: false,
        mcp_resources: None,
        executor_capability_discovery: None,
        executor_capability_discovery_v2: Some(snapshots.clone()),
    };
    // Standalone skills without discovery-root metadata use the skills namespace.
    let mut fallback_query = query.clone();
    for snapshot in fallback_query
        .executor_capability_discovery_v2
        .as_mut()
        .unwrap()
    {
        Arc::make_mut(snapshot.result.as_mut().unwrap()).skills[0].discovery_root = None;
    }
    let fallback = provider.list(fallback_query).await?;
    assert_eq!(fallback.entries.len(), 4);
    assert_eq!(
        fallback.entries[1].id.0,
        "skill://skills/executor/skills/dashboard"
    );
    assert_eq!(
        fallback.entries[1].main_prompt.environment_id(),
        Some("remote-a")
    );
    let catalog = provider.list(query).await?;
    assert_eq!(catalog.entries.len(), 4);
    let merged = merge_catalogs(/*cloud*/ None, catalog.clone());
    assert_eq!(merged, catalog);
    for entries in [
        (&merged.entries[0], &merged.entries[2]),
        (&merged.entries[1], &merged.entries[3]),
    ] {
        let (first, second) = entries;
        assert_eq!(first.main_prompt.environment_id(), Some("remote-a"));
        assert_eq!(second.main_prompt.environment_id(), Some("remote-b"));
        assert_eq!(first.authority, second.authority);
        assert_eq!(first.id, second.id);
        assert_ne!(first.main_prompt, second.main_prompt);
        assert!(first.sources.is_empty());
        assert!(second.sources.is_empty());
    }

    let mut rebound = merged.entries[0].clone();
    rebound.main_prompt = merged.entries[2].main_prompt.clone();
    let mut deduplicated = SkillCatalog::default();
    deduplicated.push_entry(merged.entries[0].clone());
    deduplicated.push_entry(rebound);
    deduplicated.push_entry(merged.entries[2].clone());
    assert_eq!(
        deduplicated.entries,
        vec![merged.entries[0].clone(), merged.entries[2].clone()]
    );

    let plugin = merged
        .entries
        .iter()
        .find(|entry| entry.plugin_id.is_some())
        .unwrap();
    assert_eq!(plugin.authority.id, "local-name@marketplace");
    assert_eq!(
        plugin.id.0,
        "skill://local-name@marketplace/plugins/local-name/1/skills/other-directory"
    );

    let entry = merged
        .entries
        .iter()
        .find(|entry| entry.plugin_id.is_none())
        .unwrap();
    assert_eq!(entry.dependencies, dependencies);
    assert!(entry.enabled);
    assert!(!entry.prompt_visible);
    assert_eq!(entry.authority.id, "codex-home-skills");
    assert_eq!(
        entry.id.0,
        "skill://codex-home-skills/executor/skills/dashboard"
    );
    assert_eq!(
        entry.main_prompt,
        SkillResourceId::environment(entry.main_prompt.as_str(), "remote-a", skill_path.clone(),)
    );
    let providers = SkillProviders::new().with_executor_provider(provider);
    let mut builder = ExtensionRegistryBuilder::new();
    install_with_providers(&mut builder, providers, skills_extension_config);
    let registry = builder.build();
    let session_store = ExtensionData::new("session");
    let thread_store = ExtensionData::new("thread");
    registry.thread_lifecycle_contributors()[0]
        .on_thread_start(ThreadStartInput {
            config: &default_config(),
            session_source: &SessionSource::Cli,
            persistent_thread_state_available: true,
            environments: &[],
            mcp_resource_client: None,
            extension_metrics: None,
            session_store: &session_store,
            thread_store: &thread_store,
        })
        .await;
    let turn_store = ExtensionData::new("turn-1");
    for description in [Some("original"), Some("updated"), None] {
        if let Some(description) = description {
            for snapshot in &mut snapshots {
                Arc::make_mut(snapshot.result.as_mut().unwrap()).plugins[0].skills[0].description =
                    description.to_string();
            }
        } else {
            snapshots.clear();
        }
        let step_store = ExtensionData::new("step");
        step_store.insert(snapshots.clone());
        step_store.insert(SelectedPluginSnapshot {
            plugins: snapshots
                .iter()
                .map(|snapshot| codex_extension_api::SelectedPluginIdentity {
                    selected_root_id: Some(format!(
                        "{}:local-name@marketplace",
                        snapshot.environment_id
                    )),
                    plugin_id: codex_core_plugin_common::PluginIdentity::Remote {
                        remote_plugin_id: "Plugin_123".into(),
                    },
                })
                .collect(),
            ..Default::default()
        });
        let sections = prepared_world_state(
            &registry,
            &[],
            WorldStateContributionInput {
                previous_world_state: None,
                step_store: &step_store,
                model_info: &catalog_model_info(),
                thread_id: codex_protocol::ThreadId::new(),
                turn_id: "turn-1",
                environments: &[],
                extension_metrics: None,
                session_store: &session_store,
                thread_store: &thread_store,
                turn_store: &turn_store,
            },
        )
        .await;
        let rendered = sections
            .iter()
            .filter_map(|section| section.render_diff(PreviousWorldStateSection::Absent).1)
            .map(|fragment| fragment.body().clone().expect_text())
            .collect::<Vec<_>>()
            .join("\n");
        if let Some(description) = description {
            assert!(
                rendered.contains("skill://local-name@marketplace/"),
                "{rendered}"
            );
            assert!(
                rendered.contains(&format!("- local-name:work: {description}")),
                "{rendered}"
            );
            assert!(!rendered.contains("- dashboard:"), "{rendered}");
        } else {
            assert!(!rendered.contains("- local-name:work:"), "{rendered}");
        }
    }
    Ok(())
}

#[tokio::test]
async fn v2_empty_failed_and_partial_results_never_load_legacy_roots() -> TestResult {
    let provider = ExecutorSkillProvider::new_with_restriction_product(
        Arc::new(EnvironmentManager::default_for_tests()),
        /*restriction_product*/ None,
    );
    let successful = installation("executor-a")?;
    let failed = ExecutorCapabilityDiscoveryV2Snapshot {
        environment_id: "executor-b".to_string(),
        result: Err("discovery unavailable".to_string()),
    };
    let query = |snapshots| SkillListQuery {
        turn_id: "turn-1".to_string(),
        // A legacy lookup would report an unavailable environment. V2 must ignore this root.
        executor_roots: vec![SelectedCapabilityRoot {
            id: "legacy".to_string(),
            location: CapabilityRootLocation::Environment {
                environment_id: "unavailable-legacy-executor".to_string(),
                path: PathUri::parse("file:///legacy").unwrap(),
            },
        }],
        resolved_executor_roots: Vec::new(),
        host_snapshot: None,
        include_host_skills: false,
        include_bundled_skills: false,
        include_cloud_skills: false,
        mcp_resources: None,
        executor_capability_discovery: None,
        executor_capability_discovery_v2: Some(snapshots),
    };
    let successful_catalog = provider.list(query(vec![successful.clone()])).await?;
    let failure_warning =
        "Environment `executor-b` discovery failed: discovery unavailable".to_string();
    for (snapshots, expected) in [
        (Vec::new(), SkillCatalog::default()),
        (
            vec![failed.clone()],
            SkillCatalog {
                is_partial: true,
                entries: Vec::new(),
                warnings: vec![failure_warning.clone()],
            },
        ),
        (
            vec![successful, failed],
            SkillCatalog {
                is_partial: true,
                warnings: vec![failure_warning],
                ..successful_catalog
            },
        ),
    ] {
        assert_eq!(provider.list(query(snapshots)).await?, expected);
    }
    Ok(())
}
