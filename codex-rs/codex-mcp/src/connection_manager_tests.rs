use super::*;
use crate::codex_apps::CODEX_APPS_TOOLS_CACHE_SCHEMA_VERSION;
use crate::codex_apps::CodexAppsToolsCacheContext;
use crate::codex_apps::load_startup_cached_codex_apps_server_info;
use crate::codex_apps::load_startup_cached_codex_apps_tools_snapshot;
use crate::codex_apps::read_cached_codex_apps_tools;
use crate::codex_apps::write_cached_codex_apps_tools;
use crate::codex_apps::write_codex_apps_tools_cache;
use crate::declared_openai_file_input_param_names;
use crate::elicitation::ElicitationRequestManager;
use crate::elicitation::elicitation_is_rejected_by_policy;
use crate::rmcp_client::AsyncManagedClient;
use crate::rmcp_client::ManagedClient;
use crate::rmcp_client::StartupOutcomeError;
use crate::server::EffectiveMcpServer;
use crate::server::McpServerMetadata;
use crate::server::McpServerOrigin;
use crate::tools::ToolFilter;
use crate::tools::ToolInfo;
use crate::tools::filter_tools;
use crate::tools::normalize_tools_for_model_with_prefix;
use crate::tools::tool_with_model_visible_input_schema;
use codex_config::AppToolApproval;
use codex_config::Constrained;
use codex_config::McpServerConfig;
use codex_config::McpServerToolConfig;
use codex_config::types::AuthKeyringBackendKind;
use codex_exec_server::EnvironmentManager;
use codex_protocol::ToolName;
use codex_protocol::mcp::McpServerInfo;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::GranularApprovalConfig;
use codex_protocol::protocol::McpAuthStatus;
use futures::FutureExt;
use pretty_assertions::assert_eq;
use rmcp::model::CreateElicitationRequestParams;
use rmcp::model::ElicitationAction;
use rmcp::model::ElicitationCapability;
use rmcp::model::JsonObject;
use rmcp::model::Meta;
use rmcp::model::NumberOrString;
use rmcp::model::Tool;
use std::collections::HashSet;
use std::sync::Arc;
use tempfile::tempdir;

