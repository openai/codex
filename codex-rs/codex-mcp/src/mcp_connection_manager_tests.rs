use super::*;
use codex_protocol::ToolName;
use codex_protocol::protocol::GranularApprovalConfig;
use codex_protocol::protocol::McpAuthStatus;
use pretty_assertions::assert_eq;
use rmcp::model::JsonObject;
use rmcp::model::Meta;
use rmcp::model::NumberOrString;
use std::collections::HashSet;
use std::sync::Arc;
use tempfile::tempdir;

fn create_test_tool(server_name: &str, tool_name: &str) -> ToolInfo {
    let tool_namespace = format!("mcp__{server_name}__");
    ToolInfo {
        server_name: server_name.to_string(),
        callable_name: tool_name.to_string(),
        callable_namespace: tool_namespace,
        server_instructions: None,
        tool: Tool {
            name: tool_name.to_string().into(),
            title: None,
            description: Some(format!("Test tool: {tool_name}").into()),
            input_schema: Arc::new(JsonObject::default()),
            output_schema: None,
            annotations: None,
            execution: None,
            icons: None,
            meta: None,
        },
        connector_id: None,
        connector_name: None,
        plugin_display_names: Vec::new(),
        connector_description: None,
    }
}

fn create_test_tool_with_connector(
    server_name: &str,
    tool_name: &str,
    connector_id: &str,
    connector_name: Option<&str>,
) -> ToolInfo {
    let mut tool = create_test_tool(server_name, tool_name);
    tool.connector_id = Some(connector_id.to_string());
    tool.connector_name = connector_name.map(ToOwned::to_owned);
    tool
}

fn create_codex_apps_tools_cache_context(
    codex_home: PathBuf,
    account_id: Option<&str>,
    chatgpt_user_id: Option<&str>,
) -> CodexAppsToolsCacheContext {
    CodexAppsToolsCacheContext {
        codex_home,
        user_key: CodexAppsToolsCacheKey {
            account_id: account_id.map(ToOwned::to_owned),
            chatgpt_user_id: chatgpt_user_id.map(ToOwned::to_owned),
            is_workspace_account: false,
        },
    }
}

#[test]
fn declared_openai_file_fields_treat_names_literally() {
    let meta = serde_json::json!({
        "openai/fileParams": ["file", "input_file", "attachments"]
    });
    let meta = meta.as_object().expect("meta object");

    assert_eq!(
        declared_openai_file_input_param_names(Some(meta)),
        vec![
            "file".to_string(),
            "input_file".to_string(),
            "attachments".to_string(),
        ]
    );
}

#[test]
fn tool_with_model_visible_input_schema_masks_file_params() {
    let mut tool = create_test_tool(CODEX_APPS_MCP_SERVER_NAME, "upload").tool;
    tool.input_schema = Arc::new(
        serde_json::json!({
            "type": "object",
            "properties": {
                "file": {
                    "type": "object",
                    "description": "Original file payload."
                },
                "files": {
                    "type": "array",
                    "items": {"type": "object"}
                }
            }
        })
        .as_object()
        .expect("object")
        .clone(),
    );
    tool.meta = Some(Meta(
        serde_json::json!({
            "openai/fileParams": ["file", "files"]
        })
        .as_object()
        .expect("object")
        .clone(),
    ));

    let tool = tool_with_model_visible_input_schema(&tool);

    assert_eq!(
        *tool.input_schema,
        serde_json::json!({
            "type": "object",
            "properties": {
                "file": {
                    "type": "string",
                    "description": "Original file payload. This parameter expects an absolute local file path. If you want to upload a file, provide the absolute path to that file here."
                },
                "files": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "This parameter expects an absolute local file path. If you want to upload a file, provide the absolute path to that file here."
                }
            }
        })
        .as_object()
        .expect("object")
        .clone()
    );
}

#[test]
fn tool_with_model_visible_input_schema_leaves_tools_without_file_params_unchanged() {
    let original_tool = create_test_tool("custom", "upload").tool;

    let tool = tool_with_model_visible_input_schema(&original_tool);

    assert_eq!(tool, original_tool);
}

