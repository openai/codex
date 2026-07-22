use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use codex_core_skills::HostSkillsSnapshot;
use codex_core_skills::SKILLS_INTRO_WITH_ABSOLUTE_PATHS;
use codex_core_skills::SkillLoadOutcome;
use codex_core_skills::injection::InjectedHostSkillPrompts;
use codex_extension_api::ConversationHistory;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionEventSink;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::NoopTurnItemEmitter;
use codex_extension_api::PreviousWorldStateSection;
use codex_extension_api::ThreadStartInput;
use codex_extension_api::ToolCall;
use codex_extension_api::ToolPayload;
use codex_extension_api::TurnInputContext;
use codex_extension_api::WorldStateContributionInput;
use codex_models_manager::model_info::model_info_from_slug;
use codex_protocol::capabilities::CapabilityRootLocation;
use codex_protocol::capabilities::SelectedCapabilityRoot;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::SKILLS_INSTRUCTIONS_CLOSE_TAG;
use codex_protocol::protocol::SKILLS_INSTRUCTIONS_OPEN_TAG;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SkillScope;
use codex_protocol::protocol::TruncationPolicy;
use codex_protocol::protocol::TurnEnvironmentSelection;
use codex_protocol::user_input::UserInput;
use codex_skills::SkillMetadata;
use codex_skills_extension::SkillProviders;
use codex_skills_extension::SkillsExtensionConfig;
use codex_skills_extension::catalog::SkillAuthority;
use codex_skills_extension::catalog::SkillCatalog;
use codex_skills_extension::catalog::SkillCatalogEntry;
use codex_skills_extension::catalog::SkillPackageId;
use codex_skills_extension::catalog::SkillProviderError;
use codex_skills_extension::catalog::SkillReadResult;
use codex_skills_extension::catalog::SkillResourceId;
use codex_skills_extension::catalog::SkillSearchResult;
use codex_skills_extension::catalog::SkillSourceKind;
use codex_skills_extension::install;
use codex_skills_extension::install_with_providers;
use codex_skills_extension::provider::SkillListQuery;
use codex_skills_extension::provider::SkillProvider;
use codex_skills_extension::provider::SkillProviderFuture;
use codex_skills_extension::provider::SkillReadRequest;
use codex_skills_extension::provider::SkillSearchRequest;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
use pretty_assertions::assert_eq;

type TestResult = Result<(), Box<dyn std::error::Error>>;

static NEXT_CODEX_HOME_ID: AtomicUsize = AtomicUsize::new(0);
const DEMO_SKILL_CONTENTS: &str =
    "---\nname: demo\ndescription: Demo skill.\n---\n# Demo\n\nUse the demo skill.\n";

#[tokio::test]
async fn installed_extension_uses_host_service_snapshot() -> TestResult {
    let codex_home = test_codex_home();
    let skill_path = codex_home.join("skills").join("demo").join("SKILL.md");
    std::fs::create_dir_all(
        skill_path
            .parent()
            .ok_or("skill path should have a parent")?,
    )?;
    std::fs::write(&skill_path, DEMO_SKILL_CONTENTS)?;
    let mut config = default_config();
    config.shadow_selection_enabled = true;

    let mut builder = ExtensionRegistryBuilder::new();
    install(&mut builder, skills_extension_config);
    let registry = builder.build();
    let session_store = ExtensionData::new("session");
    let thread_store = ExtensionData::new("thread");
    let session_source = SessionSource::Cli;
    registry.thread_lifecycle_contributors()[0]
        .on_thread_start(ThreadStartInput {
            config: &config,
            session_source: &session_source,
            persistent_thread_state_available: true,
            environments: &[],
            mcp_resource_client: None,
            session_store: &session_store,
            thread_store: &thread_store,
        })
        .await;

    let skill_path = AbsolutePathBuf::try_from(skill_path)?;
    let skill_path_string = skill_path.to_string_lossy().into_owned();
    let mut outcome = SkillLoadOutcome::default();
    outcome.skills.push(SkillMetadata {
        name: "demo".to_string(),
        description: "Demo skill.".to_string(),
        short_description: None,
        interface: None,
        dependencies: None,
        policy: None,
        path_to_skills_md: skill_path,
        scope: SkillScope::User,
        plugin_id: None,
    });
    let loaded_skills = Arc::new(outcome);
    let skill_prompt_path = skill_path_string.replace('\\', "/");
    let turn_store = ExtensionData::new("turn-1");
    turn_store.insert(HostSkillsSnapshot::new(Arc::clone(&loaded_skills)));

    let fragments = registry.turn_input_contributors()[0]
        .contribute(
            TurnInputContext {
                turn_id: "turn-1".to_string(),
                user_input: vec![UserInput::Text {
                    text: "$demo".to_string(),
                    text_elements: Vec::new(),
                }],
                environments: Vec::new(),
            },
            &session_store,
            &thread_store,
            &turn_store,
        )
        .await;

    let expected_catalog = format!(
        "{SKILLS_INSTRUCTIONS_OPEN_TAG}\n## Skills\n{SKILLS_INTRO_WITH_ABSOLUTE_PATHS}\n### Available skills\n- demo: Demo skill. (file: {skill_prompt_path})\n{SKILLS_INSTRUCTIONS_CLOSE_TAG}"
    );
    let expected_skill = format!(
        "<skill>\n<name>demo</name>\n<path>{skill_prompt_path}</path>\n{DEMO_SKILL_CONTENTS}\n</skill>"
    );
    assert_eq!(
        vec![("developer", expected_catalog), ("user", expected_skill),],
        fragments
            .iter()
            .map(|fragment| (fragment.role(), fragment.render()))
            .collect::<Vec<_>>()
    );
    let injected_host_skill_prompts = turn_store
        .get::<InjectedHostSkillPrompts>()
        .ok_or("host skill prompt marker should be set")?;
    assert!(injected_host_skill_prompts.contains_path(&skill_path_string));

    std::fs::remove_dir_all(codex_home)?;
    Ok(())
}

