use std::collections::BTreeMap;
use std::sync::Arc;

use codex_core_skills::HostSkillsSnapshot;
use codex_core_skills::SkillLoadOutcome;
use codex_core_skills::SkillMetadata;
use codex_extension_api::ConversationHistory;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionRegistry;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::NoopTurnItemEmitter;
use codex_extension_api::SkillInvocationInput;
use codex_extension_api::SkillInvocationKind;
use codex_extension_api::ThreadStartInput;
use codex_extension_api::ToolCall;
use codex_extension_api::ToolPayload;
use codex_extension_api::TurnInputContext;
use codex_extension_api::WorldStateContributionInput;
use codex_mcp::CODEX_APPS_MCP_SERVER_NAME;
use codex_otel::MetricsClient;
use codex_otel::MetricsConfig;
use codex_protocol::capabilities::CapabilityRootLocation;
use codex_protocol::capabilities::SelectedCapabilityRoot;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SkillScope;
use codex_protocol::protocol::TruncationPolicy;
use codex_protocol::user_input::UserInput;
use codex_skills_extension::SkillProviders;
use codex_skills_extension::SkillsExtensionConfig;
use codex_skills_extension::catalog::SkillAuthority;
use codex_skills_extension::catalog::SkillCatalog;
use codex_skills_extension::catalog::SkillCatalogEntry;
use codex_skills_extension::catalog::SkillPackageId;
use codex_skills_extension::catalog::SkillReadResult;
use codex_skills_extension::catalog::SkillResourceId;
use codex_skills_extension::catalog::SkillSearchResult;
use codex_skills_extension::catalog::SkillSourceKind;
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
type InvocationPoint = (BTreeMap<String, String>, u64);

const INVOCATION_METRIC: &str = "codex.skills.shadow_selection.invocation";
const PACKAGE: &str = "orchestrator/demo";
const MAIN_RESOURCE: &str = "skill://orchestrator/demo/SKILL.md";

#[derive(Clone)]
struct OrchestratorProvider;

impl SkillProvider for OrchestratorProvider {
    fn list(&self, _query: SkillListQuery) -> SkillProviderFuture<'_, SkillCatalog> {
        Box::pin(std::future::ready(Ok(SkillCatalog {
            entries: vec![SkillCatalogEntry::new(
                SkillPackageId(PACKAGE.to_string()),
                SkillAuthority::new(SkillSourceKind::Orchestrator, CODEX_APPS_MCP_SERVER_NAME),
                "demo",
                "Use the demo skill.",
                SkillResourceId::new(MAIN_RESOURCE),
            )],
            warnings: Vec::new(),
        })))
    }

    fn read(&self, request: SkillReadRequest) -> SkillProviderFuture<'_, SkillReadResult> {
        Box::pin(std::future::ready(Ok(SkillReadResult {
            resource: request.resource,
            contents: "# Demo\n\nUse the demo skill.".to_string(),
        })))
    }

    fn search(&self, _request: SkillSearchRequest) -> SkillProviderFuture<'_, SkillSearchResult> {
        Box::pin(std::future::ready(Ok(SkillSearchResult::default())))
    }
}

#[derive(Clone)]
struct ExecutorProvider;

impl SkillProvider for ExecutorProvider {
    fn list(&self, _query: SkillListQuery) -> SkillProviderFuture<'_, SkillCatalog> {
        Box::pin(std::future::ready(Ok(SkillCatalog {
            entries: (0..5)
                .map(|index| {
                    let resource = format!("skill://executor/demo-{index}/SKILL.md");
                    SkillCatalogEntry::new(
                        SkillPackageId(format!("executor/demo-{index}")),
                        SkillAuthority::new(SkillSourceKind::Executor, "executor"),
                        "演示文稿",
                        "创建演示文稿。",
                        SkillResourceId::new(resource),
                    )
                })
                .collect(),
            warnings: Vec::new(),
        })))
    }

    fn read(&self, request: SkillReadRequest) -> SkillProviderFuture<'_, SkillReadResult> {
        Box::pin(std::future::ready(Ok(SkillReadResult {
            resource: request.resource,
            contents: "# 演示文稿".to_string(),
        })))
    }

    fn search(&self, _request: SkillSearchRequest) -> SkillProviderFuture<'_, SkillSearchResult> {
        Box::pin(std::future::ready(Ok(SkillSearchResult::default())))
    }
}

