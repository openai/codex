use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use codex_config::ConfigLayerEntry;
use codex_config::ConfigLayerSource;
use codex_config::ConfigLayerStack;
use codex_config::ConfigRequirementsToml;
use codex_exec_server::CapabilityRootDiscovery;
use codex_exec_server::ExecutorCapabilityDiscoverySnapshot;
use codex_exec_server::LOCAL_FS;
use codex_extension_api::ConversationHistory;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionEventSink;
use codex_extension_api::ExtensionMetrics;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::ExtensionWarning;
use codex_extension_api::NoopTurnItemEmitter;
use codex_extension_api::PreviousWorldStateSection;
use codex_extension_api::RenderedWorldStateFragment;
use codex_extension_api::SkillInvocationInput;
use codex_extension_api::SkillInvocationKind;
use codex_extension_api::ThreadStartInput;
use codex_extension_api::ToolCall;
use codex_extension_api::ToolPayload;
use codex_extension_api::TurnInputContext;
use codex_extension_api::WorldStateContributionInput;
use codex_extension_api::WorldStateSectionContribution;
use codex_models_manager::model_info::model_info_from_slug;
use codex_otel::MetricsClient;
use codex_otel::MetricsConfig;
use codex_otel::THREAD_SKILLS_DESCRIPTION_TRUNCATED_CHARS_METRIC;
use codex_otel::THREAD_SKILLS_ENABLED_TOTAL_METRIC;
use codex_otel::THREAD_SKILLS_KEPT_TOTAL_METRIC;
use codex_otel::THREAD_SKILLS_TRUNCATED_METRIC;
use codex_protocol::capabilities::CapabilityRootLocation;
use codex_protocol::capabilities::SelectedCapabilityRoot;
use codex_protocol::protocol::EnvironmentConfigState;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::SKILLS_INSTRUCTIONS_CLOSE_TAG;
use codex_protocol::protocol::SKILLS_INSTRUCTIONS_OPEN_TAG;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SkillScope;
use codex_protocol::protocol::TruncationPolicy;
use codex_protocol::protocol::TurnEnvironmentSelection;
use codex_protocol::user_input::UserInput;
use codex_skills::SkillMetadata;
use codex_skills_extension::HostSkillProvider;
use codex_skills_extension::HostSkillsLoadInput;
use codex_skills_extension::HostSkillsService;
use codex_skills_extension::HostSkillsSnapshot;
use codex_skills_extension::InjectedHostSkillPrompts;
use codex_skills_extension::SkillLoadOutcome;
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
use codex_skills_extension::install_with_providers_and_metrics;
use codex_skills_extension::provider::SkillListQuery;
use codex_skills_extension::provider::SkillProvider;
use codex_skills_extension::provider::SkillProviderFuture;
use codex_skills_extension::provider::SkillReadRequest;
use codex_skills_extension::provider::SkillSearchRequest;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
use opentelemetry_sdk::metrics::InMemoryMetricExporter;
use opentelemetry_sdk::metrics::data::AggregatedMetrics;
use opentelemetry_sdk::metrics::data::MetricData;
use pretty_assertions::assert_eq;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[path = "skills_extension/shadow_task_context_tests.rs"]
mod shadow_task_context_tests;

static NEXT_CODEX_HOME_ID: AtomicUsize = AtomicUsize::new(0);
const SKILLS_INTRO_WITH_ABSOLUTE_PATHS: &str = "A skill is a set of instructions provided through a `SKILL.md` source. Below is the list of skills that can be used. Each entry includes a name, description, and source locator. `file` locators are on the host filesystem, `executor package` locators are owned by their execution environment, `orchestrator package` locators are opaque package identifiers, and `custom resource` locators use their provider's access mechanism.";
const DEMO_SKILL_CONTENTS: &str =
    "---\nname: demo\ndescription: Demo skill.\n---\n# Demo\n\nUse the demo skill.\n";

fn world_state_section<'a>(
    sections: &'a [WorldStateSectionContribution],
    id: &str,
) -> &'a WorldStateSectionContribution {
    sections
        .iter()
        .find(|section| section.id() == id)
        .unwrap_or_else(|| panic!("world-state section {id} should exist"))
}