#[test]
fn elicitation_granular_policy_defaults_to_prompting() {
    assert!(!elicitation_is_rejected_by_policy(
        AskForApproval::OnFailure
    ));
    assert!(!elicitation_is_rejected_by_policy(
        AskForApproval::OnRequest
    ));
    assert!(!elicitation_is_rejected_by_policy(
        AskForApproval::UnlessTrusted
    ));
    assert!(elicitation_is_rejected_by_policy(AskForApproval::Granular(
        GranularApprovalConfig {
            sandbox_approval: true,
            rules: true,
            skill_approval: true,
            request_permissions: true,
            mcp_elicitations: false,
        }
    )));
}

#[test]
fn elicitation_granular_policy_respects_never_and_config() {
    assert!(elicitation_is_rejected_by_policy(AskForApproval::Never));
    assert!(elicitation_is_rejected_by_policy(AskForApproval::Granular(
        GranularApprovalConfig {
            sandbox_approval: true,
            rules: true,
            skill_approval: true,
            request_permissions: true,
            mcp_elicitations: false,
        }
    )));
}

#[tokio::test]
async fn full_access_auto_accepts_elicitation_with_empty_form_schema() {
    let manager =
        ElicitationRequestManager::new(AskForApproval::Never, SandboxPolicy::DangerFullAccess);
    let (tx_event, _rx_event) = async_channel::bounded(1);
    let sender = manager.make_sender("server".to_string(), tx_event);

    let response = sender(
        NumberOrString::Number(1),
        CreateElicitationRequestParams::FormElicitationParams {
            meta: None,
            message: "Confirm?".to_string(),
            requested_schema: rmcp::model::ElicitationSchema::builder()
                .build()
                .expect("schema should build"),
        },
    )
    .await
    .expect("elicitation should auto accept");

    assert_eq!(
        response,
        ElicitationResponse {
            action: ElicitationAction::Accept,
            content: Some(serde_json::json!({})),
            meta: None,
        }
    );
}

#[tokio::test]
async fn full_access_does_not_auto_accept_elicitation_with_requested_fields() {
    let manager =
        ElicitationRequestManager::new(AskForApproval::Never, SandboxPolicy::DangerFullAccess);
    let (tx_event, _rx_event) = async_channel::bounded(1);
    let sender = manager.make_sender("server".to_string(), tx_event);

    let response = sender(
        NumberOrString::Number(1),
        CreateElicitationRequestParams::FormElicitationParams {
            meta: None,
            message: "What should I say?".to_string(),
            requested_schema: rmcp::model::ElicitationSchema::builder()
                .required_property(
                    "message",
                    rmcp::model::PrimitiveSchema::String(rmcp::model::StringSchema::new()),
                )
                .build()
                .expect("schema should build"),
        },
    )
    .await
    .expect("elicitation should auto decline");

    assert_eq!(
        response,
        ElicitationResponse {
            action: ElicitationAction::Decline,
            content: None,
            meta: None,
        }
    );
}

#[test]
fn test_qualify_tools_short_non_duplicated_names() {
    let tools = vec![
        create_test_tool("server1", "tool1"),
        create_test_tool("server1", "tool2"),
    ];

    let qualified_tools = qualify_tools(tools);

    assert_eq!(qualified_tools.len(), 2);
    assert!(qualified_tools.contains_key(&ToolName::namespaced("mcp__server1__", "tool1")));
    assert!(qualified_tools.contains_key(&ToolName::namespaced("mcp__server1__", "tool2")));
}

#[test]
fn test_qualify_tools_duplicated_names_skipped() {
    let tools = vec![
        create_test_tool("server1", "duplicate_tool"),
        create_test_tool("server1", "duplicate_tool"),
    ];

    let qualified_tools = qualify_tools(tools);

    // Only the first tool should remain, the second is skipped
    assert_eq!(qualified_tools.len(), 1);
    assert!(
        qualified_tools.contains_key(&ToolName::namespaced("mcp__server1__", "duplicate_tool"))
    );
}