fn create_test_tool(server_name: &str, tool_name: &str) -> ToolInfo {
    ToolInfo {
        server_name: server_name.to_string(),
        supports_parallel_tool_calls: false,
        server_origin: None,
        callable_name: tool_name.to_string(),
        callable_namespace: server_name.to_string(),
        namespace_description: None,
        tool: Tool::new(
            tool_name.to_string(),
            format!("Test tool: {tool_name}"),
            Arc::new(JsonObject::default()),
        ),
        connector_id: None,
        connector_name: None,
        plugin_display_names: Vec::new(),
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

fn create_test_server_info(title: &str) -> McpServerInfo {
    McpServerInfo {
        name: "codex-apps".to_string(),
        title: Some(title.to_string()),
        version: "1.0.0".to_string(),
        description: None,
        icons: None,
        website_url: None,
    }
}

fn model_tool_names(tools: &[ToolInfo]) -> HashSet<ToolName> {
    tools
        .iter()
        .map(ToolInfo::canonical_tool_name)
        .collect::<HashSet<_>>()
}

fn model_tool_name_len(name: &ToolName) -> usize {
    name.namespace
        .as_deref()
        .map_or(0, |namespace| namespace.len() + "__".len())
        + name.name.len()
}

fn is_code_mode_compatible_tool_name(name: &ToolName) -> bool {
    name.namespace
        .as_deref()
        .into_iter()
        .chain(std::iter::once(name.name.as_str()))
        .flat_map(str::chars)
        .all(|c| c.is_ascii_alphanumeric() || c == '_')
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
async fn disabled_permissions_auto_accept_elicitation_with_empty_form_schema() {
    let manager = ElicitationRequestManager::new(
        AskForApproval::Never,
        PermissionProfile::Disabled,
        /*reviewer*/ None,
    );
    let (tx_event, _rx_event) = async_channel::bounded(1);
    let sender = manager.make_sender("server".to_string(), tx_event);

    let response = sender(
        NumberOrString::Number(1),
        codex_rmcp_client::Elicitation::Mcp(
            CreateElicitationRequestParams::FormElicitationParams {
                meta: None,
                message: "Confirm?".to_string(),
                requested_schema: rmcp::model::ElicitationSchema::builder()
                    .build()
                    .expect("schema should build"),
            },
        ),
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
async fn disabled_permissions_do_not_auto_accept_elicitation_with_requested_fields() {
    let manager = ElicitationRequestManager::new(
        AskForApproval::Never,
        PermissionProfile::Disabled,
        /*reviewer*/ None,
    );
    let (tx_event, _rx_event) = async_channel::bounded(1);
    let sender = manager.make_sender("server".to_string(), tx_event);

    let response = sender(
        NumberOrString::Number(1),
        codex_rmcp_client::Elicitation::Mcp(
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
        ),
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
fn test_normalize_tools_short_non_duplicated_names() {
    let tools = vec![
        create_test_tool("server1", "tool1"),
        create_test_tool("server1", "tool2"),
    ];

    let model_tools =
        normalize_tools_for_model_with_prefix(tools, /*prefix_mcp_tool_names*/ true);

    assert_eq!(
        model_tool_names(&model_tools),
        HashSet::from([
            ToolName::namespaced("mcp__server1", "tool1"),
            ToolName::namespaced("mcp__server1", "tool2")
        ])
    );
}

#[test]
fn test_normalize_tools_duplicated_names_skipped() {
    let tools = vec![
        create_test_tool("server1", "duplicate_tool"),
        create_test_tool("server1", "duplicate_tool"),
    ];

    let model_tools =
        normalize_tools_for_model_with_prefix(tools, /*prefix_mcp_tool_names*/ true);

    // Only the first tool should remain, the second is skipped
    assert_eq!(
        model_tool_names(&model_tools),
        HashSet::from([ToolName::namespaced("mcp__server1", "duplicate_tool")])
    );
}

#[test]
fn test_normalize_tools_long_names_same_server() {
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

    let model_tools =
        normalize_tools_for_model_with_prefix(tools, /*prefix_mcp_tool_names*/ true);

    assert_eq!(model_tools.len(), 2);

    let names = model_tool_names(&model_tools);

    assert!(names.iter().all(|name| model_tool_name_len(name) == 64));
    assert!(
        names
            .iter()
            .all(|name| name.namespace.as_deref() == Some("mcp__my_server"))
    );
    assert!(
        names.iter().all(is_code_mode_compatible_tool_name),
        "model-visible names must be code-mode compatible: {names:?}"
    );
}

#[test]
fn test_normalize_tools_sanitizes_invalid_characters() {
    let tools = vec![create_test_tool("server.one", "tool.two-three")];

    let model_tools =
        normalize_tools_for_model_with_prefix(tools, /*prefix_mcp_tool_names*/ true);

    assert_eq!(model_tools.len(), 1);
    let tool = model_tools.into_iter().next().expect("one tool");
    let model_name = tool.canonical_tool_name();
    assert_eq!(
        model_name,
        ToolName::namespaced("mcp__server_one", "tool_two_three")
    );
    assert_eq!(
        ToolName::namespaced(tool.callable_namespace.clone(), tool.callable_name.clone()),
        model_name
    );
    // The callable parts are sanitized for model-visible tool calls, but the raw
    // MCP name is preserved for the actual MCP call.
    assert_eq!(tool.server_name, "server.one");
    assert_eq!(tool.callable_namespace, "mcp__server_one");
    assert_eq!(tool.callable_name, "tool_two_three");
    assert_eq!(tool.tool.name, "tool.two-three");

    assert!(
        is_code_mode_compatible_tool_name(&model_name),
        "model-visible name must be code-mode compatible: {model_name:?}"
    );
}

#[test]
fn test_normalize_tools_keeps_hyphenated_mcp_tools_callable() {
    let tools = vec![create_test_tool("music-studio", "get-strudel-guide")];

    let model_tools =
        normalize_tools_for_model_with_prefix(tools, /*prefix_mcp_tool_names*/ true);

    assert_eq!(model_tools.len(), 1);
    let tool = model_tools.into_iter().next().expect("one tool");
    assert_eq!(
        tool.canonical_tool_name(),
        ToolName::namespaced("mcp__music_studio", "get_strudel_guide")
    );
    assert_eq!(tool.callable_namespace, "mcp__music_studio");
    assert_eq!(tool.callable_name, "get_strudel_guide");
    assert_eq!(tool.tool.name, "get-strudel-guide");
}

#[test]
fn test_normalize_tools_disambiguates_sanitized_namespace_collisions() {
    let tools = vec![
        create_test_tool("basic-server", "lookup"),
        create_test_tool("basic_server", "query"),
    ];

    let model_tools =
        normalize_tools_for_model_with_prefix(tools, /*prefix_mcp_tool_names*/ true);

    assert_eq!(model_tools.len(), 2);
    let mut namespaces = model_tools
        .iter()
        .map(|tool| tool.callable_namespace.as_str())
        .collect::<Vec<_>>();
    namespaces.sort();
    namespaces.dedup();
    assert_eq!(namespaces.len(), 2);

    let raw_servers = model_tools
        .iter()
        .map(|tool| tool.server_name.as_str())
        .collect::<HashSet<_>>();
    assert_eq!(raw_servers, HashSet::from(["basic-server", "basic_server"]));
    let model_names = model_tool_names(&model_tools);
    assert!(
        model_names.iter().all(is_code_mode_compatible_tool_name),
        "model-visible names must be code-mode compatible: {model_names:?}"
    );
}

#[test]
fn test_normalize_tools_disambiguates_sanitized_tool_name_collisions() {
    let tools = vec![
        create_test_tool("server", "tool-name"),
        create_test_tool("server", "tool_name"),
    ];

    let model_tools =
        normalize_tools_for_model_with_prefix(tools, /*prefix_mcp_tool_names*/ true);

    assert_eq!(model_tools.len(), 2);
    let raw_tool_names = model_tools
        .iter()
        .map(|tool| tool.tool.name.to_string())
        .collect::<HashSet<_>>();
    assert_eq!(
        raw_tool_names,
        HashSet::from(["tool-name".to_string(), "tool_name".to_string()])
    );
    let callable_tool_names = model_tools
        .iter()
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
        cache_context_user_1.tools_cache_path(),
        cache_context_user_2.tools_cache_path(),
        "each user should get an isolated cache file"
    );
}

#[test]
fn codex_apps_tools_cache_preserves_formerly_disallowed_connectors() {
    let codex_home = tempdir().expect("tempdir");
    let cache_context = create_codex_apps_tools_cache_context(
        codex_home.path().to_path_buf(),
        Some("account-one"),
        Some("user-one"),
    );
    let tools = vec![
        create_test_tool_with_connector(
            CODEX_APPS_MCP_SERVER_NAME,
            "formerly_blocked_tool",
            "connector_2b0a9009c9c64bf9933a3dae3f2b1254",
            Some("Formerly Blocked"),
        ),
        create_test_tool_with_connector(
            CODEX_APPS_MCP_SERVER_NAME,
            "calendar_tool",
            "calendar",
            Some("Calendar"),
        ),
    ];

    write_cached_codex_apps_tools(&cache_context, &tools);
    let cached = read_cached_codex_apps_tools(&cache_context).expect("cache entry exists for user");

    assert_eq!(
        cached
            .iter()
            .map(|tool| (tool.callable_name.as_str(), tool.connector_id.as_deref()))
            .collect::<Vec<_>>(),
        vec![
            (
                "formerly_blocked_tool",
                Some("connector_2b0a9009c9c64bf9933a3dae3f2b1254")
            ),
            ("calendar_tool", Some("calendar")),
        ]
    );
}

#[test]
fn codex_apps_tools_cache_is_ignored_when_schema_version_mismatches() {
    let codex_home = tempdir().expect("tempdir");
    let cache_context = create_codex_apps_tools_cache_context(
        codex_home.path().to_path_buf(),
        Some("account-one"),
        Some("user-one"),
    );
    let cache_path = cache_context.tools_cache_path();
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
    let cache_path = cache_context.tools_cache_path();
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
    let server_info = create_test_server_info("Codex Apps");
    write_codex_apps_tools_cache(Some(&cache_context), &server_info, &cached_tools);

    let startup_tools = load_startup_cached_codex_apps_tools_snapshot(Some(&cache_context))
        .expect("expected startup snapshot to load from cache");
    let cached_server_info = load_startup_cached_codex_apps_server_info(Some(&cache_context));

    assert_eq!(startup_tools.len(), 1);
    assert_eq!(startup_tools[0].server_name, CODEX_APPS_MCP_SERVER_NAME);
    assert_eq!(startup_tools[0].callable_name, "calendar_search");
    assert_eq!(cached_server_info, Some(server_info));
}

#[test]
fn startup_cached_codex_apps_tools_loads_without_server_info_cache() {
    let codex_home = tempdir().expect("tempdir");
    let cache_context = create_codex_apps_tools_cache_context(
        codex_home.path().to_path_buf(),
        Some("account-one"),
        Some("user-one"),
    );
    let cache_path = cache_context.tools_cache_path();
    if let Some(parent) = cache_path.parent() {
        std::fs::create_dir_all(parent).expect("create parent");
    }
    let bytes = serde_json::to_vec_pretty(&serde_json::json!({
        "schema_version": CODEX_APPS_TOOLS_CACHE_SCHEMA_VERSION,
        "tools": [create_test_tool(CODEX_APPS_MCP_SERVER_NAME, "calendar_search")],
    }))
    .expect("serialize");
    std::fs::write(cache_path, bytes).expect("write");

    let startup_tools = load_startup_cached_codex_apps_tools_snapshot(Some(&cache_context))
        .expect("legacy startup snapshot should remain available");
    let cached_server_info = load_startup_cached_codex_apps_server_info(Some(&cache_context));

    assert_eq!(startup_tools.len(), 1);
    assert_eq!(startup_tools[0].callable_name, "calendar_search");
    assert_eq!(cached_server_info, None);
}

#[test]
fn codex_apps_server_info_cache_survives_legacy_tools_cache_write() {
    let codex_home = tempdir().expect("tempdir");
    let cache_context = create_codex_apps_tools_cache_context(
        codex_home.path().to_path_buf(),
        Some("account-one"),
        Some("user-one"),
    );
    let server_info = create_test_server_info("Codex Apps");
    write_codex_apps_tools_cache(
        Some(&cache_context),
        &server_info,
        &[create_test_tool(
            CODEX_APPS_MCP_SERVER_NAME,
            "calendar_search",
        )],
    );

    let cache_path = cache_context.tools_cache_path();
    if let Some(parent) = cache_path.parent() {
        std::fs::create_dir_all(parent).expect("create parent");
    }
    let bytes = serde_json::to_vec_pretty(&serde_json::json!({
        "schema_version": CODEX_APPS_TOOLS_CACHE_SCHEMA_VERSION - 1,
        "tools": [create_test_tool(CODEX_APPS_MCP_SERVER_NAME, "calendar_search")],
    }))
    .expect("serialize");
    std::fs::write(cache_path, bytes).expect("write legacy tools cache");

    assert_eq!(
        load_startup_cached_codex_apps_server_info(Some(&cache_context)),
        Some(server_info)
    );
    assert!(load_startup_cached_codex_apps_tools_snapshot(Some(&cache_context)).is_none());
}

#[tokio::test]
async fn list_all_tools_uses_cached_tool_info_snapshot_while_client_is_pending() {
    let startup_tools = vec![create_test_tool(
        CODEX_APPS_MCP_SERVER_NAME,
        "calendar_create_event",
    )];
    let pending_client = futures::future::pending::<Result<ManagedClient, StartupOutcomeError>>()
        .boxed()
        .shared();
    let approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
    let permission_profile = Constrained::allow_any(PermissionProfile::default());
    let mut manager = McpConnectionManager::new_uninitialized(
        &approval_policy,
        &permission_profile,
        /*prefix_mcp_tool_names*/ true,
    );
    manager.clients.insert(
        CODEX_APPS_MCP_SERVER_NAME.to_string(),
        AsyncManagedClient {
            client: pending_client,
            is_codex_apps_mcp_server: true,
            cached_tool_info_snapshot: Some(startup_tools),
            cached_server_info: None,
            startup_complete: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            tool_plugin_provenance: Arc::new(ToolPluginProvenance::default()),
            cancel_token: CancellationToken::new(),
        },
    );

    let tools = manager.list_all_tools().await;
    let tool = tools
        .iter()
        .find(|tool| {
            tool.canonical_tool_name()
                == ToolName::namespaced("mcp__codex_apps", "calendar_create_event")
        })
        .expect("tool from startup cache");
    assert_eq!(tool.server_name, CODEX_APPS_MCP_SERVER_NAME);
    assert_eq!(tool.callable_name, "calendar_create_event");
}

#[tokio::test]
async fn list_available_server_infos_uses_cache_while_client_is_pending() {
    let pending_client = futures::future::pending::<Result<ManagedClient, StartupOutcomeError>>()
        .boxed()
        .shared();
    let approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
    let permission_profile = Constrained::allow_any(PermissionProfile::default());
    let mut manager = McpConnectionManager::new_uninitialized(
        &approval_policy,
        &permission_profile,
        /*prefix_mcp_tool_names*/ true,
    );
    let server_info = create_test_server_info("Codex Apps");
    manager.clients.insert(
        CODEX_APPS_MCP_SERVER_NAME.to_string(),
        AsyncManagedClient {
            client: pending_client,
            is_codex_apps_mcp_server: true,
            cached_tool_info_snapshot: Some(Vec::new()),
            cached_server_info: Some(server_info.clone()),
            startup_complete: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            tool_plugin_provenance: Arc::new(ToolPluginProvenance::default()),
            cancel_token: CancellationToken::new(),
        },
    );

    let timeout_result = tokio::time::timeout(
        Duration::from_millis(10),
        manager.list_available_server_infos(),
    )
    .await;
    let server_infos = timeout_result.expect("server info lookup should not block on startup");
    assert_eq!(
        server_infos.get(CODEX_APPS_MCP_SERVER_NAME),
        Some(&server_info)
    );
}

#[tokio::test]
async fn list_all_tools_accepts_canonical_namespaced_tool_names() {
    let startup_tools = vec![create_test_tool("rmcp", "echo")];
    let pending_client = futures::future::pending::<Result<ManagedClient, StartupOutcomeError>>()
        .boxed()
        .shared();
    let approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
    let permission_profile = Constrained::allow_any(PermissionProfile::default());
    let mut manager = McpConnectionManager::new_uninitialized(
        &approval_policy,
        &permission_profile,
        /*prefix_mcp_tool_names*/ false,
    );
    manager.clients.insert(
        "rmcp".to_string(),
        AsyncManagedClient {
            client: pending_client,
            is_codex_apps_mcp_server: false,
            cached_tool_info_snapshot: Some(startup_tools),
            cached_server_info: None,
            startup_complete: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            tool_plugin_provenance: Arc::new(ToolPluginProvenance::default()),
            cancel_token: CancellationToken::new(),
        },
    );

    let tools = manager.list_all_tools().await;
    let tool = tools
        .iter()
        .find(|tool| tool.canonical_tool_name() == ToolName::namespaced("rmcp", "echo"))
        .expect("split MCP tool namespace and name should resolve");

    let expected = ("rmcp", "rmcp", "echo", "echo");
    assert_eq!(
        (
            tool.server_name.as_str(),
            tool.callable_namespace.as_str(),
            tool.callable_name.as_str(),
            tool.tool.name.as_ref(),
        ),
        expected
    );
}

#[tokio::test]
async fn list_all_tools_applies_legacy_mcp_prefix_by_default() {
    let startup_tools = vec![create_test_tool("rmcp", "echo")];
    let pending_client = futures::future::pending::<Result<ManagedClient, StartupOutcomeError>>()
        .boxed()
        .shared();
    let approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
    let permission_profile = Constrained::allow_any(PermissionProfile::default());
    let mut manager = McpConnectionManager::new_uninitialized(
        &approval_policy,
        &permission_profile,
        /*prefix_mcp_tool_names*/ true,
    );
    manager.clients.insert(
        "rmcp".to_string(),
        AsyncManagedClient {
            client: pending_client,
            is_codex_apps_mcp_server: false,
            cached_tool_info_snapshot: Some(startup_tools),
            cached_server_info: None,
            startup_complete: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            tool_plugin_provenance: Arc::new(ToolPluginProvenance::default()),
            cancel_token: CancellationToken::new(),
        },
    );

    let tools = manager.list_all_tools().await;
    let tool = tools
        .iter()
        .find(|tool| tool.canonical_tool_name() == ToolName::namespaced("mcp__rmcp", "echo"))
        .expect("legacy-prefixed MCP tool name should resolve");

    let expected = ("rmcp", "mcp__rmcp", "echo", "echo");
    assert_eq!(
        (
            tool.server_name.as_str(),
            tool.callable_namespace.as_str(),
            tool.callable_name.as_str(),
            tool.tool.name.as_ref(),
        ),
        expected
    );
}

#[tokio::test]
async fn list_all_tools_blocks_while_client_is_pending_without_cached_tool_info_snapshot() {
    let pending_client = futures::future::pending::<Result<ManagedClient, StartupOutcomeError>>()
        .boxed()
        .shared();
    let approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
    let permission_profile = Constrained::allow_any(PermissionProfile::default());
    let mut manager = McpConnectionManager::new_uninitialized(
        &approval_policy,
        &permission_profile,
        /*prefix_mcp_tool_names*/ true,
    );
    manager.clients.insert(
        CODEX_APPS_MCP_SERVER_NAME.to_string(),
        AsyncManagedClient {
            client: pending_client,
            is_codex_apps_mcp_server: true,
            cached_tool_info_snapshot: None,
            cached_server_info: None,
            startup_complete: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            tool_plugin_provenance: Arc::new(ToolPluginProvenance::default()),
            cancel_token: CancellationToken::new(),
        },
    );

    let timeout_result =
        tokio::time::timeout(Duration::from_millis(10), manager.list_all_tools()).await;
    assert!(timeout_result.is_err());
}

#[tokio::test]
async fn shutdown_cancels_pending_tool_listing() {
    let cancel_token = CancellationToken::new();
    let cancel_token_for_startup = cancel_token.clone();
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let pending_client = async move {
        let _ = started_tx.send(());
        cancel_token_for_startup.cancelled().await;
        Err(StartupOutcomeError::Cancelled)
    }
    .boxed()
    .shared();
    let approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
    let permission_profile = Constrained::allow_any(PermissionProfile::default());
    let mut manager = McpConnectionManager::new_uninitialized(
        &approval_policy,
        &permission_profile,
        /*prefix_mcp_tool_names*/ true,
    );
    manager.clients.insert(
        CODEX_APPS_MCP_SERVER_NAME.to_string(),
        AsyncManagedClient {
            client: pending_client,
            is_codex_apps_mcp_server: true,
            cached_tool_info_snapshot: None,
            cached_server_info: None,
            startup_complete: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            tool_plugin_provenance: Arc::new(ToolPluginProvenance::default()),
            cancel_token,
        },
    );
    let manager = Arc::new(manager);
    let manager_for_list = Arc::clone(&manager);
    let list_task = tokio::spawn(async move { manager_for_list.list_all_tools().await });

    started_rx.await.expect("tool listing should start");
    tokio::time::timeout(Duration::from_secs(1), manager.shutdown())
        .await
        .expect("shutdown should cancel speculative tool listing");
    let tools = list_task.await.expect("tool listing task should not panic");
    assert!(tools.is_empty());
}

#[tokio::test]
async fn shutdown_continues_after_caller_is_aborted() {
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let (completed_tx, completed_rx) = tokio::sync::oneshot::channel();
    let release = Arc::new(tokio::sync::Notify::new());
    let release_for_client = Arc::clone(&release);
    let blocking_client = async move {
        let _ = started_tx.send(());
        release_for_client.notified().await;
        let _ = completed_tx.send(());
        Err(StartupOutcomeError::Cancelled)
    }
    .boxed()
    .shared();
    let approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
    let permission_profile = Constrained::allow_any(PermissionProfile::default());
    let mut manager = McpConnectionManager::new_uninitialized(
        &approval_policy,
        &permission_profile,
        /*prefix_mcp_tool_names*/ true,
    );
    manager.clients.insert(
        CODEX_APPS_MCP_SERVER_NAME.to_string(),
        AsyncManagedClient {
            client: blocking_client,
            is_codex_apps_mcp_server: true,
            cached_tool_info_snapshot: None,
            cached_server_info: None,
            startup_complete: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            tool_plugin_provenance: Arc::new(ToolPluginProvenance::default()),
            cancel_token: CancellationToken::new(),
        },
    );
    let manager = Arc::new(manager);
    let shutdown_task = tokio::spawn({
        let manager = Arc::clone(&manager);
        async move { manager.shutdown().await }
    });

    started_rx.await.expect("client shutdown should start");
    shutdown_task.abort();
    let shutdown_error = shutdown_task
        .await
        .expect_err("caller shutdown task should be aborted");
    assert!(shutdown_error.is_cancelled());
    release.notify_one();

    tokio::time::timeout(Duration::from_secs(1), completed_rx)
        .await
        .expect("client shutdown should survive caller cancellation")
        .expect("client shutdown completion sender should stay alive");
}

#[tokio::test]
async fn list_all_tools_does_not_block_when_cached_tool_info_snapshot_is_empty() {
    let pending_client = futures::future::pending::<Result<ManagedClient, StartupOutcomeError>>()
        .boxed()
        .shared();
    let approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
    let permission_profile = Constrained::allow_any(PermissionProfile::default());
    let mut manager = McpConnectionManager::new_uninitialized(
        &approval_policy,
        &permission_profile,
        /*prefix_mcp_tool_names*/ true,
    );
    manager.clients.insert(
        CODEX_APPS_MCP_SERVER_NAME.to_string(),
        AsyncManagedClient {
            client: pending_client,
            is_codex_apps_mcp_server: true,
            cached_tool_info_snapshot: Some(Vec::new()),
            cached_server_info: None,
            startup_complete: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            tool_plugin_provenance: Arc::new(ToolPluginProvenance::default()),
            cancel_token: CancellationToken::new(),
        },
    );

    let timeout_result =
        tokio::time::timeout(Duration::from_millis(10), manager.list_all_tools()).await;
    let tools = timeout_result.expect("cache-hit startup snapshot should not block");
    assert!(tools.is_empty());
}

#[tokio::test]
async fn list_all_tools_uses_cached_tool_info_snapshot_when_client_startup_fails() {
    let startup_tools = vec![create_test_tool(
        CODEX_APPS_MCP_SERVER_NAME,
        "calendar_create_event",
    )];
    let server_info = create_test_server_info("Codex Apps");
    let failed_client = futures::future::ready::<Result<ManagedClient, StartupOutcomeError>>(Err(
        StartupOutcomeError::Failed {
            error: "startup failed".to_string(),
        },
    ))
    .boxed()
    .shared();
    let approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
    let permission_profile = Constrained::allow_any(PermissionProfile::default());
    let mut manager = McpConnectionManager::new_uninitialized(
        &approval_policy,
        &permission_profile,
        /*prefix_mcp_tool_names*/ true,
    );
    let startup_complete = Arc::new(std::sync::atomic::AtomicBool::new(true));
    manager.clients.insert(
        CODEX_APPS_MCP_SERVER_NAME.to_string(),
        AsyncManagedClient {
            client: failed_client,
            is_codex_apps_mcp_server: true,
            cached_tool_info_snapshot: Some(startup_tools),
            cached_server_info: Some(server_info.clone()),
            startup_complete,
            tool_plugin_provenance: Arc::new(ToolPluginProvenance::default()),
            cancel_token: CancellationToken::new(),
        },
    );

    let tools = manager.list_all_tools().await;
    let tool = tools
        .iter()
        .find(|tool| {
            tool.canonical_tool_name()
                == ToolName::namespaced("mcp__codex_apps", "calendar_create_event")
        })
        .expect("tool from startup cache");
    assert_eq!(tool.server_name, CODEX_APPS_MCP_SERVER_NAME);
    assert_eq!(tool.callable_name, "calendar_create_event");
    assert_eq!(
        manager
            .list_available_server_infos()
            .await
            .get(CODEX_APPS_MCP_SERVER_NAME),
        Some(&server_info)
    );
}

#[tokio::test]
async fn list_all_tools_adds_server_metadata_to_cached_tools() {
    let server_name = "docs";
    let startup_tools = vec![create_test_tool(server_name, "search")];
    let pending_client = futures::future::pending::<Result<ManagedClient, StartupOutcomeError>>()
        .boxed()
        .shared();
    let approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
    let permission_profile = Constrained::allow_any(PermissionProfile::default());
    let mut manager = McpConnectionManager::new_uninitialized(
        &approval_policy,
        &permission_profile,
        /*prefix_mcp_tool_names*/ true,
    );
    manager.server_metadata.insert(
        server_name.to_string(),
        McpServerMetadata {
            environment_id: codex_config::DEFAULT_MCP_SERVER_ENVIRONMENT_ID.to_string(),
            pollutes_memory: true,
            origin: Some(McpServerOrigin::StreamableHttp(
                "https://docs.example".to_string(),
            )),
            supports_parallel_tool_calls: true,
            default_tools_approval_mode: None,
            tool_approval_modes: HashMap::new(),
        },
    );
    manager.clients.insert(
        server_name.to_string(),
        AsyncManagedClient {
            client: pending_client,
            is_codex_apps_mcp_server: false,
            cached_tool_info_snapshot: Some(startup_tools),
            cached_server_info: None,
            startup_complete: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            tool_plugin_provenance: Arc::new(ToolPluginProvenance::default()),
            cancel_token: CancellationToken::new(),
        },
    );

    let tools = manager.list_all_tools().await;
    assert_eq!(tools.len(), 1);
    let tool = &tools[0];
    assert_eq!(tool.server_name, server_name);
    assert!(tool.supports_parallel_tool_calls);
    assert_eq!(tool.server_origin.as_deref(), Some("https://docs.example"));
}

#[test]
fn server_metadata_preserves_tool_approval_policy() {
    let mut config = crate::codex_apps_mcp_server_config(
        "https://docs.example",
        /*apps_mcp_product_sku*/ None,
    );
    config.environment_id = "remote".to_string();
    config.default_tools_approval_mode = Some(AppToolApproval::Prompt);
    config.tools.insert(
        "search".to_string(),
        McpServerToolConfig {
            approval_mode: Some(AppToolApproval::Approve),
        },
    );
    let metadata = McpServerMetadata::from(&EffectiveMcpServer::configured(config));

    assert_eq!(metadata.environment_id, "remote");
    assert_eq!(metadata.tool_approval_mode("read"), AppToolApproval::Prompt);
    assert_eq!(
        metadata.tool_approval_mode("search"),
        AppToolApproval::Approve
    );
}

#[test]
fn host_owned_codex_apps_requires_server_metadata() {
    let approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
    let permission_profile = Constrained::allow_any(PermissionProfile::default());
    let manager = McpConnectionManager::new_uninitialized(
        &approval_policy,
        &permission_profile,
        /*prefix_mcp_tool_names*/ true,
    );

    assert!(!manager.is_host_owned_codex_apps_server(CODEX_APPS_MCP_SERVER_NAME));
}

#[test]
fn host_owned_codex_apps_matches_reserved_name_with_server_metadata() {
    let approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
    let permission_profile = Constrained::allow_any(PermissionProfile::default());
    let mut manager = McpConnectionManager::new_uninitialized(
        &approval_policy,
        &permission_profile,
        /*prefix_mcp_tool_names*/ true,
    );
    let server = EffectiveMcpServer::configured(crate::codex_apps_mcp_server_config(
        "https://chatgpt.com",
        /*apps_mcp_product_sku*/ None,
    ));
    manager.server_metadata.insert(
        CODEX_APPS_MCP_SERVER_NAME.to_string(),
        McpServerMetadata::from(&server),
    );

    assert!(manager.is_host_owned_codex_apps_server(CODEX_APPS_MCP_SERVER_NAME));
    assert!(!manager.is_host_owned_codex_apps_server("docs"));
}

#[tokio::test]
async fn no_local_runtime_fails_local_stdio_but_keeps_local_http_server() {
    let approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
    let (tx_event, rx_event) = async_channel::unbounded();
    drop(rx_event);
    let codex_home = tempdir().expect("tempdir");
    let mcp_servers = HashMap::from([
        (
            "stdio".to_string(),
            EffectiveMcpServer::configured(McpServerConfig {
                auth: Default::default(),
                transport: McpServerTransportConfig::Stdio {
                    command: "echo".to_string(),
                    args: Vec::new(),
                    env: None,
                    env_vars: Vec::new(),
                    cwd: None,
                },
                environment_id: codex_config::DEFAULT_MCP_SERVER_ENVIRONMENT_ID.to_string(),
                enabled: true,
                required: false,
                supports_parallel_tool_calls: false,
                disabled_reason: None,
                startup_timeout_sec: None,
                tool_timeout_sec: None,
                default_tools_approval_mode: None,
                enabled_tools: None,
                disabled_tools: None,
                scopes: None,
                oauth: None,
                oauth_resource: None,
                tools: HashMap::new(),
            }),
        ),
        (
            "http".to_string(),
            EffectiveMcpServer::configured(McpServerConfig {
                auth: Default::default(),
                transport: McpServerTransportConfig::StreamableHttp {
                    url: "http://127.0.0.1:1".to_string(),
                    bearer_token_env_var: None,
                    http_headers: None,
                    env_http_headers: None,
                },
                environment_id: codex_config::DEFAULT_MCP_SERVER_ENVIRONMENT_ID.to_string(),
                enabled: true,
                required: false,
                supports_parallel_tool_calls: false,
                disabled_reason: None,
                startup_timeout_sec: None,
                tool_timeout_sec: None,
                default_tools_approval_mode: None,
                enabled_tools: None,
                disabled_tools: None,
                scopes: None,
                oauth: None,
                oauth_resource: None,
                tools: HashMap::new(),
            }),
        ),
    ]);

    let cancel_token = CancellationToken::new();
    let manager = McpConnectionManager::new(
        &mcp_servers,
        OAuthCredentialsStoreMode::default(),
        AuthKeyringBackendKind::default(),
        HashMap::new(),
        &approval_policy,
        String::new(),
        tx_event,
        cancel_token.clone(),
        PermissionProfile::default(),
        McpRuntimeContext::new(
            Arc::new(EnvironmentManager::without_environments()),
            PathBuf::from("/tmp"),
        ),
        codex_home.path().to_path_buf(),
        CodexAppsToolsCacheKey {
            account_id: None,
            chatgpt_user_id: None,
            is_workspace_account: false,
        },
        /*prefix_mcp_tool_names*/ true,
        ElicitationCapability::default(),
        /*supports_openai_form_elicitation*/ false,
        ToolPluginProvenance::default(),
        /*auth*/ None,
        /*elicitation_reviewer*/ None,
    )
    .await;

    assert!(manager.clients.contains_key("stdio"));
    assert!(manager.clients.contains_key("http"));
    assert!(
        !manager
            .wait_for_server_ready("stdio", Duration::from_millis(10))
            .await
    );
    let error = match manager
        .clients
        .get("stdio")
        .expect("stdio client")
        .client()
        .await
    {
        Ok(_) => panic!("local stdio MCP startup should fail"),
        Err(error) => error,
    };
    assert_eq!(
        startup_outcome_error_message(error),
        "local stdio MCP server `stdio` requires a local environment"
    );
    cancel_token.cancel();
}

#[test]
fn elicitation_capability_uses_2025_06_18_shape_for_form_only_support() {
    let capability = Some(ElicitationCapability::default());
    assert_eq!(
        serde_json::to_value(capability).expect("serialize elicitation capability"),
        serde_json::json!({})
    );
}

#[test]
fn elicitation_capability_advertises_url_support_when_enabled() {
    let capability = Some(ElicitationCapability {
        form: Some(rmcp::model::FormElicitationCapability::default()),
        url: Some(rmcp::model::UrlElicitationCapability::default()),
    });
    assert_eq!(
        serde_json::to_value(capability).expect("serialize elicitation capability"),
        serde_json::json!({
            "form": {},
            "url": {},
        })
    );
}

#[test]
fn mcp_init_error_display_prompts_for_github_pat() {
    let server_name = "github";
    let entry = McpAuthStatusEntry {
        config: Some(McpServerConfig {
            auth: Default::default(),
            transport: McpServerTransportConfig::StreamableHttp {
                url: "https://api.githubcopilot.com/mcp/".to_string(),
                bearer_token_env_var: None,
                http_headers: None,
                env_http_headers: None,
            },
            environment_id: codex_config::DEFAULT_MCP_SERVER_ENVIRONMENT_ID.to_string(),
            enabled: true,
            required: false,
            supports_parallel_tool_calls: false,
            disabled_reason: None,
            startup_timeout_sec: None,
            tool_timeout_sec: None,
            default_tools_approval_mode: None,
            enabled_tools: None,
            disabled_tools: None,
            scopes: None,
            oauth: None,
            oauth_resource: None,
            tools: HashMap::new(),
        }),
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
        config: Some(McpServerConfig {
            auth: Default::default(),
            transport: McpServerTransportConfig::StreamableHttp {
                url: "https://example.com".to_string(),
                bearer_token_env_var: Some("TOKEN".to_string()),
                http_headers: None,
                env_http_headers: None,
            },
            environment_id: codex_config::DEFAULT_MCP_SERVER_ENVIRONMENT_ID.to_string(),
            enabled: true,
            required: false,
            supports_parallel_tool_calls: false,
            disabled_reason: None,
            startup_timeout_sec: None,
            tool_timeout_sec: None,
            default_tools_approval_mode: None,
            enabled_tools: None,
            disabled_tools: None,
            scopes: None,
            oauth: None,
            oauth_resource: None,
            tools: HashMap::new(),
        }),
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