async fn skill_world_state_fragments(
    registry: &codex_extension_api::ExtensionRegistry<TestConfig>,
    session_store: &ExtensionData,
    thread_store: &ExtensionData,
    turn_id: &str,
) -> Result<(RenderedWorldStateFragment, RenderedWorldStateFragment), Box<dyn std::error::Error>> {
    let turn_store = ExtensionData::new(turn_id);
    let selected_roots = [SelectedCapabilityRoot {
        id: "skills".to_string(),
        location: CapabilityRootLocation::Environment {
            environment_id: "env-1".to_string(),
            path: PathUri::parse("file:///skills")?,
        },
    }];
    let sections = registry.context_contributors()[0]
        .contribute_world_state(WorldStateContributionInput {
            thread_id: codex_protocol::ThreadId::new(),
            turn_id,
            environments: &[],
            ready_selected_capability_roots: &selected_roots,
            executor_capability_discovery: None,
            extension_metrics: None,
            session_store,
            thread_store,
            turn_store: &turn_store,
        })
        .await;

    let executor = world_state_section(&sections, "skills")
        .render_diff(PreviousWorldStateSection::Absent)
        .ok_or("executor skills should render through world state")?;
    let orchestrator = world_state_section(&sections, "orchestrator_skills")
        .render_diff(PreviousWorldStateSection::Absent)
        .ok_or("orchestrator skills should render through world state")?;
    Ok((executor, orchestrator))
}

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
            extension_metrics: None,
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
        remote_plugin_id: None,
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
            /*extension_metrics*/ None,
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
async fn host_world_state_records_catalog_metrics_on_publish_and_change() -> TestResult {
    let mut builder = ExtensionRegistryBuilder::new();
    install(&mut builder, skills_extension_config);
    let registry = builder.build();
    let startup_metrics = Arc::new(RecordingMetrics::default());
    let turn_metrics = Arc::new(RecordingMetrics::default());
    let session_store = ExtensionData::new("session");
    let thread_store = ExtensionData::new("thread");
    let config = default_config();
    registry.thread_lifecycle_contributors()[0]
        .on_thread_start(ThreadStartInput {
            config: &config,
            session_source: &SessionSource::Cli,
            persistent_thread_state_available: true,
            environments: &[],
            mcp_resource_client: None,
            extension_metrics: Some(startup_metrics.clone()),
            session_store: &session_store,
            thread_store: &thread_store,
        })
        .await;

    let skill_path = AbsolutePathBuf::try_from(test_codex_home().join("skills/demo/SKILL.md"))?;
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
        remote_plugin_id: None,
    });
    let turn_store = ExtensionData::new("turn-1");
    turn_store.insert(HostSkillsSnapshot::new(Arc::new(outcome.clone())));

    let sections = registry.context_contributors()[0]
        .contribute_world_state(WorldStateContributionInput {
            thread_id: codex_protocol::ThreadId::new(),
            turn_id: "turn-1",
            environments: &[],
            ready_selected_capability_roots: &[],
            executor_capability_discovery: None,
            extension_metrics: Some(turn_metrics.clone()),
            session_store: &session_store,
            thread_store: &thread_store,
            turn_store: &turn_store,
        })
        .await;

    assert_eq!(sections.len(), 2);
    let host_section = world_state_section(&sections, "host_skills");
    let published_snapshot = host_section.snapshot().clone();
    assert!(
        host_section
            .render_diff(PreviousWorldStateSection::Absent)
            .is_some()
    );
    let mut expected = expected_catalog_metric_samples("host_world_state", /*count*/ 1);
    assert!(startup_metrics.samples().is_empty());
    assert_eq!(turn_metrics.samples(), expected);

    let sections = registry.context_contributors()[0]
        .contribute_world_state(WorldStateContributionInput {
            thread_id: codex_protocol::ThreadId::new(),
            turn_id: "turn-1",
            environments: &[],
            ready_selected_capability_roots: &[],
            executor_capability_discovery: None,
            extension_metrics: Some(turn_metrics.clone()),
            session_store: &session_store,
            thread_store: &thread_store,
            turn_store: &turn_store,
        })
        .await;
    assert!(
        world_state_section(&sections, "host_skills")
            .render_diff(PreviousWorldStateSection::Known(&published_snapshot))
            .is_none()
    );
    assert!(startup_metrics.samples().is_empty());
    assert_eq!(turn_metrics.samples(), expected);

    let second_skill_path =
        AbsolutePathBuf::try_from(test_codex_home().join("skills/other/SKILL.md"))?;
    outcome.skills.push(SkillMetadata {
        name: "other".to_string(),
        description: "Other skill.".to_string(),
        short_description: None,
        interface: None,
        dependencies: None,
        policy: None,
        path_to_skills_md: second_skill_path,
        scope: SkillScope::User,
        plugin_id: None,
        remote_plugin_id: None,
    });
    turn_store.insert(HostSkillsSnapshot::new(Arc::new(outcome)));
    let sections = registry.context_contributors()[0]
        .contribute_world_state(WorldStateContributionInput {
            thread_id: codex_protocol::ThreadId::new(),
            turn_id: "turn-1",
            environments: &[],
            ready_selected_capability_roots: &[],
            executor_capability_discovery: None,
            extension_metrics: Some(turn_metrics.clone()),
            session_store: &session_store,
            thread_store: &thread_store,
            turn_store: &turn_store,
        })
        .await;
    assert!(
        world_state_section(&sections, "host_skills")
            .render_diff(PreviousWorldStateSection::Known(&published_snapshot))
            .is_some()
    );
    expected.extend(expected_catalog_metric_samples(
        "host_world_state",
        /*count*/ 2,
    ));
    assert!(startup_metrics.samples().is_empty());
    assert_eq!(turn_metrics.samples(), expected);

    Ok(())
}

#[tokio::test]
async fn persisted_host_snapshot_deduplicates_warning_after_reinitialization() -> TestResult {
    let (event_tx, event_rx) = std::sync::mpsc::channel();
    let mut builder =
        ExtensionRegistryBuilder::with_event_sink(Arc::new(ChannelEventSink(event_tx)));
    install(&mut builder, skills_extension_config);
    let registry = builder.build();
    let session_store = ExtensionData::new("session");
    let config = default_config();
    let skill_path = AbsolutePathBuf::try_from(test_codex_home().join("skills/demo/SKILL.md"))?;
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
        remote_plugin_id: None,
    });
    let host_snapshot = HostSkillsSnapshot::new(Arc::new(outcome));

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
    let mut model_info = model_info_from_slug("test-model");
    model_info.context_window = Some(50);
    thread_store.insert(model_info);
    let turn_store = ExtensionData::new("turn-1");
    turn_store.insert(host_snapshot.clone());
    let sections = registry.context_contributors()[0]
        .contribute_world_state(WorldStateContributionInput {
            thread_id: codex_protocol::ThreadId::new(),
            turn_id: "turn-1",
            environments: &[],
            ready_selected_capability_roots: &[],
            executor_capability_discovery: None,
            extension_metrics: None,
            session_store: &session_store,
            thread_store: &thread_store,
            turn_store: &turn_store,
        })
        .await;
    let host_section = world_state_section(&sections, "host_skills");
    let published_snapshot = host_section.snapshot().clone();
    assert!(
        host_section
            .render_diff(PreviousWorldStateSection::Absent)
            .is_some()
    );
    event_rx.try_recv()?.into_warning();
    assert!(event_rx.try_recv().is_err());

    let resumed_thread_store = ExtensionData::new("thread");
    registry.thread_lifecycle_contributors()[0]
        .on_thread_start(ThreadStartInput {
            config: &config,
            session_source: &SessionSource::Cli,
            persistent_thread_state_available: true,
            environments: &[],
            mcp_resource_client: None,
            extension_metrics: None,
            session_store: &session_store,
            thread_store: &resumed_thread_store,
        })
        .await;
    let mut model_info = model_info_from_slug("test-model");
    model_info.context_window = Some(50);
    resumed_thread_store.insert(model_info);
    let resumed_turn_store = ExtensionData::new("turn-2");
    resumed_turn_store.insert(host_snapshot);
    let sections = registry.context_contributors()[0]
        .contribute_world_state(WorldStateContributionInput {
            thread_id: codex_protocol::ThreadId::new(),
            turn_id: "turn-2",
            environments: &[],
            ready_selected_capability_roots: &[],
            executor_capability_discovery: None,
            extension_metrics: None,
            session_store: &session_store,
            thread_store: &resumed_thread_store,
            turn_store: &resumed_turn_store,
        })
        .await;
    assert!(
        world_state_section(&sections, "host_skills")
            .render_diff(PreviousWorldStateSection::Known(&published_snapshot))
            .is_none()
    );
    assert!(event_rx.try_recv().is_err());

    Ok(())
}

