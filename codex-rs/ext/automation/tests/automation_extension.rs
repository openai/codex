use std::sync::Arc;

use codex_automation_extension::AUTOMATION_UPDATE_TOOL_NAME;
use codex_automation_extension::install;
use codex_core::config::Config;
use codex_core::config::ConfigBuilder;
use codex_core::config::ConfigOverrides;
use codex_extension_api::ConversationHistory;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::NoopTurnItemEmitter;
use codex_extension_api::ThreadStartInput;
use codex_extension_api::ToolCall;
use codex_extension_api::ToolExecutor;
use codex_extension_api::ToolName;
use codex_extension_api::ToolPayload;
use codex_protocol::ThreadId;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::TruncationPolicy;
use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;

#[tokio::test]
async fn automation_update_tool_visibility_is_gated() -> anyhow::Result<()> {
    let visible = AutomationExtensionHarness::new(
        /*automations_enabled*/ true,
        /*persistent_thread_state_available*/ true,
        SessionSource::Cli,
    )
    .await?;
    assert_eq!(
        vec![AUTOMATION_UPDATE_TOOL_NAME.to_string()],
        tool_names(&visible.tools())
    );

    let disabled = AutomationExtensionHarness::new(
        /*automations_enabled*/ false,
        /*persistent_thread_state_available*/ true,
        SessionSource::Cli,
    )
    .await?;
    assert_eq!(Vec::<String>::new(), tool_names(&disabled.tools()));

    let ephemeral = AutomationExtensionHarness::new(
        /*automations_enabled*/ true,
        /*persistent_thread_state_available*/ false,
        SessionSource::Cli,
    )
    .await?;
    assert_eq!(Vec::<String>::new(), tool_names(&ephemeral.tools()));

    let automation_thread = AutomationExtensionHarness::new(
        /*automations_enabled*/ true,
        /*persistent_thread_state_available*/ true,
        SessionSource::automation(),
    )
    .await?;
    assert_eq!(Vec::<String>::new(), tool_names(&automation_thread.tools()));
    Ok(())
}

#[tokio::test]
async fn automation_update_tool_creates_and_lists_thread_owned_automations() -> anyhow::Result<()> {
    let harness = AutomationExtensionHarness::new(
        /*automations_enabled*/ true,
        /*persistent_thread_state_available*/ true,
        SessionSource::Cli,
    )
    .await?;
    let tools = harness.tools();
    let tool = tool_by_name(&tools, AUTOMATION_UPDATE_TOOL_NAME);
    let cwd = harness.cwd_string();

    let create_invocation = tool_call(
        AUTOMATION_UPDATE_TOOL_NAME,
        "call-create-automation",
        json!({
            "mode": "create",
            "kind": "cron",
            "name": "Daily plan",
            "prompt": "Plan the day",
            "cwds": [cwd],
            "status": "ACTIVE",
        }),
    );
    let create_output = tool.handle(create_invocation.clone()).await?;
    let create_result = create_output.code_mode_result(&create_invocation.payload);
    let automation = create_result["automation"].clone();
    assert_eq!(
        create_result,
        json!({
            "automation": {
                "id": automation["id"],
                "name": "Daily plan",
                "prompt": "Plan the day",
                "status": "ACTIVE",
                "rrule": "FREQ=HOURLY;INTERVAL=24;BYMINUTE=0",
                "nextRunAt": automation["nextRunAt"],
                "lastRunAt": null,
                "createdAt": automation["createdAt"],
                "updatedAt": automation["updatedAt"],
                "model": null,
                "reasoningEffort": null,
                "target": {
                    "type": "cron",
                    "cwds": [harness.cwd_string()],
                },
            },
            "automations": [],
            "deleted": null,
        })
    );
    let stored = harness
        .state_db
        .get_automation(automation["id"].as_str().expect("automation id"))
        .await?
        .expect("created automation should be stored");
    let workspace_roots = stored
        .dispatch_settings
        .expect("cron automation should store dispatch settings")
        .workspace_roots;
    assert!(
        workspace_roots
            .iter()
            .any(|root| harness.cwd.path().starts_with(root)),
        "cron cwd should be covered by stored dispatch roots: {workspace_roots:?}"
    );

    let list_invocation = tool_call(
        AUTOMATION_UPDATE_TOOL_NAME,
        "call-list-automations",
        json!({ "mode": "list" }),
    );
    let list_output = tool.handle(list_invocation.clone()).await?;
    assert_eq!(
        list_output.code_mode_result(&list_invocation.payload),
        json!({
            "automation": null,
            "automations": [automation],
            "deleted": null,
        })
    );
    Ok(())
}