#[test]
fn test_qualify_tools_long_names_same_server() {
    let server_name = "my_server";

    let tools = vec![
        create_test_tool(
            server_name,
            "extremely_lengthy_function_name_that_absolutely_surpasses_all_reasonable_limits",
        ),
        create_test_tool(
            server_name,
            "yet_another_extremely_lengthy_function_name_that_absolutely_surpasses_all_reasonable_limits",
        ),
    ];

    let qualified_tools = qualify_tools(tools);

    assert_eq!(qualified_tools.len(), 2);

    let mut keys: Vec<_> = qualified_tools.keys().map(ToolName::display).collect();
    keys.sort();

    assert!(keys.iter().all(|key| key.len() == 64));
    assert!(keys.iter().all(|key| key.starts_with("mcp__my_server__")));
    assert!(
        keys.iter()
            .all(|key| key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')),
        "qualified names must be code-mode compatible: {keys:?}"
    );
}

#[test]
fn test_qualify_tools_sanitizes_invalid_characters() {
    let tools = vec![create_test_tool("server.one", "tool.two-three")];

    let qualified_tools = qualify_tools(tools);

    assert_eq!(qualified_tools.len(), 1);
    let (qualified_name, tool) = qualified_tools.into_iter().next().expect("one tool");
    assert_eq!(
        qualified_name,
        ToolName::namespaced("mcp__server_one__", "tool_two_three")
    );
    assert_eq!(
        format!("{}{}", tool.callable_namespace, tool.callable_name),
        qualified_name.display()
    );

    // The key and callable parts are sanitized for model-visible tool calls, but
    // the raw MCP name is preserved for the actual MCP call.
    assert_eq!(tool.server_name, "server.one");
    assert_eq!(tool.callable_namespace, "mcp__server_one__");
    assert_eq!(tool.callable_name, "tool_two_three");
    assert_eq!(tool.tool.name, "tool.two-three");

    assert!(
        qualified_name
            .display()
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_'),
        "qualified name must be code-mode compatible: {qualified_name:?}"
    );
}

#[test]
fn test_qualify_tools_keeps_hyphenated_mcp_tools_callable() {
    let tools = vec![create_test_tool("music-studio", "get-strudel-guide")];

    let qualified_tools = qualify_tools(tools);

    assert_eq!(qualified_tools.len(), 1);
    let (qualified_name, tool) = qualified_tools.into_iter().next().expect("one tool");
    assert_eq!(
        qualified_name,
        ToolName::namespaced("mcp__music_studio__", "get_strudel_guide")
    );
    assert_eq!(tool.callable_namespace, "mcp__music_studio__");
    assert_eq!(tool.callable_name, "get_strudel_guide");
    assert_eq!(tool.tool.name, "get-strudel-guide");
}

#[test]
fn test_qualify_tools_disambiguates_sanitized_namespace_collisions() {
    let tools = vec![
        create_test_tool("basic-server", "lookup"),
        create_test_tool("basic_server", "query"),
    ];

    let qualified_tools = qualify_tools(tools);

    assert_eq!(qualified_tools.len(), 2);
    let mut namespaces = qualified_tools
        .values()
        .map(|tool| tool.callable_namespace.as_str())
        .collect::<Vec<_>>();
    namespaces.sort();
    namespaces.dedup();
    assert_eq!(namespaces.len(), 2);

    let raw_servers = qualified_tools
        .values()
        .map(|tool| tool.server_name.as_str())
        .collect::<HashSet<_>>();
    assert_eq!(raw_servers, HashSet::from(["basic-server", "basic_server"]));
    assert!(
        qualified_tools.keys().all(|key| key
            .display()
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')),
        "qualified names must be code-mode compatible: {qualified_tools:?}"
    );
}

#[test]
fn test_qualify_tools_disambiguates_sanitized_tool_name_collisions() {
    let tools = vec![
        create_test_tool("server", "tool-name"),
        create_test_tool("server", "tool_name"),
    ];

    let qualified_tools = qualify_tools(tools);

    assert_eq!(qualified_tools.len(), 2);
    let raw_tool_names = qualified_tools
        .values()
        .map(|tool| tool.tool.name.to_string())
        .collect::<HashSet<_>>();
    assert_eq!(
        raw_tool_names,
        HashSet::from(["tool-name".to_string(), "tool_name".to_string()])
    );
    let callable_tool_names = qualified_tools
        .values()
        .map(|tool| tool.callable_name.as_str())
        .collect::<HashSet<_>>();
    assert_eq!(callable_tool_names.len(), 2);
}

#[test]
fn tool_filter_allows_by_default() {
    let filter = ToolFilter::default();

    assert!(filter.allows("any"));
}

#[test]
fn tool_filter_applies_enabled_list() {
    let filter = ToolFilter {
        enabled: Some(HashSet::from(["allowed".to_string()])),
        disabled: HashSet::new(),
    };

    assert!(filter.allows("allowed"));
    assert!(!filter.allows("denied"));
}

#[test]
fn tool_filter_applies_disabled_list() {
    let filter = ToolFilter {
        enabled: None,
        disabled: HashSet::from(["blocked".to_string()]),
    };

    assert!(!filter.allows("blocked"));
    assert!(filter.allows("open"));
}

#[test]
fn tool_filter_applies_enabled_then_disabled() {
    let filter = ToolFilter {
        enabled: Some(HashSet::from(["keep".to_string(), "remove".to_string()])),
        disabled: HashSet::from(["remove".to_string()]),
    };

    assert!(filter.allows("keep"));
    assert!(!filter.allows("remove"));
    assert!(!filter.allows("unknown"));
}

#[test]
fn filter_tools_applies_per_server_filters() {
    let server1_tools = vec![
        create_test_tool("server1", "tool_a"),
        create_test_tool("server1", "tool_b"),
    ];
    let server2_tools = vec![create_test_tool("server2", "tool_a")];
    let server1_filter = ToolFilter {
        enabled: Some(HashSet::from(["tool_a".to_string(), "tool_b".to_string()])),
        disabled: HashSet::from(["tool_b".to_string()]),
    };
    let server2_filter = ToolFilter {
        enabled: None,
        disabled: HashSet::from(["tool_a".to_string()]),
    };

    let filtered: Vec<_> = filter_tools(server1_tools, &server1_filter)
        .into_iter()
        .chain(filter_tools(server2_tools, &server2_filter))
        .collect();

    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].server_name, "server1");
    assert_eq!(filtered[0].callable_name, "tool_a");
}

