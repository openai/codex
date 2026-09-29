//! Pre-budget deduplication preserves source packages and aliases across executor readiness changes.

use super::*;
use codex_exec_server::EnvironmentManager;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn cloud_preference_preserves_aliases_reads_and_executor_fallback() -> TestResult {
    for (cloud_enabled, all_overlap) in [(true, false), (false, false), (true, true)] {
        let description = format!("{} UNIQUE_DESCRIPTION_END", "Skill guidance. ".repeat(60));
        let executor_roots = [
            format!("skill://executor-a/{}", "a".repeat(180)),
            format!("skill://executor-b/{}", "b".repeat(180)),
        ];
        let mut executor_entries = Vec::new();
        for (plugin, root) in ["demo", "other"].into_iter().zip(&executor_roots) {
            for index in 0..3 {
                let package = format!("{root}/s{index}");
                let mut entry = test_entry(
                    SkillSourceKind::Executor,
                    "skills",
                    &package,
                    &format!("{package}/SKILL.md"),
                );
                entry.name = format!("{plugin}:s{index}");
                entry.description = description.clone();
                executor_entries.push(entry.with_alias_root(root));
            }
        }
        let mut cloud_entries = Vec::new();
        for plugin in if all_overlap {
            vec!["demo", "other"]
        } else {
            vec!["demo"]
        } {
            for index in 0..3 {
                let mut entry = test_entry(
                    SkillSourceKind::Cloud,
                    "codex_apps",
                    &format!("cloud/{plugin}/s{index}"),
                    &format!("cloud/{plugin}/s{index}/SKILL.md"),
                );
                entry.name = format!("{plugin}:s{index}");
                cloud_entries.push(entry);
            }
        }
        let provider = |entries| {
            Arc::new(StaticSkillProvider {
                catalog: SkillCatalog {
                    entries,
                    warnings: Vec::new(),
                },
                read_requests: Arc::new(Mutex::new(Vec::new())),
                list_calls: None,
                fail_first_list: false,
            })
        };
        let providers = SkillProviders::new()
            .with_executor_provider(provider(executor_entries.clone()))
            .with_cloud_provider(provider(cloud_entries));
        let mut builder = ExtensionRegistryBuilder::new();
        install_with_providers(&mut builder, providers, skills_extension_config);
        let registry = builder.build();
        let session_store = ExtensionData::new("session");
        let thread_store = ExtensionData::new("thread");
        let config = TestConfig {
            cloud_skill_enabled: cloud_enabled,
            ..default_config()
        };
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
        let roots = [SelectedCapabilityRoot {
            id: "skills".to_string(),
            location: CapabilityRootLocation::Environment {
                environment_id: "local".to_string(),
                path: PathUri::parse("file:///skills")?,
            },
        }];
        let resolved_roots = EnvironmentManager::default_for_tests()
            .resolve_selected_capability_roots(&roots, &Default::default())
            .await;
        assert_eq!(resolved_roots.len(), 1);
        let mut previous = serde_json::Map::new();
        let mut initial_cloud_body = None;
        let model_info = ModelInfo {
            context_window: Some(70_000),
            ..catalog_model_info()
        };
        for (stage, ready) in [
            ("before", false),
            ("connected", true),
            ("disconnected", false),
        ] {
            start_registered_turn(&registry, &session_store, &thread_store, stage).await;
            let turn_store = ExtensionData::new(stage);
            let sections = registry.context_contributors()[0]
                .contribute_world_state(WorldStateContributionInput {
                    previous_world_state: Some(&previous),
                    model_info: &model_info,
                    thread_id: codex_protocol::ThreadId::new(),
                    turn_id: stage,
                    environments: &[],
                    ready_selected_capability_roots: if ready { &roots } else { &[] },
                    executor_capability_discovery: None,
                    extension_metrics: None,
                    session_store: &session_store,
                    thread_store: &thread_store,
                    turn_store: &turn_store,
                })
                .await;
            let executor_body = world_state_section(&sections, "skills").snapshot()["body"]
                .as_str()
                .unwrap_or_default()
                .to_string();
            let cloud_body = world_state_section(&sections, "cloud_skills").snapshot()["body"]
                .as_str()
                .unwrap_or_default()
                .to_string();
            for index in 0..3 {
                let listing = format!("- demo:s{index}:");
                assert_eq!(executor_body.contains(&listing), ready && !cloud_enabled);
                assert_eq!(cloud_body.contains(&listing), cloud_enabled);
                assert_eq!(
                    executor_body.contains(&format!("- other:s{index}:")),
                    ready && !all_overlap
                );
            }
            assert_eq!(
                executor_body.matches("UNIQUE_DESCRIPTION_END").count(),
                if ready && cloud_enabled && !all_overlap {
                    3
                } else {
                    0
                }
            );
            if stage == "before" {
                initial_cloud_body = Some(cloud_body.clone());
            } else {
                assert_eq!(Some(&cloud_body), initial_cloud_body.as_ref());
            }
            if !ready || all_overlap {
                assert!(executor_body.is_empty());
            } else {
                assert!(executor_body.contains("(executor package: e1/s0)"));
                for (index, root) in executor_roots.iter().enumerate() {
                    assert!(executor_body.contains(&format!("- `e{index}` = `{root}`")));
                }
            }
            previous = sections
                .iter()
                .map(|section| (section.id().to_string(), section.snapshot().clone()))
                .collect();
            if !ready {
                continue;
            }
            turn_store.insert(resolved_roots.clone());
            let tools = registry.tool_contributors()[0].tools_for_step(
                &session_store,
                &thread_store,
                &turn_store,
            );
            for (name, arguments, expected_resource) in [
                (
                    "list",
                    serde_json::json!({"authority": {"kind": "executor"}}),
                    None,
                ),
                (
                    "read",
                    serde_json::json!({"package": "e0/s0"}),
                    Some(executor_entries[0].main_prompt.as_str()),
                ),
                (
                    "read",
                    serde_json::json!({"package": "e1/s0"}),
                    Some(executor_entries[3].main_prompt.as_str()),
                ),
                (
                    "read",
                    serde_json::json!({"package": executor_entries[0].id.0}),
                    Some(executor_entries[0].main_prompt.as_str()),
                ),
            ] {
                let tool = tools
                    .iter()
                    .find(|tool| tool.tool_name().name == name)
                    .ok_or("missing tool")?;
                let payload = ToolPayload::Function {
                    arguments: arguments.to_string(),
                };
                let output = tool
                    .handle(ToolCall {
                        turn_id: stage.to_string(),
                        call_id: "call".to_string(),
                        tool_name: tool.tool_name(),
                        model: "test".to_string(),
                        codex_turn_metadata: None,
                        truncation_policy: TruncationPolicy::Bytes(20_000),
                        source: ToolCallSource::Direct,
                        conversation_history: ConversationHistory::default(),
                        turn_item_emitter: Arc::new(NoopTurnItemEmitter),
                        environments: Vec::new(),
                        payload: payload.clone(),
                    })
                    .await?;
                let response = output
                    .post_tool_use_response("call", &payload)
                    .ok_or("missing response")?;
                if let Some(resource) = expected_resource {
                    assert_eq!(response["resource"], resource);
                } else {
                    assert_eq!(
                        response["skills"]
                            .as_array()
                            .ok_or("missing skills")?
                            .iter()
                            .map(|skill| skill["package"].as_str())
                            .collect::<Vec<_>>(),
                        executor_entries
                            .iter()
                            .map(|entry| Some(entry.id.0.as_str()))
                            .collect::<Vec<_>>()
                    );
                }
            }
        }
    }
    Ok(())
}