struct AutomationExtensionHarness {
    _codex_home: TempDir,
    cwd: TempDir,
    state_db: Arc<codex_state::StateRuntime>,
    registry: codex_extension_api::ExtensionRegistry<Config>,
    session_store: ExtensionData,
    thread_store: ExtensionData,
}

impl AutomationExtensionHarness {
    async fn new(
        automations_enabled: bool,
        persistent_thread_state_available: bool,
        session_source: SessionSource,
    ) -> anyhow::Result<Self> {
        let codex_home = TempDir::new()?;
        let cwd = TempDir::new()?;
        let state_db = codex_state::StateRuntime::init(
            codex_home.path().to_path_buf(),
            "test-provider".into(),
        )
        .await?;
        let config = ConfigBuilder::default()
            .codex_home(codex_home.path().to_path_buf())
            .harness_overrides(ConfigOverrides {
                cwd: Some(cwd.path().to_path_buf()),
                ..ConfigOverrides::default()
            })
            .cli_overrides(vec![(
                "features.automations".to_string(),
                toml::Value::Boolean(automations_enabled),
            )])
            .build()
            .await?;
        let mut builder = ExtensionRegistryBuilder::<Config>::new();
        install(&mut builder, Arc::clone(&state_db));
        let registry = builder.build();
        let session_store = ExtensionData::new("session-1");
        let thread_store = ExtensionData::new(test_thread_id()?.to_string());
        for contributor in registry.thread_lifecycle_contributors() {
            contributor
                .on_thread_start(ThreadStartInput {
                    config: &config,
                    session_source: &session_source,
                    persistent_thread_state_available,
                    session_store: &session_store,
                    thread_store: &thread_store,
                })
                .await;
        }
        Ok(Self {
            _codex_home: codex_home,
            cwd,
            state_db,
            registry,
            session_store,
            thread_store,
        })
    }

    fn tools(&self) -> Vec<Arc<dyn ToolExecutor<ToolCall>>> {
        self.registry
            .tool_contributors()
            .iter()
            .flat_map(|contributor| contributor.tools(&self.session_store, &self.thread_store))
            .collect()
    }

    fn cwd_string(&self) -> String {
        self.cwd.path().to_string_lossy().to_string()
    }
}

fn tool_names(tools: &[Arc<dyn ToolExecutor<ToolCall>>]) -> Vec<String> {
    tools.iter().map(|tool| tool.tool_name().name).collect()
}

fn tool_by_name<'a>(
    tools: &'a [Arc<dyn ToolExecutor<ToolCall>>],
    name: &str,
) -> &'a Arc<dyn ToolExecutor<ToolCall>> {
    tools
        .iter()
        .find(|tool| tool.tool_name().namespace.is_none() && tool.tool_name().name == name)
        .unwrap_or_else(|| panic!("missing tool {name}"))
}

fn tool_call(tool_name: &str, call_id: &str, arguments: serde_json::Value) -> ToolCall {
    ToolCall {
        turn_id: "turn-1".to_string(),
        call_id: call_id.to_string(),
        tool_name: ToolName::plain(tool_name),
        model: "gpt-test".to_string(),
        truncation_policy: TruncationPolicy::Bytes(1024),
        conversation_history: ConversationHistory::default(),
        turn_item_emitter: Arc::new(NoopTurnItemEmitter),
        payload: ToolPayload::Function {
            arguments: arguments.to_string(),
        },
    }
}

fn test_thread_id() -> anyhow::Result<ThreadId> {
    ThreadId::from_string("11111111-1111-4111-8111-111111111111").map_err(anyhow::Error::msg)
}