#[test]
fn codex_apps_tools_cache_is_overwritten_by_last_write() {
    let codex_home = tempdir().expect("tempdir");
    let cache_context = create_codex_apps_tools_cache_context(
        codex_home.path().to_path_buf(),
        Some("account-one"),
        Some("user-one"),
    );
    let tools_gateway_1 = vec![create_test_tool(CODEX_APPS_MCP_SERVER_NAME, "one")];
    let tools_gateway_2 = vec![create_test_tool(CODEX_APPS_MCP_SERVER_NAME, "two")];

    write_cached_codex_apps_tools(&cache_context, &tools_gateway_1);
    let cached_gateway_1 =
        read_cached_codex_apps_tools(&cache_context).expect("cache entry exists for first write");
    assert_eq!(cached_gateway_1[0].callable_name, "one");

    write_cached_codex_apps_tools(&cache_context, &tools_gateway_2);
    let cached_gateway_2 =
        read_cached_codex_apps_tools(&cache_context).expect("cache entry exists for second write");
    assert_eq!(cached_gateway_2[0].callable_name, "two");
}

#[test]
fn codex_apps_tools_cache_is_scoped_per_user() {
    let codex_home = tempdir().expect("tempdir");
    let cache_context_user_1 = create_codex_apps_tools_cache_context(
        codex_home.path().to_path_buf(),
        Some("account-one"),
        Some("user-one"),
    );
    let cache_context_user_2 = create_codex_apps_tools_cache_context(
        codex_home.path().to_path_buf(),
        Some("account-two"),
        Some("user-two"),
    );
    let tools_user_1 = vec![create_test_tool(CODEX_APPS_MCP_SERVER_NAME, "one")];
    let tools_user_2 = vec![create_test_tool(CODEX_APPS_MCP_SERVER_NAME, "two")];

    write_cached_codex_apps_tools(&cache_context_user_1, &tools_user_1);
    write_cached_codex_apps_tools(&cache_context_user_2, &tools_user_2);

    let read_user_1 =
        read_cached_codex_apps_tools(&cache_context_user_1).expect("cache entry for user one");
    let read_user_2 =
        read_cached_codex_apps_tools(&cache_context_user_2).expect("cache entry for user two");

    assert_eq!(read_user_1[0].callable_name, "one");
    assert_eq!(read_user_2[0].callable_name, "two");
    assert_ne!(
        cache_context_user_1.cache_path(),
        cache_context_user_2.cache_path(),
        "each user should get an isolated cache file"
    );
}

