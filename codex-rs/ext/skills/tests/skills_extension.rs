use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use codex_config::ConfigLayerEntry;
use codex_config::ConfigLayerSource;
use codex_config::ConfigLayerStack;
use codex_config::ConfigRequirementsToml;
use codex_config::TomlValue;
use codex_connectors::ExplicitConnectorMentions;
use codex_core_plugins::PluginLoadOutcome;
use codex_core_skills::HostSkillsSnapshot;
use codex_core_skills::SKILLS_HOW_TO_USE_WITH_ABSOLUTE_PATHS;
use codex_core_skills::SKILLS_INTRO_WITH_ABSOLUTE_PATHS;
use codex_core_skills::SkillLoadOutcome;
use codex_core_skills::SkillMetadata;
use codex_exec_server::ExecutorFileSystem;
use codex_extension_api::ContextContributionContext;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionEventSink;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::ThreadStartInput;
use codex_extension_api::TurnInputContext;
use codex_mcp::McpServerDependencies;
use codex_protocol::ThreadId;
use codex_protocol::capabilities::CapabilityRootLocation;
use codex_protocol::capabilities::SelectedCapabilityRoot;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::SKILLS_INSTRUCTIONS_CLOSE_TAG;
use codex_protocol::protocol::SKILLS_INSTRUCTIONS_OPEN_TAG;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SkillScope;
use codex_protocol::user_input::UserInput;
use codex_skills_extension::HostSkillsConfig;
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
use codex_utils_string::approx_token_count;
use pretty_assertions::assert_eq;

type TestResult = Result<(), Box<dyn std::error::Error>>;

static NEXT_CODEX_HOME_ID: AtomicUsize = AtomicUsize::new(0);
const DEMO_SKILL_CONTENTS: &str =
    "---\nname: demo\ndescription: Demo skill.\n---\n# Demo\n\nUse [$calendar](app://calendar).\n";
const STATIC_SKILL_CONTENTS: &str = "# Lint Fix\n\nRun the formatter.";

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
    let metadata_path = skill_path
        .parent()
        .ok_or("skill path should have a parent")?
        .join("agents")
        .join("openai.yaml");
    std::fs::create_dir_all(
        metadata_path
            .parent()
            .ok_or("metadata path should have a parent")?,
    )?;
    std::fs::write(
        metadata_path,
        r#"{
  "dependencies": {
    "tools": [{
      "type": "mcp",
      "value": "docs",
      "transport": "streamable_http",
      "url": "https://example.com/mcp"
    }]
  }
}"#,
    )?;
    let config = default_config();
    let codex_home_abs = AbsolutePathBuf::try_from(codex_home.clone())?;
    let host = HostSkillsConfig::new(
        codex_home_abs.clone(),
        codex_home_abs,
        ConfigLayerStack::new(
            vec![ConfigLayerEntry::new(
                ConfigLayerSource::User {
                    file: AbsolutePathBuf::try_from(codex_home.join("config.toml"))?,
                    profile: None,
                },
                TomlValue::Table(Default::default()),
            )],
            Default::default(),
            ConfigRequirementsToml::default(),
        )?,
        /*bundled_skills_enabled*/ false,
    );

    let mut builder = ExtensionRegistryBuilder::new();
    install(&mut builder, move |config: &TestConfig| {
        SkillsExtensionConfig {
            include_instructions: config.include_instructions,
            bundled_skills_enabled: config.bundled_skills_enabled,
            host: Some(host.clone()),
        }
    });
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
            session_store: &session_store,
            thread_store: &thread_store,
        })
        .await;

    let skill_path = AbsolutePathBuf::try_from(std::fs::canonicalize(skill_path)?)?;
    let skill_path_string = skill_path.to_string_lossy().into_owned();
    let skill_prompt_path = skill_path_string.replace('\\', "/");
    let turn_store = ExtensionData::new("turn-1");
    turn_store.insert(PluginLoadOutcome::default());
    let fs: Arc<dyn ExecutorFileSystem> = Arc::clone(&codex_exec_server::LOCAL_FS);
    turn_store.insert(fs);

    let prompt_fragments = registry.context_contributors()[0]
        .contribute(prompt_context(&session_store, &thread_store, &turn_store))
        .await;

    let fragments = registry.turn_input_contributors()[0]
        .contribute(
            TurnInputContext {
                thread_id: ThreadId::default(),
                turn_id: "turn-1".to_string(),
                model: "test-model".to_string(),
                user_input: vec![UserInput::Text {
                    text: "$demo".to_string(),
                    text_elements: Vec::new(),
                }],
                reserved_plain_tool_names: Default::default(),
                environments: Vec::new(),
            },
            &session_store,
            &thread_store,
            &turn_store,
        )
        .await;

    let expected_catalog = format!(
        "{SKILLS_INSTRUCTIONS_OPEN_TAG}\n## Skills\n{SKILLS_INTRO_WITH_ABSOLUTE_PATHS}\n### Available skills\n- demo: Demo skill. (file: {skill_prompt_path})\n### How to use skills\n{SKILLS_HOW_TO_USE_WITH_ABSOLUTE_PATHS}\n{SKILLS_INSTRUCTIONS_CLOSE_TAG}"
    );
    let expected_skill = format!(
        "<skill>\n<name>demo</name>\n<path>{skill_prompt_path}</path>\n{DEMO_SKILL_CONTENTS}\n</skill>"
    );
    assert_eq!(1, prompt_fragments.len());
    assert_eq!(expected_catalog, prompt_fragments[0].text());
    assert_eq!(
        vec![("user", expected_skill)],
        fragments
            .iter()
            .map(|fragment| (fragment.role(), fragment.render()))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        turn_store
            .get::<ExplicitConnectorMentions>()
            .ok_or("expected connector mentions")?
            .resolve(&[]),
        ["calendar".to_string()].into_iter().collect()
    );
    assert_eq!(
        turn_store
            .get::<McpServerDependencies>()
            .ok_or("expected MCP dependencies")?
            .missing_from(&HashMap::new())
            .keys()
            .cloned()
            .collect::<Vec<_>>(),
        vec!["docs".to_string()]
    );
    std::fs::remove_dir_all(codex_home)?;
    Ok(())
}