#[tokio::test]
async fn selected_executor_catalog_follows_step_availability_and_reuses_its_cache() -> TestResult {
    let read_requests = Arc::new(Mutex::new(Vec::new()));
    let list_calls = Arc::new(AtomicUsize::new(0));
    let executor_provider = Arc::new(StaticSkillProvider {
        catalog: SkillCatalog {
            entries: vec![test_entry(
                SkillSourceKind::Executor,
                "env-1",
                "executor/lint-fix",
                "lint-fix/SKILL.md",
            )],
            warnings: Vec::new(),
        },
        read_requests: Arc::clone(&read_requests),
        list_calls: Some(Arc::clone(&list_calls)),
        fail_first_list: false,
    });
    let providers = SkillProviders::new().with_executor_provider(executor_provider);
    let mut builder = ExtensionRegistryBuilder::new();
    install_with_providers(&mut builder, providers, skills_extension_config);
    let registry = builder.build();

    let session_store = ExtensionData::new("session");
    let thread_store = ExtensionData::new("thread");
    let selected_roots = vec![SelectedCapabilityRoot {
        id: "lint-fix".to_string(),
        location: CapabilityRootLocation::Environment {
            environment_id: "env-1".to_string(),
            path: PathUri::parse("file:///skills/lint-fix").expect("skill root URI"),
        },
    }];
    let session_source = SessionSource::Cli;
    let config = default_config();
    registry.thread_lifecycle_contributors()[0]
        .on_thread_start(ThreadStartInput {
            config: &config,
            session_source: &session_source,
            persistent_thread_state_available: true,
            environments: &[],
            mcp_resource_client: None,
            session_store: &session_store,
            thread_store: &thread_store,
        })
        .await;

    let prompt_fragments = registry.context_contributors()[0]
        .contribute_thread_context(&session_store, &thread_store)
        .await;
    assert!(prompt_fragments.is_empty());

    let turn_store = ExtensionData::new("turn-1");
    let turn_environment = TurnEnvironmentSelection {
        environment_id: "turn-env".to_string(),
        cwd: PathUri::parse("file:///workspace").expect("cwd URI"),
        workspace_roots: Vec::new(),
    };
    let available_sections = registry.context_contributors()[0]
        .contribute_world_state(WorldStateContributionInput {
            thread_id: codex_protocol::ThreadId::new(),
            turn_id: "turn-1",
            environments: std::slice::from_ref(&turn_environment),
            ready_selected_capability_roots: &selected_roots,
            executor_capability_discovery: None,
            session_store: &session_store,
            thread_store: &thread_store,
            turn_store: &turn_store,
        })
        .await;
    assert_eq!(1, available_sections.len());
    let available_snapshot = available_sections[0].snapshot().clone();
    let available_fragment = available_sections[0]
        .render_diff(PreviousWorldStateSection::Absent)
        .ok_or("available skills should render")?;
    assert!(available_fragment.body().contains("lint-fix"));
    assert!(
        available_fragment
            .body()
            .contains("(environment resource: skill://executor/lint-fix/SKILL.md)")
    );

    let fragments = registry.turn_input_contributors()[0]
        .contribute(
            TurnInputContext {
                turn_id: "turn-1".to_string(),
                user_input: vec![UserInput::Text {
                    text: "$lint-fix please".to_string(),
                    text_elements: Vec::new(),
                }],
                environments: Vec::new(),
            },
            &session_store,
            &thread_store,
            &turn_store,
        )
        .await;

    assert_eq!(1, fragments.len());
    assert_eq!("user", fragments[0].role());
    assert!(fragments[0].render().contains("<name>lint-fix</name>"));
    assert!(fragments[0].render().contains("# Lint Fix"));
    assert_eq!(
        vec![(
            SkillAuthority::new(SkillSourceKind::Executor, "env-1"),
            SkillPackageId("executor/lint-fix".to_string()),
            SkillResourceId::new("lint-fix/SKILL.md"),
        )],
        read_request_keys(&read_requests)
    );
    let unavailable_turn_store = ExtensionData::new("turn-2");
    let unavailable_sections = registry.context_contributors()[0]
        .contribute_world_state(WorldStateContributionInput {
            thread_id: codex_protocol::ThreadId::new(),
            turn_id: "turn-2",
            environments: &[],
            ready_selected_capability_roots: &[],
            executor_capability_discovery: None,
            session_store: &session_store,
            thread_store: &thread_store,
            turn_store: &unavailable_turn_store,
        })
        .await;
    let unavailable_snapshot = unavailable_sections[0].snapshot().clone();
    let unavailable_fragment = unavailable_sections[0]
        .render_diff(PreviousWorldStateSection::Known(&available_snapshot))
        .ok_or("removed skills should render")?;
    assert!(
        unavailable_fragment
            .body()
            .contains("No selected-environment skills")
    );

    let restored_turn_store = ExtensionData::new("turn-3");
    let restored_sections = registry.context_contributors()[0]
        .contribute_world_state(WorldStateContributionInput {
            thread_id: codex_protocol::ThreadId::new(),
            turn_id: "turn-3",
            environments: &[turn_environment],
            ready_selected_capability_roots: &selected_roots,
            executor_capability_discovery: None,
            session_store: &session_store,
            thread_store: &thread_store,
            turn_store: &restored_turn_store,
        })
        .await;
    let restored_snapshot = restored_sections[0].snapshot().clone();
    let restored_fragment = restored_sections[0]
        .render_diff(PreviousWorldStateSection::Known(&unavailable_snapshot))
        .ok_or("restored skills should render")?;
    assert!(restored_fragment.body().contains("lint-fix"));
    assert_eq!(1, list_calls.load(Ordering::Relaxed));

    let mut listing_disabled_config = config.clone();
    listing_disabled_config.include_instructions = false;
    registry.config_contributors()[0].on_config_changed(
        &session_store,
        &thread_store,
        &config,
        &listing_disabled_config,
    );
    let listing_disabled_turn_store = ExtensionData::new("turn-4");
    let listing_disabled_sections = registry.context_contributors()[0]
        .contribute_world_state(WorldStateContributionInput {
            thread_id: codex_protocol::ThreadId::new(),
            turn_id: "turn-4",
            environments: &[],
            ready_selected_capability_roots: &selected_roots,
            executor_capability_discovery: None,
            session_store: &session_store,
            thread_store: &thread_store,
            turn_store: &listing_disabled_turn_store,
        })
        .await;
    let listing_disabled_fragment = listing_disabled_sections[0]
        .render_diff(PreviousWorldStateSection::Known(&restored_snapshot))
        .ok_or("disabled skill listing should render")?;
    assert_eq!(
        "\n## Skills update\nSelected-environment skills are not listed automatically. Explicit skill mentions can still be resolved when available.\n",
        listing_disabled_fragment.body()
    );
    let mut normalized_listing_disabled_snapshot = listing_disabled_sections[0].snapshot().clone();
    normalized_listing_disabled_snapshot
        .as_object_mut()
        .ok_or("skills snapshot should be an object")?
        .remove("body");
    assert!(
        listing_disabled_sections[0]
            .render_diff(PreviousWorldStateSection::Known(
                &normalized_listing_disabled_snapshot
            ))
            .is_none()
    );

    Ok(())
}