#[tokio::test]
async fn implicit_core_and_native_read_invocations_share_turn_local_recording() -> TestResult {
    let metrics = MetricsClient::new(
        MetricsConfig::in_memory(
            "test",
            "codex-skills-extension",
            env!("CARGO_PKG_VERSION"),
            InMemoryMetricExporter::default(),
        )
        .with_runtime_reader(),
    )?;
    let mut builder = ExtensionRegistryBuilder::<()>::new();
    install_with_providers_and_metrics(
        &mut builder,
        SkillProviders::new().with_orchestrator_provider(Arc::new(OrchestratorProvider)),
        Some(metrics.clone()),
        |_| SkillsExtensionConfig {
            include_instructions: false,
            bundled_skills_enabled: false,
            orchestrator_skills_enabled: true,
            shadow_selection_enabled: true,
        },
    );
    let registry = builder.build();
    let session_store = ExtensionData::new("session");
    let thread_store = ExtensionData::new("thread");
    registry.thread_lifecycle_contributors()[0]
        .on_thread_start(ThreadStartInput {
            config: &(),
            session_source: &SessionSource::Cli,
            persistent_thread_state_available: true,
            environments: &[],
            session_store: &session_store,
            thread_store: &thread_store,
        })
        .await;

    let core_turn = start_turn(&registry, &session_store, &thread_store, "turn-core").await;
    registry.skill_invocation_contributors()[0]
        .on_skill_invocation(SkillInvocationInput {
            session_store: &session_store,
            thread_store: &thread_store,
            turn_store: core_turn.as_ref(),
            turn_id: "turn-core",
            skill_resource: MAIN_RESOURCE,
            kind: SkillInvocationKind::Implicit,
        })
        .await;

    start_turn(&registry, &session_store, &thread_store, "turn-tool").await;
    let tools = registry.tool_contributors()[0].tools(&session_store, &thread_store);
    let read_tool = tools
        .iter()
        .find(|tool| tool.tool_name().name == "read")
        .ok_or("skills.read tool should be registered")?;
    for call_id in ["call-1", "call-2"] {
        read_tool
            .handle(ToolCall {
                turn_id: "turn-tool".to_string(),
                call_id: call_id.to_string(),
                tool_name: read_tool.tool_name(),
                model: "gpt-test".to_string(),
                codex_turn_metadata: None,
                truncation_policy: TruncationPolicy::Bytes(1_024),
                conversation_history: ConversationHistory::default(),
                turn_item_emitter: Arc::new(NoopTurnItemEmitter),
                environments: Vec::new(),
                payload: ToolPayload::Function {
                    arguments: serde_json::json!({
                        "authority": {"kind": "orchestrator"},
                        "package": PACKAGE,
                        "resource": MAIN_RESOURCE,
                    })
                    .to_string(),
                },
            })
            .await?;
    }

    assert_eq!(
        vec![(
            BTreeMap::from([
                ("hit".to_string(), "true".to_string()),
                ("method".to_string(), "weighted_lexical_v1".to_string()),
                ("query_script".to_string(), "ascii_latin".to_string()),
                ("rank".to_string(), "1".to_string()),
            ]),
            2,
        )],
        invocation_points(&metrics)?,
    );
    Ok(())
}