#[tokio::test]
async fn selected_executor_catalog_is_context_and_selected_entrypoint_is_turn_input() -> TestResult
{
    let read_requests = Arc::new(Mutex::new(Vec::new()));
    let mut executor_entries = vec![test_entry(
        SkillSourceKind::Executor,
        "env-1",
        "executor/lint-fix",
        "lint-fix/SKILL.md",
    )];
    executor_entries.extend((0..20).map(|index| {
        test_entry(
            SkillSourceKind::Executor,
            "env-1",
            &format!("executor/extra-{index}"),
            &format!("extra-{index}/SKILL.md"),
        )
        .with_short_description(Some("x".repeat(1_000)))
    }));
    let executor_provider = Arc::new(StaticSkillProvider {
        catalog: SkillCatalog {
            entries: executor_entries,
            warnings: Vec::new(),
        },
        contents: STATIC_SKILL_CONTENTS.to_string(),
        read_requests: Arc::clone(&read_requests),
        list_calls: None,
        fail_first_list: false,
    });
    let providers = SkillProviders::new()
        .with_host_provider(Arc::new(StaticSkillProvider {
            catalog: SkillCatalog {
                entries: vec![test_entry(
                    SkillSourceKind::Host,
                    "host",
                    "host/host-demo",
                    "host-demo/SKILL.md",
                )],
                warnings: Vec::new(),
            },
            contents: STATIC_SKILL_CONTENTS.to_string(),
            read_requests: Arc::new(Mutex::new(Vec::new())),
            list_calls: None,
            fail_first_list: false,
        }))
        .with_executor_provider(executor_provider);
    let (event_tx, event_rx) = std::sync::mpsc::channel();
    let mut builder =
        ExtensionRegistryBuilder::with_event_sink(Arc::new(ChannelEventSink(event_tx)));
    install_with_providers(&mut builder, providers, skills_extension_config);
    let registry = builder.build();

    let session_store = ExtensionData::new("session");
    let thread_store = ExtensionData::new("thread");
    thread_store.insert(vec![SelectedCapabilityRoot {
        id: "lint-fix".to_string(),
        location: CapabilityRootLocation::Environment {
            environment_id: "env-1".to_string(),
            path: "/skills/lint-fix".to_string(),
        },
    }]);
    let session_source = SessionSource::Cli;
    let config = default_config();
    registry.thread_lifecycle_contributors()[0]
        .on_thread_start(ThreadStartInput {
            config: &config,
            session_source: &session_source,
            persistent_thread_state_available: true,
            environments: &[],
            session_store: &session_store,
            thread_store: &thread_store,
        })
        .await;

    let initial_turn_store = ExtensionData::new("initial-turn");
    let mut outcome = SkillLoadOutcome::default();
    outcome.skills.push(SkillMetadata {
        name: "host-demo".to_string(),
        description: "x".repeat(40_000),
        short_description: None,
        interface: None,
        dependencies: None,
        policy: None,
        path_to_skills_md: AbsolutePathBuf::try_from(
            std::env::temp_dir().join("host-demo").join("SKILL.md"),
        )?,
        scope: SkillScope::User,
        plugin_id: None,
    });
    initial_turn_store.insert(HostSkillsSnapshot::new(Arc::new(outcome)));
    let prompt_fragments = registry.context_contributors()[0]
        .contribute(ContextContributionContext {
            thread_id: ThreadId::default(),
            session_store: &session_store,
            thread_store: &thread_store,
            turn_store: &initial_turn_store,
            model_context_window: Some(2_000_000),
        })
        .await;
    assert_eq!(1, prompt_fragments.len());
    assert!(
        prompt_fragments[0]
            .text()
            .starts_with(SKILLS_INSTRUCTIONS_OPEN_TAG)
    );
    assert!(prompt_fragments[0].text().contains("lint-fix"));
    assert!(prompt_fragments[0].text().contains("host-demo"));
    assert!(
        prompt_fragments[0]
            .text()
            .contains("(environment resource: skill://executor/lint-fix/SKILL.md)")
    );
    assert!(approx_token_count(prompt_fragments[0].text()) <= 10_000);
    let EventMsg::Warning(warning) = event_rx.try_recv()?.1.msg else {
        panic!("expected host budget warning");
    };
    assert!(warning.message.contains("skills context budget"));
    assert!(!warning.message.contains("2% skills context budget"));

    let turn_store = ExtensionData::new("turn-1");
    let fragments = registry.turn_input_contributors()[0]
        .contribute(
            TurnInputContext {
                thread_id: ThreadId::default(),
                turn_id: "turn-1".to_string(),
                model: "test-model".to_string(),
                user_input: vec![UserInput::Text {
                    text: "$lint-fix please".to_string(),
                    text_elements: Vec::new(),
                }],
                reserved_plain_tool_names: Default::default(),
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
    let rebuilt_prompt_fragments = registry.context_contributors()[0]
        .contribute(prompt_context(&session_store, &thread_store, &turn_store))
        .await;
    assert_eq!(1, rebuilt_prompt_fragments.len());
    assert!(rebuilt_prompt_fragments[0].text().contains("lint-fix"));

    let next_turn_store = ExtensionData::new("turn-2");
    let next_fragments = registry.turn_input_contributors()[0]
        .contribute(
            TurnInputContext {
                thread_id: ThreadId::default(),
                turn_id: "turn-2".to_string(),
                model: "test-model".to_string(),
                user_input: vec![UserInput::Text {
                    text: "no skill this time".to_string(),
                    text_elements: Vec::new(),
                }],
                reserved_plain_tool_names: Default::default(),
                environments: Vec::new(),
            },
            &session_store,
            &thread_store,
            &next_turn_store,
        )
        .await;

    assert!(next_fragments.is_empty());

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
            contents: STATIC_SKILL_CONTENTS.to_string(),
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
            session_store: &session_store,
            thread_store: &thread_store,
        })
        .await;

    let initial_fragments = registry.context_contributors()[0]
        .contribute(prompt_context(
            &session_store,
            &thread_store,
            &ExtensionData::new("initial-turn"),
        ))
        .await;
    assert!(initial_fragments.is_empty());
    let EventMsg::Warning(warning) = event_rx.try_recv()?.1.msg else {
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
                    thread_id: ThreadId::default(),
                    turn_id: turn_id.to_string(),
                    model: "test-model".to_string(),
                    user_input: vec![UserInput::Text {
                        text: "$first".to_string(),
                        text_elements: Vec::new(),
                    }],
                    reserved_plain_tool_names: Default::default(),
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
        contents: STATIC_SKILL_CONTENTS.to_string(),
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
    thread_store.insert(
        [("root-a", "/skills/root-a"), ("root-b", "/skills/root-b")]
            .into_iter()
            .map(|(id, path)| SelectedCapabilityRoot {
                id: id.to_string(),
                location: CapabilityRootLocation::Environment {
                    environment_id: "env-1".to_string(),
                    path: path.to_string(),
                },
            })
            .collect::<Vec<_>>(),
    );
    let session_source = SessionSource::Cli;
    let config = default_config();
    registry.thread_lifecycle_contributors()[0]
        .on_thread_start(ThreadStartInput {
            config: &config,
            session_source: &session_source,
            persistent_thread_state_available: true,
            environments: &[],
            session_store: &session_store,
            thread_store: &thread_store,
        })
        .await;

    let fragments = registry.turn_input_contributors()[0]
        .contribute(
            TurnInputContext {
                thread_id: ThreadId::default(),
                turn_id: "turn-1".to_string(),
                model: "test-model".to_string(),
                user_input: vec![UserInput::Mention {
                    name: "lint-fix".to_string(),
                    path: root_b_locator.to_string(),
                }],
                reserved_plain_tool_names: Default::default(),
                environments: Vec::new(),
            },
            &session_store,
            &thread_store,
            &ExtensionData::new("turn-1"),
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
        contents: STATIC_SKILL_CONTENTS.to_string(),
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
            session_store: &session_store,
            thread_store: &thread_store,
        })
        .await;

    let initial_fragments = registry.context_contributors()[0]
        .contribute(prompt_context(
            &session_store,
            &thread_store,
            &ExtensionData::new("initial-turn"),
        ))
        .await;
    assert_eq!(1, initial_fragments.len());
    assert!(initial_fragments[0].text().contains("visible-skill"));
    assert!(!initial_fragments[0].text().contains("hidden-skill"));

    let fragments = registry.turn_input_contributors()[0]
        .contribute(
            TurnInputContext {
                thread_id: ThreadId::default(),
                turn_id: "turn-1".to_string(),
                model: "test-model".to_string(),
                user_input: vec![UserInput::Text {
                    text: "$hidden-skill".to_string(),
                    text_elements: Vec::new(),
                }],
                reserved_plain_tool_names: Default::default(),
                environments: Vec::new(),
            },
            &session_store,
            &thread_store,
            &ExtensionData::new("turn-1"),
        )
        .await;

    assert_eq!(1, fragments.len());
    assert!(fragments[0].render().contains("<name>hidden-skill</name>"));
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

#[tokio::test]
async fn host_plain_mentions_preserve_legacy_disambiguation() -> TestResult {
    let read_requests = Arc::new(Mutex::new(Vec::new()));
    let calendar_locator = "skill://host/calendar/SKILL.md";
    let provider = Arc::new(StaticSkillProvider {
        catalog: SkillCatalog {
            entries: vec![
                SkillCatalogEntry::new(
                    SkillPackageId("host/deploy-user".to_string()),
                    SkillAuthority::new(SkillSourceKind::Host, "host"),
                    "deploy",
                    "Deploy from the user scope.",
                    SkillResourceId::new("deploy-user/SKILL.md"),
                ),
                SkillCatalogEntry::new(
                    SkillPackageId("host/deploy-repo".to_string()),
                    SkillAuthority::new(SkillSourceKind::Host, "host"),
                    "deploy",
                    "Deploy from the repo scope.",
                    SkillResourceId::new("deploy-repo/SKILL.md"),
                ),
                SkillCatalogEntry::new(
                    SkillPackageId("host/calendar".to_string()),
                    SkillAuthority::new(SkillSourceKind::Host, "host"),
                    "calendar",
                    "Use the calendar skill.",
                    SkillResourceId::new("calendar/SKILL.md"),
                )
                .with_display_path(calendar_locator),
            ],
            warnings: Vec::new(),
        },
        contents: STATIC_SKILL_CONTENTS.to_string(),
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
            session_store: &session_store,
            thread_store: &thread_store,
        })
        .await;

    let ambiguous_fragments = registry.turn_input_contributors()[0]
        .contribute(
            TurnInputContext {
                thread_id: ThreadId::default(),
                turn_id: "turn-ambiguous".to_string(),
                model: "test-model".to_string(),
                user_input: vec![UserInput::Text {
                    text: "$deploy".to_string(),
                    text_elements: Vec::new(),
                }],
                reserved_plain_tool_names: Default::default(),
                environments: Vec::new(),
            },
            &session_store,
            &thread_store,
            &ExtensionData::new("turn-ambiguous"),
        )
        .await;
    assert!(ambiguous_fragments.is_empty());

    let connector_conflict_fragments = registry.turn_input_contributors()[0]
        .contribute(
            TurnInputContext {
                thread_id: ThreadId::default(),
                turn_id: "turn-connector-conflict".to_string(),
                model: "test-model".to_string(),
                user_input: vec![UserInput::Text {
                    text: "$calendar".to_string(),
                    text_elements: Vec::new(),
                }],
                reserved_plain_tool_names: ["calendar".to_string()].into_iter().collect(),
                environments: Vec::new(),
            },
            &session_store,
            &thread_store,
            &ExtensionData::new("turn-connector-conflict"),
        )
        .await;
    assert!(connector_conflict_fragments.is_empty());

    let explicit_fragments = registry.turn_input_contributors()[0]
        .contribute(
            TurnInputContext {
                thread_id: ThreadId::default(),
                turn_id: "turn-explicit".to_string(),
                model: "test-model".to_string(),
                user_input: vec![UserInput::Mention {
                    name: "calendar".to_string(),
                    path: calendar_locator.to_string(),
                }],
                reserved_plain_tool_names: ["calendar".to_string()].into_iter().collect(),
                environments: Vec::new(),
            },
            &session_store,
            &thread_store,
            &ExtensionData::new("turn-explicit"),
        )
        .await;
    assert_eq!(1, explicit_fragments.len());
    assert_eq!(
        vec![(
            SkillAuthority::new(SkillSourceKind::Host, "host"),
            SkillPackageId("host/calendar".to_string()),
            SkillResourceId::new("calendar/SKILL.md"),
        )],
        read_request_keys(&read_requests)
    );

    Ok(())
}

#[tokio::test]
async fn connector_mentions_only_include_injected_skill_text() -> TestResult {
    let provider = Arc::new(StaticSkillProvider {
        catalog: SkillCatalog {
            entries: vec![test_entry(
                SkillSourceKind::Executor,
                "env-1",
                "executor/lint-fix",
                "lint-fix/SKILL.md",
            )],
            warnings: Vec::new(),
        },
        contents: format!("{}[$calendar](app://calendar)", "x".repeat(9_000)),
        read_requests: Arc::new(Mutex::new(Vec::new())),
        list_calls: None,
        fail_first_list: false,
    });
    let providers = SkillProviders::new().with_executor_provider(provider);
    let mut builder = ExtensionRegistryBuilder::new();
    install_with_providers(&mut builder, providers, skills_extension_config);
    let registry = builder.build();
    let session_store = ExtensionData::new("session");
    let thread_store = ExtensionData::new("thread");
    thread_store.insert(vec![SelectedCapabilityRoot {
        id: "lint-fix".to_string(),
        location: CapabilityRootLocation::Environment {
            environment_id: "env-1".to_string(),
            path: "/skills/lint-fix".to_string(),
        },
    }]);
    let session_source = SessionSource::Cli;
    let config = default_config();
    registry.thread_lifecycle_contributors()[0]
        .on_thread_start(ThreadStartInput {
            config: &config,
            session_source: &session_source,
            persistent_thread_state_available: true,
            environments: &[],
            session_store: &session_store,
            thread_store: &thread_store,
        })
        .await;

    let turn_store = ExtensionData::new("turn-1");
    let fragments = registry.turn_input_contributors()[0]
        .contribute(
            TurnInputContext {
                thread_id: ThreadId::default(),
                turn_id: "turn-1".to_string(),
                model: "test-model".to_string(),
                user_input: vec![UserInput::Text {
                    text: "$lint-fix".to_string(),
                    text_elements: Vec::new(),
                }],
                reserved_plain_tool_names: Default::default(),
                environments: Vec::new(),
            },
            &session_store,
            &thread_store,
            &turn_store,
        )
        .await;

    assert_eq!(1, fragments.len());
    assert!(!fragments[0].render().contains("app://calendar"));
    assert!(turn_store.get::<ExplicitConnectorMentions>().is_none());

    Ok(())
}

#[derive(Clone)]
struct StaticSkillProvider {
    catalog: SkillCatalog,
    contents: String,
    read_requests: Arc<Mutex<Vec<SkillReadRequest>>>,
    list_calls: Option<Arc<AtomicUsize>>,
    fail_first_list: bool,
}

struct ChannelEventSink(std::sync::mpsc::Sender<(ThreadId, Event)>);

impl ExtensionEventSink for ChannelEventSink {
    fn emit(&self, thread_id: ThreadId, event: Event) {
        let _ = self.0.send((thread_id, event));
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
        let contents = self.contents.clone();
        let read_requests = Arc::clone(&self.read_requests);
        Box::pin(async move {
            read_requests
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(request.clone());
            Ok(SkillReadResult {
                resource: request.resource,
                contents,
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
}

fn default_config() -> TestConfig {
    TestConfig {
        include_instructions: true,
        bundled_skills_enabled: true,
    }
}

fn skills_extension_config(config: &TestConfig) -> SkillsExtensionConfig {
    SkillsExtensionConfig {
        include_instructions: config.include_instructions,
        bundled_skills_enabled: config.bundled_skills_enabled,
        host: None,
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

fn prompt_context<'a>(
    session_store: &'a ExtensionData,
    thread_store: &'a ExtensionData,
    turn_store: &'a ExtensionData,
) -> ContextContributionContext<'a> {
    ContextContributionContext {
        thread_id: ThreadId::default(),
        session_store,
        thread_store,
        turn_store,
        model_context_window: None,
    }
}
