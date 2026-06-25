use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use codex_core_skills::runtime::SkillSources;
use codex_extension_api::ConversationHistory;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::NoopTurnItemEmitter;
use codex_extension_api::ThreadStartInput;
use codex_extension_api::ToolCall;
use codex_extension_api::ToolPayload;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::TruncationPolicy;
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
use codex_skills_extension::install_with_providers;
use codex_skills_extension::provider::SkillListQuery;
use codex_skills_extension::provider::SkillProvider;
use codex_skills_extension::provider::SkillProviderFuture;
use codex_skills_extension::provider::SkillReadRequest;
use codex_skills_extension::provider::SkillSearchRequest;
use pretty_assertions::assert_eq;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[tokio::test]
async fn skills_list_truncates_catalog_descriptions_in_tool_output() -> TestResult {
    let description = "x".repeat(1_025);
    let mut entry = test_entry("orchestrator/long-description");
    entry.description = description.clone();
    let (registry, session_store, thread_store) = start_extension(StaticSkillProvider {
        catalog: SkillCatalog {
            entries: vec![entry],
            warnings: Vec::new(),
        },
        list_calls: None,
        fail_first_list: false,
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
async fn transient_orchestrator_failure_is_not_cached() -> TestResult {
    let list_calls = Arc::new(AtomicUsize::new(0));
    let (_registry, _session_store, thread_store) = start_extension(StaticSkillProvider {
        catalog: SkillCatalog {
            entries: vec![test_entry("orchestrator/first")],
            warnings: Vec::new(),
        },
        list_calls: Some(Arc::clone(&list_calls)),
        fail_first_list: true,
    })
    .await;
    let sources = thread_store
        .get::<SkillSources>()
        .ok_or("orchestrator skill source should be registered")?;

    let first = sources.list().await;
    let second = sources.list().await;

    assert!(first.entries.is_empty());
    assert_eq!(first.warnings.len(), 1);
    assert_eq!(second.entries, vec![test_entry("orchestrator/first")]);
    assert_eq!(list_calls.load(Ordering::Relaxed), 2);
    Ok(())
}

async fn start_extension(
    provider: StaticSkillProvider,
) -> (
    codex_extension_api::ExtensionRegistry<TestConfig>,
    ExtensionData,
    ExtensionData,
) {
    let providers = SkillProviders::new().with_orchestrator_provider(Arc::new(provider));
    let mut builder = ExtensionRegistryBuilder::new();
    install_with_providers(&mut builder, providers, skills_extension_config);
    let registry = builder.build();
    let session_store = ExtensionData::new("session");
    let thread_store = ExtensionData::new("thread");
    let session_source = SessionSource::Cli;
    registry.thread_lifecycle_contributors()[0]
        .on_thread_start(ThreadStartInput {
            config: &TestConfig,
            session_source: &session_source,
            persistent_thread_state_available: true,
            environments: &[],
            session_store: &session_store,
            thread_store: &thread_store,
        })
        .await;
    (registry, session_store, thread_store)
}

#[derive(Clone)]
struct StaticSkillProvider {
    catalog: SkillCatalog,
    list_calls: Option<Arc<AtomicUsize>>,
    fail_first_list: bool,
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
        Box::pin(async move {
            Ok(SkillReadResult {
                resource: request.resource,
                contents: "# Skill".to_string(),
            })
        })
    }

    fn search(&self, _request: SkillSearchRequest) -> SkillProviderFuture<'_, SkillSearchResult> {
        Box::pin(async { Ok(SkillSearchResult::default()) })
    }
}

fn test_entry(package_id: &str) -> SkillCatalogEntry {
    SkillCatalogEntry::new(
        SkillPackageId(package_id.to_string()),
        SkillAuthority::new(SkillSourceKind::Orchestrator, "codex_apps"),
        package_id.rsplit('/').next().unwrap_or(package_id),
        "Fix lint errors.",
        SkillResourceId::new(format!("skill://{package_id}/SKILL.md")),
    )
    .with_display_path(format!("skill://{package_id}/SKILL.md"))
}

#[derive(Clone, Debug)]
struct TestConfig;

fn skills_extension_config(_config: &TestConfig) -> SkillsExtensionConfig {
    SkillsExtensionConfig {
        include_instructions: true,
        bundled_skills_enabled: true,
        orchestrator_skills_enabled: true,
    }
}