#[tokio::test]
async fn default_context_truncates_catalog_descriptions() -> TestResult {
    let description = "x".repeat(1_025);
    let mut entry = test_entry(
        SkillSourceKind::Orchestrator,
        "codex_apps",
        "orchestrator/long-description",
        "skill://orchestrator/long-description/SKILL.md",
    );
    entry.description = description.clone();
    let providers =
        SkillProviders::new().with_orchestrator_provider(Arc::new(StaticSkillProvider {
            catalog: SkillCatalog {
                entries: vec![entry],
                warnings: Vec::new(),
            },
            read_requests: Arc::new(Mutex::new(Vec::new())),
            list_calls: None,
            fail_first_list: false,
        }));
    let mut builder = ExtensionRegistryBuilder::new();
    install_with_providers(&mut builder, providers, skills_extension_config);
    let registry = builder.build();
    let session_store = ExtensionData::new("session");
    let thread_store = ExtensionData::new("thread");
    let session_source = SessionSource::Cli;
    let config = default_config();
    registry.thread_lifecycle_contributors()[0]
        .on_thread_start(ThreadStartInput {
            config: &config,
            session_source: &session_source,
            persistent_thread_state_available: true,
            environments: &[],
            mcp_resource_client: None,
            session_store: &session_store,
            thread_store: &thread_store,
        })
        .await;

    let fragments = registry.context_contributors()[0]
        .contribute_thread_context(&session_store, &thread_store)
        .await;
    assert_eq!(1, fragments.len());
    let rendered = fragments[0].text();
    assert!(rendered.contains(&("x".repeat(1_021) + "...")));
    assert!(!rendered.contains(&"x".repeat(1_024)));
    assert!(!rendered.contains(&description));

    Ok(())
}