#[test]
fn codex_apps_tools_cache_filters_disallowed_connectors() {
    let codex_home = tempdir().expect("tempdir");
    let cache_context = create_codex_apps_tools_cache_context(
        codex_home.path().to_path_buf(),
        Some("account-one"),
        Some("user-one"),
    );
    let tools = vec![
        create_test_tool_with_connector(
            CODEX_APPS_MCP_SERVER_NAME,
            "blocked_tool",
            "connector_openai_hidden",
            Some("Hidden"),
        ),
        create_test_tool_with_connector(
            CODEX_APPS_MCP_SERVER_NAME,
            "allowed_tool",
            "calendar",
            Some("Calendar"),
        ),
    ];

    write_cached_codex_apps_tools(&cache_context, &tools);
    let cached = read_cached_codex_apps_tools(&cache_context).expect("cache entry exists for user");

    assert_eq!(cached.len(), 1);
    assert_eq!(cached[0].callable_name, "allowed_tool");
    assert_eq!(cached[0].connector_id.as_deref(), Some("calendar"));
}

#[test]
fn codex_apps_tools_cache_is_ignored_when_schema_version_mismatches() {
    let codex_home = tempdir().expect("tempdir");
    let cache_context = create_codex_apps_tools_cache_context(
        codex_home.path().to_path_buf(),
        Some("account-one"),
        Some("user-one"),
    );
    let cache_path = cache_context.cache_path();
    if let Some(parent) = cache_path.parent() {
        std::fs::create_dir_all(parent).expect("create parent");
    }
    let bytes = serde_json::to_vec_pretty(&serde_json::json!({
        "schema_version": CODEX_APPS_TOOLS_CACHE_SCHEMA_VERSION + 1,
        "tools": [create_test_tool(CODEX_APPS_MCP_SERVER_NAME, "one")],
    }))
    .expect("serialize");
    std::fs::write(cache_path, bytes).expect("write");

    assert!(read_cached_codex_apps_tools(&cache_context).is_none());
}

#[test]
fn codex_apps_tools_cache_is_ignored_when_json_is_invalid() {
    let codex_home = tempdir().expect("tempdir");
    let cache_context = create_codex_apps_tools_cache_context(
        codex_home.path().to_path_buf(),
        Some("account-one"),
        Some("user-one"),
    );
    let cache_path = cache_context.cache_path();
    if let Some(parent) = cache_path.parent() {
        std::fs::create_dir_all(parent).expect("create parent");
    }
    std::fs::write(cache_path, b"{not json").expect("write");

    assert!(read_cached_codex_apps_tools(&cache_context).is_none());
}

