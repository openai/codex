use super::*;
use codex_protocol::protocol::GranularApprovalConfig;
use codex_protocol::protocol::McpAuthStatus;
use rmcp::model::JsonObject;
use std::collections::HashSet;
use std::sync::Arc;
use tempfile::tempdir;

fn create_test_tool(server_name: &str, tool_name: &str) -> ToolInfo {
    ToolInfo {
        server_name: server_name.to_string(),
        tool_name: tool_name.to_string(),
        tool_namespace: if server_name == CODEX_APPS_MCP_SERVER_NAME {
            format!("mcp__{server_name}__")
        } else {
            server_name.to_string()
        },
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

#[test]
fn test_qualify_tools_short_non_duplicated_names() {
    let tools = vec![
        create_test_tool("server1", "tool1"),
        create_test_tool("server1", "tool2"),
    ];

    let qualified_tools = qualify_tools(tools);

    assert_eq!(qualified_tools.len(), 2);
    assert!(qualified_tools.contains_key("mcp__server1__tool1"));
    assert!(qualified_tools.contains_key("mcp__server1__tool2"));
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
    assert!(qualified_tools.contains_key("mcp__server1__duplicate_tool"));
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

    let mut keys: Vec<_> = qualified_tools.keys().cloned().collect();
    keys.sort();

    assert_eq!(keys[0].len(), 64);
    assert_eq!(
        keys[0],
        "mcp__my_server__extremel119a2b97664e41363932dc84de21e2ff1b93b3e9"
    );

    assert_eq!(keys[1].len(), 64);
    assert_eq!(
        keys[1],
        "mcp__my_server__yet_anot419a82a89325c1b477274a41f8c65ea5f3a7f341"
    );
}

#[test]
fn test_qualify_tools_sanitizes_invalid_characters() {
    let tools = vec![create_test_tool("server.one", "tool.two-three")];

    let qualified_tools = qualify_tools(tools);

    assert_eq!(qualified_tools.len(), 1);
    let (qualified_name, tool) = qualified_tools.into_iter().next().expect("one tool");
    assert_eq!(qualified_name, "mcp__server_one__tool_two_three");

    // The key is sanitized for OpenAI, but we keep original parts for the actual MCP call.
    assert_eq!(tool.server_name, "server.one");
    assert_eq!(tool.tool_name, "tool.two-three");

    assert!(
        qualified_name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-'),
        "qualified name must be Responses API compatible: {qualified_name:?}"
    );
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
    assert_eq!(filtered[0].tool_name, "tool_a");
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
    assert_eq!(cached_gateway_1[0].tool_name, "one");

    write_cached_codex_apps_tools(&cache_context, &tools_gateway_2);
    let cached_gateway_2 =
        read_cached_codex_apps_tools(&cache_context).expect("cache entry exists for second write");
    assert_eq!(cached_gateway_2[0].tool_name, "two");
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

    assert_eq!(read_user_1[0].tool_name, "one");
    assert_eq!(read_user_2[0].tool_name, "two");
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
    assert_eq!(cached[0].tool_name, "allowed_tool");
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
    assert_eq!(startup_tools[0].tool_name, "calendar_search");
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
    let mut manager = McpConnectionManager::new_uninitialized(&approval_policy);
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
        .get("mcp__codex_apps__calendar_create_event")
        .expect("tool from startup cache");
    assert_eq!(tool.server_name, CODEX_APPS_MCP_SERVER_NAME);
    assert_eq!(tool.tool_name, "calendar_create_event");
}

#[tokio::test]
async fn list_all_tools_blocks_while_client_is_pending_without_startup_snapshot() {
    let pending_client = futures::future::pending::<Result<ManagedClient, StartupOutcomeError>>()
        .boxed()
        .shared();
    let approval_policy = Constrained::allow_any(AskForApproval::OnFailure);
    let mut manager = McpConnectionManager::new_uninitialized(&approval_policy);
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
    let mut manager = McpConnectionManager::new_uninitialized(&approval_policy);
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
    let mut manager = McpConnectionManager::new_uninitialized(&approval_policy);
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
        .get("mcp__codex_apps__calendar_create_event")
        .expect("tool from startup cache");
    assert_eq!(tool.server_name, CODEX_APPS_MCP_SERVER_NAME);
    assert_eq!(tool.tool_name, "calendar_create_event");
}

#[test]
fn elicitation_capability_enabled_only_for_codex_apps() {
    let codex_apps_capability = elicitation_capability_for_server(CODEX_APPS_MCP_SERVER_NAME);
    assert!(matches!(
        codex_apps_capability,
        Some(ElicitationCapability {
            form: Some(FormElicitationCapability {
                schema_validation: None
            }),
            url: None,
        })
    ));

    assert!(elicitation_capability_for_server("custom_mcp").is_none());
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
        "MCP client for `slow` timed out after 10 seconds. Add or adjust `startup_timeout_sec` in your config.toml:\n[mcp_servers.slow]\nstartup_timeout_sec = XX",
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