#[tokio::test]
async fn moderate_budget_pressure_keeps_every_catalog_entry() -> TestResult {
    let description = "x".repeat(1_025);
    let entries = (0..10)
        .map(|index| {
            let package_id = format!("orchestrator/skill-{index:02}");
            let mut entry = test_entry(
                SkillSourceKind::Orchestrator,
                "codex_apps",
                &package_id,
                &format!("skill://{package_id}/SKILL.md"),
            );
            entry.description = description.clone();
            entry
        })
        .collect();
    let providers =
        SkillProviders::new().with_orchestrator_provider(Arc::new(StaticSkillProvider {
            catalog: SkillCatalog {
                entries,
                warnings: Vec::new(),
            },
            read_requests: Arc::new(Mutex::new(Vec::new())),
            list_calls: None,
            fail_first_list: false,
        }));
    let mut builder = ExtensionRegistryBuilder::new();
    install_with_providers(&mut builder, providers, skills_extension_config);
    let registry = builder.build();
    let session_store = ExtensionData::new("session");
    let thread_store = ExtensionData::new("thread");
    let session_source = SessionSource::Cli;
    let config = default_config();
    registry.thread_lifecycle_contributors()[0]
        .on_thread_start(ThreadStartInput {
            config: &config,
            session_source: &session_source,
            persistent_thread_state_available: true,
            environments: &[],
            mcp_resource_client: None,
            session_store: &session_store,
            thread_store: &thread_store,
        })
        .await;

    let fragments = registry.context_contributors()[0]
        .contribute_thread_context(&session_store, &thread_store)
        .await;
    assert_eq!(1, fragments.len());
    let rendered = fragments[0].text();
    let description_lengths = (0..10)
        .map(|index| {
            let package_id = format!("orchestrator/skill-{index:02}");
            let line_prefix = format!("- skill-{index:02}: ");
            let line_suffix = format!(" (orchestrator resource: skill://{package_id}/SKILL.md)");
            rendered
                .lines()
                .find_map(|line| {
                    line.strip_prefix(&line_prefix)
                        .and_then(|line| line.strip_suffix(&line_suffix))
                })
                .unwrap_or_else(|| panic!("rendered catalog should include skill-{index:02}"))
                .chars()
                .count()
        })
        .collect::<Vec<_>>();
    let shortest_description = *description_lengths
        .iter()
        .min()
        .expect("catalog should include descriptions");
    let longest_description = *description_lengths
        .iter()
        .max()
        .expect("catalog should include descriptions");
    assert!(shortest_description > 0);
    assert!(longest_description < 1_024);
    assert!(longest_description.abs_diff(shortest_description) <= 1);
    assert!(!rendered.contains("additional skills omitted from this bounded skills list"));
    assert!(!rendered.contains(&"x".repeat(1_021)));

    Ok(())
}

#[tokio::test]
async fn extreme_budget_pressure_removes_descriptions_before_omitting_entries() -> TestResult {
    let entries = (0..200)
        .map(|index| {
            let package_id = format!("orchestrator/skill-{index:03}");
            let mut entry = test_entry(
                SkillSourceKind::Orchestrator,
                "codex_apps",
                &package_id,
                &format!("skill://{package_id}/SKILL.md"),
            );
            entry.description = format!("description-{index:03}");
            entry
        })
        .collect();
    let providers =
        SkillProviders::new().with_orchestrator_provider(Arc::new(StaticSkillProvider {
            catalog: SkillCatalog {
                entries,
                warnings: Vec::new(),
            },
            read_requests: Arc::new(Mutex::new(Vec::new())),
            list_calls: None,
            fail_first_list: false,
        }));
    let mut builder = ExtensionRegistryBuilder::new();
    install_with_providers(&mut builder, providers, skills_extension_config);
    let registry = builder.build();
    let session_store = ExtensionData::new("session");
    let thread_store = ExtensionData::new("thread");
    let session_source = SessionSource::Cli;
    let config = default_config();
    registry.thread_lifecycle_contributors()[0]
        .on_thread_start(ThreadStartInput {
            config: &config,
            session_source: &session_source,
            persistent_thread_state_available: true,
            environments: &[],
            mcp_resource_client: None,
            session_store: &session_store,
            thread_store: &thread_store,
        })
        .await;

    let fragments = registry.context_contributors()[0]
        .contribute_thread_context(&session_store, &thread_store)
        .await;
    assert_eq!(1, fragments.len());
    let rendered = fragments[0].text();
    let included_count = rendered
        .lines()
        .filter(|line| line.starts_with("- skill-"))
        .count();
    assert!(included_count > 0);
    assert!(included_count < 200);
    assert!(rendered.contains("- skill-000: (orchestrator resource:"));
    assert!(!rendered.contains("- skill-199:"));
    assert!(!rendered.contains("description-"));
    assert!(rendered.contains("additional skills omitted from this bounded skills list"));

    Ok(())
}