#[test]
fn startup_cached_codex_apps_tools_loads_from_disk_cache() {
    let codex_home = tempdir().expect("tempdir");
    let cache_context = create_codex_apps_tools_cache_context(
        codex_home.path().to_path_buf(),
        Some("account-one"),
        Some("user-one"),
    );
    let cached_tools = vec![create_test_tool(
        CODEX_APPS_MCP_SERVER_NAME,
        "calendar_search",
    )];
    write_cached_codex_apps_tools(&cache_context, &cached_tools);

    let startup_snapshot = load_startup_cached_codex_apps_tools_snapshot(
        CODEX_APPS_MCP_SERVER_NAME,
        Some(&cache_context),
    );
    let startup_tools = startup_snapshot.expect("expected startup snapshot to load from cache");

    assert_eq!(startup_tools.len(), 1);
    assert_eq!(startup_tools[0].server_name, CODEX_APPS_MCP_SERVER_NAME);
    assert_eq!(startup_tools[0].callable_name, "calendar_search");
}

#[tokio::test]
async fn list_all_tools_uses_startup_snapshot_while_client_is_pending() {
    let startup_tools = vec![create_test_tool(
        CODEX_APPS_MCP_SERVER_NAME,
        "calendar_create_event",
    )];
    let pending_client = futures::future::pending::<Result<ManagedClient, StartupOutcomeError>>()
        .boxed()
        .shared();
    let approval_policy = Constrained::allow_any(AskForApproval::OnFailure);
    let sandbox_policy = Constrained::allow_any(SandboxPolicy::new_read_only_policy());
    let mut manager = McpConnectionManager::new_uninitialized(&approval_policy, &sandbox_policy);
    manager.clients.insert(
        CODEX_APPS_MCP_SERVER_NAME.to_string(),
        AsyncManagedClient {
            client: pending_client,
            startup_snapshot: Some(startup_tools),
            startup_complete: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            tool_plugin_provenance: Arc::new(ToolPluginProvenance::default()),
        },
    );

    let tools = manager.list_all_tools().await;
    let tool = tools
        .get(&ToolName::namespaced(
            "mcp__codex_apps__",
            "calendar_create_event",
        ))
        .expect("tool from startup cache");
    assert_eq!(tool.server_name, CODEX_APPS_MCP_SERVER_NAME);
    assert_eq!(tool.callable_name, "calendar_create_event");
}

#[tokio::test]
async fn resolve_tool_info_accepts_plain_and_namespaced_tool_names() {
    let startup_tools = vec![create_test_tool("rmcp", "echo")];
    let pending_client = futures::future::pending::<Result<ManagedClient, StartupOutcomeError>>()
        .boxed()
        .shared();
    let approval_policy = Constrained::allow_any(AskForApproval::OnFailure);
    let sandbox_policy = Constrained::allow_any(SandboxPolicy::new_read_only_policy());
    let mut manager = McpConnectionManager::new_uninitialized(&approval_policy, &sandbox_policy);
    manager.clients.insert(
        "rmcp".to_string(),
        AsyncManagedClient {
            client: pending_client,
            startup_snapshot: Some(startup_tools),
            startup_complete: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            tool_plugin_provenance: Arc::new(ToolPluginProvenance::default()),
        },
    );

    let flat = manager
        .resolve_tool_info(&ToolName::plain("mcp__rmcp__echo"))
        .await
        .expect("flat qualified MCP tool name should resolve");
    let split = manager
        .resolve_tool_info(&ToolName::namespaced("mcp__rmcp__", "echo"))
        .await
        .expect("split MCP tool namespace and name should resolve");
    let split_with_flat_name = manager
        .resolve_tool_info(&ToolName::namespaced("mcp__rmcp__", "mcp__rmcp__echo"))
        .await
        .expect("split namespace with flat qualified MCP tool name should resolve");

    let expected = ("rmcp", "mcp__rmcp__", "echo", "echo");
    assert_eq!(
        (
            flat.server_name.as_str(),
            flat.callable_namespace.as_str(),
            flat.callable_name.as_str(),
            flat.tool.name.as_ref(),
        ),
        expected
    );
    assert_eq!(
        (
            split.server_name.as_str(),
            split.callable_namespace.as_str(),
            split.callable_name.as_str(),
            split.tool.name.as_ref(),
        ),
        expected
    );
    assert_eq!(
        (
            split_with_flat_name.server_name.as_str(),
            split_with_flat_name.callable_namespace.as_str(),
            split_with_flat_name.callable_name.as_str(),
            split_with_flat_name.tool.name.as_ref(),
        ),
        expected
    );
}