#[tokio::test]
async fn executor_orchestrator_and_host_share_catalog_world_state_flow() -> TestResult {
    let provider = |entry: SkillCatalogEntry| {
        Arc::new(StaticSkillProvider {
            catalog: SkillCatalog {
                entries: vec![entry],
                warnings: Vec::new(),
            },
            read_requests: Arc::new(Mutex::new(Vec::new())),
            list_calls: None,
            fail_first_list: false,
        })
    };
    let providers = SkillProviders::new()
        .with_executor_provider(provider(test_entry(
            SkillSourceKind::Executor,
            "env-1",
            "executor/executor-skill",
            "skill://executor/executor-skill/SKILL.md",
        )))
        .with_orchestrator_provider(provider(test_entry(
            SkillSourceKind::Orchestrator,
            "codex_apps",
            "orchestrator/orchestrator-skill",
            "skill://orchestrator/orchestrator-skill/SKILL.md",
        )))
        .with_host_provider(provider(
            test_entry(
                SkillSourceKind::Host,
                "host",
                "host/host-skill",
                "/skills/host-skill/SKILL.md",
            )
            .with_display_path("/skills/host-skill/SKILL.md"),
        ));
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

    let selected_roots = [SelectedCapabilityRoot {
        id: "skills".to_string(),
        location: CapabilityRootLocation::Environment {
            environment_id: "env-1".to_string(),
            path: PathUri::parse("file:///skills")?,
        },
    }];
    let turn_store = ExtensionData::new("turn-1");
    turn_store.insert(HostSkillsSnapshot::new(Arc::new(
        SkillLoadOutcome::default(),
    )));
    let metrics = Arc::new(RecordingMetrics::default());
    let sections = registry.context_contributors()[0]
        .contribute_world_state(WorldStateContributionInput {
            thread_id: codex_protocol::ThreadId::new(),
            turn_id: "turn-1",
            environments: &[],
            ready_selected_capability_roots: &selected_roots,
            executor_capability_discovery: None,
            extension_metrics: Some(metrics.clone()),
            session_store: &session_store,
            thread_store: &thread_store,
            turn_store: &turn_store,
        })
        .await;

    assert_eq!(
        sections
            .iter()
            .map(WorldStateSectionContribution::id)
            .collect::<Vec<_>>(),
        vec!["skills", "orchestrator_skills", "host_skills"]
    );
    assert!(metrics.samples().is_empty());

    for (section_id, expected_line) in [
        (
            "skills",
            "- executor-skill: Fix lint errors. (executor package: executor/executor-skill)",
        ),
        (
            "orchestrator_skills",
            "- orchestrator-skill: Fix lint errors. (orchestrator package: orchestrator/orchestrator-skill)",
        ),
        (
            "host_skills",
            "- host-skill: Fix lint errors. (file: /skills/host-skill/SKILL.md)",
        ),
    ] {
        let fragment = world_state_section(&sections, section_id)
            .render_diff(PreviousWorldStateSection::Absent)
            .ok_or("skill catalog should render through world state")?;
        assert!(fragment.body().contains(expected_line));
    }

    let expected_metrics = [
        "executor_world_state",
        "orchestrator_world_state",
        "host_world_state",
    ]
    .into_iter()
    .flat_map(|surface| expected_catalog_metric_samples(surface, /*count*/ 1))
    .collect::<Vec<_>>();
    assert_eq!(metrics.samples(), expected_metrics);

    Ok(())
}

#[tokio::test]
async fn nonempty_executor_empty_host_records_catalog_metrics() -> TestResult {
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
        read_requests: Arc::new(Mutex::new(Vec::new())),
        list_calls: None,
        fail_first_list: false,
    });
    let providers = SkillProviders::new()
        .with_host_provider(Arc::new(HostSkillProvider::new()))
        .with_executor_provider(executor_provider);
    let mut builder = ExtensionRegistryBuilder::new();
    install_with_providers(&mut builder, providers, skills_extension_config);
    let registry = builder.build();
    let metrics = Arc::new(RecordingMetrics::default());
    let session_store = ExtensionData::new("session");
    let thread_store = ExtensionData::new("thread");
    let config = default_config();
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

    let selected_roots = vec![SelectedCapabilityRoot {
        id: "lint-fix".to_string(),
        location: CapabilityRootLocation::Environment {
            environment_id: "env-1".to_string(),
            path: PathUri::parse("file:///skills/lint-fix").expect("skill root URI"),
        },
    }];
    let turn_store = ExtensionData::new("turn-1");
    turn_store.insert(HostSkillsSnapshot::new(Arc::new(
        SkillLoadOutcome::default(),
    )));

    let sections = registry.context_contributors()[0]
        .contribute_world_state(WorldStateContributionInput {
            thread_id: codex_protocol::ThreadId::new(),
            turn_id: "turn-1",
            environments: &[],
            ready_selected_capability_roots: &selected_roots,
            executor_capability_discovery: None,
            extension_metrics: Some(metrics.clone()),
            session_store: &session_store,
            thread_store: &thread_store,
            turn_store: &turn_store,
        })
        .await;

    assert_eq!(sections.len(), 2);
    assert!(
        sections[0]
            .render_diff(PreviousWorldStateSection::Absent)
            .is_some()
    );
    assert!(
        world_state_section(&sections, "host_skills")
            .render_diff(PreviousWorldStateSection::Absent)
            .is_none()
    );
    let expected = expected_catalog_metric_samples("executor_world_state", /*count*/ 1);
    assert_eq!(metrics.samples(), expected);
    Ok(())
}

