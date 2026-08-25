use std::sync::Arc;

use codex_config::LoaderOverrides;
use codex_core::config::Config;
use codex_core::config::ConfigBuilder;
use codex_core::config::TokenBudgetConfig;
use codex_extension_api::ContentItemKind;
use codex_extension_api::ConversationHistory;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionRegistry;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::NoopTurnItemEmitter;
use codex_extension_api::PromptFragment;
use codex_extension_api::PromptSlot;
use codex_extension_api::ThreadStartInput;
use codex_extension_api::ToolCall;
use codex_extension_api::ToolCallSource;
use codex_extension_api::ToolExecutor;
use codex_extension_api::ToolName;
use codex_extension_api::ToolPayload;
use codex_history_notes_extension::install;
use codex_login::AuthHeaders;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use codex_model_provider_info::ModelProviderInfo;
use codex_protocol::AgentPath;
use codex_protocol::ThreadId;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::protocol::TruncationPolicy;
use http::HeaderMap;
use http::HeaderValue;
use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

type TestResult = Result<(), Box<dyn std::error::Error>>;
const THREAD_HINT: &str = "Recent notes (up to 5, most-recent first):\n- /root/worker/notes/latest.md (2 lines, 14 UTF-8 bytes)";

#[tokio::test]
async fn installed_extension_exposes_and_invokes_history_notes_tools() -> TestResult {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/backend-api/codex/alpha/notes/v2/read_file"))
        .and(header("x-openai-actor-authorization", "actor-biscuit"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "encrypted_output": "enc_payload"
        })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/backend-api/codex/alpha/notes/v2/thread_hint"))
        .and(header("x-openai-actor-authorization", "actor-biscuit"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"text": THREAD_HINT})))
        .mount(&server)
        .await;
    let codex_home = TempDir::new()?;
    let mut config = ConfigBuilder::default()
        .loader_overrides(LoaderOverrides::without_managed_config_for_tests())
        .codex_home(codex_home.path().to_path_buf())
        .build()
        .await?;
    config.model_provider.base_url = Some(format!("{}/backend-api/codex", server.uri()));
    config.token_budget = Some(TokenBudgetConfig {
        use_history_notes_extension: true,
        ..TokenBudgetConfig::default()
    });

    let mut headers = HeaderMap::new();
    headers.insert(
        "x-openai-actor-authorization",
        HeaderValue::from_static("actor-biscuit"),
    );
    let auth_manager =
        AuthManager::from_auth_for_testing(CodexAuth::Headers(AuthHeaders::new(headers)));
    let mut builder = ExtensionRegistryBuilder::<Config>::new();
    install(&mut builder, auth_manager);
    let registry = builder.build();
    let session_store = ExtensionData::new("session-123");
    let thread_store = ExtensionData::new("thread-123");
    let session_source = SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
        parent_thread_id: ThreadId::new(),
        depth: 1,
        agent_path: Some(AgentPath::root().join("worker").expect("agent path")),
        agent_nickname: None,
        agent_role: None,
    });

    for contributor in registry.thread_lifecycle_contributors() {
        contributor
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
    }

    let tools = exposed_tools(&registry, &session_store, &thread_store);
    assert_eq!(
        tools
            .iter()
            .map(|tool| tool.tool_name())
            .collect::<Vec<_>>(),
        vec![
            ToolName::namespaced("history", "list_windows"),
            ToolName::namespaced("history", "list_items"),
            ToolName::namespaced("history", "read_item"),
            ToolName::namespaced("history", "search_contents"),
            ToolName::namespaced("notes", "list_files_by_prefix"),
            ToolName::namespaced("notes", "read_file"),
            ToolName::namespaced("notes", "search_contents"),
            ToolName::namespaced("notes", "append_to_file"),
            ToolName::namespaced("notes", "write_file"),
        ]
    );

    let read_file = tools
        .iter()
        .find(|tool| tool.tool_name() == ToolName::namespaced("notes", "read_file"))
        .expect("notes.read_file should be exposed");
    let call = tool_call(
        ToolName::namespaced("notes", "read_file"),
        json!({"path": "notes.md"}),
    );
    let output = read_file.handle(call.clone()).await?;
    let ResponseInputItem::FunctionCallOutput { output, .. } =
        output.to_response_item(&call.call_id, &call.payload)
    else {
        panic!("expected function-call output");
    };
    assert_eq!(
        output.content_items(),
        Some(
            [FunctionCallOutputContentItem::EncryptedContent {
                encrypted_content: "enc_payload".to_string(),
            }]
            .as_slice()
        )
    );

    let hints = registry.context_contributors()[0]
        .contribute_thread_context(&session_store, &thread_store)
        .await;
    assert_eq!(
        hints,
        vec![PromptFragment::new(
            PromptSlot::ContextWindow,
            THREAD_HINT,
            ContentItemKind("notes.thread_hint".to_string()),
        )]
    );

    let requests = server.received_requests().await.expect("recorded requests");
    assert_eq!(requests.len(), 2);
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&requests[0].body)?,
        json!({
            "path": "notes.md",
            "context": {
                "session_id": "session-123",
                "current_agent_name": "/root/worker",
            }
        })
    );

    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&requests[1].body)?,
        json!({
            "context": {
                "session_id": "session-123",
                "current_agent_name": "/root/worker",
            }
        })
    );

    for result in [
        json!({"text": ""}),
        json!({"text": "x".repeat(4_001)}),
        json!({"encrypted_output": "old-backend-hint"}),
    ] {
        server.reset().await;
        Mock::given(method("POST"))
            .and(path("/backend-api/codex/alpha/notes/v2/thread_hint"))
            .respond_with(ResponseTemplate::new(200).set_body_json(result))
            .mount(&server)
            .await;
        assert!(
            registry.context_contributors()[0]
                .contribute_thread_context(&session_store, &thread_store)
                .await
                .is_empty()
        );
    }

    let mut disabled_config = config.clone();
    disabled_config.token_budget = None;
    for contributor in registry.config_contributors() {
        contributor.on_config_changed(&session_store, &thread_store, &config, &disabled_config);
    }
    assert!(exposed_tools(&registry, &session_store, &thread_store).is_empty());
    assert!(
        registry.context_contributors()[0]
            .contribute_thread_context(&session_store, &thread_store)
            .await
            .is_empty()
    );

    Ok(())
}