#[tokio::test]
async fn skills_list_truncates_catalog_descriptions_in_tool_output() -> TestResult {
    let description = "x".repeat(1_025);
    let mut entry = test_entry(
        SkillSourceKind::Orchestrator,
        "codex_apps",
        "orchestrator/long-description",
        "skill://orchestrator/long-description/SKILL.md",
    );
    entry.description = description.clone();
    let providers =
        SkillProviders::new().with_orchestrator_provider(Arc::new(StaticSkillProvider {
            catalog: SkillCatalog {
                entries: vec![entry],
                warnings: Vec::new(),
            },
            read_requests: Arc::new(Mutex::new(Vec::new())),
            list_calls: None,
            fail_first_list: false,
        }));
    let mut builder = ExtensionRegistryBuilder::new();
    install_with_providers(&mut builder, providers, skills_extension_config);
    let registry = builder.build();
    let session_store = ExtensionData::new("session");
    let thread_store = ExtensionData::new("thread");
    let session_source = SessionSource::Cli;
    let config = default_config();
    registry.thread_lifecycle_contributors()[0]
        .on_thread_start(ThreadStartInput {
            config: &config,
            session_source: &session_source,
            persistent_thread_state_available: true,
            environments: &[],
            mcp_resource_client: None,
            session_store: &session_store,
            thread_store: &thread_store,
        })
        .await;

    let tools = registry.tool_contributors()[0].tools(&session_store, &thread_store);
    let list_tool = tools
        .iter()
        .find(|tool| tool.tool_name().name == "list")
        .ok_or("skills.list tool should be registered")?;
    let payload = ToolPayload::Function {
        arguments: serde_json::json!({"authority": {"kind": "orchestrator"}}).to_string(),
    };
    let output = list_tool
        .handle(ToolCall {
            turn_id: "turn-1".to_string(),
            call_id: "call-1".to_string(),
            tool_name: list_tool.tool_name(),
            model: "gpt-test".to_string(),
            codex_turn_metadata: None,
            truncation_policy: TruncationPolicy::Bytes(1_024),
            conversation_history: ConversationHistory::default(),
            turn_item_emitter: Arc::new(NoopTurnItemEmitter),
            environments: Vec::new(),
            payload: payload.clone(),
        })
        .await?;
    let response = output
        .post_tool_use_response("call-1", &payload)
        .ok_or("skills.list should expose structured output")?;
    let rendered_description = response["skills"][0]["description"]
        .as_str()
        .ok_or("skills.list response should include a description")?;

    assert_eq!(rendered_description, "x".repeat(1_021) + "...");
    assert_ne!(rendered_description, description);

    Ok(())
}

#[tokio::test]
async fn orchestrator_catalog_snapshot_caches_failure() -> TestResult {
    let list_calls = Arc::new(AtomicUsize::new(0));
    let providers =
        SkillProviders::new().with_orchestrator_provider(Arc::new(StaticSkillProvider {
            catalog: SkillCatalog {
                entries: vec![test_entry(
                    SkillSourceKind::Orchestrator,
                    "codex_apps",
                    "orchestrator/first",
                    "skill://orchestrator/first/SKILL.md",
                )],
                warnings: Vec::new(),
            },
            read_requests: Arc::new(Mutex::new(Vec::new())),
            list_calls: Some(Arc::clone(&list_calls)),
            fail_first_list: true,
        }));
    let (event_tx, event_rx) = std::sync::mpsc::channel();
    let mut builder =
        ExtensionRegistryBuilder::with_event_sink(Arc::new(ChannelEventSink(event_tx)));
    install_with_providers(&mut builder, providers, skills_extension_config);
    let registry = builder.build();
    let session_store = ExtensionData::new("session");
    let thread_store = ExtensionData::new("thread");
    let session_source = SessionSource::Cli;
    let config = default_config();
    registry.thread_lifecycle_contributors()[0]
        .on_thread_start(ThreadStartInput {
            config: &config,
            session_source: &session_source,
            persistent_thread_state_available: true,
            environments: &[],
            mcp_resource_client: None,
            session_store: &session_store,
            thread_store: &thread_store,
        })
        .await;

    let initial_fragments = registry.context_contributors()[0]
        .contribute_thread_context(&session_store, &thread_store)
        .await;
    assert!(initial_fragments.is_empty());
    let EventMsg::Warning(warning) = event_rx.try_recv()?.msg else {
        panic!("expected warning event");
    };
    assert_eq!(
        warning.message,
        "orchestrator skills unavailable: temporary orchestrator failure"
    );

    for turn_id in ["turn-1", "turn-2"] {
        let fragments = registry.turn_input_contributors()[0]
            .contribute(
                TurnInputContext {
                    turn_id: turn_id.to_string(),
                    user_input: vec![UserInput::Text {
                        text: "$first".to_string(),
                        text_elements: Vec::new(),
                    }],
                    environments: Vec::new(),
                },
                &session_store,
                &thread_store,
                &ExtensionData::new(turn_id),
            )
            .await;
        assert!(fragments.is_empty());
    }
    assert_eq!(1, list_calls.load(Ordering::Relaxed));

    Ok(())
}