#[tokio::test]
async fn host_world_state_uses_provider_catalog_with_core_compatible_rendering() -> TestResult {
    let list_calls = Arc::new(AtomicUsize::new(0));
    let provider = Arc::new(StaticSkillProvider {
        catalog: SkillCatalog {
            entries: vec![
                test_entry(
                    SkillSourceKind::Host,
                    "host",
                    "host/provider-skill",
                    "provider-skill/SKILL.md",
                )
                .with_short_description(Some("Short description.".to_string())),
            ],
            warnings: Vec::new(),
        },
        read_requests: Arc::new(Mutex::new(Vec::new())),
        list_calls: Some(Arc::clone(&list_calls)),
        fail_first_list: false,
    });
    let mut builder = ExtensionRegistryBuilder::new();
    install_with_providers(
        &mut builder,
        SkillProviders::new().with_host_provider(provider),
        skills_extension_config,
    );
    let registry = builder.build();
    let session_store = ExtensionData::new("session");
    let thread_store = ExtensionData::new("thread");
    let config = default_config();
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
    let turn_store = ExtensionData::new("turn-1");
    turn_store.insert(HostSkillsSnapshot::new(Arc::new(
        SkillLoadOutcome::default(),
    )));

    let sections = registry.context_contributors()[0]
        .contribute_world_state(WorldStateContributionInput {
            thread_id: codex_protocol::ThreadId::new(),
            turn_id: "turn-1",
            environments: &[],
            ready_selected_capability_roots: &[],
            executor_capability_discovery: None,
            extension_metrics: None,
            session_store: &session_store,
            thread_store: &thread_store,
            turn_store: &turn_store,
        })
        .await;
    let host_fragment = world_state_section(&sections, "host_skills")
        .render_diff(PreviousWorldStateSection::Absent)
        .ok_or("host provider catalog should render")?;

    assert!(host_fragment.body().contains("Fix lint errors."));
    assert!(!host_fragment.body().contains("Short description."));
    assert_eq!(1, list_calls.load(Ordering::Relaxed));
    Ok(())
}

#[tokio::test]
async fn shadow_selection_uses_host_catalog_when_instructions_are_disabled() -> TestResult {
    let list_calls = Arc::new(AtomicUsize::new(0));
    let provider = Arc::new(StaticSkillProvider {
        catalog: SkillCatalog {
            entries: vec![test_entry(
                SkillSourceKind::Host,
                "host",
                "host/lint-fix",
                "lint-fix/SKILL.md",
            )],
            warnings: Vec::new(),
        },
        read_requests: Arc::new(Mutex::new(Vec::new())),
        list_calls: Some(Arc::clone(&list_calls)),
        fail_first_list: false,
    });
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
    let thread_store = ExtensionData::new("thread");
    let mut config = default_config();
    config.include_instructions = false;
    config.shadow_selection_enabled = true;
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
    let turn_store = ExtensionData::new("turn-1");
    turn_store.insert(HostSkillsSnapshot::new(Arc::new(
        SkillLoadOutcome::default(),
    )));

    let sections = registry.context_contributors()[0]
        .contribute_world_state(WorldStateContributionInput {
            thread_id: codex_protocol::ThreadId::new(),
            turn_id: "turn-1",
            environments: &[],
            ready_selected_capability_roots: &[],
            executor_capability_discovery: None,
            extension_metrics: None,
            session_store: &session_store,
            thread_store: &thread_store,
            turn_store: &turn_store,
        })
        .await;
    let fragments = registry.turn_input_contributors()[0]
        .contribute(
            TurnInputContext {
                turn_id: "turn-1".to_string(),
                user_input: vec![UserInput::Text {
                    text: "Fix lint errors.".to_string(),
                    text_elements: Vec::new(),
                }],
                environments: Vec::new(),
            },
            /*extension_metrics*/ None,
            &session_store,
            &thread_store,
            &turn_store,
        )
        .await;

    assert!(
        world_state_section(&sections, "host_skills")
            .render_diff(PreviousWorldStateSection::Absent)
            .is_none()
    );
    assert!(fragments.is_empty());
    let snapshot = metrics.snapshot()?;
    let catalog_entry_counts = snapshot
        .scope_metrics()
        .flat_map(opentelemetry_sdk::metrics::data::ScopeMetrics::metrics)
        .find(|metric| metric.name() == "codex.skills.shadow_selection.catalog_entries")
        .map(|metric| match metric.data() {
            AggregatedMetrics::F64(MetricData::Histogram(histogram)) => histogram
                .data_points()
                .map(opentelemetry_sdk::metrics::data::HistogramDataPoint::sum)
                .collect::<Vec<_>>(),
            data => panic!("unexpected shadow catalog metric data: {data:?}"),
        })
        .ok_or("shadow catalog metric should be recorded")?;

    assert!(
        catalog_entry_counts.iter().all(|count| *count == 1.0),
        "every shadow selector should see the cached host skill: {catalog_entry_counts:?}"
    );
    assert_eq!(1, list_calls.load(Ordering::Relaxed));
    Ok(())
}