#[tokio::test]
async fn list_all_tools_blocks_while_client_is_pending_without_startup_snapshot() {
    let pending_client = futures::future::pending::<Result<ManagedClient, StartupOutcomeError>>()
        .boxed()
        .shared();
    let approval_policy = Constrained::allow_any(AskForApproval::OnFailure);
    let sandbox_policy = Constrained::allow_any(SandboxPolicy::new_read_only_policy());
    let mut manager = McpConnectionManager::new_uninitialized(&approval_policy, &sandbox_policy);
    manager.clients.insert(
        CODEX_APPS_MCP_SERVER_NAME.to_string(),
        AsyncManagedClient {
            client: pending_client,
            startup_snapshot: None,
            startup_complete: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            tool_plugin_provenance: Arc::new(ToolPluginProvenance::default()),
        },
    );

    let timeout_result =
        tokio::time::timeout(Duration::from_millis(10), manager.list_all_tools()).await;
    assert!(timeout_result.is_err());
}

#[tokio::test]
async fn list_all_tools_does_not_block_when_startup_snapshot_cache_hit_is_empty() {
    let pending_client = futures::future::pending::<Result<ManagedClient, StartupOutcomeError>>()
        .boxed()
        .shared();
    let approval_policy = Constrained::allow_any(AskForApproval::OnFailure);
    let sandbox_policy = Constrained::allow_any(SandboxPolicy::new_read_only_policy());
    let mut manager = McpConnectionManager::new_uninitialized(&approval_policy, &sandbox_policy);
    manager.clients.insert(
        CODEX_APPS_MCP_SERVER_NAME.to_string(),
        AsyncManagedClient {
            client: pending_client,
            startup_snapshot: Some(Vec::new()),
            startup_complete: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            tool_plugin_provenance: Arc::new(ToolPluginProvenance::default()),
        },
    );

    let timeout_result =
        tokio::time::timeout(Duration::from_millis(10), manager.list_all_tools()).await;
    let tools = timeout_result.expect("cache-hit startup snapshot should not block");
    assert!(tools.is_empty());
}

#[tokio::test]
async fn list_all_tools_uses_startup_snapshot_when_client_startup_fails() {
    let startup_tools = vec![create_test_tool(
        CODEX_APPS_MCP_SERVER_NAME,
        "calendar_create_event",
    )];
    let failed_client = futures::future::ready::<Result<ManagedClient, StartupOutcomeError>>(Err(
        StartupOutcomeError::Failed {
            error: "startup failed".to_string(),
        },
    ))
    .boxed()
    .shared();
    let approval_policy = Constrained::allow_any(AskForApproval::OnFailure);
    let sandbox_policy = Constrained::allow_any(SandboxPolicy::new_read_only_policy());
    let mut manager = McpConnectionManager::new_uninitialized(&approval_policy, &sandbox_policy);
    let startup_complete = Arc::new(std::sync::atomic::AtomicBool::new(true));
    manager.clients.insert(
        CODEX_APPS_MCP_SERVER_NAME.to_string(),
        AsyncManagedClient {
            client: failed_client,
            startup_snapshot: Some(startup_tools),
            startup_complete,
            tool_plugin_provenance: Arc::new(ToolPluginProvenance::default()),
        },
    );

    let tools = manager.list_all_tools().await;
    let tool = tools
        .get(&ToolName::namespaced(
            "mcp__codex_apps__",
            "calendar_create_event",
        ))
        .expect("tool from startup cache");
    assert_eq!(tool.server_name, CODEX_APPS_MCP_SERVER_NAME);
    assert_eq!(tool.callable_name, "calendar_create_event");
}

#[test]
fn elicitation_capability_enabled_for_custom_servers() {
    for server_name in [CODEX_APPS_MCP_SERVER_NAME, "custom_mcp"] {
        let capability = elicitation_capability_for_server(server_name);
        assert!(matches!(
            capability,
            Some(ElicitationCapability {
                form: Some(FormElicitationCapability {
                    schema_validation: None
                }),
                url: None,
            })
        ));
    }
}