#[tokio::test]
async fn root_qualified_locator_selects_only_the_matching_executor_skill() -> TestResult {
    let read_requests = Arc::new(Mutex::new(Vec::new()));
    let root_a_locator = "skill://root-a/shared/lint-fix/SKILL.md";
    let root_b_locator = "skill://root-b/shared/lint-fix/SKILL.md";
    let executor_provider = Arc::new(StaticSkillProvider {
        catalog: SkillCatalog {
            entries: [("root-a", root_a_locator), ("root-b", root_b_locator)]
                .into_iter()
                .map(|(root_id, locator)| {
                    SkillCatalogEntry::new(
                        SkillPackageId(locator.to_string()),
                        SkillAuthority::new(SkillSourceKind::Executor, root_id),
                        "lint-fix",
                        "Fix lint errors.",
                        SkillResourceId::new(locator),
                    )
                    .with_display_path(locator)
                })
                .collect(),
            warnings: Vec::new(),
        },
        read_requests: Arc::clone(&read_requests),
        list_calls: None,
        fail_first_list: false,
    });
    let providers = SkillProviders::new().with_executor_provider(executor_provider);
    let mut builder = ExtensionRegistryBuilder::new();
    install_with_providers(&mut builder, providers, skills_extension_config);
    let registry = builder.build();
    let session_store = ExtensionData::new("session");
    let thread_store = ExtensionData::new("thread");
    let selected_roots = [("root-a", "/skills/root-a"), ("root-b", "/skills/root-b")]
        .into_iter()
        .map(|(id, path)| SelectedCapabilityRoot {
            id: id.to_string(),
            location: CapabilityRootLocation::Environment {
                environment_id: "env-1".to_string(),
                path: PathUri::parse(&format!("file://{path}")).expect("skill root URI"),
            },
        })
        .collect::<Vec<_>>();
    let session_source = SessionSource::Cli;
    let config = default_config();
    registry.thread_lifecycle_contributors()[0]
        .on_thread_start(ThreadStartInput {
            config: &config,
            session_source: &session_source,
            persistent_thread_state_available: true,
            environments: &[],
            mcp_resource_client: None,
            session_store: &session_store,
            thread_store: &thread_store,
        })
        .await;

    let turn_store = ExtensionData::new("turn-1");
    registry.context_contributors()[0]
        .contribute_world_state(WorldStateContributionInput {
            thread_id: codex_protocol::ThreadId::new(),
            turn_id: "turn-1",
            environments: &[TurnEnvironmentSelection {
                environment_id: "env-1".to_string(),
                cwd: PathUri::parse("file:///workspace").expect("cwd URI"),
                workspace_roots: Vec::new(),
            }],
            ready_selected_capability_roots: &selected_roots,
            executor_capability_discovery: None,
            session_store: &session_store,
            thread_store: &thread_store,
            turn_store: &turn_store,
        })
        .await;
    let fragments = registry.turn_input_contributors()[0]
        .contribute(
            TurnInputContext {
                turn_id: "turn-1".to_string(),
                user_input: vec![UserInput::Mention {
                    name: "lint-fix".to_string(),
                    path: root_b_locator.to_string(),
                }],
                environments: Vec::new(),
            },
            &session_store,
            &thread_store,
            &turn_store,
        )
        .await;

    assert_eq!(1, fragments.len());
    assert!(fragments[0].render().contains(root_b_locator));
    assert_eq!(
        vec![(
            SkillAuthority::new(SkillSourceKind::Executor, "root-b"),
            SkillPackageId(root_b_locator.to_string()),
            SkillResourceId::new(root_b_locator),
        )],
        read_request_keys(&read_requests)
    );

    Ok(())
}