#[tokio::test]
async fn shadow_lru_selector_recovers_a_skill_invoked_on_an_earlier_turn() -> TestResult {
    let provider = Arc::new(StaticSkillProvider {
        catalog: SkillCatalog {
            entries: vec![test_entry(
                SkillSourceKind::Host,
                "host",
                "host/lint-fix",
                "lint-fix/SKILL.md",
            )],
            warnings: Vec::new(),
        },
        read_requests: Arc::new(Mutex::new(Vec::new())),
        list_calls: None,
        fail_first_list: false,
    });
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
    let thread_store = ExtensionData::new("thread");
    let mut config = default_config();
    config.include_instructions = false;
    config.shadow_selection_enabled = true;
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

    for (turn_id, text) in [("turn-1", "Fix lint errors."), ("turn-2", "continue")] {
        let turn_store = ExtensionData::new(turn_id);
        let fragments = registry.turn_input_contributors()[0]
            .contribute(
                TurnInputContext {
                    turn_id: turn_id.to_string(),
                    user_input: vec![UserInput::Text {
                        text: text.to_string(),
                        text_elements: Vec::new(),
                    }],
                    environments: Vec::new(),
                },
                /*extension_metrics*/ None,
                &session_store,
                &thread_store,
                &turn_store,
            )
            .await;
        assert!(fragments.is_empty());
        registry.skill_invocation_contributors()[0]
            .on_skill_invocation(SkillInvocationInput {
                session_store: &session_store,
                thread_store: &thread_store,
                turn_store: &turn_store,
                turn_id,
                skill_resource: "lint-fix/SKILL.md",
                kind: SkillInvocationKind::Implicit,
            })
            .await;
    }

    let snapshot = metrics.snapshot()?;
    let metric = snapshot
        .scope_metrics()
        .flat_map(opentelemetry_sdk::metrics::data::ScopeMetrics::metrics)
        .find(|metric| metric.name() == "codex.skills.shadow_selection.invocation")
        .ok_or("shadow invocation metric should be recorded")?;
    let mut selector_hits = match metric.data() {
        AggregatedMetrics::U64(MetricData::Sum(sum)) => sum
            .data_points()
            .filter_map(|point| {
                let method = point
                    .attributes()
                    .find(|attribute| attribute.key.as_str() == "method")?
                    .value
                    .as_str();
                if !matches!(
                    method.as_ref(),
                    "lru_v1"
                        | "lru_plus_lexical_v1"
                        | "lru_plus_character_routing_v1"
                        | "lru_plus_lexical_character_routing_v1"
                ) {
                    return None;
                }
                let hit = point
                    .attributes()
                    .find(|attribute| attribute.key.as_str() == "hit")?
                    .value
                    .as_str()
                    .to_string();
                Some((method.to_string(), hit, point.value()))
            })
            .collect::<Vec<_>>(),
        data => panic!("unexpected shadow invocation metric data: {data:?}"),
    };
    selector_hits.sort();

    assert_eq!(
        vec![
            (
                "lru_plus_character_routing_v1".to_string(),
                "true".to_string(),
                2,
            ),
            (
                "lru_plus_lexical_character_routing_v1".to_string(),
                "true".to_string(),
                2,
            ),
            ("lru_plus_lexical_v1".to_string(), "true".to_string(), 2),
            ("lru_v1".to_string(), "false".to_string(), 1),
            ("lru_v1".to_string(), "true".to_string(), 1),
        ],
        selector_hits
    );
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
            extension_metrics: None,
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
        config: EnvironmentConfigState::FromThread,
    };
    let available_sections = registry.context_contributors()[0]
        .contribute_world_state(WorldStateContributionInput {
            thread_id: codex_protocol::ThreadId::new(),
            turn_id: "turn-1",
            environments: std::slice::from_ref(&turn_environment),
            ready_selected_capability_roots: &selected_roots,
            executor_capability_discovery: None,
            extension_metrics: None,
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
            .contains("(executor package: executor/lint-fix)")
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
            /*extension_metrics*/ None,
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
            extension_metrics: None,
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
            extension_metrics: None,
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

    let failed_discovery = ExecutorCapabilityDiscoverySnapshot::new(
        &selected_roots,
        vec![Err("exec-server transport disconnected".to_string())],
        Default::default(),
    );
    let recovered_discovery = ExecutorCapabilityDiscoverySnapshot::new(
        &selected_roots,
        vec![Ok(Arc::new(CapabilityRootDiscovery {
            id: "lint-fix".to_string(),
            path: PathUri::parse("file:///skills/lint-fix")?,
            plugin: None,
            skills: Vec::new(),
            namespace_manifests: Vec::new(),
            warnings: Vec::new(),
            error: None,
        }))],
        Default::default(),
    );
    for (turn_id, discovery, expected_list_calls) in [
        ("failed-discovery", &failed_discovery, 2),
        ("recovered-discovery", &recovered_discovery, 3),
        ("cached-discovery", &recovered_discovery, 3),
    ] {
        registry.context_contributors()[0]
            .contribute_world_state(WorldStateContributionInput {
                thread_id: codex_protocol::ThreadId::new(),
                turn_id,
                environments: &[],
                ready_selected_capability_roots: &selected_roots,
                executor_capability_discovery: Some(discovery),
                extension_metrics: None,
                session_store: &session_store,
                thread_store: &thread_store,
                turn_store: &ExtensionData::new(turn_id),
            })
            .await;
        assert_eq!(expected_list_calls, list_calls.load(Ordering::Relaxed));
    }

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
            extension_metrics: None,
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
    let mut executor_entry = test_entry(
        SkillSourceKind::Executor,
        "env-1",
        "executor/executor-long-description",
        "skill://executor/executor-long-description/SKILL.md",
    );
    executor_entry.description = description.clone();
    let mut orchestrator_entry = test_entry(
        SkillSourceKind::Orchestrator,
        "codex_apps",
        "orchestrator/orchestrator-long-description",
        "skill://orchestrator/orchestrator-long-description/SKILL.md",
    );
    orchestrator_entry.description = description.clone();
    let providers = SkillProviders::new()
        .with_executor_provider(Arc::new(StaticSkillProvider {
            catalog: SkillCatalog {
                entries: vec![executor_entry],
                warnings: Vec::new(),
            },
            read_requests: Arc::new(Mutex::new(Vec::new())),
            list_calls: None,
            fail_first_list: false,
        }))
        .with_orchestrator_provider(Arc::new(StaticSkillProvider {
            catalog: SkillCatalog {
                entries: vec![orchestrator_entry],
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
            extension_metrics: None,
            session_store: &session_store,
            thread_store: &thread_store,
        })
        .await;

    let (executor, orchestrator) =
        skill_world_state_fragments(&registry, &session_store, &thread_store, "turn-1").await?;
    assert!(executor.body().contains("- executor-long-description:"));
    assert!(
        orchestrator
            .body()
            .contains("- orchestrator-long-description:")
    );
    for rendered in [executor.body(), orchestrator.body()] {
        assert!(rendered.contains(&("x".repeat(1_021) + "...")));
        assert!(!rendered.contains(&"x".repeat(1_024)));
        assert!(!rendered.contains(&description));
    }

    Ok(())
}

#[tokio::test]
async fn moderate_budget_pressure_keeps_every_catalog_entry() -> TestResult {
    let description = "x".repeat(1_025);
    let executor_entries = (0..5)
        .map(|index| {
            let package_id = format!("executor/executor-skill-{index:02}");
            let mut entry = test_entry(
                SkillSourceKind::Executor,
                "env-1",
                &package_id,
                &format!("skill://{package_id}/SKILL.md"),
            );
            entry.description = description.clone();
            entry
        })
        .collect();
    let orchestrator_entries = (0..5)
        .map(|index| {
            let package_id = format!("orchestrator/orchestrator-skill-{index:02}");
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
    let providers = SkillProviders::new()
        .with_executor_provider(Arc::new(StaticSkillProvider {
            catalog: SkillCatalog {
                entries: executor_entries,
                warnings: Vec::new(),
            },
            read_requests: Arc::new(Mutex::new(Vec::new())),
            list_calls: None,
            fail_first_list: false,
        }))
        .with_orchestrator_provider(Arc::new(StaticSkillProvider {
            catalog: SkillCatalog {
                entries: orchestrator_entries,
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
            extension_metrics: None,
            session_store: &session_store,
            thread_store: &thread_store,
        })
        .await;

    let (executor, orchestrator) =
        skill_world_state_fragments(&registry, &session_store, &thread_store, "turn-1").await?;
    let description_lengths = [
        ("executor", executor.body()),
        ("orchestrator", orchestrator.body()),
    ]
    .into_iter()
    .flat_map(|(source, rendered)| (0..5).map(move |index| (source, rendered, index)))
    .map(|(source, rendered, index)| {
        let name = format!("{source}-skill-{index:02}");
        let package_id = format!("{source}/{name}");
        let line_prefix = format!("- {name}: ");
        let line_suffix = if source == "executor" {
            format!(" (executor package: {package_id})")
        } else {
            format!(" (orchestrator package: {package_id})")
        };
        rendered
            .lines()
            .find_map(|line| {
                line.strip_prefix(&line_prefix)
                    .and_then(|line| line.strip_suffix(&line_suffix))
            })
            .unwrap_or_else(|| panic!("rendered catalog should include {name}"))
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
    for rendered in [executor.body(), orchestrator.body()] {
        assert!(!rendered.contains("additional skills omitted from this bounded skills list"));
        assert!(!rendered.contains(&"x".repeat(1_021)));
    }

    Ok(())
}

#[tokio::test]
async fn extreme_budget_pressure_removes_descriptions_before_omitting_entries() -> TestResult {
    let executor_entries = (0..40)
        .map(|index| {
            let package_id = format!("executor/executor-skill-{index:03}");
            let mut entry = test_entry(
                SkillSourceKind::Executor,
                "env-1",
                &package_id,
                &format!("skill://{package_id}/SKILL.md"),
            );
            entry.description = format!("executor-description-{index:03}");
            entry
        })
        .collect();
    let orchestrator_entries = (0..160)
        .map(|index| {
            let package_id = format!("orchestrator/orchestrator-skill-{index:03}");
            let mut entry = test_entry(
                SkillSourceKind::Orchestrator,
                "codex_apps",
                &package_id,
                &format!("skill://{package_id}/SKILL.md"),
            );
            entry.description = format!("orchestrator-description-{index:03}");
            entry
        })
        .collect();
    let providers = SkillProviders::new()
        .with_executor_provider(Arc::new(StaticSkillProvider {
            catalog: SkillCatalog {
                entries: executor_entries,
                warnings: Vec::new(),
            },
            read_requests: Arc::new(Mutex::new(Vec::new())),
            list_calls: None,
            fail_first_list: false,
        }))
        .with_orchestrator_provider(Arc::new(StaticSkillProvider {
            catalog: SkillCatalog {
                entries: orchestrator_entries,
                warnings: Vec::new(),
            },
            read_requests: Arc::new(Mutex::new(Vec::new())),
            list_calls: None,
            fail_first_list: false,
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
            extension_metrics: None,
            session_store: &session_store,
            thread_store: &thread_store,
        })
        .await;

    let (executor, orchestrator) =
        skill_world_state_fragments(&registry, &session_store, &thread_store, "turn-1").await?;
    let included_executor_count = executor
        .body()
        .lines()
        .filter(|line| line.starts_with("- executor-skill-"))
        .count();
    let included_orchestrator_count = orchestrator
        .body()
        .lines()
        .filter(|line| line.starts_with("- orchestrator-skill-"))
        .count();
    assert_eq!(40, included_executor_count);
    assert!(included_orchestrator_count > 0);
    assert!(included_orchestrator_count < 160);
    assert!(
        executor
            .body()
            .contains("- executor-skill-039: (executor package:")
    );
    assert!(
        orchestrator
            .body()
            .contains("- orchestrator-skill-000: (orchestrator package:")
    );
    assert!(!orchestrator.body().contains("- orchestrator-skill-159:"));
    for rendered in [executor.body(), orchestrator.body()] {
        assert!(!rendered.contains("description-"));
    }
    assert!(
        orchestrator
            .body()
            .contains("additional skills omitted from this bounded skills list")
    );
    let omitted_count = 200 - included_executor_count - included_orchestrator_count;
    let warning = event_rx.try_recv()?.into_warning();
    assert_eq!(warning.thread_id, "thread");
    assert_eq!(warning.turn_id.as_deref(), Some("turn-1"));
    assert_eq!(
        warning.message,
        format!(
            "Exceeded skills context budget. All skill descriptions were removed and {omitted_count} additional skills were not included in the model-visible skills list."
        )
    );
    assert!(event_rx.try_recv().is_err());

    Ok(())
}

#[tokio::test]
async fn skills_list_only_returns_model_visible_bounded_metadata() -> TestResult {
    let description = "x".repeat(1_025);
    let opaque_suffix = "\\".repeat(1_500);
    let mut entry = test_entry(
        SkillSourceKind::Orchestrator,
        "codex_apps",
        &format!("orchestrator/{opaque_suffix}"),
        &format!("skill://orchestrator/{opaque_suffix}/SKILL.md"),
    );
    entry.description = description.clone();
    let providers =
        SkillProviders::new().with_orchestrator_provider(Arc::new(StaticSkillProvider {
            catalog: SkillCatalog {
                entries: vec![
                    entry,
                    test_entry(
                        SkillSourceKind::Orchestrator,
                        "codex_apps",
                        "orchestrator/hidden",
                        "skill://orchestrator/hidden/SKILL.md",
                    )
                    .hidden_from_prompt(),
                ],
                warnings: vec!["w".repeat(256); 4],
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
            extension_metrics: None,
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

    assert_eq!(response["skills"].as_array().map(Vec::len), Some(1));
    assert_eq!(response["warnings"].as_array().map(Vec::len), Some(4));
    assert_eq!(response["next_cursor"], serde_json::Value::Null);
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
            extension_metrics: None,
            session_store: &session_store,
            thread_store: &thread_store,
        })
        .await;

    let initial_fragments = registry.context_contributors()[0]
        .contribute_thread_context(&session_store, &thread_store)
        .await;
    assert!(initial_fragments.is_empty());
    assert!(event_rx.try_recv().is_err());

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
                /*extension_metrics*/ None,
                &session_store,
                &thread_store,
                &ExtensionData::new(turn_id),
            )
            .await;
        assert!(fragments.is_empty());
        let warning = event_rx.try_recv()?.into_warning();
        assert_eq!(warning.thread_id, thread_store.level_id());
        assert_eq!(warning.turn_id.as_deref(), Some(turn_id));
        assert_eq!(
            warning.message,
            "orchestrator skills unavailable: temporary orchestrator failure"
        );
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
            extension_metrics: None,
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
                config: EnvironmentConfigState::FromThread,
            }],
            ready_selected_capability_roots: &selected_roots,
            executor_capability_discovery: None,
            extension_metrics: None,
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
            /*extension_metrics*/ None,
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
async fn model_context_window_scales_executor_and_orchestrator_catalogs() -> TestResult {
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
    let (event_tx, event_rx) = std::sync::mpsc::channel();
    let mut builder =
        ExtensionRegistryBuilder::with_event_sink(Arc::new(ChannelEventSink(event_tx)));
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
            extension_metrics: None,
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
    assert!(thread_fragments.is_empty());

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
            extension_metrics: None,
            session_store: &session_store,
            thread_store: &thread_store,
            turn_store: &turn_store,
        })
        .await;
    // Core rebuilds world state before each sampling step.
    let _repeated_sections = registry.context_contributors()[0]
        .contribute_world_state(WorldStateContributionInput {
            thread_id: codex_protocol::ThreadId::new(),
            turn_id: "turn-1",
            environments: &[],
            ready_selected_capability_roots: &selected_roots,
            executor_capability_discovery: None,
            extension_metrics: None,
            session_store: &session_store,
            thread_store: &thread_store,
            turn_store: &turn_store,
        })
        .await;
    assert!(event_rx.try_recv().is_err());
    let executor_section = world_state_section(&sections, "skills");
    let snapshot = executor_section.snapshot().clone();
    let executor_fragment = executor_section
        .render_diff(PreviousWorldStateSection::Absent)
        .ok_or("bounded executor catalog should render")?;
    assert!(!executor_fragment.body().contains("skill-39"));
    let orchestrator_fragment = world_state_section(&sections, "orchestrator_skills")
        .render_diff(PreviousWorldStateSection::Absent)
        .ok_or("bounded orchestrator catalog should render")?;
    assert!(
        orchestrator_fragment
            .body()
            .contains("additional skills omitted")
    );
    let warnings = event_rx
        .try_iter()
        .map(CapturedExtensionEvent::into_warning)
        .collect::<Vec<_>>();
    assert_eq!(warnings.len(), 2);
    for warning in warnings {
        assert_eq!(warning.thread_id, thread_store.level_id());
        assert_eq!(warning.turn_id.as_deref(), Some("turn-1"));
        assert!(
            warning
                .message
                .starts_with("Exceeded skills context budget.")
        );
        assert!(
            warning
                .message
                .ends_with("additional skills were not included in the model-visible skills list.")
        );
    }
    assert!(
        executor_section
            .render_diff(PreviousWorldStateSection::Known(&snapshot))
            .is_none()
    );
    assert!(event_rx.try_recv().is_err());

    Ok(())
}

#[tokio::test]
async fn executor_catalog_emits_at_most_four_warnings() -> TestResult {
    let executor_provider = Arc::new(StaticSkillProvider {
        catalog: SkillCatalog {
            entries: Vec::new(),
            warnings: (0..6).map(|index| format!("warning-{index}")).collect(),
        },
        read_requests: Arc::new(Mutex::new(Vec::new())),
        list_calls: None,
        fail_first_list: false,
    });
    let providers = SkillProviders::new().with_executor_provider(executor_provider);
    let (event_tx, event_rx) = std::sync::mpsc::channel();
    let mut builder =
        ExtensionRegistryBuilder::with_event_sink(Arc::new(ChannelEventSink(event_tx)));
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
            extension_metrics: None,
            session_store: &session_store,
            thread_store: &thread_store,
        })
        .await;
    let selected_roots = vec![SelectedCapabilityRoot {
        id: "skills".to_string(),
        location: CapabilityRootLocation::Environment {
            environment_id: "env-1".to_string(),
            path: PathUri::parse("file:///skills").expect("skill root URI"),
        },
    }];
    let turn_store = ExtensionData::new("turn-1");

    registry.context_contributors()[0]
        .contribute_world_state(WorldStateContributionInput {
            thread_id: codex_protocol::ThreadId::new(),
            turn_id: "turn-1",
            environments: &[],
            ready_selected_capability_roots: &selected_roots,
            executor_capability_discovery: None,
            extension_metrics: None,
            session_store: &session_store,
            thread_store: &thread_store,
            turn_store: &turn_store,
        })
        .await;
    registry.turn_input_contributors()[0]
        .contribute(
            TurnInputContext {
                turn_id: "turn-1".to_string(),
                user_input: Vec::new(),
                environments: Vec::new(),
            },
            /*extension_metrics*/ None,
            &session_store,
            &thread_store,
            &turn_store,
        )
        .await;

    let messages = event_rx
        .try_iter()
        .map(|event| event.into_warning().message)
        .collect::<Vec<_>>();
    assert_eq!(
        messages,
        vec!["warning-0", "warning-1", "warning-2", "warning-3"]
    );

    Ok(())
}

#[tokio::test]
async fn host_catalog_compacts_shared_paths_under_budget_pressure() -> TestResult {
    let test_root = test_codex_home();
    let root = test_root.join(
        "plugins/cache/openai-curated/example/hash1234567890/skills-with-a-very-long-shared-prefix",
    );
    for index in 0..12 {
        let skill_dir = root.join(format!("skill-{index:02}"));
        std::fs::create_dir_all(&skill_dir)?;
        std::fs::write(
            skill_dir.join("SKILL.md"),
            format!(
                "---\nname: skill-{index:02}\ndescription: Fix lint errors.\n---\n# Skill {index:02}\n"
            ),
        )?;
    }
    #[cfg(unix)]
    {
        let source_skill_dir = test_root.join("shared-skills/linked-skill");
        std::fs::create_dir_all(&source_skill_dir)?;
        std::fs::write(
            source_skill_dir.join("SKILL.md"),
            "---\nname: linked-skill\ndescription: Linked skill.\n---\n# Linked skill\n",
        )?;
        std::os::unix::fs::symlink(&source_skill_dir, root.join("linked-skill"))?;
    }
    let root = AbsolutePathBuf::try_from(std::fs::canonicalize(root)?)?;
    let rendered_root = root.to_string_lossy().replace('\\', "/");
    let config_layer_stack = ConfigLayerStack::new(
        vec![ConfigLayerEntry::new(
            ConfigLayerSource::SessionFlags,
            toml::from_str("[skills.bundled]\nenabled = false\n")?,
        )],
        Default::default(),
        ConfigRequirementsToml::default(),
    )?;
    let codex_home = AbsolutePathBuf::try_from(test_root.clone())?;
    let service = HostSkillsService::new_with_restriction_product(
        codex_home.clone(),
        /*bundled_skills_enabled*/ false,
        /*restriction_product*/ None,
    );
    service.set_extra_roots(vec![root]);
    let snapshot = service
        .snapshot_for_config(
            &HostSkillsLoadInput::new(codex_home, Vec::new(), config_layer_stack),
            Some(Arc::clone(&LOCAL_FS)),
        )
        .await;
    assert_eq!(snapshot.outcome().errors, Vec::new());
    assert_eq!(
        snapshot.outcome().skills.len(),
        12 + usize::from(cfg!(unix))
    );

    let mut builder = ExtensionRegistryBuilder::new();
    install(&mut builder, skills_extension_config);
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
            extension_metrics: None,
            session_store: &session_store,
            thread_store: &thread_store,
        })
        .await;
    let mut model_info = model_info_from_slug("test-model");
    model_info.context_window = Some(10_000);
    thread_store.insert(model_info);
    let turn_store = ExtensionData::new("turn-1");
    turn_store.insert(snapshot);

    let fragments = registry.turn_input_contributors()[0]
        .contribute(
            TurnInputContext {
                turn_id: "turn-1".to_string(),
                user_input: vec![UserInput::Text {
                    text: "hello".to_string(),
                    text_elements: Vec::new(),
                }],
                environments: Vec::new(),
            },
            /*extension_metrics*/ None,
            &session_store,
            &thread_store,
            &turn_store,
        )
        .await;
    let catalog = fragments
        .iter()
        .find(|fragment| fragment.role() == "developer")
        .ok_or("host catalog should render")?
        .render();

    assert!(
        catalog.contains(&format!("- `r0` = `{rendered_root}`")),
        "{catalog}"
    );
    assert!(catalog.contains("(file: r0/skill-00/SKILL.md)"));
    assert!(catalog.contains("(file: r0/skill-11/SKILL.md)"));
    #[cfg(unix)]
    assert!(catalog.contains("(file: r0/linked-skill/SKILL.md)"));
    assert!(!catalog.contains("additional skills omitted"));

    std::fs::remove_dir_all(test_root)?;
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
            extension_metrics: None,
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
            /*extension_metrics*/ None,
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

#[derive(Debug)]
enum CapturedExtensionEvent {
    Event(Box<Event>),
    Warning(ExtensionWarning),
}

impl CapturedExtensionEvent {
    fn into_warning(self) -> ExtensionWarning {
        match self {
            Self::Warning(warning) => warning,
            Self::Event(event) => panic!("expected extension warning, got {event:?}"),
        }
    }
}

struct ChannelEventSink(std::sync::mpsc::Sender<CapturedExtensionEvent>);

impl ExtensionEventSink for ChannelEventSink {
    fn emit(&self, event: Event) {
        let _ = self.0.send(CapturedExtensionEvent::Event(Box::new(event)));
    }

    fn emit_warning(&self, warning: ExtensionWarning) {
        let _ = self.0.send(CapturedExtensionEvent::Warning(warning));
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RecordedHistogram {
    name: String,
    value: i64,
    tags: Vec<(String, String)>,
}

#[derive(Default)]
struct RecordingMetrics {
    samples: Mutex<Vec<RecordedHistogram>>,
}

impl RecordingMetrics {
    fn samples(&self) -> Vec<RecordedHistogram> {
        self.samples
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

impl ExtensionMetrics for RecordingMetrics {
    fn counter(&self, name: &str, _inc: i64, _tags: &[(&str, &str)]) {
        panic!("unexpected counter: {name}");
    }

    fn histogram(&self, name: &str, value: i64, tags: &[(&str, &str)]) {
        self.samples
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(RecordedHistogram {
                name: name.to_string(),
                value,
                tags: tags
                    .iter()
                    .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
                    .collect(),
            });
    }
}

fn expected_catalog_metric_samples(catalog_surface: &str, count: i64) -> Vec<RecordedHistogram> {
    let tags = vec![("catalog_surface".to_string(), catalog_surface.to_string())];
    vec![
        RecordedHistogram {
            name: THREAD_SKILLS_ENABLED_TOTAL_METRIC.to_string(),
            value: count,
            tags: tags.clone(),
        },
        RecordedHistogram {
            name: THREAD_SKILLS_KEPT_TOTAL_METRIC.to_string(),
            value: count,
            tags: tags.clone(),
        },
        RecordedHistogram {
            name: THREAD_SKILLS_TRUNCATED_METRIC.to_string(),
            value: 0,
            tags: tags.clone(),
        },
        RecordedHistogram {
            name: THREAD_SKILLS_DESCRIPTION_TRUNCATED_CHARS_METRIC.to_string(),
            value: 0,
            tags,
        },
    ]
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
        max_context_tokens: None,
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