#[test]
fn mcp_init_error_display_prompts_for_github_pat() {
    let server_name = "github";
    let entry = McpAuthStatusEntry {
        config: McpServerConfig {
            transport: McpServerTransportConfig::StreamableHttp {
                url: "https://api.githubcopilot.com/mcp/".to_string(),
                bearer_token_env_var: None,
                http_headers: None,
                env_http_headers: None,
            },
            enabled: true,
            required: false,
            disabled_reason: None,
            startup_timeout_sec: None,
            tool_timeout_sec: None,
            enabled_tools: None,
            disabled_tools: None,
            scopes: None,
            oauth_resource: None,
            tools: HashMap::new(),
        },
        auth_status: McpAuthStatus::Unsupported,
    };
    let err: StartupOutcomeError = anyhow::anyhow!("OAuth is unsupported").into();

    let display = mcp_init_error_display(server_name, Some(&entry), &err);

    let expected = format!(
        "GitHub MCP does not support OAuth. Log in by adding a personal access token (https://github.com/settings/personal-access-tokens) to your environment and config.toml:\n[mcp_servers.{server_name}]\nbearer_token_env_var = CODEX_GITHUB_PERSONAL_ACCESS_TOKEN"
    );

    assert_eq!(expected, display);
}

#[test]
fn mcp_init_error_display_prompts_for_login_when_auth_required() {
    let server_name = "example";
    let err: StartupOutcomeError = anyhow::anyhow!("Auth required for server").into();

    let display = mcp_init_error_display(server_name, /*entry*/ None, &err);

    let expected = format!(
        "The {server_name} MCP server is not logged in. Run `codex mcp login {server_name}`."
    );

    assert_eq!(expected, display);
}

#[test]
fn mcp_init_error_display_reports_generic_errors() {
    let server_name = "custom";
    let entry = McpAuthStatusEntry {
        config: McpServerConfig {
            transport: McpServerTransportConfig::StreamableHttp {
                url: "https://example.com".to_string(),
                bearer_token_env_var: Some("TOKEN".to_string()),
                http_headers: None,
                env_http_headers: None,
            },
            enabled: true,
            required: false,
            disabled_reason: None,
            startup_timeout_sec: None,
            tool_timeout_sec: None,
            enabled_tools: None,
            disabled_tools: None,
            scopes: None,
            oauth_resource: None,
            tools: HashMap::new(),
        },
        auth_status: McpAuthStatus::Unsupported,
    };
    let err: StartupOutcomeError = anyhow::anyhow!("boom").into();

    let display = mcp_init_error_display(server_name, Some(&entry), &err);

    let expected = format!("MCP client for `{server_name}` failed to start: {err:#}");

    assert_eq!(expected, display);
}

#[test]
fn mcp_init_error_display_includes_startup_timeout_hint() {
    let server_name = "slow";
    let err: StartupOutcomeError = anyhow::anyhow!("request timed out").into();

    let display = mcp_init_error_display(server_name, /*entry*/ None, &err);

    assert_eq!(
        "MCP client for `slow` timed out after 30 seconds. Add or adjust `startup_timeout_sec` in your config.toml:\n[mcp_servers.slow]\nstartup_timeout_sec = XX",
        display
    );
}

#[test]
fn transport_origin_extracts_http_origin() {
    let transport = McpServerTransportConfig::StreamableHttp {
        url: "https://example.com:8443/path?query=1".to_string(),
        bearer_token_env_var: None,
        http_headers: None,
        env_http_headers: None,
    };

    assert_eq!(
        transport_origin(&transport),
        Some("https://example.com:8443".to_string())
    );
}

#[test]
fn transport_origin_is_stdio_for_stdio_transport() {
    let transport = McpServerTransportConfig::Stdio {
        command: "server".to_string(),
        args: Vec::new(),
        env: None,
        env_vars: Vec::new(),
        cwd: None,
    };

    assert_eq!(transport_origin(&transport), Some("stdio".to_string()));
}