#[tokio::test]
async fn model_context_window_scales_executor_catalog_but_not_thread_catalog() -> TestResult {
    let orchestrator_entries = (0..40)
        .map(|index| {
            test_entry(
                SkillSourceKind::Orchestrator,
                "orchestrator",
                &format!("orchestrator/skill-{index:02}"),
                &format!("skill-{index:02}/SKILL.md"),
            )
        })
        .collect();
    let executor_entries = (0..200)
        .map(|index| {
            test_entry(
                SkillSourceKind::Executor,
                "env-1",
                &format!("executor/skill-{index:02}"),
                &format!("skill-{index:02}/SKILL.md"),
            )
        })
        .collect();
    let providers = SkillProviders::new()
        .with_orchestrator_provider(Arc::new(StaticSkillProvider {
            catalog: SkillCatalog {
                entries: orchestrator_entries,
                warnings: Vec::new(),
            },
            read_requests: Arc::new(Mutex::new(Vec::new())),
            list_calls: None,
            fail_first_list: false,
        }))
        .with_executor_provider(Arc::new(StaticSkillProvider {
            catalog: SkillCatalog {
                entries: executor_entries,
                warnings: Vec::new(),
            },
            read_requests: Arc::new(Mutex::new(Vec::new())),
            list_calls: None,
            fail_first_list: false,
        }));
    let mut builder = ExtensionRegistryBuilder::new();
    install_with_providers(&mut builder, providers, skills_extension_config);
    let registry = builder.build();
    let session_store = ExtensionData::new("session");
    let thread_store = ExtensionData::new("thread");
    let mut config = default_config();
    config.bundled_skills_enabled = false;
    registry.thread_lifecycle_contributors()[0]
        .on_thread_start(ThreadStartInput {
            config: &config,
            session_source: &SessionSource::Cli,
            persistent_thread_state_available: true,
            environments: &[],
            mcp_resource_client: None,
            session_store: &session_store,
            thread_store: &thread_store,
        })
        .await;
    let mut model_info = model_info_from_slug("test-model");
    model_info.context_window = Some(10_000);
    thread_store.insert(model_info);

    let thread_fragments = registry.context_contributors()[0]
        .contribute_thread_context(&session_store, &thread_store)
        .await;
    assert_eq!(1, thread_fragments.len());
    assert!(thread_fragments[0].text().contains("skill-39"));
    assert!(
        !thread_fragments[0]
            .text()
            .contains("additional skills omitted")
    );

    let selected_roots = vec![SelectedCapabilityRoot {
        id: "skills".to_string(),
        location: CapabilityRootLocation::Environment {
            environment_id: "env-1".to_string(),
            path: PathUri::parse("file:///skills").expect("skill root URI"),
        },
    }];
    let turn_store = ExtensionData::new("turn-1");
    let sections = registry.context_contributors()[0]
        .contribute_world_state(WorldStateContributionInput {
            thread_id: codex_protocol::ThreadId::new(),
            turn_id: "turn-1",
            environments: &[],
            ready_selected_capability_roots: &selected_roots,
            executor_capability_discovery: None,
            session_store: &session_store,
            thread_store: &thread_store,
            turn_store: &turn_store,
        })
        .await;
    let fragment = sections[0]
        .render_diff(PreviousWorldStateSection::Absent)
        .ok_or("bounded executor catalog should render")?;
    assert!(fragment.body().contains("additional skills omitted"));
    assert!(!fragment.body().contains("skill-39"));

    Ok(())
}