#[tokio::test]
async fn shadow_selection_uses_host_snapshot_and_excludes_executor_candidates() -> TestResult {
    let metrics = MetricsClient::new(
        MetricsConfig::in_memory(
            "test",
            "codex-skills-extension",
            env!("CARGO_PKG_VERSION"),
            InMemoryMetricExporter::default(),
        )
        .with_runtime_reader(),
    )?;
    let mut builder = ExtensionRegistryBuilder::<()>::new();
    install_with_providers_and_metrics(
        &mut builder,
        SkillProviders::new().with_executor_provider(Arc::new(ExecutorProvider)),
        Some(metrics.clone()),
        |_| SkillsExtensionConfig {
            include_instructions: true,
            bundled_skills_enabled: false,
            orchestrator_skills_enabled: false,
            shadow_selection_enabled: true,
        },
    );
    let registry = builder.build();
    let session_store = ExtensionData::new("session");
    let thread_store = ExtensionData::new("thread");
    registry.thread_lifecycle_contributors()[0]
        .on_thread_start(ThreadStartInput {
            config: &(),
            session_source: &SessionSource::Cli,
            persistent_thread_state_available: true,
            environments: &[],
            session_store: &session_store,
            thread_store: &thread_store,
        })
        .await;

    let skill_path = AbsolutePathBuf::try_from(
        std::env::temp_dir()
            .join("codex-shadow-host-skill")
            .join("SKILL.md"),
    )?;
    let skill_resource = skill_path.to_string_lossy().into_owned();
    let mut outcome = SkillLoadOutcome::default();
    outcome.skills.push(SkillMetadata {
        name: "演示文稿".to_string(),
        description: "创建演示文稿。".to_string(),
        short_description: None,
        interface: None,
        dependencies: None,
        policy: None,
        path_to_skills_md: skill_path,
        scope: SkillScope::User,
        plugin_id: None,
    });
    let turn_store = ExtensionData::new("turn-host");
    turn_store.insert(HostSkillsSnapshot::new(Arc::new(outcome)));
    let selected_roots = [SelectedCapabilityRoot {
        id: "executor-root".to_string(),
        location: CapabilityRootLocation::Environment {
            environment_id: "executor".to_string(),
            path: PathUri::parse("file:///skills").expect("executor skill root URI"),
        },
    }];
    registry.context_contributors()[0]
        .contribute_world_state(WorldStateContributionInput {
            thread_id: codex_protocol::ThreadId::new(),
            turn_id: "turn-host",
            environments: &[],
            ready_selected_capability_roots: &selected_roots,
            session_store: &session_store,
            thread_store: &thread_store,
            turn_store: &turn_store,
        })
        .await;

    let fragments = registry.turn_input_contributors()[0]
        .contribute(
            TurnInputContext {
                turn_id: "turn-host".to_string(),
                user_input: vec![UserInput::Text {
                    text: "演示文稿".to_string(),
                    text_elements: Vec::new(),
                }],
                environments: Vec::new(),
            },
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
            turn_id: "turn-host",
            skill_resource: &skill_resource,
            kind: SkillInvocationKind::Implicit,
        })
        .await;

    assert_eq!(
        vec![(
            BTreeMap::from([
                ("hit".to_string(), "true".to_string()),
                ("method".to_string(), "weighted_lexical_v1".to_string()),
                ("query_script".to_string(), "cjk".to_string()),
                ("rank".to_string(), "1".to_string()),
            ]),
            1,
        )],
        invocation_points(&metrics)?,
    );
    Ok(())
}

async fn start_turn(
    registry: &ExtensionRegistry<()>,
    session_store: &ExtensionData,
    thread_store: &ExtensionData,
    turn_id: &str,
) -> Arc<ExtensionData> {
    let turn_store = Arc::new(ExtensionData::new(turn_id));
    registry.turn_input_contributors()[0]
        .contribute(
            TurnInputContext {
                turn_id: turn_id.to_string(),
                user_input: vec![UserInput::Text {
                    text: "use demo".to_string(),
                    text_elements: Vec::new(),
                }],
                environments: Vec::new(),
            },
            session_store,
            thread_store,
            turn_store.as_ref(),
        )
        .await;
    turn_store
}

fn invocation_points(
    metrics: &MetricsClient,
) -> Result<Vec<InvocationPoint>, Box<dyn std::error::Error>> {
    let snapshot = metrics.snapshot()?;
    let metric = snapshot
        .scope_metrics()
        .flat_map(opentelemetry_sdk::metrics::data::ScopeMetrics::metrics)
        .find(|metric| metric.name() == INVOCATION_METRIC)
        .ok_or("invocation metric was not emitted")?;
    match metric.data() {
        AggregatedMetrics::U64(MetricData::Sum(sum)) => Ok(sum
            .data_points()
            .map(|point| {
                (
                    point
                        .attributes()
                        .map(|attribute| {
                            (
                                attribute.key.as_str().to_string(),
                                attribute.value.as_str().to_string(),
                            )
                        })
                        .collect(),
                    point.value(),
                )
            })
            .collect()),
        other => Err(format!("unexpected invocation metric data: {other:?}").into()),
    }
}