#[tokio::test]
async fn history_notes_require_an_openai_provider_and_codex_backend_auth() -> TestResult {
    let codex_home = TempDir::new()?;
    let mut config = ConfigBuilder::default()
        .loader_overrides(LoaderOverrides::without_managed_config_for_tests())
        .codex_home(codex_home.path().to_path_buf())
        .build()
        .await?;
    config.token_budget = Some(TokenBudgetConfig {
        use_history_notes_extension: true,
        ..TokenBudgetConfig::default()
    });

    for (provider, auth) in [
        (
            config.model_provider.clone(),
            CodexAuth::from_api_key("test-api-key"),
        ),
        (
            ModelProviderInfo::create_amazon_bedrock_provider(/*aws*/ None),
            CodexAuth::create_dummy_chatgpt_auth_for_testing(),
        ),
    ] {
        config.model_provider = provider;
        let mut builder = ExtensionRegistryBuilder::<Config>::new();
        install(&mut builder, AuthManager::from_auth_for_testing(auth));
        let registry = builder.build();
        let session_store = ExtensionData::new("session-123");
        let thread_store = ExtensionData::new("thread-123");

        for contributor in registry.thread_lifecycle_contributors() {
            contributor
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
        }

        assert!(exposed_tools(&registry, &session_store, &thread_store).is_empty());
    }

    Ok(())
}

fn exposed_tools(
    registry: &ExtensionRegistry<Config>,
    session_store: &ExtensionData,
    thread_store: &ExtensionData,
) -> Vec<Arc<dyn ToolExecutor<ToolCall>>> {
    registry
        .tool_contributors()
        .iter()
        .flat_map(|contributor| contributor.tools(session_store, thread_store))
        .collect()
}

fn tool_call(tool_name: ToolName, arguments: serde_json::Value) -> ToolCall {
    ToolCall {
        turn_id: "turn-1".to_string(),
        call_id: "call-read-file".to_string(),
        tool_name,
        model: "gpt-test".to_string(),
        codex_turn_metadata: None,
        truncation_policy: TruncationPolicy::Bytes(1024),
        source: ToolCallSource::Direct,
        conversation_history: ConversationHistory::default(),
        turn_item_emitter: Arc::new(NoopTurnItemEmitter),
        environments: Vec::new(),
        payload: ToolPayload::Function {
            arguments: arguments.to_string(),
        },
    }
}