#[tokio::test]
async fn prompt_hidden_skill_can_still_be_invoked() -> TestResult {
    let read_requests = Arc::new(Mutex::new(Vec::new()));
    let provider = Arc::new(StaticSkillProvider {
        catalog: SkillCatalog {
            entries: vec![
                test_entry(
                    SkillSourceKind::Host,
                    "host",
                    "host/visible-skill",
                    "visible-skill/SKILL.md",
                ),
                test_entry(
                    SkillSourceKind::Host,
                    "host",
                    "host/hidden-skill",
                    "hidden-skill/SKILL.md",
                )
                .hidden_from_prompt(),
            ],
            warnings: Vec::new(),
        },
        read_requests: Arc::clone(&read_requests),
        list_calls: None,
        fail_first_list: false,
    });
    let providers = SkillProviders::new().with_host_provider(provider);
    let mut builder = ExtensionRegistryBuilder::new();
    install_with_providers(&mut builder, providers, skills_extension_config);
    let registry = builder.build();
    let session_store = ExtensionData::new("session");
    let thread_store = ExtensionData::new("thread");
    let session_source = SessionSource::Cli;
    let config = default_config();
    registry.thread_lifecycle_contributors()[0]
        .on_thread_start(ThreadStartInput {
            config: &config,
            session_source: &session_source,
            persistent_thread_state_available: true,
            environments: &[],
            mcp_resource_client: None,
            session_store: &session_store,
            thread_store: &thread_store,
        })
        .await;

    let fragments = registry.turn_input_contributors()[0]
        .contribute(
            TurnInputContext {
                turn_id: "turn-1".to_string(),
                user_input: vec![UserInput::Text {
                    text: "$hidden-skill".to_string(),
                    text_elements: Vec::new(),
                }],
                environments: Vec::new(),
            },
            &session_store,
            &thread_store,
            &ExtensionData::new("turn-1"),
        )
        .await;

    assert_eq!(2, fragments.len());
    assert!(fragments[0].render().contains("visible-skill"));
    assert!(!fragments[0].render().contains("hidden-skill"));
    assert!(fragments[1].render().contains("<name>hidden-skill</name>"));
    assert_eq!(
        vec![(
            SkillAuthority::new(SkillSourceKind::Host, "host"),
            SkillPackageId("host/hidden-skill".to_string()),
            SkillResourceId::new("hidden-skill/SKILL.md"),
        )],
        read_request_keys(&read_requests)
    );

    Ok(())
}

#[derive(Clone)]
struct StaticSkillProvider {
    catalog: SkillCatalog,
    read_requests: Arc<Mutex<Vec<SkillReadRequest>>>,
    list_calls: Option<Arc<AtomicUsize>>,
    fail_first_list: bool,
}

struct ChannelEventSink(std::sync::mpsc::Sender<Event>);

impl ExtensionEventSink for ChannelEventSink {
    fn emit(&self, event: Event) {
        let _ = self.0.send(event);
    }
}

impl SkillProvider for StaticSkillProvider {
    fn list(&self, _query: SkillListQuery) -> SkillProviderFuture<'_, SkillCatalog> {
        let list_call = self
            .list_calls
            .as_ref()
            .map(|list_calls| list_calls.fetch_add(1, Ordering::Relaxed));
        let fail = self.fail_first_list && list_call == Some(0);
        let catalog = self.catalog.clone();
        Box::pin(async move {
            if fail {
                Err(SkillProviderError::new("temporary orchestrator failure"))
            } else {
                Ok(catalog)
            }
        })
    }

    fn read(&self, request: SkillReadRequest) -> SkillProviderFuture<'_, SkillReadResult> {
        let read_requests = Arc::clone(&self.read_requests);
        Box::pin(async move {
            read_requests
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(request.clone());
            Ok(SkillReadResult {
                resource: request.resource,
                contents: "# Lint Fix\n\nRun the formatter.".to_string(),
            })
        })
    }

    fn search(&self, _request: SkillSearchRequest) -> SkillProviderFuture<'_, SkillSearchResult> {
        Box::pin(async { Ok(SkillSearchResult::default()) })
    }
}

fn test_entry(
    kind: SkillSourceKind,
    authority_id: &str,
    package_id: &str,
    main_prompt: &str,
) -> SkillCatalogEntry {
    let name = package_id.rsplit('/').next().unwrap_or(package_id);
    SkillCatalogEntry::new(
        SkillPackageId(package_id.to_string()),
        SkillAuthority::new(kind, authority_id),
        name,
        "Fix lint errors.",
        SkillResourceId::new(main_prompt),
    )
    .with_display_path(format!("skill://{package_id}/SKILL.md"))
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TestConfig {
    include_instructions: bool,
    bundled_skills_enabled: bool,
    orchestrator_skills_enabled: bool,
    shadow_selection_enabled: bool,
}

fn default_config() -> TestConfig {
    TestConfig {
        include_instructions: true,
        bundled_skills_enabled: true,
        orchestrator_skills_enabled: true,
        shadow_selection_enabled: false,
    }
}

fn skills_extension_config(config: &TestConfig) -> SkillsExtensionConfig {
    SkillsExtensionConfig {
        include_instructions: config.include_instructions,
        bundled_skills_enabled: config.bundled_skills_enabled,
        orchestrator_skills_enabled: config.orchestrator_skills_enabled,
        shadow_selection_enabled: config.shadow_selection_enabled,
    }
}

fn test_codex_home() -> PathBuf {
    let id = NEXT_CODEX_HOME_ID.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "codex-skills-extension-test-{}-{id}",
        std::process::id(),
    ))
}

fn read_request_keys(
    requests: &Arc<Mutex<Vec<SkillReadRequest>>>,
) -> Vec<(SkillAuthority, SkillPackageId, SkillResourceId)> {
    requests
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .iter()
        .map(|request| {
            (
                request.authority.clone(),
                request.package.clone(),
                request.resource.clone(),
            )
        })
        .collect()
}
